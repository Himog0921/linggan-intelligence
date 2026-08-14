import { collectionRunStore } from '../db/collectionRunStore.js';
import { MSG as RUNTIME_MSG, TASK_STATE } from '../shared/constants.js';
import { sendToBackground as sendRuntimeToBackground } from '../shared/messaging.js';
import { isPausedTaskState, resolveTaskState } from '../shared/taskUi.js';
import { createCollectionRunHeartbeatReporter } from '../workbench/runtime/heartbeat.js';
import JSZip from 'jszip';
import {
  detectFileExt,
  normalizeCandidates,
  sanitizePathSegment,
  downloadBlob,
} from './mediaDownloadUtils.js';

const ZIP_FETCH_CONCURRENCY = 4;

export function describeCommentImageTaskMessage({
  phase = 'scan',
  state = TASK_STATE.RUNNING,
  discovered = 0,
  current = 0,
  total = 0,
} = {}) {
  const normalizedPhase = String(phase || 'scan').trim();
  const normalizedState = resolveTaskState({
    taskState: state,
    fallback: TASK_STATE.RUNNING,
  });
  const scanned = Math.max(0, Number(discovered || 0) || 0);
  const done = Math.max(0, Number(current || 0) || 0);
  const planned = Math.max(0, Number(total || 0) || 0);

  if (normalizedPhase === 'download') {
    const progress = planned > 0 ? `${done}/${planned}` : `${done}`;
    if (normalizedState === TASK_STATE.PAUSED) {
      return `打包已暂停：已完成 ${progress} 张`;
    }
    return `继续打包中：已完成 ${progress} 张`;
  }

  if (normalizedState === TASK_STATE.PAUSED) {
    return scanned > 0 ? `扫描已暂停：已扫描 ${scanned} 张图片` : '扫描已暂停';
  }
  return scanned > 0 ? `继续扫描中：已扫描 ${scanned} 张图片` : '继续扫描中...';
}

export function createCommentImageTaskController({
  MSG,
  collectCommentImages,
  sendToBackground,
  extractNoteId,
  showToast,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
} = {}) {
  let task = null;
  let abortPendingCommentImageFetches = () => {};
  const reportHeartbeat = createCollectionRunHeartbeatReporter({ collectionRunStore });

  function cleanup() {
    abortPendingCommentImageFetches();
    task = null;
    hideTaskControlBar();
    setActiveTaskType(null);
    toggleStopButton(false);
  }

  function buildProgress(partial = {}) {
    if (!task) return null;
    const phase = partial.phase || task.phase || 'scan';
    const taskState = resolveTaskState({
      taskState: partial.taskState,
      fallback: task.isPaused ? TASK_STATE.PAUSED : TASK_STATE.RUNNING,
    });
    const isPaused = isPausedTaskState(taskState) || task.isPaused;
    const current = phase === 'download'
      ? Number(partial.current ?? task.current ?? 0)
      : Number(partial.current ?? task.discovered ?? 0);
    const total = phase === 'download'
      ? Number(partial.total ?? task.total ?? 0)
      : Number(partial.total ?? 0);
    return {
      taskType: 'commentImages',
      taskState,
      current,
      total,
      message: partial.message || '',
    };
  }

  async function fetchImageBlobForZip(candidates, { waitIfPaused = async () => {}, signal = null } = {}) {
    const list = normalizeCandidates(candidates);
    for (let i = 0; i < list.length; i += 1) {
      if (signal?.aborted) return { success: false, aborted: true };
      await waitIfPaused();
      if (signal?.aborted) return { success: false, aborted: true };
      const candidate = list[i];
      try {
        let response = await fetch(candidate, { mode: 'cors', credentials: 'include', signal: signal || undefined });
        if (!response.ok) {
          response = await fetch(candidate, { mode: 'no-cors', credentials: 'include', signal: signal || undefined });
        }
        const blob = await response.blob();
        const isImage = String(blob.type || '').startsWith('image/');
        const hasData = Number(blob.size || 0) > 0;
        if (!isImage && !hasData) continue;
        return { success: true, blob, candidate, candidateIndex: i };
      } catch (err) {
        if (String(err?.name || '') === 'AbortError') return { success: false, aborted: true };
      }
    }
    try {
      const fallback = await sendRuntimeToBackground(RUNTIME_MSG.FETCH_BINARY_AS_DATA_URL, { candidates: list });
      if (fallback?.success && fallback?.dataUrl) {
        const blob = await fetch(fallback.dataUrl).then((resp) => resp.blob());
        if (blob && Number(blob.size || 0) > 0) {
          return {
            success: true,
            blob,
            candidate: fallback.candidate || list[0] || '',
            candidateIndex: Number(fallback.candidateIndex || 0) || 0,
          };
        }
      }
    } catch {
      // ignore background fallback errors
    }
    return { success: false, aborted: Boolean(signal?.aborted) };
  }

  async function downloadCommentImagesAsZip(imageItems, {
    noteId,
    waitIfPaused = async () => {},
    onProgress = null,
    isStopRequested = () => false,
  } = {}) {
    const total = Array.isArray(imageItems) ? imageItems.length : 0;
    if (total === 0) {
      return { total: 0, success: 0, failed: 0, hdCount: 0, sdCount: 0, zipped: false };
    }

    const results = new Array(total).fill(null);
    let nextIndex = 0;
    let completed = 0;
    let success = 0;
    let failed = 0;
    let hdCount = 0;
    let sdCount = 0;

    const workerCount = Math.max(1, Math.min(Math.max(ZIP_FETCH_CONCURRENCY, 6), total));
    const activeFetchControllers = new Set();
    abortPendingCommentImageFetches = () => {
      activeFetchControllers.forEach((controller) => {
        try {
          controller.abort();
        } catch {
          // ignore abort errors
        }
      });
      activeFetchControllers.clear();
    };

    async function worker() {
      while (true) {
        await waitIfPaused();
        const currentIndex = nextIndex;
        nextIndex += 1;
        if (currentIndex >= total) return;

        const imageItem = imageItems[currentIndex];
        const candidates = Array.isArray(imageItem?.candidates)
          ? imageItem.candidates
          : [typeof imageItem === 'string' ? imageItem : imageItem?.url].filter(Boolean);

        if (candidates.length === 0) {
          failed += 1;
          completed += 1;
          onProgress?.({
            current: completed,
            total,
            success,
            failed,
            hdCount,
            sdCount,
            stopping: isStopRequested(),
          });
          continue;
        }

        const controller = new AbortController();
        activeFetchControllers.add(controller);
        let blobResult;
        try {
          blobResult = await fetchImageBlobForZip(candidates, {
            waitIfPaused,
            signal: controller.signal,
          });
        } finally {
          activeFetchControllers.delete(controller);
        }
        if (blobResult?.success) {
          const ext = detectFileExt(blobResult.candidate, 'jpg');
          results[currentIndex] = {
            filename: `评论图片_${String(currentIndex + 1).padStart(3, '0')}.${ext}`,
            blob: blobResult.blob,
          };
          success += 1;
          if (blobResult.candidateIndex === 0) hdCount += 1;
          else sdCount += 1;
        } else if (!blobResult?.aborted) {
          failed += 1;
        }

        completed += 1;
        onProgress?.({
          current: completed,
          total,
          success,
          failed,
          hdCount,
          sdCount,
          stopping: isStopRequested(),
        });

      }
    }

    await Promise.all(Array.from({ length: workerCount }, () => worker()));

    if (success === 0) {
      return { total, success, failed, hdCount, sdCount, zipped: false, stopped: isStopRequested() };
    }

    const zip = new JSZip();
    results.forEach((item) => {
      if (!item) return;
      zip.file(item.filename, item.blob);
    });

    const zipBlob = await zip.generateAsync({
      type: 'blob',
      compression: 'STORE',
    });
    const safeNoteId = sanitizePathSegment(noteId || 'unknown');
    const zipName = `灵感爆爆爆_评论图片区_${safeNoteId}_${Date.now()}.zip`;
    downloadBlob(zipBlob, zipName);
    return {
      total,
      success,
      failed,
      hdCount,
      sdCount,
      zipped: true,
      stopped: isStopRequested(),
      zipName,
    };
  }

  async function markRun(status, payload = {}) {
    const runId = String(task?.collectionRunId || '').trim();
    if (!runId) return;
    if (status === 'done') {
      await collectionRunStore.markDone(runId, payload);
      return;
    }
    if (status === 'stopped') {
      await collectionRunStore.markStopped(runId, payload);
      return;
    }
    if (status === 'failed') {
      await collectionRunStore.markFailed(runId, payload.error || '评论图片区任务失败', payload);
    }
  }

  return {
    isRunning() {
      return Boolean(task?.isRunning);
    },

    getTask() {
      return task;
    },

    cleanup,

    buildProgress,

    pause() {
      if (!task?.isRunning) return;
      task.isPaused = true;
      const progress = buildProgress({
        taskState: TASK_STATE.PAUSED,
        message: describeCommentImageTaskMessage({
          phase: task.phase,
          state: TASK_STATE.PAUSED,
          discovered: task.discovered,
          current: task.current,
          total: task.total,
        }),
      });
      if (progress) syncTaskUI(progress);
    },

    resume() {
      if (!task?.isRunning) return;
      task.isPaused = false;
      if (task.pauseResolve) {
        task.pauseResolve();
        task.pauseResolve = null;
      }
      const progress = buildProgress({
        taskState: TASK_STATE.RUNNING,
        message: describeCommentImageTaskMessage({
          phase: task.phase,
          state: TASK_STATE.RUNNING,
          discovered: task.discovered,
          current: task.current,
          total: task.total,
        }),
      });
      if (progress) syncTaskUI(progress);
    },

    stop() {
      if (!task?.isRunning) return;
      task.stopRequested = true;
      if (task.phase === 'download') {
        task.isPaused = false;
        if (task.pauseResolve) {
          task.pauseResolve();
          task.pauseResolve = null;
      }
      const progress = buildProgress({
        taskState: TASK_STATE.RUNNING,
        current: task.current || 0,
        total: task.total || 0,
        message: '已停止新增扫描，继续完成已发现图片的下载与打包...',
        });
        if (progress) syncTaskUI(progress);
        return;
      }
      task.isRunning = false;
      task.isPaused = false;
      if (task.pauseResolve) {
        task.pauseResolve();
        task.pauseResolve = null;
      }
      const progress = buildProgress({
        taskState: TASK_STATE.RUNNING,
        current: task.current || 0,
        total: task.total || 0,
        message: '正在停止扫描并收尾...',
      });
      if (progress) syncTaskUI(progress);
    },

    async start() {
      if (task?.isRunning) {
        showToast('评论图片区下载任务进行中，可在右下角暂停或停止', 'warning');
        return { success: false, error: 'task_already_running' };
      }

      const noteId = extractNoteId(window.location.href) || 'unknown';
      const run = await collectionRunStore.createRun({
        platform: 'xhs',
        taskType: 'commentImages',
        pageType: 'noteDetail',
        triggerSource: 'manual_comment_images',
        config: { noteId },
        meta: { pageUrl: window.location.href },
      });

      showToast('正在扫描评论区图片...', 'info');
      task = {
        collectionRunId: run.collectionRunId,
        isRunning: true,
        isPaused: false,
        stopRequested: false,
        pauseResolve: null,
        phase: 'scan',
        discovered: 0,
        total: 0,
        current: 0,
        downloaded: 0,
        failed: 0,
        hdCount: 0,
        sdCount: 0,
      };
      startBatchTask('commentImages');

      let lastToastAt = 0;
      const waitIfPaused = async () => {
        if (!task?.isPaused) return;
        await new Promise((resolve) => {
          if (task) task.pauseResolve = resolve;
        });
      };
      const shouldStop = () => Boolean(task?.stopRequested);

      try {
        const imgResult = await collectCommentImages({
          noteId,
          shouldStop,
          waitIfPaused,
          onProgress: (progress) => {
            if (!task) return;
            task.phase = 'scan';
            task.discovered = progress.current || task.discovered || 0;
            const next = buildProgress({
              phase: 'scan',
              current: task.discovered,
              message: progress.message || `正在扫描评论区图片，已发现 ${task.discovered} 张`,
            });
            if (next) syncTaskUI(next);
            if (Date.now() - lastToastAt > 1400) {
              showToast(progress.message || `已发现 ${task.discovered} 张评论图片`, 'info');
              lastToastAt = Date.now();
            }
            void reportHeartbeat.report(run.collectionRunId, {
              taskState: task.isPaused ? TASK_STATE.PAUSED : TASK_STATE.RUNNING,
              stage: 'discovering',
              current: task.discovered,
              total: progress.total || task.total || 0,
              message: progress.message || `正在扫描评论区图片，已发现 ${task.discovered} 张`,
            }).catch(() => {});
          },
        });

        const scanStopped = shouldStop();
        if (scanStopped && imgResult.total === 0) {
          const stoppedProgress = buildProgress({
            taskState: TASK_STATE.IDLE,
            current: task.discovered || 0,
            total: task.discovered || 0,
            message: '评论图片区下载已停止',
          });
          if (stoppedProgress) syncTaskUI(stoppedProgress);
          await markRun('stopped', {
            discovered: task.discovered || 0,
            downloaded: 0,
            failed: 0,
          });
          cleanup();
          showToast('评论图片区下载已停止', 'warning');
          return { success: true, stopped: true };
        }

        if (imgResult.total === 0) {
          const emptyProgress = buildProgress({
            taskState: TASK_STATE.DONE,
            current: 0,
            total: 0,
            message: '未发现评论区图片',
          });
          if (emptyProgress) syncTaskUI(emptyProgress);
          await markRun('done', {
            discovered: 0,
            downloaded: 0,
            failed: 0,
          });
          cleanup();
          showToast('未发现评论区图片', 'warning');
          return { success: true, total: 0 };
        }

        task.phase = 'download';
        task.total = imgResult.images.length;
        task.current = 0;
        showToast(
          scanStopped
            ? `已停止继续扫描，开始打包已发现的 ${imgResult.total} 张图片...`
            : `发现 ${imgResult.total} 张图片，开始打包 ZIP...`,
          'info',
        );
        const initProgress = buildProgress({
          phase: 'download',
          total: imgResult.images.length,
          current: 0,
          message: scanStopped
            ? `已停止继续扫描，正在打包 0/${imgResult.images.length} 张`
            : `准备打包 0/${imgResult.images.length} 张`,
        });
        if (initProgress) syncTaskUI(initProgress);

        const zipResult = await downloadCommentImagesAsZip(imgResult.images, {
          noteId,
          waitIfPaused,
          isStopRequested: shouldStop,
          onProgress: (progress) => {
            if (!task) return;
            task.current = progress.current;
            task.downloaded = progress.success;
            task.failed = progress.failed;
            task.hdCount = progress.hdCount;
            task.sdCount = progress.sdCount;
            const next = buildProgress({
              phase: 'download',
              current: progress.current,
              total: progress.total,
              message: progress.stopping
                ? `停止中，正在打包已发现图片 ${progress.current}/${progress.total}`
                : `正在打包 ${progress.current}/${progress.total} 张`,
            });
            if (next) syncTaskUI(next);
            if (Date.now() - lastToastAt > 1500) {
              showToast(
                progress.stopping
                  ? `停止中，继续完成已发现图片 ${progress.success}/${progress.total}`
                  : `打包中 ${progress.current}/${progress.total}（成功 ${progress.success}）`,
                'info',
              );
              lastToastAt = Date.now();
            }
            void reportHeartbeat.report(run.collectionRunId, {
              taskState: progress.stopping ? TASK_STATE.STOPPING : (task.isPaused ? TASK_STATE.PAUSED : TASK_STATE.RUNNING),
              stage: 'downloading',
              current: progress.current,
              total: progress.total,
              message: progress.stopping
                ? `停止中，正在打包已发现图片 ${progress.current}/${progress.total}`
                : `正在打包 ${progress.current}/${progress.total} 张`,
            }).catch(() => {});
          },
        });

        const stopped = scanStopped || shouldStop();
        const downloaded = zipResult.success || 0;
        const failed = zipResult.failed || 0;

        if (downloaded === 0) {
          const failedProgress = buildProgress({
            taskState: stopped ? TASK_STATE.IDLE : TASK_STATE.ERROR,
            current: 0,
            total: imgResult.total,
            message: stopped
              ? '评论图片区已停止，未完成任何图片下载'
              : '评论图片区下载失败，未成功打包任何图片',
          });
          if (failedProgress) syncTaskUI(failedProgress);
          await markRun(stopped ? 'stopped' : 'failed', {
            discovered: imgResult.total,
            downloaded,
            failed,
            hdCount: zipResult.hdCount || 0,
            sdCount: zipResult.sdCount || 0,
            error: '评论图片区未成功下载任何图片',
          });
          cleanup();
          showToast(
            stopped ? '评论图片区已停止，未完成任何图片下载' : '评论图片区下载失败，未成功打包任何图片',
            'warning',
          );
          return {
            success: stopped,
            stopped,
            total: imgResult.total,
            downloaded,
            failed,
          };
        }

        await markRun(stopped ? 'stopped' : 'done', {
          discovered: imgResult.total,
          downloaded,
          failed,
          hdCount: zipResult.hdCount || 0,
          sdCount: zipResult.sdCount || 0,
          zipName: zipResult.zipName || '',
        });

        const finalProgress = buildProgress({
          taskState: stopped ? TASK_STATE.IDLE : TASK_STATE.DONE,
          current: downloaded,
          total: imgResult.total,
          message: stopped
            ? `评论图片区下载已停止：已打包 ${downloaded}/${imgResult.total} 张`
            : `评论图片区下载完成：成功 ${downloaded}/${imgResult.total}，高清 ${zipResult.hdCount || 0}，失败 ${failed}`,
        });
        if (finalProgress) syncTaskUI(finalProgress);
        cleanup();
        showToast(
          stopped
            ? `评论图片区下载已停止，已打包 ${downloaded}/${imgResult.total} 张`
            : `下载完成：成功 ${downloaded}/${imgResult.total}，高清 ${zipResult.hdCount || 0}，失败 ${failed}`,
          stopped ? 'warning' : 'success',
        );
        return {
          success: true,
          stopped,
          total: imgResult.total,
          downloaded,
          failed,
          hdCount: zipResult.hdCount || 0,
          sdCount: zipResult.sdCount || 0,
        };
      } catch (err) {
        await markRun('failed', {
          error: String(err?.message || err),
          discovered: task?.discovered || 0,
          downloaded: task?.downloaded || 0,
          failed: task?.failed || 0,
        });
        cleanup();
        throw err;
      }
    },
  };
}
