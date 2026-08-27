import '../extensionPublicPath.js';
import '../content.css';
import { MSG } from '../shared/constants.js';
import { discoverWithScroll } from '../platforms/xhs/noteCollector.js';
import { collectXhsNoteDetailPackage } from '../platforms/xhs/detailPackageCollector.js';
import { readCurrentXhsSearchSurfaceContext } from '../platforms/xhs/searchFilters.js';
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
let activeDouyinAdapter = null;
const dashboardBridge = createDashboardBridge({
  noteStore,
  commentStore,
  authorStore,
  // The familiar dashboard still reads the mature local execution cache, while this action now
  // enters the same Linggan-owned media lane as page controls. It never invokes legacy downloads.
  downloadNoteMediaFromRecord: (note) => runtime.acquireMediaSlots(note),
});

class LingganBatchNoteController extends BatchNoteController {
  constructor(...args) {
    super(...args);
    // A note-detail package includes its attached comments. Install the existing bridge only
    // when a batch collection controller is actually created, never on ordinary page load.
    ensureXhsCommentApiBridge();
  }

  _emitProgress(payload) {
    super._emitProgress(payload);
    void runtime.submitBatchCheckpoint('xhs_batch_notes', payload).catch(() => {});
  }
}

class LingganBatchCommentController extends BatchCommentController {
  constructor(...args) {
    super(...args);
    // The page bridge is needed by the mature comment collector, not by the passive
    // current-surface discovery control.  Keep initialization at the actual comment action.
    ensureXhsCommentApiBridge();
  }

  _emitProgress(payload) {
    super._emitProgress(payload);
    void runtime.submitBatchCheckpoint('xhs_batch_comments', payload).catch(() => {});
  }
}

const xhsPageController = createXhsPageController({
  MSG,
  assertPluginAuthorized: localTrustedAuthorization,
  collectNote: async (...args) => {
    // A direct detail request carries its attached comments in the same logical package.
    // The bridge remains task-triggered rather than becoming an always-on page observer.
    ensureXhsCommentApiBridge();
    const result = await collectXhsNoteDetailPackage(...args);
    return result.note;
  },
  collectComments: async (...args) => {
    // Do not install the old page bridge when a search/profile page merely loads.  It is only
    // needed if a user explicitly starts comment collection.
    ensureXhsCommentApiBridge();
    return collectComments(...args);
  },
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
  discoverSurface: async ({ mode, maximumQuota }) => {
    const expectedCount = Math.max(1, Number(maximumQuota) || 20);
    const cards = await discoverWithScroll(
      mode === 'profile' ? '#userPostedFeeds' : '.feeds-container',
      undefined,
      { expectedCount },
    );
    if (mode === 'profile') {
      return { cards, discoveryMeta: cards.discoveryMeta };
    }
    return {
      cards,
      pageFacts: readCurrentXhsSearchSurfaceContext({
        requestedLimit: expectedCount,
        loadedCount: cards.length,
        stopReason: cards.discoveryMeta?.stopReason,
      }),
      discoveryMeta: cards.discoveryMeta,
    };
  },
  submitDiscovery: (cards, context) => runtime.submitDiscovery(cards, context),
});

async function initXhs() {
  registerCollectorReceiptSink({
    contentDetail: (note) => runtime.submitContentDetail(note),
    mediaSlots: (note) => runtime.submitMediaSlots(note),
    comments: (result, context) => runtime.submitComments(result, context?.noteId || extractNoteId(location.href), context?.options || {}),
    authorProfile: (author) => runtime.submitAuthor(author),
    batchCheckpoint: (progress, context) => runtime.submitBatchCheckpoint(context?.kind || 'xhs_batch', progress),
  });
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
    batchCheckpoint: (progress, context) => douyinRuntime.submitBatchCheckpoint(context?.kind || 'douyin_batch', progress),
  });
  activeDouyinAdapter = module.DouyinAdapter;
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

function dispatchXhsRuntimeAction(nextAction, params = {}) {
  // Reuse the mature XHS page controller without allowing Popup to invoke the old message
  // catalogue.  The synthetic element only supplies the existing UI action contract.
  const button = { dataset: { action: nextAction, params: JSON.stringify(params || {}) } };
  return xhsPageController.handleButtonClick({
    target: { closest: (selector) => selector === '.lgboom-btn' ? button : null },
  });
}

async function dispatchProducerRuntimeAction(action, message) {
  const isDouyin = platform() === 'douyin';
  const map = isDouyin
    ? {
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_CONTENT]: 'dy_collectVideo',
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS]: 'dy_collectComments',
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR]: 'dy_collectAuthor',
      [LINGGAN_RUNTIME_ACTION.START_BATCH_CONTENT]: 'dy_batchVideos',
      [LINGGAN_RUNTIME_ACTION.START_BATCH_COMMENTS]: 'dy_batchComments',
      [LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA]: 'dy_collectCommentImages',
      [LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH]: 'dy_pauseBatch',
      [LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH]: 'dy_resumeBatch',
      [LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH]: 'dy_stopBatch',
    }
    : {
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_CONTENT]: 'collectNote',
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS]: 'collectComment',
      [LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR]: 'collectAuthor',
      [LINGGAN_RUNTIME_ACTION.START_BATCH_CONTENT]: 'batchNotes',
      [LINGGAN_RUNTIME_ACTION.START_BATCH_COMMENTS]: 'batchComments',
      [LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA]: 'collectCommentImages',
      [LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH]: 'pauseBatch',
      [LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH]: 'resumeBatch',
      [LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH]: 'stopBatch',
    };
  const pageAction = map[action];
  if (!pageAction) return null;
  if (action === LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA) {
    return {
      success: true,
      state: 'not_available',
      message: '评论图片区尚未具备 Linggan MediaSlot 回传合同，未执行下载。',
    };
  }
  const params = {
    mode: message.mode,
    count: message.count,
    topByLikes: message.topByLikes,
    searchFilters: message.searchFilters,
    commentLimit: message.commentLimit,
    commentDepthMode: message.commentDepthMode,
    maxTotal: message.maxTotal,
    maxSubComments: message.maxSubComments,
    maximumQuota: message.maximumQuota ?? message.count,
    sortMode: message.sortMode,
    triggerSource: 'popup_linggan_runtime',
  };
  const isControl = [
    LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH,
  ].includes(action);
  let pageResult;
  if (isDouyin) {
    if (!activeDouyinAdapter) throw new Error('linggan_douyin_runtime_not_ready');
    pageResult = await activeDouyinAdapter.handleButtonClick(pageAction, params);
  } else {
    pageResult = await dispatchXhsRuntimeAction(pageAction, params);
  }
  // This means the page reader actually accepted the action. It is deliberately not an
  // admission receipt; Popup must continue to describe delivery as pending until one exists.
  if (isControl) {
    // Do not infer controls from a dispatched message. The controller is the only authority on
    // whether an active task actually transitioned state.
    return pageResult && typeof pageResult === 'object'
      ? pageResult
      : { success: false, state: 'no_active_task' };
  }
  return { success: true, state: 'page_read_started', delivery: 'pending' };
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
  if ([
    LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_CONTENT,
    LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS,
    LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR,
    LINGGAN_RUNTIME_ACTION.START_BATCH_CONTENT,
    LINGGAN_RUNTIME_ACTION.START_BATCH_COMMENTS,
    LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA,
    LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH,
  ].includes(action)) {
    dispatchProducerRuntimeAction(action, message)
      .then((result) => sendResponse(result || { success: false, code: 'linggan_page_action_unavailable' }))
      .catch((error) => sendResponse({ success: false, code: 'linggan_page_action_failed', message: String(error?.message || error) }));
    return true;
  }
  return false;
});

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => { void init(); }, { once: true });
else void init();
