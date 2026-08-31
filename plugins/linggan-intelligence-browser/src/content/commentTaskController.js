export function createCommentTaskController({
  collectComments,
  showToast,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  submitCommentCheckpoint,
} = {}) {
  let task = null;

  function nonNegativeInteger(value, fallback = 0) {
    const number = Number(value);
    return Number.isFinite(number) && number >= 0 ? Math.floor(number) : fallback;
  }

  function progressTotal(current = task?.collectedCount || 0, activeTask = task) {
    if (!activeTask) return 0;
    const requested = nonNegativeInteger(activeTask.requestedLimit);
    const pageCount = activeTask.pageCommentCount == null
      ? null
      : nonNegativeInteger(activeTask.pageCommentCount);
    const candidate = pageCount == null
      ? requested
      : (requested > 0 ? Math.min(requested, pageCount) : pageCount);
    // A stale or misread denominator must never produce an impossible 130/53 display.
    return candidate >= nonNegativeInteger(current) ? candidate : 0;
  }

  function pausedCheckpoint(snapshot = {}) {
    const count = nonNegativeInteger(snapshot.total, Array.isArray(snapshot.comments) ? snapshot.comments.length : 0);
    const receipt = snapshot.collectionReceipt && typeof snapshot.collectionReceipt === 'object'
      ? snapshot.collectionReceipt
      : {};
    return {
      ...snapshot,
      total: count,
      collectionState: 'partial',
      analysisUsability: count > 0 ? 'usable' : 'empty',
      stopReason: 'manual_pause',
      collectionReceipt: {
        ...receipt,
        uniqueCollectedCount: count,
        state: 'partial',
        analysisUsability: count > 0 ? 'usable' : 'empty',
        stopReason: 'manual_pause',
      },
    };
  }

  async function submitLatestPauseCheckpoint(activeTask) {
    if (!activeTask || typeof submitCommentCheckpoint !== 'function') return;
    const snapshot = activeTask.latestSnapshot;
    const count = nonNegativeInteger(snapshot?.total, Array.isArray(snapshot?.comments) ? snapshot.comments.length : 0);
    if (!snapshot || count <= 0 || count <= activeTask.lastCheckpointCount
        || count <= activeTask.checkpointInFlightCount) return;
    activeTask.checkpointInFlightCount = count;
    try {
      await submitCommentCheckpoint(pausedCheckpoint(snapshot), activeTask.noteId, {
        maxTotal: activeTask.requestedLimit,
        maxSubComments: activeTask.maxSubComments,
        commentDepthMode: activeTask.commentDepthMode,
        taskSpec: activeTask.taskSpec,
      });
      if (task === activeTask) activeTask.lastCheckpointCount = count;
      showToast(`已暂停；当前 ${count} 条评论已写入 Linggan 本机交付队列`, 'info');
    } catch (error) {
      showToast(`评论已暂停，但当前数据交付失败：${error?.message || 'unknown'}`, 'warning');
    } finally {
      if (task === activeTask && activeTask.checkpointInFlightCount === count) {
        activeTask.checkpointInFlightCount = 0;
      }
    }
  }

  function cleanup(activeTask = task) {
    if (task !== activeTask) return false;
    task = null;
    hideTaskControlBar();
    setActiveTaskType(null);
    toggleStopButton(false);
    return true;
  }

  function buildProgress(partial = {}, activeTask = task) {
    if (!activeTask || task !== activeTask) return null;
    return {
      taskType: 'singleComments',
      taskState: partial.taskState || (activeTask.isPaused ? 'paused' : 'running'),
      current: nonNegativeInteger(partial.current ?? activeTask.collectedCount),
      total: nonNegativeInteger(partial.total ?? progressTotal(partial.current ?? activeTask.collectedCount, activeTask)),
      message: partial.message || '',
    };
  }

  function publishProgress(partial = {}, activeTask = task) {
    const progress = buildProgress(partial, activeTask);
    if (progress) syncTaskUI(progress);
    return progress;
  }

  async function waitIfPaused(activeTask = task) {
    if (!activeTask?.isPaused || activeTask?.stopRequested || task !== activeTask) return;
    await new Promise((resolve) => {
      if (task === activeTask) activeTask.pauseResolve = resolve;
      else resolve();
    });
  }

  return {
    isRunning() {
      return Boolean(task?.isRunning);
    },

    cleanup,

    pause() {
      if (!task?.isRunning) return;
      task.isPaused = true;
      const activeTask = task;
      publishProgress({
        taskState: 'paused',
        message: '评论采集已暂停',
      });
      void submitLatestPauseCheckpoint(activeTask);
    },

    resume() {
      if (!task?.isRunning) return;
      task.isPaused = false;
      if (task.pauseResolve) {
        task.pauseResolve();
        task.pauseResolve = null;
      }
      publishProgress({
        taskState: 'running',
        message: '评论采集中...',
      });
    },

    stop() {
      if (!task?.isRunning) return;
      task.stopRequested = true;
      task.isStopping = true;
      if (task.pauseResolve) {
        task.pauseResolve();
        task.pauseResolve = null;
      }
      publishProgress({
        taskState: 'running',
        message: '正在停止评论采集...',
      });
    },

    async start({
      noteId = '',
      noteUrl = '',
      maxTotal = 0,
      maxSubComments = 0,
      commentDepthMode = 'twoLevel',
      taskSpec = undefined,
    } = {}) {
      if (task?.isRunning) {
        showToast('评论采集任务进行中，可在右下角暂停或停止', 'warning');
        return { success: false, error: 'task_already_running' };
      }

      const safeNoteId = String(noteId || noteUrl.split('/').pop()?.split('?')[0] || '').trim() || 'unknown';
      const safeNoteUrl = String(noteUrl || window.location.href || '').trim();
      const safeMaxTotal = Math.max(0, Number(maxTotal || 0) || 0);
      const safeMaxSubComments = Math.max(0, Number(maxSubComments || 0) || 0);

      task = {
        isRunning: true,
        isPaused: false,
        stopRequested: false,
        isStopping: false,
        pauseResolve: null,
        collectedCount: 0,
        requestedLimit: safeMaxTotal,
        pageCommentCount: null,
        latestSnapshot: null,
        lastCheckpointCount: 0,
        checkpointInFlightCount: 0,
        noteId: safeNoteId,
        noteUrl: safeNoteUrl,
        commentDepthMode,
        maxSubComments: safeMaxSubComments,
        taskSpec,
      };
      const activeTask = task;

      startBatchTask('singleComments');
      showToast('正在采集评论...', 'info');
      publishProgress({
        taskState: 'running',
        current: 0,
        total: safeMaxTotal,
        message: '正在准备评论采集',
      });

      let lastToastAt = 0;
      const shouldStop = () => Boolean(activeTask.stopRequested || task !== activeTask);

      try {
        const result = await collectComments({
          noteId: safeNoteId,
          noteUrl: safeNoteUrl,
          maxTotal: safeMaxTotal,
          maxSubComments: safeMaxSubComments,
          commentDepthMode,
          shouldStop,
          waitIfPaused: () => waitIfPaused(activeTask),
          onProgress: (progress) => {
            if (task !== activeTask) return;
            activeTask.collectedCount = nonNegativeInteger(progress.current, activeTask.collectedCount);
            if (progress.pageCommentCount != null && Number.isFinite(Number(progress.pageCommentCount))) {
              activeTask.pageCommentCount = nonNegativeInteger(progress.pageCommentCount);
            }
            const next = publishProgress({
              taskState: activeTask.isPaused ? 'paused' : 'running',
              current: activeTask.collectedCount,
              total: progressTotal(activeTask.collectedCount, activeTask),
              message: progress.message || `已采集 ${activeTask.collectedCount} 条评论`,
            }, activeTask);
            if (next && Date.now() - lastToastAt > 1200) {
              showToast(progress.message || `已采集 ${task.collectedCount} 条评论`, 'info');
              lastToastAt = Date.now();
            }
          },
          onSnapshot: (snapshot) => {
            if (task !== activeTask || !snapshot || typeof snapshot !== 'object') return;
            activeTask.latestSnapshot = snapshot;
            activeTask.collectedCount = nonNegativeInteger(snapshot.total, activeTask.collectedCount);
            const pageCount = snapshot.collectionReceipt?.pageCommentCount ?? snapshot.publicCommentCount;
            if (pageCount != null && Number.isFinite(Number(pageCount))) {
              activeTask.pageCommentCount = nonNegativeInteger(pageCount);
            }
            if (activeTask.isPaused) void submitLatestPauseCheckpoint(activeTask);
          },
          taskSpec,
        });

        const total = Number(result?.total || 0);
        if (task === activeTask) {
          activeTask.collectedCount = nonNegativeInteger(total);
          const resultPageCount = result?.collectionReceipt?.pageCommentCount ?? result?.publicCommentCount;
          if (resultPageCount != null && Number.isFinite(Number(resultPageCount))) {
            activeTask.pageCommentCount = nonNegativeInteger(resultPageCount);
          }
        }
        const finalProgressTotal = progressTotal(total, activeTask);
        if (shouldStop()) {
          publishProgress({
            taskState: 'idle',
            current: total,
            total: finalProgressTotal,
            message: total > 0 ? `评论采集已停止：共 ${total} 条` : '评论采集已停止',
          });
          showToast(total > 0 ? `评论采集已停止，已采集 ${total} 条` : '评论采集已停止', 'warning');
        } else {
          const collectionState = String(result?.collectionState || 'partial');
          const expected = Number.isFinite(Number(result?.collectionReceipt?.expectedCount))
            ? Number(result.collectionReceipt.expectedCount)
            : null;
          const countText = expected === null || expected < total
            ? `已取得 ${total} 条`
            : `${total} / ${expected}`;
          const complete = collectionState === 'complete';
          const invalidTarget = collectionState === 'invalid_target';
          const completion = complete
            ? (result?.explicitEmptyState ? '页面明确没有公开评论' : `本次范围完整（${countText}）`)
            : (invalidTarget
              ? `目标笔记身份不一致（${countText}）`
              : `本次为部分采集（${countText}；${result?.stopReason || 'collector_partial'}）`);
          publishProgress({
            taskState: 'done',
            current: total,
            total: finalProgressTotal,
            message: `${complete ? '评论采集完成' : (invalidTarget ? '评论采集未接纳' : '评论部分采集')}：${completion}`,
          });
          showToast(
            `${complete ? '评论采集完成' : (invalidTarget ? '评论采集未接纳' : '评论部分采集')}：${completion}`,
            complete ? 'success' : (invalidTarget ? 'error' : 'warning'),
          );
        }
        cleanup(activeTask);
        return result;
      } catch (err) {
        cleanup(activeTask);
        throw err;
      }
    },
  };
}
