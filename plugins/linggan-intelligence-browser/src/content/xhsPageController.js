import { createCommentTaskController } from './commentTaskController.js';
import { createCommentImageTaskController } from './commentImageTask.js';
import { COLLECT_MODE, COMMENT_DEPTH_MODE, TASK_STATE } from '../shared/constants.js';
import { consumeSelectorHealthAlertMessage } from '../shared/selectorHealth.js';
import { isTerminalTaskState, resolveTaskState } from '../shared/taskUi.js';
import {
  runXhsSelectorBootstrapProbe,
  runXhsSelectorPreflight,
} from '../platforms/xhs/selectorHealth.js';
import {
  applyXhsSearchFilters,
  hasExplicitXhsSearchFilters,
  normalizeXhsSearchFilters,
  readCurrentXhsSearchFilterSnapshot,
} from '../platforms/xhs/searchFilters.js';
import { parseCount } from '../shared/utils.js';


/**
 * 把服务端下发的采样口径翻成页面筛选。
 *
 * 服务端用「几天内」表达时间范围，而平台只提供四档（不限／一天内／一周内／半年内）。
 * 这里向上取到**不小于**请求的那一档：要 3 天却给「一天内」会漏掉第 2、3 天的内容，
 * 给「一周内」只是多采了一些——多采可以在读取时筛掉，漏采则无从补回。
 *
 * 这层映射是有损的，所以采完之后回写的是**页面上实际生效的筛选**，不是这里请求的值。
 */
function samplingFiltersFromTaskSpec(taskSpec) {
  const target = taskSpec?.target || {};
  const filters = {};
  const ranking = String(target.ranking || '').trim();
  // 服务端的排序词与筛选表的取值是同一套（most_liked / most_commented / most_collected /
  // latest / comprehensive→general）；认不出来的一概不设，让页面保持当前排序。
  const RANKING_TO_SORT = {
    most_liked: 'most_liked',
    most_collected: 'most_collected',
    most_commented: 'most_commented',
    latest: 'latest',
    comprehensive: 'general',
  };
  if (RANKING_TO_SORT[ranking]) filters.sortBasis = RANKING_TO_SORT[ranking];
  const days = Number(target.publishedWithinDays);
  if (Number.isFinite(days) && days > 0) {
    filters.publishTime = days <= 1 ? 'one_day' : (days <= 7 ? 'one_week' : 'half_year');
  }
  return filters;
}

/**
 * 从加载出来的内容里取点赞最高的前 N 篇。
 *
 * **Top N 只决定「采哪几篇」，返回顺序仍按页面原始位置**——按名次重排会让后续逐篇打开
 * 时来回滚动。名次单独记在 `__topRank` 上，它就是「爆」徽章的依据。
 */
function pickTopByLikes(cards, topByLikes) {
  const limit = Number(topByLikes);
  if (!Number.isFinite(limit) || limit <= 0 || !Array.isArray(cards) || cards.length <= limit) {
    return cards;
  }
  const ranked = cards
    .map((card, index) => ({ card, likes: parseCount(card?.likes), position: index }))
    .sort((a, b) => (b.likes || 0) - (a.likes || 0) || a.position - b.position)
    .slice(0, limit)
    .map((entry, rank) => ({ ...entry, rank: rank + 1 }))
    .sort((a, b) => a.position - b.position);
  return ranked.map((entry) => ({ ...entry.card, __topRank: entry.rank }));
}

/**
 * 把这一轮**实际发生**的采样情况附到页面事实上。
 *
 * 记的是生效值而不是请求值：平台改了筛选文案、筛选没点上、时间档位被向上取整——
 * 这些都会让实际口径与下发的口径不同。只回写「我请求了什么」，回执就会替一次并未
 * 发生的采样背书。`snapshot` 直接读页面自己的筛选状态，是这里唯一可信的一手事实。
 *
 * `loadedCount` 与 `retained` 也一并记下：规格要求「实际取得数」不可省略，
 * 采不满是已知会发生的情况，只记目标数会让后续复核的基数是错的。
 */
function withAppliedSampling(pageFacts, { filterOutcome, requested, loadedCount, retained }) {
  const requestedKeys = Object.keys(requested || {});
  if (!requestedKeys.length && !filterOutcome) return pageFacts;
  return {
    ...(pageFacts || {}),
    appliedSampling: {
      requested: requested || {},
      // 请求了筛选却没能应用时，reason 会说明为什么——那比默默采一轮有用。
      applied: Boolean(filterOutcome?.applied),
      reason: filterOutcome?.reason || null,
      effective: filterOutcome?.snapshot || readCurrentXhsSearchFilterSnapshot(window),
      loadedCount,
      retained,
    },
  };
}

const XHS_CONTEXT_REFRESH_MESSAGE = '插件刚更新，请刷新当前页面后再点一次，刷新后即可继续。';

const DETAIL_DELIVERY_LANES = [
  ['content', '笔记详情'],
  ['mediaSlots', '媒体观察'],
  ['comments', '评论与回复'],
];

function describeDeliveryState(state) {
  switch (String(state || '').trim()) {
    case 'acknowledged':
      return '已接纳';
    case 'pending':
    case 'queued':
      return '待本机交付';
    case 'rejected':
    case 'terminal':
      return '未接纳';
    default:
      return '状态未知';
  }
}

export function formatXhsDetailDeliveryMessage(packageDelivery = {}) {
  const laneSummary = DETAIL_DELIVERY_LANES
    .map(([key, label]) => `${label}：${describeDeliveryState(packageDelivery?.lanes?.[key])}`)
    .join('；');
  const allAccepted = packageDelivery?.state === 'acknowledged';

  return {
    allAccepted,
    message: allAccepted
      ? `Linggan 接纳回执：${laneSummary}`
      : `笔记详情已读取。Linggan 接纳回执：${laneSummary}`,
  };
}

function hasDownloadableNoteMedia(note = {}) {
  return Boolean(
    (Array.isArray(note.images) && note.images.length > 0)
    || note.video
    || note.cover
    || note.coverUrl
    || (Array.isArray(note.livePhotoStreams) && note.livePhotoStreams.length > 0),
  );
}

function getDownloadableNoteMediaCount(note = {}) {
  return (note.cover || note.coverUrl || note.coverImg || note.thumbnail ? 1 : 0)
    + (note.images?.length || 0)
    + (note.video ? 1 : 0)
    + (note.livePhotoStreams?.length || 0);
}

export function createXhsPageController({
  MSG,
  assertPluginAuthorized,
  collectComments,
  submitCommentCheckpoint,
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
  discoverSurface,
  submitDiscovery,
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
    submitCommentCheckpoint,
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
    const running = [
      singleCommentCtrl?.isRunning() ? singleCommentCtrl : null,
      batchNoteCtrl?.isRunning ? batchNoteCtrl : null,
      batchCommentCtrl?.isRunning ? batchCommentCtrl : null,
      commentImageController?.isRunning() ? commentImageController : null,
    ].filter(Boolean);
    if (running.length === 0) return { success: false, state: 'no_active_task' };
    running.forEach((controller) => controller.pause());
    togglePauseResumeButtons(true);
    if (commentImageController?.isRunning() || singleCommentCtrl?.isRunning()) return { success: true, state: 'paused' };
    const current = Number(lastTaskSnapshot?.current || 0);
    const total = Number(lastTaskSnapshot?.total || 0);
    syncTaskUI({
      taskType: lastTaskSnapshot?.taskType || activeTaskType || (batchCommentCtrl?.isRunning ? 'batchComments' : 'batchNotes'),
      taskState: TASK_STATE.PAUSED,
      message: '已暂停',
      total,
      current,
    });
    return { success: true, state: 'paused' };
  }

  function resumeActiveTask() {
    const running = [
      singleCommentCtrl?.isRunning() ? singleCommentCtrl : null,
      batchNoteCtrl?.isRunning ? batchNoteCtrl : null,
      batchCommentCtrl?.isRunning ? batchCommentCtrl : null,
      commentImageController?.isRunning() ? commentImageController : null,
    ].filter(Boolean);
    if (running.length === 0) return { success: false, state: 'no_active_task' };
    running.forEach((controller) => controller.resume());
    togglePauseResumeButtons(false);
    if (commentImageController?.isRunning() || singleCommentCtrl?.isRunning()) return { success: true, state: 'running' };
    const current = Number(lastTaskSnapshot?.current || 0);
    const total = Number(lastTaskSnapshot?.total || 0);
    syncTaskUI({
      taskType: lastTaskSnapshot?.taskType || activeTaskType || (batchCommentCtrl?.isRunning ? 'batchComments' : 'batchNotes'),
      taskState: TASK_STATE.RUNNING,
      message: '继续采集',
      total,
      current,
    });
    return { success: true, state: 'running' };
  }

  function stopActiveTask() {
    const running = [
      singleCommentCtrl?.isRunning() ? singleCommentCtrl : null,
      batchNoteCtrl?.isRunning ? batchNoteCtrl : null,
      batchCommentCtrl?.isRunning ? batchCommentCtrl : null,
      commentImageController?.isRunning() ? commentImageController : null,
    ].filter(Boolean);
    if (running.length === 0) return { success: false, state: 'no_active_task' };
    running.forEach((controller) => controller.stop());
    toggleStopButton(false);
    hideTaskControlBar();
    activeTaskType = null;
    return { success: true, state: 'stopped' };
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

  async function collectCurrentNoteToLinggan(options = {}) {
    showToast('正在采集笔记并交付 Linggan…', 'info');
    const note = await collectNote(window, options);
    const delivery = formatXhsDetailDeliveryMessage(note?.lingganDetailPackageDelivery);
    showToast(delivery.message, delivery.allAccepted ? 'success' : 'info');
    return note;
  }

  async function downloadSelectedNoteMedia(note) {
    if (!hasDownloadableNoteMedia(note)) {
      showToast('当前笔记未发现可供人工下载的媒体。', 'info');
      return;
    }

    try {
      const selection = await showMediaDownloadDialog(note);
      const mediaTypes = selection === true ? undefined : selection?.mediaTypes;
      if (!selection || (selection !== true && mediaTypes?.length === 0)) return;

      const mediaCount = getDownloadableNoteMediaCount(note);
      showToast(`正在下载 ${selection?.count || mediaCount} 个媒体文件…`, 'info');
      const summary = await downloadNoteMediaFromRecord(note, { mediaTypes });
      showToast(
        summary?.zipped
          ? `媒体下载完成：已打包 ZIP（成功 ${summary.success}/${summary.total}，失败 ${summary.failed}）`
          : `媒体下载完成：成功 ${summary.success}/${summary.total}，失败 ${summary.failed}`,
        summary?.failed > 0 ? 'warning' : 'success',
      );
    } catch {
      // The user can close the manual selection window without starting a download.
    }
  }

  async function handleButtonClick(e) {
    const btn = e.target.closest('.lgboom-btn');
    if (btn == null) return;

    const action = btn.dataset.action;
    const params = btn.dataset.params ? JSON.parse(btn.dataset.params) : {};

    if (isContextValid() === false) {
      showToast(XHS_CONTEXT_REFRESH_MESSAGE, 'warning');
      return { success: false, state: 'context_invalid', message: XHS_CONTEXT_REFRESH_MESSAGE };
    }

    if (!['stopBatch', 'pauseBatch', 'resumeBatch'].includes(action)) {
      try {
        await ensurePluginAuthorized();
      } catch (error) {
        const message = String(error?.message || 'Linggan 采集 adapter 尚未接通。');
        showToast(message, 'warning');
        return { success: false, state: 'authorization_unavailable', message };
      }
    }

    const preflight = runXhsSelectorPreflight(action, {
      params,
      document,
      win: window,
    });
    if (preflight.ok === false) {
      showToast(preflight.message || '当前页面结构未通过预检，请刷新后重试。', 'warning');
      return {
        success: false,
        state: preflight.code || 'selector_preflight_failed',
        message: preflight.message || '当前页面结构未通过预检，请刷新后重试。',
      };
    }

    try {
      switch (action) {
        case 'collectNote': {
          const note = await collectCurrentNoteToLinggan({
            taskSpec: params.taskSpec,
            expectedNoteId: params.taskSpec?.target?.contentExternalId,
          });
          return {
            success: true,
            state: 'page_read_completed',
            delivery: note?.lingganDelivery?.delivery || 'pending',
          };
        }

        case 'collectNoteWithManualMedia': {
          const note = await collectCurrentNoteToLinggan();
          await downloadSelectedNoteMedia(note);
          break;
        }

        case 'collectComment': {
          await ensurePluginAuthorized();
          let commentSettings;
          if (params.taskSpec?.source === 'scheduled') {
            const capability = params.taskSpec.capabilitiesRequested?.[0];
            commentSettings = {
              // Background already decoded the server TaskSpec into the collector's execution
              // value. Zero means all public comments until a real stop, never "one comment".
              maxComments: Math.max(0, Number(params.maxTotal || 0) || 0),
              commentDepthMode: capability === 'replies' ? COMMENT_DEPTH_MODE.ALL_REPLIES : COMMENT_DEPTH_MODE.TWO_LEVEL,
              maxSubComments: Number(params.taskSpec.target?.replyExpandLimit) || 0,
            };
          } else {
            try {
              commentSettings = await showCommentLimitDialog({
                title: '单篇评论设置',
                description: '选择评论上限和采集深度。留空或填 0 表示不限；“尽量全部回复”会继续展开更多回复。',
                confirmText: '开始采集',
              });
            } catch {
              break;
            }
          }
          const commentResult = await singleCommentCtrl.start({
            noteId: params.taskSpec?.target?.contentExternalId || extractNoteId(window.location.href),
            noteUrl: window.location.href,
            maxTotal: commentSettings.maxComments,
            maxSubComments: params.taskSpec?.source === 'scheduled'
              ? commentSettings.maxSubComments
              : (commentSettings.commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : 200),
            commentDepthMode: commentSettings.commentDepthMode,
            taskSpec: params.taskSpec,
          });
          if (commentResult?.success === false) return commentResult;
          return { success: true, state: 'page_read_completed', delivery: 'pending' };
        }

        case 'collectAuthor': {
          await ensurePluginAuthorized();
          showToast('正在采集博主信息...', 'info');
          const author = await collectAuthor({ taskSpec: params.taskSpec });
          showToast(author?.lingganDelivery?.delivery === 'acknowledged'
            ? `博主资料已被 Linggan 接纳：${author.name}`
            : `博主资料已读取，待本机 Linggan 交付：${author.name}`, author?.lingganDelivery?.delivery === 'acknowledged' ? 'success' : 'info');
          return {
            success: true,
            state: 'page_read_completed',
            delivery: author?.lingganDelivery?.delivery || 'pending',
          };
        }

        case 'discoverSurface': {
          if (typeof discoverSurface !== 'function' || typeof submitDiscovery !== 'function') {
            throw new Error('linggan_discovery_adapter_unavailable');
          }
          const mode = String(params.mode || '').trim();
          const suppliedQuota = Number(params.maximumQuota || params.limit || 0);
          // 服务端派下来的任务自带采样口径；此前这里恒为空筛选，于是排序从来没被设过，
          // 采回来的永远是页面当时碰巧的排序——回执里那句「按最多点赞采」是假的。
          const dispatchedSampling = samplingFiltersFromTaskSpec(params.taskSpec);
          let discoverySettings = {
            count: Number.isFinite(suppliedQuota) && suppliedQuota > 0 ? suppliedQuota : 0,
            searchFilters: normalizeXhsSearchFilters(dispatchedSampling),
          };
          // Page buttons ask for a real target.  Runtime dispatches may supply a quota directly
          // and therefore do not open another dialog.
          if (!discoverySettings.count) {
            try {
              discoverySettings = await showBatchSettingsDialog({
                title: mode === COLLECT_MODE.PROFILE ? '博主页发现设置' : '搜索发现设置',
                enableTopLikes: false,
                enableSearchFilters: mode === COLLECT_MODE.SEARCH,
              });
            } catch {
              break;
            }
          }
          const maximumQuota = Math.max(1, Number(discoverySettings.count || 20));
          const searchFilters = normalizeXhsSearchFilters(discoverySettings.searchFilters || {});
          let filterOutcome = null;
          if (mode === COLLECT_MODE.SEARCH && hasExplicitXhsSearchFilters(searchFilters)) {
            filterOutcome = await applyXhsSearchFilters(searchFilters, { document, win: window });
          }
          showToast(`正在按目标加载，最多 ${maximumQuota} 条…`, 'info');
          const discovered = await discoverSurface({
            mode,
            maximumQuota,
            scrollRounds: params.taskSpec?.target?.scrollRounds,
          });
          const loaded = Array.isArray(discovered) ? discovered : (Array.isArray(discovered?.cards) ? discovered.cards : []);
          // 先按点赞取前 N，再交付：口径说的是「从加载出来的里面取 20 篇」，
          // 把全部加载结果都提交上去会让「取前 20」这句话没有落到实处。
          const cards = mode === COLLECT_MODE.SEARCH
            ? pickTopByLikes(loaded, params.taskSpec?.target?.topByLikes)
            : loaded;
          const target = new URL(window.location.href);
          const query = target.searchParams.get('keyword') || target.searchParams.get('q') || '';
          const authorExternalId = mode === COLLECT_MODE.PROFILE
            ? (target.pathname.match(/\/user\/profile\/([^/?#]+)/)?.[1] || '')
            : '';
          const delivery = await submitDiscovery(cards, {
            query,
            authorExternalId,
            surface: 'target_driven_surface',
            pageFacts: withAppliedSampling(
              Array.isArray(discovered) ? undefined : discovered?.pageFacts,
              { filterOutcome, requested: dispatchedSampling, loadedCount: loaded.length, retained: cards.length },
            ),
            maximumQuota,
            taskSpec: params.taskSpec,
          });
          const stopReason = Array.isArray(discovered) ? '' : discovered?.discoveryMeta?.stopReason;
          const resultText = stopReason === 'target_reached'
            ? `已达到目标，采集 ${cards.length}/${maximumQuota} 条`
            : `已采集 ${cards.length}/${maximumQuota} 条，${stopReason || '页面加载已停止'}`;
          showToast(delivery?.delivery === 'acknowledged'
            ? `Linggan 已接纳：${resultText}`
            : `${resultText}，待本机 Linggan 交付`, delivery?.delivery === 'acknowledged' ? 'success' : 'info');
          return {
            success: true,
            state: 'page_read_completed',
            delivery: delivery?.delivery || 'pending',
          };
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
          return stopActiveTask();

        case 'pauseBatch':
          return pauseActiveTask();

        case 'resumeBatch':
          return resumeActiveTask();

        case 'collectCommentImages':
          // The old ZIP downloader is deliberately not a Linggan media export.  Keep the
          // familiar control visible but make the unavailable adapter explicit and side-effect
          // free until comment images have a reviewed MediaSlot mapping.
          showToast('评论图片区暂不可用：尚未具备 Linggan MediaSlot 回传合同，未执行下载。', 'warning');
          break;
      }
    } catch (err) {
      console.error('[灵感爆爆爆]', err);
      const errMsg = String(err?.message || '');
      if (/Extension context invalidated|context invalidated/i.test(errMsg) || isContextValid() === false) {
        showToast(XHS_CONTEXT_REFRESH_MESSAGE, 'warning');
        return { success: false, state: 'context_invalid', message: XHS_CONTEXT_REFRESH_MESSAGE };
      }
      if (action === 'batchNotes' || action === 'batchComments') {
        toggleStopButton(false);
        hideTaskControlBar();
        activeTaskType = null;
      }
      showToast(`操作失败：${err.message}`, 'error');
      return { success: false, state: 'page_read_failed', message: errMsg || '页面采集失败' };
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
    stopActiveTask,
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
