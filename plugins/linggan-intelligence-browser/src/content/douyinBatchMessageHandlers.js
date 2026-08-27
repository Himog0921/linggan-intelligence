import { MSG, COMMENT_DEPTH_MODE, TASK_STATE } from '../shared/constants.js';
import { sendToBackground } from '../shared/messaging.js';
import { isPausedTaskState, resolveTaskState } from '../shared/taskUi.js';
import { hasExplicitXhsSearchFilters, normalizeXhsSearchFilters } from '../platforms/xhs/searchFilters.js';

const XHS_FILTERED_REMOTE_STARTUP_TIMEOUT_MS = 30_000;

async function defaultReleaseExecutionLock(lock = {}) {
  if (!lock || typeof lock !== 'object' || Array.isArray(lock)) return;
  if (!lock.platform || !lock.accountId || !lock.taskId) return;
  try {
    await sendToBackground(MSG.RELEASE_EXECUTION_ACCOUNT_LOCK, { executionLock: lock }, { timeoutMs: 4000 });
  } catch {
    // The lock has an expiry, so release failures must not hide task results.
  }
}

function finalizeManagedTask({ toggleStopButton, hideTaskControlBar, setActiveTaskType, clearController } = {}) {
  toggleStopButton(false);
  hideTaskControlBar();
  setActiveTaskType(null);
  clearController?.();
}

function finalizeXhsBatchStartFailure({
  taskType,
  msg = {},
  error,
  reportProgress,
  syncTaskUI,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  setController,
} = {}) {
  const total = Number(msg.count || 0) || 0;
  const message = String(error?.message || error || '批量采集失败');
  reportProgress?.(0, total, message, {
    taskType,
    taskState: TASK_STATE.ERROR,
    phase: 'startup',
  });
  syncTaskUI?.({
    taskType,
    taskState: TASK_STATE.ERROR,
    current: 0,
    total,
    message: `批量采集失败：${message}`,
  });
  toggleStopButton?.(false);
  hideTaskControlBar?.();
  setActiveTaskType?.(null);
  setController?.(null);
}

function waitForControllerStartup(controller, startPromise, timeoutMs = 8000) {
  const readCollectionRunId = () => String(controller?.collectionRunId || '').trim();
  const existingRunId = readCollectionRunId();
  if (existingRunId) {
    return Promise.resolve({ collectionRunId: existingRunId });
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    let intervalId = null;
    let timeoutId = null;

    const finishResolve = (value) => {
      if (settled) return;
      settled = true;
      if (intervalId) clearInterval(intervalId);
      if (timeoutId) clearTimeout(timeoutId);
      resolve(value);
    };
    const finishReject = (error) => {
      if (settled) return;
      settled = true;
      if (intervalId) clearInterval(intervalId);
      if (timeoutId) clearTimeout(timeoutId);
      reject(error);
    };

    intervalId = setInterval(() => {
      const collectionRunId = readCollectionRunId();
      if (collectionRunId) {
        finishResolve({ collectionRunId });
      }
    }, 50);

    timeoutId = setTimeout(() => {
      finishReject(new Error('页面没有真正启动任务'));
    }, timeoutMs);

    Promise.resolve(startPromise).then(() => {
      const collectionRunId = readCollectionRunId();
      if (collectionRunId) {
        finishResolve({ collectionRunId });
        return;
      }
      finishReject(new Error('页面没有生成任务记录'));
    }).catch((error) => {
      finishReject(error);
    });
  });
}

function createDouyinManagedBatchStartHandler({
  taskType,
  startMessageTaskType,
  createManagedTaskController,
  runBatchTask,
  buildDoneMessage,
  buildStoppedMessage,
  buildErrorMessage,
  reportProgress,
  reportDone,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  getController,
  setController,
  attachExternalController,
  pauseManagedTask,
  releaseExecutionLock = defaultReleaseExecutionLock,
} = {}) {
  return async (msg = {}) => {
    getController()?.stop?.();
    const isRemoteDispatch = Boolean(String(msg.externalTaskMeta?.externalTaskId || '').trim());
    let resolveStartup = null;
    let rejectStartup = null;
    const startupPromise = isRemoteDispatch
      ? new Promise((resolve, reject) => {
        resolveStartup = resolve;
        rejectStartup = reject;
      })
      : null;
    const controller = createManagedTaskController(async ({ shouldStop, waitIfPaused }) => {
      try {
        const result = await runBatchTask(msg, {
          shouldStop,
          waitIfPaused,
          onCollectionRun: (collectionRunId = '') => {
            const normalizedRunId = String(collectionRunId || '').trim();
            if (!normalizedRunId) return;
            controller.collectionRunId = normalizedRunId;
            resolveStartup?.({ collectionRunId: normalizedRunId });
            resolveStartup = null;
            rejectStartup = null;
          },
          onManagedProgress: (progress = {}) => {
            const taskState = resolveTaskState({
              taskState: progress.taskState,
              status: progress.status,
              fallback: TASK_STATE.RUNNING,
            });
            const current = Number(progress.current || 0);
            const total = Number(progress.total || 0);
            const message = String(progress.message || '').trim();
            const phase = progress.phase || progress.stage || (isPausedTaskState(taskState) ? TASK_STATE.PAUSED : 'collecting');

            reportProgress(current, total, message, {
              taskType: startMessageTaskType,
              taskState,
              phase,
            });

            if (isPausedTaskState(taskState)) {
              pauseManagedTask?.({
                taskType: startMessageTaskType,
                current,
                total,
                message,
              });
              return;
            }

            syncTaskUI({
              taskType: startMessageTaskType,
              taskState,
              current,
              total,
              message,
            });
          },
        });
        if (!result?.ok && !result?.stopped) {
          throw new Error(result?.error || `${taskType} 失败`);
        }
        if (result?.stopped) {
          syncTaskUI(buildStoppedMessage(result));
        } else {
          reportDone?.(result);
          syncTaskUI(buildDoneMessage(result));
        }
      } catch (err) {
        rejectStartup?.(err);
        resolveStartup = null;
        rejectStartup = null;
        const safeTotal = Number(msg.count || 0) || 0;
        reportProgress(0, safeTotal, String(err?.message || err), {
          taskType: startMessageTaskType,
          taskState: TASK_STATE.ERROR,
          phase: 'collecting',
        });
        syncTaskUI(buildErrorMessage(msg, err));
      } finally {
        await releaseExecutionLock(msg.executionLock);
        finalizeManagedTask({
          toggleStopButton,
          hideTaskControlBar,
          setActiveTaskType,
          clearController: () => {
            setController(null);
            attachExternalController?.(null);
          },
        });
      }
    });
    controller.collectionRunId = '';
    setController(controller);
    attachExternalController?.(controller);
    startBatchTask(startMessageTaskType);
    controller.start();
    if (!isRemoteDispatch) {
      return { success: true };
    }

    const startup = await waitForControllerStartup(controller, startupPromise);
    return {
      success: true,
      accepted: true,
      pending: true,
      collectionRunId: startup.collectionRunId,
    };
  };
}

function createXhsBatchStartHandler({
  ControllerClass,
  taskType,
  reportProgress,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  setController,
  buildOptions,
  releaseExecutionLock = defaultReleaseExecutionLock,
} = {}) {
  return async (msg = {}) => {
    const controller = new ControllerClass();
    setController(controller);
    startBatchTask(taskType);
    const mode = msg.mode || 'search';
    const options = buildOptions(msg);
    const handleProgress = (progress) => {
      const taskState = resolveTaskState({
        taskState: progress.taskState,
        status: progress.status,
        fallback: TASK_STATE.RUNNING,
      });
      reportProgress(progress.current, progress.total, progress.message, {
        taskType: progress.taskType || taskType,
        taskState,
        phase: progress.phase || progress.status,
      });
      const isDone = taskState === TASK_STATE.DONE;
      syncTaskUI({
        ...progress,
        taskState,
      });
      if (isDone) {
        toggleStopButton(false);
        hideTaskControlBar();
        setActiveTaskType(null);
      }
    };

    const startPromise = Promise.resolve()
      .then(() => controller.start(mode, handleProgress, options))
      .catch((error) => {
        finalizeXhsBatchStartFailure({
          taskType,
          msg,
          error,
          reportProgress,
          syncTaskUI,
          toggleStopButton,
          hideTaskControlBar,
          setActiveTaskType,
          setController,
        });
        throw error;
      })
      .finally(() => {
        void releaseExecutionLock(msg.executionLock);
      });

    const isRemoteDispatch = Boolean(String(msg.externalTaskMeta?.externalTaskId || '').trim());
    if (!isRemoteDispatch) {
      void startPromise.catch(() => {});
      return { success: true };
    }

    const startupTimeoutMs = mode === 'search' && hasExplicitXhsSearchFilters(options.searchFilters)
      ? XHS_FILTERED_REMOTE_STARTUP_TIMEOUT_MS
      : undefined;
    const startup = await waitForControllerStartup(controller, startPromise, startupTimeoutMs);
    void startPromise.catch(() => {});
    return {
      success: true,
      accepted: true,
      pending: true,
      collectionRunId: startup.collectionRunId,
    };
  };
}

function createStopHandler({
  isDouyinPage,
  getController,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  getDouyinAdapter = null,
  douyinAdapter = null,
  releaseExecutionLock = defaultReleaseExecutionLock,
} = {}) {
  const resolveDouyinAdapter = () => {
    if (typeof getDouyinAdapter === 'function') {
      return getDouyinAdapter();
    }
    return douyinAdapter;
  };

  return () => {
    getController()?.stop?.();
    if (isDouyinPage()) {
      const activeDouyinAdapter = resolveDouyinAdapter();
      if (activeDouyinAdapter) {
        activeDouyinAdapter.stopBatch();
        activeDouyinAdapter.hideTaskControlBar();
        activeDouyinAdapter.setActiveTaskType(null);
      }
      return { success: true };
    }
    toggleStopButton(false);
    hideTaskControlBar();
    setActiveTaskType(null);
    return { success: true };
  };
}

function normalizeCommentDepthMode(value) {
  return String(value || COMMENT_DEPTH_MODE.TWO_LEVEL) === COMMENT_DEPTH_MODE.ALL_REPLIES
    ? COMMENT_DEPTH_MODE.ALL_REPLIES
    : COMMENT_DEPTH_MODE.TWO_LEVEL;
}

export function createBatchMessageHandlers({
  isDouyinPage,
  createManagedTaskController,
  batchCollectDouyinProfileVideos,
  batchCollectDouyinProfileComments,
  BatchNoteController,
  BatchCommentController,
  reportProgress,
  reportDone,
  syncTaskUI,
  startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType,
  pauseActiveTask,
  resumeActiveTask,
  getBatchNoteCtrl,
  setBatchNoteCtrl,
  getBatchCommentCtrl,
  setBatchCommentCtrl,
  getDouyinAdapter = null,
  douyinAdapter = null,
  releaseExecutionLock = defaultReleaseExecutionLock,
} = {}) {
  const resolveDouyinAdapter = () => {
    if (typeof getDouyinAdapter === 'function') {
      return getDouyinAdapter();
    }
    return douyinAdapter;
  };

  const dySyncTaskUI = (progress) => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    if (activeDouyinAdapter) {
      activeDouyinAdapter.syncTaskUI(progress);
    } else {
      syncTaskUI(progress);
    }
  };
  const dyStartBatchTask = (taskType) => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    if (activeDouyinAdapter) {
      activeDouyinAdapter.startBatchTask(taskType);
    } else {
      startBatchTask(taskType);
    }
  };
  const dyToggleStopButton = (show) => {
    if (!resolveDouyinAdapter()) toggleStopButton(show);
  };
  const dyHideTaskControlBar = () => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    if (activeDouyinAdapter) {
      activeDouyinAdapter.hideTaskControlBar();
    } else {
      hideTaskControlBar();
    }
  };
  const dySetActiveTaskType = (value) => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    if (activeDouyinAdapter) {
      activeDouyinAdapter.setActiveTaskType(value);
    } else {
      setActiveTaskType(value);
    }
  };
  const dyAttachExternalController = (controller) => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    if (activeDouyinAdapter?.attachExternalBatchController) {
      activeDouyinAdapter.attachExternalBatchController(controller || null);
    }
  };
  const dyPauseManagedTask = ({
    taskType,
    current = 0,
    total = 0,
    message = '检测到抖音安全验证，请先完成验证后点击“继续”。',
  } = {}) => {
    const activeDouyinAdapter = resolveDouyinAdapter();
    const controller = taskType === 'batchNotes'
      ? getBatchNoteCtrl?.()
      : getBatchCommentCtrl?.();
    controller?.pause?.();
    if (activeDouyinAdapter?.pauseBatch) {
      activeDouyinAdapter.pauseBatch({
        taskType,
        current,
        total,
        message,
      });
      return;
    }
    dySyncTaskUI({
      taskType,
      taskState: TASK_STATE.PAUSED,
      current,
      total,
      message,
    });
  };

  const startDouyinBatchNotes = createDouyinManagedBatchStartHandler({
    taskType: '批量视频采集',
    startMessageTaskType: 'batchNotes',
    createManagedTaskController,
    runBatchTask: (msg, { shouldStop, waitIfPaused, onCollectionRun, onManagedProgress }) => batchCollectDouyinProfileVideos({
      maxCount: Number(msg.count || 20),
      topByLikes: Boolean(msg.topByLikes),
      externalTaskMeta: msg.externalTaskMeta || {},
      monitorMeta: msg.monitorMeta || msg.externalTaskMeta?.monitorMeta || null,
      surfaceOnly: Boolean(msg.surfaceOnly),
      onCollectionRun,
      shouldStop,
      waitIfPaused,
      onProgress: onManagedProgress,
    }),
    buildDoneMessage: (result) => ({
      taskType: 'batchNotes',
      taskState: TASK_STATE.DONE,
      current: result.success || 0,
      total: result.total || 0,
      message: `批量视频完成：成功 ${result.success || 0}/${result.total || 0}`,
    }),
    buildStoppedMessage: (result) => ({
      taskType: 'batchNotes',
      taskState: TASK_STATE.IDLE,
      current: result.success || 0,
      total: result.total || 0,
      message: `批量视频已停止：成功 ${result.success || 0}/${result.total || 0}`,
    }),
    buildErrorMessage: (msg, err) => ({
      taskType: 'batchNotes',
      taskState: TASK_STATE.ERROR,
      current: 0,
      total: Number(msg.count || 20),
      message: `批量视频失败：${String(err?.message || err)}`,
    }),
    reportProgress,
    reportDone: (result) => reportDone('note', result.success || 0, { platform: 'douyin' }),
    syncTaskUI: dySyncTaskUI,
    startBatchTask: dyStartBatchTask,
    toggleStopButton: dyToggleStopButton,
    hideTaskControlBar: dyHideTaskControlBar,
    setActiveTaskType: dySetActiveTaskType,
    getController: getBatchNoteCtrl,
    setController: setBatchNoteCtrl,
    attachExternalController: dyAttachExternalController,
    pauseManagedTask: dyPauseManagedTask,
    releaseExecutionLock,
  });

  const startDouyinBatchComments = createDouyinManagedBatchStartHandler({
    taskType: '批量评论采集',
    startMessageTaskType: 'batchComments',
    createManagedTaskController,
    runBatchTask: (msg, { shouldStop, waitIfPaused, onCollectionRun, onManagedProgress }) => batchCollectDouyinProfileComments({
      maxCount: Number(msg.count || 10),
      topByLikes: Boolean(msg.topByLikes),
      maxCommentsPerVideo: Math.max(0, Number(msg.commentLimit || 0) || 0),
      commentDepthMode: normalizeCommentDepthMode(msg.commentDepthMode),
      externalTaskMeta: msg.externalTaskMeta || {},
      onCollectionRun,
      shouldStop,
      waitIfPaused,
      onProgress: onManagedProgress,
    }),
    buildDoneMessage: (result) => ({
      taskType: 'batchComments',
      taskState: TASK_STATE.DONE,
      current: result.successVideos || 0,
      total: result.totalVideos || 0,
      message: `批量评论完成：视频 ${result.successVideos || 0}/${result.totalVideos || 0}，评论 ${result.totalComments || 0} 条`,
    }),
    buildStoppedMessage: (result) => ({
      taskType: 'batchComments',
      taskState: TASK_STATE.IDLE,
      current: result.successVideos || 0,
      total: result.totalVideos || 0,
      message: `批量评论已停止：视频 ${result.successVideos || 0}/${result.totalVideos || 0}，评论 ${result.totalComments || 0} 条`,
    }),
    buildErrorMessage: (msg, err) => ({
      taskType: 'batchComments',
      taskState: TASK_STATE.ERROR,
      current: 0,
      total: Number(msg.count || 10),
      message: `批量评论失败：${String(err?.message || err)}`,
    }),
    reportProgress,
    reportDone: (result) => reportDone('comment', result.totalComments || 0, { platform: 'douyin' }),
    syncTaskUI: dySyncTaskUI,
    startBatchTask: dyStartBatchTask,
    toggleStopButton: dyToggleStopButton,
    hideTaskControlBar: dyHideTaskControlBar,
    setActiveTaskType: dySetActiveTaskType,
    getController: getBatchCommentCtrl,
    setController: setBatchCommentCtrl,
    attachExternalController: dyAttachExternalController,
    pauseManagedTask: dyPauseManagedTask,
    releaseExecutionLock,
  });

  const startXhsBatchNotes = createXhsBatchStartHandler({
    ControllerClass: BatchNoteController,
    taskType: 'batchNotes',
    reportProgress,
    syncTaskUI,
    startBatchTask,
    toggleStopButton,
    hideTaskControlBar,
    setActiveTaskType,
    setController: setBatchNoteCtrl,
    releaseExecutionLock,
    buildOptions: (msg) => ({
      count: msg.count || 10,
      topByLikes: Boolean(msg.topByLikes),
      searchFilters: normalizeXhsSearchFilters(msg.searchFilters || {}),
      includeComments: Boolean(msg.includeComments || msg.collectComments),
      commentLimit: Math.max(0, Number(msg.commentLimit ?? 20) || 0),
      commentDepthMode: normalizeCommentDepthMode(msg.commentDepthMode),
      targetNoteId: String(msg.targetNoteId || '').trim().replace(/^xhs_/, '') || undefined,
      triggerSource: String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual',
      externalTaskMeta: msg.externalTaskMeta || {},
      monitorMeta: msg.monitorMeta || msg.externalTaskMeta?.monitorMeta || null,
      surfaceOnly: Boolean(msg.surfaceOnly),
    }),
  });

  const startXhsBatchComments = createXhsBatchStartHandler({
    ControllerClass: BatchCommentController,
    taskType: 'batchComments',
    reportProgress,
    syncTaskUI,
    startBatchTask,
    toggleStopButton,
    hideTaskControlBar,
    setActiveTaskType,
    setController: setBatchCommentCtrl,
    releaseExecutionLock,
    buildOptions: (msg) => ({
      count: msg.count || 10,
      topByLikes: Boolean(msg.topByLikes),
      searchFilters: normalizeXhsSearchFilters(msg.searchFilters || {}),
      commentLimit: Math.max(0, Number(msg.commentLimit || 0) || 0),
      commentDepthMode: normalizeCommentDepthMode(msg.commentDepthMode),
      triggerSource: String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual',
      externalTaskMeta: msg.externalTaskMeta || {},
      noteList: Array.isArray(msg.noteList) ? msg.noteList : undefined,
      captchaActionTimeoutMs: msg.externalTaskMeta?.externalTaskId ? 45_000 : 0,
      noteCollectionTimeoutMs: msg.externalTaskMeta?.externalTaskId ? 120_000 : 0,
    }),
  });

  return {
    [MSG.START_BATCH_NOTES]: async (msg = {}) => (
      isDouyinPage() ? startDouyinBatchNotes(msg) : startXhsBatchNotes(msg)
    ),

    [MSG.STOP_BATCH_NOTES]: createStopHandler({
      isDouyinPage,
      getController: getBatchNoteCtrl,
      toggleStopButton,
      hideTaskControlBar,
      setActiveTaskType,
      getDouyinAdapter: resolveDouyinAdapter,
    }),

    [MSG.PAUSE_BATCH_NOTES]: () => {
      const activeDouyinAdapter = resolveDouyinAdapter();
      if (isDouyinPage() && activeDouyinAdapter) {
        getBatchNoteCtrl()?.pause?.();
        activeDouyinAdapter.pauseBatch();
      } else {
        pauseActiveTask();
      }
      return { success: true };
    },
    [MSG.RESUME_BATCH_NOTES]: () => {
      const activeDouyinAdapter = resolveDouyinAdapter();
      if (isDouyinPage() && activeDouyinAdapter) {
        getBatchNoteCtrl()?.resume?.();
        activeDouyinAdapter.resumeBatch();
      } else {
        resumeActiveTask();
      }
      return { success: true };
    },

    [MSG.START_BATCH_COMMENTS]: async (msg = {}) => (
      isDouyinPage() ? startDouyinBatchComments(msg) : startXhsBatchComments(msg)
    ),

    [MSG.STOP_BATCH_COMMENTS]: createStopHandler({
      isDouyinPage,
      getController: getBatchCommentCtrl,
      toggleStopButton,
      hideTaskControlBar,
      setActiveTaskType,
      getDouyinAdapter: resolveDouyinAdapter,
    }),

    [MSG.PAUSE_BATCH_COMMENTS]: () => {
      const activeDouyinAdapter = resolveDouyinAdapter();
      if (isDouyinPage() && activeDouyinAdapter) {
        getBatchCommentCtrl()?.pause?.();
        activeDouyinAdapter.pauseBatch();
      } else {
        pauseActiveTask();
      }
      return { success: true };
    },
    [MSG.RESUME_BATCH_COMMENTS]: () => {
      const activeDouyinAdapter = resolveDouyinAdapter();
      if (isDouyinPage() && activeDouyinAdapter) {
        getBatchCommentCtrl()?.resume?.();
        activeDouyinAdapter.resumeBatch();
      } else {
        resumeActiveTask();
      }
      return { success: true };
    },
  };
}
