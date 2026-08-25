export function createCommentTaskController({
  collectComments,
  showToast,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
} = {}) {
  let task = null;

  function cleanup() {
    task = null;
    hideTaskControlBar();
    setActiveTaskType(null);
    toggleStopButton(false);
  }

  function buildProgress(partial = {}) {
    if (!task) return null;
    return {
      taskType: 'singleComments',
      taskState: partial.taskState || (task.isPaused ? 'paused' : 'running'),
      current: Number(partial.current ?? task.current ?? 0),
      total: Number(partial.total ?? task.total ?? 0),
      message: partial.message || '',
    };
  }

  function publishProgress(partial = {}) {
    const progress = buildProgress(partial);
    if (progress) syncTaskUI(progress);
    return progress;
  }

  async function waitIfPaused() {
    if (!task?.isPaused) return;
    await new Promise((resolve) => {
      if (task) task.pauseResolve = resolve;
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
      publishProgress({
        taskState: 'paused',
        message: '评论采集已暂停',
      });
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
      task.isRunning = false;
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
        pauseResolve: null,
        current: 0,
        total: safeMaxTotal,
        noteId: safeNoteId,
        noteUrl: safeNoteUrl,
        commentDepthMode,
        maxSubComments: safeMaxSubComments,
      };

      startBatchTask('singleComments');
      showToast('正在采集评论...', 'info');
      publishProgress({
        taskState: 'running',
        current: 0,
        total: safeMaxTotal,
        message: '正在准备评论采集',
      });

      let lastToastAt = 0;
      const shouldStop = () => Boolean(task?.stopRequested);

      try {
        const result = await collectComments({
          noteId: safeNoteId,
          noteUrl: safeNoteUrl,
          maxTotal: safeMaxTotal,
          maxSubComments: safeMaxSubComments,
          shouldStop,
          waitIfPaused,
          onProgress: (progress) => {
            if (!task) return;
            task.current = Number(progress.current || task.current || 0);
            task.total = safeMaxTotal || task.total || task.current || 0;
            const next = publishProgress({
              taskState: task.isPaused ? 'paused' : 'running',
              current: task.current,
              total: task.total,
              message: progress.message || `已采集 ${task.current} 条评论`,
            });
            if (next && Date.now() - lastToastAt > 1200) {
              showToast(progress.message || `已采集 ${task.current} 条评论`, 'info');
              lastToastAt = Date.now();
            }
          },
        });

        const total = Number(result?.total || 0);
        if (shouldStop()) {
          publishProgress({
            taskState: 'idle',
            current: total,
            total: safeMaxTotal || total,
            message: total > 0 ? `评论采集已停止：共 ${total} 条` : '评论采集已停止',
          });
          showToast(total > 0 ? `评论采集已停止，已采集 ${total} 条` : '评论采集已停止', 'warning');
        } else {
          publishProgress({
            taskState: 'done',
            current: total,
            total: safeMaxTotal || total,
            message: `评论采集完成：共 ${total} 条`,
          });
          showToast(`评论采集完成：共 ${total} 条`, 'success');
        }
        cleanup();
        return result;
      } catch (err) {
        cleanup();
        throw err;
      }
    },
  };
}
