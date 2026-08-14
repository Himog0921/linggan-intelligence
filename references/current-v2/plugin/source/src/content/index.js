import '../extensionPublicPath.js';
import { MSG } from '../shared/constants.js';
import { collectNote, discoverSurfaceNotesFromBestSource } from '../platforms/xhs/noteCollector.js';
import { collectComments, collectCommentImages } from '../platforms/xhs/commentCollector.js';
import { collectAuthor } from '../platforms/xhs/authorCollector.js';
import { BatchNoteController, BatchCommentController } from '../platforms/xhs/batchController.js';
import { ensureXhsCommentApiBridge } from '../platforms/xhs/commentApi.js';
import {
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
} from '../platforms/xhs/uiInjector.js';
import { createBatchMessageHandlers } from './douyinBatchMessageHandlers.js';
import { CONTENT_PLATFORM, createContentRouter } from './contentRouter.js';
import { loadContentDataRuntimeFactory } from './contentDataRuntimeLoader.js';
import { loadDouyinRuntime } from './douyinRuntime.js';
import { createRuntimeMessageListener } from './messageListener.js';
import { createXhsPageController } from './xhsPageController.js';
import {
  reportDone,
  reportProgress,
  reportWorkbenchRecord,
  isContextValid,
  sendToBackground,
} from '../shared/messaging.js';
import { generateCsv, downloadFile, extractNoteId } from '../shared/utils.js';
import { createManagedTaskController } from '../shared/managedTaskController.js';
import { assertActivePluginAuthorization } from '../workbench/runtime/pluginAuthorization.js';
import '../content.css';
import { initThemeManager } from '../themes/themeManager.js';

// 初始化主题管理器（content script 层级）
initThemeManager().catch(() => {});

async function callDouyinRuntime(method, ...args) {
  const douyinRuntime = await loadDouyinRuntime();
  const fn = douyinRuntime?.[method];
  if (typeof fn !== 'function') {
    throw new Error(`抖音运行时缺少方法：${method}`);
  }
  return fn(...args);
}

let contentDataRuntimeInstance = null;
let contentDataRuntimePromise = null;
let dashboardBridgeListenerRegistered = false;
let douyinAdapterRef = null;
let douyinClickListenerRegistered = false;
let douyinClickListener = null;

async function assertPluginAuthorized() {
  return assertActivePluginAuthorization();
}

async function loadContentDataRuntime() {
  if (contentDataRuntimeInstance) {
    return contentDataRuntimeInstance;
  }
  if (!contentDataRuntimePromise) {
    contentDataRuntimePromise = loadContentDataRuntimeFactory()
      .then((createContentDataRuntime) => createContentDataRuntime({
        MSG,
        isDouyinPage,
        collectNote,
        collectComments,
        collectAuthor,
        collectDouyinVideo: (...args) => callDouyinRuntime('collectDouyinVideo', ...args),
        collectDouyinComments: (...args) => callDouyinRuntime('collectDouyinComments', ...args),
        downloadDouyinCommentImages: (...args) => callDouyinRuntime('downloadDouyinCommentImages', ...args),
        collectDouyinAuthor: (...args) => callDouyinRuntime('collectDouyinAuthor', ...args),
        BatchNoteController,
        reportDone,
        reportProgress,
        reportWorkbenchRecord,
        batchMessageHandlers,
        getBatchNoteCtrl: xhsPageController.getBatchNoteCtrl,
        getBatchCommentCtrl: xhsPageController.getBatchCommentCtrl,
        extractNoteId,
        generateCsv,
        downloadFile,
        sendToBackground,
        loadDouyinRuntime,
        discoverXhsSurfaceNotes: discoverSurfaceNotesFromBestSource,
        discoverDouyinSurfaceTargets: (...args) => callDouyinRuntime('discoverDouyinBatchTargets', ...args),
        assertPluginAuthorized,
      }))
      .then((runtime) => {
        contentDataRuntimeInstance = runtime;
        return runtime;
      })
      .catch((error) => {
        contentDataRuntimePromise = null;
        throw error;
      });
  }
  return contentDataRuntimePromise;
}

function registerDashboardBridgeListener() {
  if (dashboardBridgeListenerRegistered) return;
  dashboardBridgeListenerRegistered = true;
  window.addEventListener('message', async (event) => {
    if (event.data?.source !== 'lgboom-dashboard') return;
    const runtime = await loadContentDataRuntime();
    await runtime.dashboardBridge.handleDashboardMessageEvent(event);
  });
}

const xhsPageController = createXhsPageController({
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
  downloadNoteMediaFromRecord: async (note, options = {}) => {
    const runtime = await loadContentDataRuntime();
    return runtime.downloadNoteMediaFromRecord(note, options);
  },
});

async function initDouyinPage() {
  const douyinRuntime = await loadDouyinRuntime();
  const { DouyinAdapter } = douyinRuntime;
  douyinAdapterRef = DouyinAdapter;
  DouyinAdapter.init();
  if (douyinClickListenerRegistered) {
    console.log('[灵感爆爆爆] 抖音模式已启动');
    return;
  }
  douyinClickListener = (e) => {
    const btn = e.target.closest('.lgboom-dy-btn, .lgboom-dy-task-btn');
    if (!btn) return;
    e.preventDefault();
    const action = btn.dataset.action;
    const params = btn.dataset.params ? JSON.parse(btn.dataset.params) : {};
    Promise.resolve(DouyinAdapter.handleButtonClick(action, params)).catch((err) => {
      console.error('[灵感爆爆爆] 抖音按钮动作失败:', err);
    });
  };
  document.addEventListener('click', douyinClickListener);
  douyinClickListenerRegistered = true;
  console.log('[灵感爆爆爆] 抖音模式已启动');
}

function initXhsPage() {
  ensureXhsCommentApiBridge();
  xhsPageController.initPage();
  console.log('[灵感爆爆爆] 插件已加载');
}

const contentRouter = createContentRouter({
  getHostname: () => location.hostname,
  initByPlatform: {
    [CONTENT_PLATFORM.DOUYIN]: initDouyinPage,
    [CONTENT_PLATFORM.XHS]: initXhsPage,
  },
});

function isDouyinPage() {
  return contentRouter.resolvePlatform() === CONTENT_PLATFORM.DOUYIN;
}

// ========== 初始化 ==========

async function init() {
  await contentRouter.init();
}

registerDashboardBridgeListener();

// ========== 消息监听（来自 popup / background / dashboard） ==========

if (isContextValid()) {
  chrome.runtime.onMessage.addListener(createRuntimeMessageListener({
    loadContentDataRuntime,
    isContextValid,
  }));
}

const batchMessageHandlers = createBatchMessageHandlers({
  isDouyinPage,
  createManagedTaskController,
  batchCollectDouyinProfileVideos: (options) => callDouyinRuntime('batchCollectDouyinProfileVideos', options),
  batchCollectDouyinProfileComments: (options) => callDouyinRuntime('batchCollectDouyinProfileComments', options),
  BatchNoteController,
  BatchCommentController,
  reportProgress,
  reportDone,
  syncTaskUI: xhsPageController.syncTaskUI,
  startBatchTask: xhsPageController.startBatchTask,
  toggleStopButton,
  hideTaskControlBar,
  setActiveTaskType: xhsPageController.setActiveTaskType,
  pauseActiveTask: xhsPageController.pauseActiveTask,
  resumeActiveTask: xhsPageController.resumeActiveTask,
  getBatchNoteCtrl: xhsPageController.getBatchNoteCtrl,
  setBatchNoteCtrl: xhsPageController.setBatchNoteCtrl,
  getBatchCommentCtrl: xhsPageController.getBatchCommentCtrl,
  setBatchCommentCtrl: xhsPageController.setBatchCommentCtrl,
  getDouyinAdapter: () => douyinAdapterRef,
});

// ========== 启动 ==========

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
