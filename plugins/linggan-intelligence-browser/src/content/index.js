import '../extensionPublicPath.js';
import '../content.css';
import { COLLECT_MODE, COMMENT_DEPTH_MODE, MSG } from '../shared/constants.js';
import { buildDiscoveryExecutionSummary, discoverWithScroll } from '../platforms/xhs/noteCollector.js';
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
import { packageContentDetail } from '../linggan/producerRuntime.js';
import { LINGGAN_RUNTIME_ACTION } from '../linggan/runtimeActions.js';
import { unavailableLingganStats } from '../linggan/adapter.js';
import { explicitXhsDetailPageUrlInvalid } from '../shared/deadPageSignals.js';
import { installSelectorHealthReporter } from '../shared/selectorHealth.js';
import {
  observeXhsAccountFromDocument,
  observeXhsDetailRiskFromDocument,
  reportPassiveAccountEligibility,
} from '../linggan/accountEligibilityProbe.js';
import {
  detailPageSessionExecutionReceipt,
  validateDetailPageSessionPlan,
} from '../linggan/detailPageSessionStore.js';
import { loadDouyinRuntime } from './douyinRuntime.js';
import { registerCollectorReceiptSink } from '../runtime/collectorReceiptSink.js';
import { createDashboardBridge } from './dashboardBridge.js';
import { createNoteMediaDownloadService } from './noteMediaDownload.js';
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

// 页面上的结构自检（探过的与动作前的）统一交给 background 存：页面会随导航消失，
// 诊断不该跟着消失。这是一条只出不进的旁注——上报失败不改变页面上正在做的任何事。
installSelectorHealthReporter((snapshot) => sendToBackground(
  LINGGAN_RUNTIME_ACTION.REPORT_SELECTOR_HEALTH,
  { snapshot },
  { timeoutMs: 4000 },
));

async function collectCurrentXhsDetailPackage(...args) {
  // A direct detail request carries its attached comments in the same logical package.
  // The bridge remains task-triggered rather than becoming an always-on page observer.
  ensureXhsCommentApiBridge();
  const result = await collectXhsNoteDetailPackage(...args);
  return result.note;
}

// Human-operated media download stays available as a local, explicit action.  It is
// deliberately separate from Linggan receipt delivery, which only observes media slots.
const manualMediaDownloadService = createNoteMediaDownloadService({
  MSG,
  noteStore,
  sendToBackground,
  collectNote: collectCurrentXhsDetailPackage,
  loadDouyinRuntime,
  extractNoteId,
});

const dashboardBridge = createDashboardBridge({
  noteStore,
  commentStore,
  authorStore,
  // The dashboard is also a human-operated surface, so it preserves the explicit local
  // download workflow instead of reusing the Intelligence observation lane.
  downloadNoteMediaFromRecord: (note, options) => manualMediaDownloadService.downloadNoteMediaFromRecord(note, options),
  // Selected cache rows re-enter the one Browser Producer outbox.  The Dashboard never posts a
  // target directly: only a normal author_profile Package may create or enrich a target.
  syncAuthorsToObservationTargets: async (authors) => {
    const settled = await Promise.allSettled(authors.map((author) => runtime.submitAuthor(author)));
    const queued = settled.filter((result) => result.status === 'fulfilled').length;
    const rejected = settled
      .map((result, index) => ({ result, author: authors[index] }))
      .filter(({ result }) => result.status === 'rejected')
      .map(({ result, author }) => ({
        authorExternalId: String(author?.userId || author?.id || ''),
        reason: String(result.reason?.message || result.reason || 'author_target_queue_failed'),
      }));
    return {
      queued,
      rejected,
      // Queue admission is deliberately not mislabelled as a server target receipt.  The
      // server returns that receipt only when the durable outbox reaches it.
      state: rejected.length === 0 ? 'queued' : (queued > 0 ? 'partial' : 'rejected'),
    };
  },
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
  collectNote: collectCurrentXhsDetailPackage,
  collectComments: async (...args) => {
    // Do not install the old page bridge when a search/profile page merely loads.  It is only
    // needed if a user explicitly starts comment collection.
    ensureXhsCommentApiBridge();
    return collectComments(...args);
  },
  submitCommentCheckpoint: (result, noteId, settings) => runtime.submitComments(result, noteId, settings),
  collectAuthor,
  collectCommentImages: async (...args) => collectCommentImages(...args),
  BatchNoteController: LingganBatchNoteController,
  BatchCommentController: LingganBatchCommentController,
  injectUI,
  toggleStopButton,
  togglePauseResumeButtons,
  showToast,
  showCommentLimitDialog,
  // This dialog is only reached by the explicit human media-download action.
  showMediaDownloadDialog,
  showBatchSettingsDialog,
  ensureTaskControlBar,
  updateTaskControlBar,
  hideTaskControlBar,
  isContextValid,
  reportDone,
  extractNoteId,
  sendToBackground,
  downloadNoteMediaFromRecord: (note, options) => manualMediaDownloadService.downloadNoteMediaFromRecord(note, options),
  discoverSurface: async ({ mode, maximumQuota, scrollRounds }) => {
    const expectedCount = Math.max(1, Number(maximumQuota) || 20);
    // 下拉次数由服务端的采样口径决定；没给就沿用采集器自己的默认。
    // **按次数控制，不按条数控制**：页面每次加载出多少条不由我们决定，
    // 只有「拉了几次」是能说准的事实，也只有它能被回执如实记下来。
    const rounds = Number(scrollRounds);
    const cards = await discoverWithScroll(
      mode === 'profile' ? '#userPostedFeeds' : '.feeds-container',
      Number.isFinite(rounds) && rounds >= 0 ? rounds : undefined,
      { expectedCount },
    );
    const executionSummary = buildDiscoveryExecutionSummary(cards.discoveryMeta);
    if (mode === 'profile') {
      return {
        cards,
        pageFacts: {
          kind: 'xhs_profile_surface',
          requestedLimit: expectedCount,
          loadedCount: cards.length,
          ...executionSummary,
        },
        discoveryMeta: cards.discoveryMeta,
      };
    }
    return {
      cards,
      pageFacts: readCurrentXhsSearchSurfaceContext({
        requestedLimit: expectedCount,
        loadedCount: cards.length,
        stopReason: cards.discoveryMeta?.stopReason,
        ...executionSummary,
      }),
      discoveryMeta: cards.discoveryMeta,
    };
  },
  submitDiscovery: (cards, context) => runtime.submitDiscovery(cards, context),
});

async function initXhs() {
  registerCollectorReceiptSink({
    contentDetail: (note, context) => runtime.submitContentDetail(note, context?.options || {}),
    mediaSlots: (note, context) => runtime.submitMediaSlots(note, context?.options || {}),
    comments: (result, context) => runtime.submitComments(result, context?.noteId || extractNoteId(location.href), context?.options || {}),
    authorProfile: (author, context) => runtime.submitAuthor(author, context?.options || {}),
    batchCheckpoint: (progress, context) => runtime.submitBatchCheckpoint(context?.kind || 'xhs_batch', progress),
  });
  dashboardBridge.registerDashboardBridge();
  xhsPageController.initPage();
  void reportPassiveAccountEligibility({
    document,
    sendMessage: (message) => chrome.runtime.sendMessage(message),
  }).catch(() => {});
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

async function collectApprovedDetailPageSession(message = {}) {
  if (platform() !== 'xhs') throw new Error('detail_page_session_platform_not_supported');
  const taskSpec = message.taskSpec || {};
  const contentExternalId = String(taskSpec?.target?.contentExternalId || '').trim();
  const leaseRef = String(message.leaseRef || '').trim();
  if (!leaseRef || !contentExternalId) throw new Error('detail_page_session_identity_required');
  const plan = validateDetailPageSessionPlan(message.pageSessionPlan, contentExternalId);
  if (explicitXhsDetailPageUrlInvalid({
    currentUrl: location.href,
    title: document.title,
    expectedContentExternalId: contentExternalId,
  })) {
    return {
      success: false,
      state: 'detail_page_url_invalid',
      message: '当前详情链接已落到平台失效页；不会自动重新打开该链接。',
    };
  }
  let contentDelivery = null;
  let contentDeliveryFailure = null;
  let contentPackage = null;
  const queueDetailBeforeComments = async (note) => {
    if (contentDelivery || contentDeliveryFailure) return;
    try {
      contentPackage ||= packageContentDetail({ platform: 'xhs', note });
      contentDelivery = await runtime.submitContentDetail(note, {
        taskSpec,
        capturePackage: contentPackage,
        idempotencyKey: `detail-session:${taskSpec.taskId}:content_detail`,
      });
    } catch (error) {
      // The note stays in the page/local store.  Do not fail the page session
      // or trigger navigation; after comments finish we make one local-only
      // retry against that same immutable note before reporting the receipt.
      contentDeliveryFailure = error;
    }
  };

  const controller = new LingganBatchNoteController();
  await controller.start(COLLECT_MODE.DETAIL, null, {
    count: 1,
    targetNoteId: contentExternalId,
    includeComments: plan.lanes.includes('comments') || plan.lanes.includes('replies'),
    commentLimit: plan.commentLimit,
    commentDepthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    maxSubComments: plan.replyExpandLimit,
    taskSpec,
    deferLingganDelivery: true,
    onDetailReady: queueDetailBeforeComments,
    triggerSource: 'linggan_detail_page_session',
  });

  const note = controller.collected[0];
  if (!note || String(note.noteId || '').trim() !== contentExternalId) {
    throw new Error('detail_page_session_content_not_collected');
  }
  const packaged = note.__xhsDetailPackage || {};
  // The first durable deliverable is the detail lane itself.  A later cache
  // failure must not discard an already obtained body/media snapshot or force
  // another page visit just to make the other lanes convenient.
  contentPackage ||= packageContentDetail({ platform: 'xhs', note });
  // The content script has the XHS origin; its IndexedDB is not shared with
  // the extension service worker.  Hand the bounded page facts to background,
  // which owns the cache used by the later independently claimed lanes.
  const stored = await sendToBackground(LINGGAN_RUNTIME_ACTION.STORE_DETAIL_PAGE_SESSION, {
    leaseRef, plan, note, commentResult: packaged.commentResult, receipt: packaged.receipt,
    detailTaskId: taskSpec.taskId, detailPackage: contentPackage,
  }).catch(() => null);
  if (stored?.success !== true) {
    return { success: false, state: 'detail_page_session_recovery_required',
      message: '同页通道缓存未能持久保存；已入队的正文继续交付，其余材料需处理。' };
  }
  const queued = contentDelivery || await runtime.submitContentDetail(note, {
    taskSpec, capturePackage: contentPackage,
    idempotencyKey: `detail-session:${taskSpec.taskId}:content_detail`,
  });
  if (stored?.success === true && String(stored.cacheKey || '').trim()) {
    // A queue-marker failure may be replayed safely through the deterministic
    // outbox key. It must not turn this already persisted content package into
    // a reason to reopen the page.
    await sendToBackground(LINGGAN_RUNTIME_ACTION.MARK_DETAIL_PAGE_SESSION_TASK_QUEUED, {
      cacheKey: stored.cacheKey, taskId: taskSpec.taskId,
    }).catch(() => null);
  }
  return detailPageSessionExecutionReceipt({
    action: LINGGAN_RUNTIME_ACTION.COLLECT_NOTE_FULL,
    taskSpec,
    delivery: queued.delivery,
    submissionId: queued.submissionId,
  });
}

async function dispatchProducerRuntimeAction(action, message) {
  if (action === LINGGAN_RUNTIME_ACTION.COLLECT_NOTE_FULL) {
    return collectApprovedDetailPageSession(message);
  }
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
      [LINGGAN_RUNTIME_ACTION.DISCOVER_SURFACE]: 'discoverSurface',
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
    taskSpec: message.taskSpec,
    sortMode: message.sortMode,
    triggerSource: message.triggerSource || 'popup_linggan_runtime',
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
  if (pageResult?.success !== true) {
    return pageResult && typeof pageResult === 'object'
      ? pageResult
      : { success: false, state: 'page_action_receipt_missing' };
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
  const capability = Array.isArray(message?.taskSpec?.capabilitiesRequested)
    ? String(message.taskSpec.capabilitiesRequested[0] || '')
    : '';
  return {
    success: true,
    state: 'page_read_started',
    delivery: 'pending',
    action,
    capability,
    taskId: String(message?.taskSpec?.taskId || ''),
  };
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '');
  if (action === LINGGAN_RUNTIME_ACTION.GET_PAGE_CONTEXT) {
    sendResponse({ success: true, context: { platform: platform(), url: location.href, mode: 'linggan_browser_producer_runtime' } });
    return true;
  }
  if (action === LINGGAN_RUNTIME_ACTION.OBSERVE_CLAIMED_TASK_ACCOUNT && platform() === 'xhs') {
    sendResponse({ success: true, observation: observeXhsAccountFromDocument(document) });
    return true;
  }
  if (action === LINGGAN_RUNTIME_ACTION.OBSERVE_CLAIMED_TASK_RISK && platform() === 'xhs') {
    sendResponse({ success: true, observation: observeXhsDetailRiskFromDocument(document) });
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
    LINGGAN_RUNTIME_ACTION.COLLECT_NOTE_FULL,
    LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS,
    LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR,
    LINGGAN_RUNTIME_ACTION.DISCOVER_SURFACE,
    LINGGAN_RUNTIME_ACTION.START_BATCH_CONTENT,
    LINGGAN_RUNTIME_ACTION.START_BATCH_COMMENTS,
    LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA,
    LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH,
    LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH,
  ].includes(action)) {
    dispatchProducerRuntimeAction(action, message)
      .then((result) => sendResponse(result || { success: false, code: 'linggan_page_action_unavailable' }))
      .catch((error) => {
        // A detail session that cannot yield the exact server-approved note is
        // not a generic selector or extension failure.  Report the bounded
        // page-unavailable fact; background.js never transmits the raw error.
        const code = String(error?.message || '') === 'detail_page_session_content_not_collected'
          ? 'page_unavailable'
          : 'linggan_page_action_failed';
        sendResponse({ success: false, code, message: String(error?.message || error) });
      });
    return true;
  }
  return false;
});

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => { void init(); }, { once: true });
else void init();
