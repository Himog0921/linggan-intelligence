import '../extensionPublicPath.js';
import '../content.css';
import { MSG } from '../shared/constants.js';
import { collectNote } from '../platforms/xhs/noteCollector.js';
import { collectComments, collectCommentImages } from '../platforms/xhs/commentCollector.js';
import { collectAuthor } from '../platforms/xhs/authorCollector.js';
import { BatchNoteController, BatchCommentController } from '../platforms/xhs/batchController.js';
import { ensureXhsCommentApiBridge } from '../platforms/xhs/commentApi.js';
import {
  injectUI, toggleStopButton, togglePauseResumeButtons, showToast, showCommentLimitDialog,
  showMediaDownloadDialog, showBatchSettingsDialog, ensureTaskControlBar, updateTaskControlBar,
  hideTaskControlBar,
} from '../platforms/xhs/uiInjector.js';
import { initThemeManager } from '../themes/themeManager.js';
import { createXhsPageController } from './xhsPageController.js';
import { isContextValid, reportDone, sendToBackground } from '../shared/messaging.js';
import { extractNoteId } from '../shared/utils.js';
import { createLingganContentRuntime } from '../linggan/contentRuntimeAdapter.js';
import { LINGGAN_RUNTIME_ACTION } from '../linggan/runtimeActions.js';
import { unavailableLingganStats } from '../linggan/adapter.js';
import { loadDouyinRuntime } from './douyinRuntime.js';
import { registerCollectorReceiptSink } from '../runtime/collectorReceiptSink.js';
import { createDashboardBridge } from './dashboardBridge.js';
import { noteStore } from '../db/noteStore.js';
import { commentStore } from '../db/commentStore.js';
import { authorStore } from '../db/authorStore.js';

function platform() {
  const host = String(location.hostname || '').toLowerCase();
  return host.includes('douyin.com') ? 'douyin' : 'xhs';
}

function localTrustedAuthorization() {
  return Promise.resolve({ mode: 'LOCAL_TRUSTED', authorized: true });
}

const runtime = createLingganContentRuntime({ platform: 'xhs' });
const dashboardBridge = createDashboardBridge({
  noteStore,
  commentStore,
  authorStore,
  // The familiar dashboard still reads the mature local execution cache, while this action now
  // enters the same Linggan-owned media lane as page controls. It never invokes legacy downloads.
  downloadNoteMediaFromRecord: (note) => runtime.acquireMediaSlots(note),
});

class LingganBatchNoteController extends BatchNoteController {
  _emitProgress(payload) {
    super._emitProgress(payload);
    void runtime.submitBatchCheckpoint('xhs_batch_notes', payload).catch(() => {});
  }
}

class LingganBatchCommentController extends BatchCommentController {
  _emitProgress(payload) {
    super._emitProgress(payload);
    void runtime.submitBatchCheckpoint('xhs_batch_comments', payload).catch(() => {});
  }
}

const xhsPageController = createXhsPageController({
  MSG,
  assertPluginAuthorized: localTrustedAuthorization,
  collectNote,
  collectComments,
  collectAuthor,
  collectCommentImages: async (...args) => collectCommentImages(...args),
  BatchNoteController: LingganBatchNoteController,
  BatchCommentController: LingganBatchCommentController,
  injectUI,
  toggleStopButton,
  togglePauseResumeButtons,
  showToast,
  showCommentLimitDialog,
  // Familiar selection remains. Its old browser-download destination is intentionally replaced
  // by Linggan's media-slot lane; it never writes a temporary machine path as evidence.
  showMediaDownloadDialog,
  showBatchSettingsDialog,
  ensureTaskControlBar,
  updateTaskControlBar,
  hideTaskControlBar,
  isContextValid,
  reportDone,
  extractNoteId,
  sendToBackground,
  downloadNoteMediaFromRecord: async (note) => runtime.acquireMediaSlots(note),
});

async function initXhs() {
  registerCollectorReceiptSink({
    contentDetail: (note) => runtime.submitContentDetail(note),
    mediaSlots: (note) => runtime.submitMediaSlots(note),
    comments: (result, context) => runtime.submitComments(result, context?.noteId || extractNoteId(location.href), context?.options || {}),
    authorProfile: (author) => runtime.submitAuthor(author),
    batchCheckpoint: (progress, context) => runtime.submitBatchCheckpoint(context?.kind || 'xhs_batch', progress),
  });
  ensureXhsCommentApiBridge();
  dashboardBridge.registerDashboardBridge();
  xhsPageController.initPage();
  console.info('[Linggan Intelligence Browser] XHS collector runtime active.');
}

async function initDouyin() {
  // Keep mature page readers and interaction locations. Its collector bridge uses the same
  // background contract; no Workbench runtime, authorization, station or poller is revived.
  const module = await loadDouyinRuntime();
  const douyinRuntime = createLingganContentRuntime({ platform: 'douyin' });
  module.DouyinAdapter.setRuntimeSink({
    contentDetail: (content) => douyinRuntime.submitContentDetail(content),
    mediaSlots: (content) => douyinRuntime.submitMediaSlots(content),
    comments: (result, context) => douyinRuntime.submitComments(result, context?.noteId || '', {}),
    authorProfile: (author) => douyinRuntime.submitAuthor(author),
  });
  module.DouyinAdapter.init();
  document.addEventListener('click', (event) => {
    const button = event.target.closest('.lgboom-dy-btn, .lgboom-dy-task-btn');
    if (!button) return;
    event.preventDefault();
    const params = button.dataset.params ? JSON.parse(button.dataset.params) : {};
    Promise.resolve(module.DouyinAdapter.handleButtonClick(button.dataset.action, params)).catch((error) => {
      console.error('[Linggan Intelligence Browser] Douyin collector action failed', error);
    });
  });
  console.info('[Linggan Intelligence Browser] Douyin collector runtime active.');
}

async function init() {
  await initThemeManager().catch(() => {});
  if (platform() === 'douyin') await initDouyin();
  else await initXhs();
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '');
  if (action === LINGGAN_RUNTIME_ACTION.GET_PAGE_CONTEXT) {
    sendResponse({ success: true, context: { platform: platform(), url: location.href, mode: 'linggan_browser_producer_runtime' } });
    return true;
  }
  if (action === LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD && platform() === 'xhs') {
    dashboardBridge.toggleDashboard().then(() => sendResponse({ success: true })).catch((error) => {
      sendResponse({ success: false, code: 'dashboard_not_opened', message: String(error?.message || error) });
    });
    return true;
  }
  if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) {
    sendResponse(unavailableLingganStats('not_read'));
    return true;
  }
  return false;
});

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => { void init(); }, { once: true });
else void init();
