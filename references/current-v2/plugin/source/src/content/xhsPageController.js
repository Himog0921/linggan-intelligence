import { createCommentTaskController } from './commentTaskController.js';
import { createCommentImageTaskController } from './commentImageTask.js';
import { COLLECT_MODE, COMMENT_DEPTH_MODE, TASK_STATE } from '../shared/constants.js';
import { consumeSelectorHealthAlertMessage } from '../shared/selectorHealth.js';
import { isTerminalTaskState, resolveTaskState } from '../shared/taskUi.js';
import {
  runXhsSelectorBootstrapProbe,
  runXhsSelectorPreflight,
} from '../platforms/xhs/selectorHealth.js';

const XHS_CONTEXT_REFRESH_MESSAGE = '插件刚更新，请刷新当前页面后再点一次，刷新后即可继续。';

export function createXhsPageController({
  MSG,
  assertPluginAuthorized,
  collectComments,
  collectCommentImages,
  collectNote,
  collectAuthor,
  BatchNoteController,
  BatchCommentController,
  injectUI,
  toggleStopButton,
  togglePauseResumeButtons,
  showToast,
  showCommentLimitDialog,
  showMediaDownloadDialog,
  showBatchSettingsDialog,
  ensureTaskControlBar,
  updateTaskControlBar,
  hideTaskControlBar,
  isContextValid,
  reportDone,
  extractNoteId,
  sendToBackground,
  downloadNoteMediaFromRecord,
} = {}) {
  let batchNoteCtrl = null;
  let batchCommentCtrl = null;
  let singleCommentCtrl = null;
  let lastUrl = '';
  let activeTaskType = null;
  let selectorProbeTimer = null;
  let lastTaskSnapshot = null;
  let pageInitialized = false;
  let pageObserver = null;
  let reinjectTimer = null;
  let reinjectPending = false;

  async function ensurePluginAuthorized() {
    if (typeof assertPluginAuthorized === 'function') {
      return assertPluginAuthorized();
    }
    return null;
  }

  function syncTaskUI(progress = {}) {
    const taskType = progress.taskType || activeTaskType;
    if (taskType == null) return;
    const taskState = resolveTaskState({
      taskState: progress.taskState,
      status: progress.status,
      fallback: TASK_STATE.RUNNING,
    });
    activeTaskType = taskType;
    lastTaskSnapshot = {
      taskType,
      taskState,
      total: progress.total || 0,
      current: progress.current || 0,
      message: progress.message || '',
    };
    updateTaskControlBar({
      visible: true,
      taskType,
      taskState: lastTaskSnapshot.taskState,
      total: lastTaskSnapshot.total,
      current: lastTaskSnapshot.current,
      message: lastTaskSnapshot.message,
    });

    if (progress.status === TASK_STATE.DONE || isTerminalTaskState(taskState)) {
      toggleStopButton(false);
      activeTaskType = null;
      lastTaskSnapshot = null;
      hideTaskControlBar();
    }
  }

  function startBatchTask(taskType) {
    activeTaskType = taskType;
    toggleStopButton(true);
    syncTaskUI({
      taskType,
      taskState: TASK_STATE.RUNNING,
      total: 0,
      current: 0,
      message: '准备中',
    });
  }

  const commentImageController = createCommentImageTaskController({
    MSG,
    collectCommentImages,
    sendToBackground,
    extractNoteId,
    showToast,
    syncTaskUI,
    startBatchTask,
    toggleStopButton,
    hideTaskControlBar,
    setActiveTaskType: (value) => {
      activeTaskType = value;
    },
  });

  singleCommentCtrl = createCommentTaskController({
    collectComments,
    showToast,
    syncTaskUI,
    startBatchTask,
    toggleStopButton,
    hideTaskControlBar,
    setActiveTaskType: (value) => {
      activeTaskType = value;
    },
  });

  function pauseActiveTask() {
    if (singleCommentCtrl?.isRunning()) singleCommentCtrl.pause();
    if (batchNoteCtrl?.isRunning) batchNoteCtrl.pause();
    if (batchCommentCtrl?.isRunning) batchCommentCtrl.pause();
    if (commentImageController?.isRunning()) commentImageController.pause();
    togglePauseResumeButtons(true);
    if (commentImageController?.isRunning() || singleCommentCtrl?.isRunning()) return;
    const current = Number(lastTaskSnapshot?.current || 0);
    const total = Number(lastTaskSnapshot?.total || 0);
    syncTaskUI({
      taskType: lastTaskSnapshot?.taskType || activeTaskType || (batchCommentCtrl?.isRunning ? 'batchComments' : 'batchNotes'),
      taskState: TASK_STATE.PAUSED,
      message: '已暂停',
      total,
      current,
    });
  }

  function resumeActiveTask() {
    if (singleCommentCtrl?.isRunning()) singleCommentCtrl.resume();
    if (batchNoteCtrl?.isRunning) batchNoteCtrl.resume();
    if (batchCommentCtrl?.isRunning) batchCommentCtrl.resume();
    if (commentImageController?.isRunning()) commentImageController.resume();
    togglePauseResumeButtons(false);
    if (commentImageController?.isRunning() || singleCommentCtrl?.isRunning()) return;
    const current = Number(lastTaskSnapshot?.current || 0);
    const total = Number(lastTaskSnapshot?.total || 0);
    syncTaskUI({
      taskType: lastTaskSnapshot?.taskType || activeTaskType || (batchCommentCtrl?.isRunning ? 'batchComments' : 'batchNotes'),
      taskState: TASK_STATE.RUNNING,
      message: '继续采集',
      total,
      current,
    });
  }

  function scheduleSelectorBootstrapProbe(delayMs = 420) {
    clearTimeout(selectorProbeTimer);
    selectorProbeTimer = setTimeout(() => {
      selectorProbeTimer = null;
      const result = runXhsSelectorBootstrapProbe({
        document,
        win: window,
      });
      const alertMessage = consumeSelectorHealthAlertMessage(result, { win: window });
      if (alertMessage) {
        showToast(alertMessage, 'warning');
      }
    }, delayMs);
  }

  function clearLifecycleTimers() {
    clearTimeout(selectorProbeTimer);
    clearTimeout(reinjectTimer);
    selectorProbeTimer = null;
    reinjectTimer = null;
    reinjectPending = false;
  }

  async function handleButtonClick(e) {
    const btn = e.target.closest('.lgboom-btn');
    if (btn == null) return;

    const action = btn.dataset.action;
    const params = btn.dataset.params ? JSON.parse(btn.dataset.params) : {};

    if (isContextValid() === false) {
      showToast(XHS_CONTEXT_REFRESH_MESSAGE, 'warning');
      return;
    }

    const preflight = runXhsSelectorPreflight(action, {
      params,
      document,
      win: window,
    });
    if (preflight.ok === false) {
      showToast(preflight.message || '当前页面结构未通过预检，请刷新后重试。', 'warning');
      return;
    }

    try {
      switch (action) {
        case 'collectNote': {
          showToast('正在采集笔记...', 'info');
          const note = await collectNote();
          showToast(`笔记采集成功：${note.title}`, 'success');
          reportDone('note', 1);
          if ((note.images && note.images.length > 0) || note.video || note.cover || note.coverUrl || note.livePhotoStreams?.length > 0) {
            const mediaCount = (note.images?.length || 0) + (note.video ? 1 : 0);
            try {
              const selection = await showMediaDownloadDialog(note);
              const mediaTypes = selection === true ? undefined : selection?.mediaTypes;
              if (selection && (selection === true || mediaTypes?.length > 0)) {
                showToast(`正在下载 ${selection?.count || mediaCount} 个媒体文件...`, 'info');
                const summary = await downloadNoteMediaFromRecord(note, { mediaTypes });
                showToast(
                  summary.zipped
                    ? `媒体下载完成：已打包 ZIP（成功 ${summary.success}/${summary.total}，失败 ${summary.failed}）`
                    : `媒体下载完成：成功 ${summary.success}/${summary.total}，失败 ${summary.failed}`,
                  summary.failed > 0 ? 'warning' : 'success',
                );
              }
            } catch {
              // user cancelled
            }
          }
          break;
        }

        case 'collectComment': {
          await ensurePluginAuthorized();
          let commentSettings;
          try {
            commentSettings = await showCommentLimitDialog({
              title: '单篇评论设置',
              description: '选择评论上限和采集深度。留空或填 0 表示不限；“尽量全部回复”会继续展开更多回复。',
              confirmText: '开始采集',
            });
          } catch {
            break;
          }
          await singleCommentCtrl.start({
            noteId: extractNoteId(window.location.href),
            noteUrl: window.location.href,
            maxTotal: commentSettings.maxComments,
            maxSubComments: commentSettings.commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : 200,
            commentDepthMode: commentSettings.commentDepthMode,
          });
          reportDone('comment', 0);
          break;
        }

        case 'collectAuthor': {
          await ensurePluginAuthorized();
          showToast('正在采集博主信息...', 'info');
          const author = await collectAuthor();
          showToast(`博主采集成功：${author.name}`, 'success');
          reportDone('author', 1);
          break;
        }

        case 'batchNotes': {
          await ensurePluginAuthorized();
          let batchSettings;
          try {
            batchSettings = await showBatchSettingsDialog({
              title: '批量采集笔记',
              enableTopLikes: params.mode !== COLLECT_MODE.SEARCH,
              enableSearchFilters: params.mode === COLLECT_MODE.SEARCH,
            });
          } catch {
            break;
          }
          batchNoteCtrl = new BatchNoteController();
          startBatchTask('batchNotes');
          await batchNoteCtrl.start(params.mode, (p) => {
            const taskState = resolveTaskState({
              taskState: p.taskState,
              status: p.status,
              fallback: TASK_STATE.RUNNING,
            });
            if (p.message) showToast(p.message, taskState === TASK_STATE.DONE ? 'success' : 'info');
            syncTaskUI({ ...p, taskState });
            if (taskState === TASK_STATE.DONE) {
              toggleStopButton(false);
              hideTaskControlBar();
              activeTaskType = null;
            }
          }, batchSettings);
          break;
        }

        case 'batchComments': {
          await ensurePluginAuthorized();
          let batchSettings;
          try {
            batchSettings = await showBatchSettingsDialog({
              title: '批量采集评论',
              enableTopLikes: params.mode !== COLLECT_MODE.SEARCH,
              enableSearchFilters: params.mode === COLLECT_MODE.SEARCH,
              enableCommentDepth: true,
              enableCommentLimit: true,
            });
          } catch {
            break;
          }
          batchCommentCtrl = new BatchCommentController();
          startBatchTask('batchComments');
          await batchCommentCtrl.start(params.mode, (p) => {
            const taskState = resolveTaskState({
              taskState: p.taskState,
              status: p.status,
              fallback: TASK_STATE.RUNNING,
            });
            if (p.message) showToast(p.message, taskState === TASK_STATE.DONE ? 'success' : 'info');
            syncTaskUI({ ...p, taskState });
            if (taskState === TASK_STATE.DONE) {
              toggleStopButton(false);
              hideTaskControlBar();
              activeTaskType = null;
            }
          }, {
            count: batchSettings.count || 10,
            topByLikes: Boolean(batchSettings.topByLikes),
            searchFilters: batchSettings.searchFilters,
            commentLimit: Number(batchSettings.commentLimit || 0) || 0,
            commentDepthMode: batchSettings.commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL,
          });
          break;
        }

        case 'stopBatch':
          singleCommentCtrl?.stop();
          batchNoteCtrl?.stop();
          batchCommentCtrl?.stop();
          if (commentImageController?.isRunning()) {
            commentImageController.stop();
            showToast('已停止采集', 'warning');
            break;
          }
          toggleStopButton(false);
          hideTaskControlBar();
          activeTaskType = null;
          showToast('已停止采集', 'warning');
          break;

        case 'pauseBatch':
          pauseActiveTask();
          showToast('批量采集已暂停', 'warning');
          break;

        case 'resumeBatch':
          resumeActiveTask();
          showToast('批量采集继续中...', 'info');
          break;

        case 'collectCommentImages':
          await ensurePluginAuthorized();
          await commentImageController.start();
          break;
      }
    } catch (err) {
      console.error('[灵感爆爆爆]', err);
      const errMsg = String(err?.message || '');
      if (/Extension context invalidated|context invalidated/i.test(errMsg) || isContextValid() === false) {
        showToast(XHS_CONTEXT_REFRESH_MESSAGE, 'warning');
        return;
      }
      if (action === 'batchNotes' || action === 'batchComments') {
        toggleStopButton(false);
        hideTaskControlBar();
        activeTaskType = null;
      }
      showToast(`操作失败：${err.message}`, 'error');
    }
  }

  function initPage() {
    if (pageInitialized) return;
    pageInitialized = true;
    lastUrl = window.location.href;
    injectUI();
    ensureTaskControlBar();
    scheduleSelectorBootstrapProbe(420);

    pageObserver = new MutationObserver(() => {
      if (window.__lgboom_injecting) return;
      const urlChanged = window.location.href !== lastUrl;
      const uiMissing = !document.querySelector('.lgboom-btn-group');
      if (urlChanged === false && uiMissing === false) return;
      if (reinjectPending) return;

      reinjectPending = true;
      clearTimeout(reinjectTimer);
      reinjectTimer = setTimeout(() => {
        lastUrl = window.location.href;
        reinjectPending = false;
        injectUI();
        scheduleSelectorBootstrapProbe(urlChanged ? 420 : 760);
      }, urlChanged ? 280 : 680);
    });
    pageObserver.observe(document.body, { childList: true, subtree: true });

    document.addEventListener('click', handleButtonClick);
  }

  function cleanupPage() {
    clearLifecycleTimers();
    pageObserver?.disconnect();
    pageObserver = null;
    if (pageInitialized) {
      document.removeEventListener('click', handleButtonClick);
    }
    pageInitialized = false;
    lastUrl = '';
  }

  return {
    initPage,
    cleanupPage,
    handleButtonClick,
    syncTaskUI,
    startBatchTask,
    pauseActiveTask,
    resumeActiveTask,
    getBatchNoteCtrl: () => batchNoteCtrl,
    setBatchNoteCtrl: (value) => {
      batchNoteCtrl = value;
    },
    getBatchCommentCtrl: () => batchCommentCtrl,
    setBatchCommentCtrl: (value) => {
      batchCommentCtrl = value;
    },
    setActiveTaskType: (value) => {
      activeTaskType = value;
    },
  };
}
