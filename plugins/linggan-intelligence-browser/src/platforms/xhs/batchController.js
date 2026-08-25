import {
  collectNote,
  discoverProfileSurfaceNotesFromApi,
  discoverSearchSurfaceNotesFromApi,
  discoverWithScroll,
  resolveExpectedNoteFromMap,
} from './noteCollector.js';
import { collectComments } from './commentCollector.js';
import { throttle, watchCaptcha, showCaptchaPauseOverlay } from './antiDetect.js';
import { sendToBackground, reportProgress, reportDone, reportWorkbenchRecord } from '../../shared/messaging.js';
import { BATCH_CONFIG, COLLECT_MODE, COMMENT_DEPTH_MODE, MSG, TASK_STATE } from '../../shared/constants.js';
import { extractProfileIdentityFromUrl } from '../../shared/targetIdentity.js';
import { looksLikeDeadPageTitle } from '../../shared/deadPageSignals.js';
import { randomDelay, parseCount, extractNoteId } from '../../shared/utils.js';
import { noteStore } from '../../db/noteStore.js';
import { collectionRunStore } from '../../db/collectionRunStore.js';
import { createCollectionRunHeartbeatReporter } from '../../workbench/runtime/heartbeat.js';
import { MONITOR_RECORD_MODE, MONITOR_TASK_STRATEGY, WORKBENCH_RECORD_TYPE } from '../../workbench/protocol/schema.js';
import { buildXhsSurfaceNoteRecords, withMonitorRecordMeta } from '../../workbench/runtime/monitorTask.js';
import {
  buildRemoteRunCreatePayload,
  buildXhsAttachedCommentResult,
  buildXhsBatchNotesProgressPatch,
  buildXhsBatchNotesRunPatch,
  publicCommentCountFromXhsNote,
} from '../../workbench/runtime/xhsBatchRunHelper.js';
import {
  applyXhsSearchFilters,
  hasExplicitXhsSearchFilters,
  normalizeXhsSearchFilters,
  readCurrentXhsSearchFilterSnapshot,
  summarizeXhsSearchFilters,
} from './searchFilters.js';
import { resolveBatchResumeState } from '../../workbench/runtime/batchResume.js';
import {
  CLOSE_SELECTORS,
  isNoteDetailReady,
  isPopupOpen,
  isRiskControlPage,
  isErrorCode300017,
  waitForNoteState,
  findNoteElementById,
  formatCompactCount,
  getActiveCommentsContext,
} from './batchShared.js';
import { BaseBatchController } from '../../shared/baseBatchController.js';
export { BatchCommentController } from './batchCommentController.js';

function toFiniteNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function firstText(value) {
  if (typeof value !== 'string') return '';
  const trimmed = value.trim();
  return trimmed && trimmed !== '[object Object]' ? trimmed : '';
}

function pickMediaUrlFromArray(value) {
  if (!Array.isArray(value)) return '';

  for (const item of value) {
    const direct = firstText(item);
    if (direct) return direct;

    const nestedArray = pickMediaUrlFromArray(item);
    if (nestedArray) return nestedArray;

    if (item && typeof item === 'object' && !Array.isArray(item)) {
      const nested = firstText(item.urlDefault) || firstText(item.url) || firstText(item.src);
      if (nested) return nested;
    }
  }

  return '';
}

function buildWorkbenchNoteRecord(note = {}) {
  const images = Array.isArray(note.images) ? note.images.filter(Boolean) : [];
  const imageCandidates = Array.isArray(note.imageCandidates) ? note.imageCandidates.filter(Boolean) : [];
  const cover = firstText(note.cover)
    || firstText(note.coverImg)
    || firstText(note.coverUrl)
    || firstText(note.thumbnail)
    || pickMediaUrlFromArray(images)
    || pickMediaUrlFromArray(imageCandidates);
  const url = String(note.url || note.noteUrl || '').trim();
  const canonicalUrl = String(note.canonicalUrl || url).trim();
  const rawUrl = String(note.rawUrl || canonicalUrl || url).trim();

  return {
    platform: String(note.platform || 'xhs').trim() || 'xhs',
    noteId: String(note.noteId || note.platformContentId || note.contentId || '').trim(),
    platformContentId: String(note.platformContentId || note.noteId || note.contentId || '').trim(),
    title: String(note.title || '').trim(),
    content: String(note.content || note.desc || note.bodyText || '').trim(),
    url,
    canonicalUrl,
    rawUrl,
    rawShareText: firstText(note.rawShareText),
    cover,
    coverImg: firstText(note.coverImg) || cover,
    coverUrl: firstText(note.coverUrl) || cover,
    images,
    imageCandidates,
    videoUrl: firstText(note.videoUrl) || firstText(note.video) || firstText(note.videoDownloadUrl) || firstText(note.videoPlayUrl),
    likes: toFiniteNumber(note.likes, 0),
    collects: toFiniteNumber(note.collects, 0),
    comments: toFiniteNumber(note.comments, 0),
    shares: toFiniteNumber(note.shares, 0),
    authorId: String(note.authorId || note.authorPlatformId || note.userId || '').trim(),
    authorPlatformId: String(note.authorPlatformId || note.authorId || note.userId || '').trim(),
    authorEntityId: String(note.authorEntityId || '').trim(),
    authorName: String(note.authorName || note.author || '').trim(),
    authorAvatar: firstText(note.authorAvatar) || firstText(note.avatar),
    publishedAt: note.publishedAt ?? note.publishTime ?? note.releaseDate ?? null,
    publishedAtText:
      firstText(note.publishedAtText)
      || firstText(note.publishTimeText)
      || firstText(note.releaseDateText)
      || firstText(note.releaseDate)
      || firstText(note.timeText)
      || firstText(note.time),
    type: String(note.type || note.contentType || note.noteType || note.itemType || '').trim(),
    contentType: String(note.contentType || note.type || note.noteType || note.itemType || '').trim(),
    dataSource: firstText(note.dataSource),
    dataQuality: firstText(note.dataQuality),
    qualityReason: firstText(note.qualityReason),
    sourceTier: firstText(note.sourceTier),
  };
}

function firstCoverFromNoteLike(note = {}) {
  const images = Array.isArray(note.images) ? note.images.filter(Boolean) : [];
  const imageCandidates = Array.isArray(note.imageCandidates) ? note.imageCandidates.filter(Boolean) : [];
  return firstText(note.cover)
    || firstText(note.coverImg)
    || firstText(note.coverUrl)
    || firstText(note.thumbnail)
    || pickMediaUrlFromArray(images)
    || pickMediaUrlFromArray(imageCandidates);
}

export function mergeSurfaceCoverFallback(note = {}, surfaceNote = {}) {
  const currentCover = firstCoverFromNoteLike(note);
  const fallbackCover = firstCoverFromNoteLike(surfaceNote);
  if (currentCover || !fallbackCover) return note;

  const images = Array.isArray(note.images) && note.images.length > 0
    ? note.images
    : [fallbackCover];

  return {
    ...note,
    cover: fallbackCover,
    coverImg: firstText(note.coverImg) || fallbackCover,
    coverUrl: firstText(note.coverUrl) || fallbackCover,
    thumbnail: firstText(note.thumbnail) || fallbackCover,
    images,
  };
}

function normalizeTargetIdentity(value = '') {
  return String(value || '').trim().toLowerCase();
}

function extractXhsProfileUserId(url = '') {
  return extractProfileIdentityFromUrl(url, { baseUrl: window.location.origin });
}

function getXhsTargetProfileUrl(monitorMeta = {}) {
  const candidates = [
    monitorMeta?.targetUrl,
    monitorMeta?.targetPageUrl,
    monitorMeta?.pageUrl,
    monitorMeta?.sourcePageUrl,
  ];
  return candidates.find((value) => String(value || '').trim()) || '';
}

export function filterTargetedXhsNoteList(noteList = [], targetNoteId = '') {
  const normalizedTargetNoteId = String(targetNoteId || '').trim().replace(/^xhs_/, '');
  if (!normalizedTargetNoteId) return Array.isArray(noteList) ? [...noteList] : [];
  return (Array.isArray(noteList) ? noteList : []).filter((note) => (
    String(note?.noteId || note?.platformContentId || note?.contentId || '').trim().replace(/^xhs_/, '') === normalizedTargetNoteId
  ));
}

export function checkXhsAuthorMonitorTarget(monitorMeta = {}, {
  mode = '',
  win = window,
} = {}) {
  const strategy = normalizeTargetIdentity(monitorMeta?.taskStrategy);
  const monitorMode = normalizeTargetIdentity(monitorMeta?.surfaceMode || monitorMeta?.monitorMode);
  const isAuthorMonitor = strategy === normalizeTargetIdentity(MONITOR_TASK_STRATEGY.AUTHOR_BASELINE)
    || monitorMode === normalizeTargetIdentity(MONITOR_RECORD_MODE.AUTHOR_SURFACE)
    || Boolean(monitorMeta?.surfaceOnly);

  if (!isAuthorMonitor || String(mode || '').trim() !== COLLECT_MODE.PROFILE) {
    return { ok: true, code: 'skipped' };
  }

  const currentUrl = String(win?.location?.href || window.location.href || '').trim();
  const targetUrl = getXhsTargetProfileUrl(monitorMeta);
  const currentAuthorId = extractXhsProfileUserId(currentUrl);
  const targetAuthorId = extractXhsProfileUserId(targetUrl);
  const targetLabel = String(
    monitorMeta?.display?.name
    || monitorMeta?.display?.nickname
    || monitorMeta?.display?.title
    || targetAuthorId
    || '',
  ).trim();

  if (!currentAuthorId || !targetAuthorId) {
    return {
      ok: false,
      code: 'target_mismatch',
      reasonCode: 'target_mismatch',
      eventType: 'target_mismatch',
      message: `当前页博主身份与任务目标无法对齐，已停止本轮采集：当前=${currentAuthorId || '未识别'}，目标=${targetLabel || targetAuthorId || '未识别'}`,
      currentAuthorId,
      targetAuthorId,
      currentUrl,
      targetUrl,
      targetLabel,
    };
  }

  if (normalizeTargetIdentity(currentAuthorId) !== normalizeTargetIdentity(targetAuthorId)) {
    return {
      ok: false,
      code: 'target_mismatch',
      reasonCode: 'target_mismatch',
      eventType: 'target_mismatch',
      message: `当前页博主身份与任务目标不一致，已停止本轮采集：当前=${currentAuthorId}，目标=${targetLabel || targetAuthorId}`,
      currentAuthorId,
      targetAuthorId,
      currentUrl,
      targetUrl,
      targetLabel,
    };
  }

  return {
    ok: true,
    code: 'ok',
    currentAuthorId,
    targetAuthorId,
    currentUrl,
    targetUrl,
    targetLabel,
  };
}

function createTargetMismatchError(check = {}) {
  const error = new Error(String(check.message || '当前页博主身份与任务目标不一致'));
  error.code = 'target_mismatch';
  error.reasonCode = 'target_mismatch';
  error.eventType = 'target_mismatch';
  error.userMessage = error.message;
  error.targetAuthorId = check.targetAuthorId || '';
  error.currentAuthorId = check.currentAuthorId || '';
  return error;
}

async function resolveExistingBatchRun({ collectionRunId = '', externalTaskId = '', taskType = '' } = {}) {
  const explicitRunId = String(collectionRunId || '').trim();
  if (explicitRunId) {
    return collectionRunStore.getById(explicitRunId).catch(() => null);
  }
  const taskId = String(externalTaskId || '').trim();
  if (!taskId) return null;
  if (typeof collectionRunStore.getLatestResumableByExternalTaskId === 'function') {
    return collectionRunStore.getLatestResumableByExternalTaskId(taskId, { taskType }).catch(() => null);
  }
  return collectionRunStore.getLatestByExternalTaskId(taskId).catch(() => null);
}

function hydrateXhsNoteResumeState(runRecord = {}, completedTargetIds = []) {
  const completedSet = new Set((Array.isArray(completedTargetIds) ? completedTargetIds : [])
    .map((value) => String(value || '').trim().replace(/^xhs_/, ''))
    .filter(Boolean));
  const collected = (Array.isArray(runRecord.contentIds) ? runRecord.contentIds : [])
    .map((contentId) => String(contentId || '').trim().replace(/^xhs_/, ''))
    .filter((noteId) => noteId && completedSet.has(noteId))
    .map((noteId) => ({ noteId, contentId: `xhs_${noteId}` }));
  const failed = (Array.isArray(runRecord.failedTargets) ? runRecord.failedTargets : [])
    .map((item) => ({
      noteId: String(item?.noteId || item?.targetId || item?.contentId || '').trim().replace(/^xhs_/, ''),
      error: String(item?.error || 'failed').trim() || 'failed',
    }))
    .filter((item) => item.noteId && completedSet.has(item.noteId));
  const commentResults = (Array.isArray(runRecord.attachedCommentResults) ? runRecord.attachedCommentResults : [])
    .map((item) => buildXhsAttachedCommentResult({
      noteId: String(item?.noteId || item?.targetId || '').trim().replace(/^xhs_/, ''),
      total: item?.total,
      publicCommentCount: item?.publicCommentCount,
      expectedCommentCount: item?.expectedCommentCount,
      error: item?.error,
    }))
    .filter((item) => item.noteId && completedSet.has(item.noteId));
  const totalComments = commentResults.reduce((sum, item) => sum + item.total, 0);
  return { collected, failed, commentResults, totalComments };
}

// 小红书弹窗/详情页相关选择器（兼容多版本）
// 小红书有两种模式：
//   A) 搜索页/发现页点击 → SPA 路由跳转到 /explore/{noteId}（URL 变化）
//   B) 博主主页点击 → 弹出 overlay 弹窗（URL 可能不变）
// 两种都需要支持

/**
 * 批量笔记采集控制器
 *
 * 核心改进：不依赖保存的 element 引用（虚拟列表会卸载 DOM）
 * 采集流程：
 *   1. 滚动发现笔记列表，只保存 noteId/url/likes 等元数据
 *   2. 逐篇采集时，先滚动到大致位置让虚拟列表渲染出卡片
 *   3. 通过 noteId 重新查找 DOM 元素
 *   4. 点击打开弹窗 → 采集 → 关闭弹窗
 *   5. 如果弹窗方式失败，fallback 到 URL 导航方式
 */
export class BatchNoteController extends BaseBatchController {
  constructor() {
    super();
    this.type = 'batchNotes';
    this.currentIndex = 0;
    this.noteList = [];
    this.collected = [];
    this.failed = [];
    this.commentResults = [];
    this.captchaWatcher = null;
    this._containerSelector = '.feeds-container';
    this._mode = COLLECT_MODE.SEARCH;
    this._topByLikes = false;
    this._allowNavigationFallback = true;
    this._originUrl = ''; // 记录列表页 URL，用于 fallback 后返回
    this.onStateChange = null;
    this.collectionRunId = '';
    this.externalTaskId = '';
    this.monitorMeta = null;
    this.targetNoteId = '';
    this.surfaceOnly = false;
    this._stoppedByUser = false;
    this._includeComments = false;
    this._commentLimit = 0;
    this._commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL;
    this._totalCommentsCollected = 0;
    this._searchFilters = normalizeXhsSearchFilters();
    this._searchFilterSnapshot = null;
    this.reportHeartbeat = createCollectionRunHeartbeatReporter({ collectionRunStore });
  }

  async start(mode, onProgress, settings = {}) {
    this.isRunning = true;
    this.isPaused = false;
    this._stoppedByUser = false;
    this._setState(TASK_STATE.RUNNING, 'init');
    this.currentIndex = 0;
    this.collected = [];
    this.failed = [];
    this.commentResults = [];
    this._totalCommentsCollected = 0;
    this._originUrl = window.location.href;
    this.onStateChange = typeof onProgress === 'function' ? onProgress : null;
    this.externalTaskId = String(settings.externalTaskMeta?.externalTaskId || '').trim();
    this.monitorMeta = settings.monitorMeta || settings.externalTaskMeta?.monitorMeta || null;
    this.targetNoteId = String(
      settings.targetNoteId
      || this.monitorMeta?.targetNoteId
      || settings.externalTaskMeta?.monitorMeta?.targetNoteId
      || '',
    ).trim().replace(/^xhs_/, '');
    this.surfaceOnly = Boolean(settings.surfaceOnly && this.monitorMeta);

    // 先做作者身份前置校验，避免走到正式采集和任务记录创建
    this._containerSelector = this._getContainerSelector(mode);
    this._mode = mode;
    this._allowNavigationFallback = mode !== COLLECT_MODE.PROFILE;
    const targetCheck = checkXhsAuthorMonitorTarget(this.monitorMeta, { mode });
    if (!targetCheck.ok && targetCheck.code === 'target_mismatch') {
      const error = createTargetMismatchError(targetCheck);
      this.isRunning = false;
      this.isPaused = false;
      this._setState(TASK_STATE.ERROR, 'target_mismatch');
      this._emitProgress({
        status: TASK_STATE.ERROR,
        phase: 'startup',
        current: 0,
        total: 0,
        message: error.message,
        errorCode: 'target_mismatch',
        reasonCode: 'target_mismatch',
      });
      reportProgress(0, 0, error.message, {
        taskType: this.type,
        taskState: TASK_STATE.ERROR,
        phase: 'startup',
        errorCode: 'target_mismatch',
        reasonCode: 'target_mismatch',
      });
      throw error;
    }

    const { count = 10, topByLikes = false } = settings;
    this._topByLikes = Boolean(topByLikes);
    this._includeComments = Boolean(settings.includeComments || settings.collectComments) && !this.surfaceOnly;
    const configuredCommentLimit = Number(settings.commentLimit ?? 30);
    this._commentLimit = this._includeComments && Number.isFinite(configuredCommentLimit)
      ? Math.max(0, Math.floor(configuredCommentLimit))
      : 0;
    this._commentDepthMode = String(settings.commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL).trim() === COMMENT_DEPTH_MODE.ALL_REPLIES
      ? COMMENT_DEPTH_MODE.ALL_REPLIES
      : COMMENT_DEPTH_MODE.TWO_LEVEL;
    this._searchFilters = normalizeXhsSearchFilters(settings.searchFilters || {});
    this._searchFilterSnapshot = mode === COLLECT_MODE.SEARCH
      ? readCurrentXhsSearchFilterSnapshot(window)
      : null;
    const searchFilterSummary = summarizeXhsSearchFilters(this._searchFilters);
    if (mode === COLLECT_MODE.SEARCH && hasExplicitXhsSearchFilters(this._searchFilters)) {
      try {
        this._emitProgress({
          status: 'filtering',
          total: 0,
          current: 0,
          message: `正在应用小红书筛选：${searchFilterSummary}`,
        });
        const filterResult = await applyXhsSearchFilters(this._searchFilters, {
          document,
          win: window,
          onResultsWait: () => {
            this._emitProgress({
              status: 'filtering',
              total: 0,
              current: 0,
              message: `等待筛选结果加载：${searchFilterSummary}`,
            });
          },
        });
        this._searchFilterSnapshot = filterResult.snapshot;
      } catch (error) {
        const message = `小红书筛选失败：${String(error?.message || '请刷新搜索页后重试')}`;
        this.isRunning = false;
        this.isPaused = false;
        this._setState(TASK_STATE.ERROR, 'filtering');
        this._emitProgress({
          status: TASK_STATE.ERROR,
          phase: 'filtering',
          total: 0,
          current: 0,
          message,
          errorCode: 'xhs_search_filter_failed',
          reasonCode: 'xhs_search_filter_failed',
        });
        reportProgress(0, 0, message, {
          taskType: this.type,
          taskState: TASK_STATE.ERROR,
          phase: 'filtering',
          errorCode: 'xhs_search_filter_failed',
          reasonCode: 'xhs_search_filter_failed',
        });
        throw new Error(message);
      }
    }
    const triggerSource = String(settings.triggerSource || 'popup_manual').trim() || 'popup_manual';
    const runConfig = {
      count: this.targetNoteId ? 1 : count,
      topByLikes: this._topByLikes,
    };
    if (this._includeComments) {
      runConfig.includeComments = true;
      runConfig.commentLimit = this._commentLimit;
      runConfig.commentDepthMode = this._commentDepthMode;
    }
    if (mode === COLLECT_MODE.SEARCH) {
      runConfig.searchFilters = this._searchFilters;
      runConfig.searchFilterSummary = searchFilterSummary;
      runConfig.searchFilterSnapshot = this._searchFilterSnapshot;
    }
    if (this.monitorMeta) {
      runConfig.surfaceOnly = this.surfaceOnly;
      runConfig.monitorMeta = this.monitorMeta;
    }
    const existingCollectionRunId = String(settings.collectionRunId || '').trim();
    let existingRun = await resolveExistingBatchRun({
      collectionRunId: existingCollectionRunId,
      externalTaskId: this.externalTaskId,
      taskType: this.type,
    });
    const runPayload = existingCollectionRunId
      || existingRun?.collectionRunId
      ? null
      : buildRemoteRunCreatePayload({
        platform: 'xhs',
        taskType: 'batchNotes',
        pageType: mode,
        triggerSource,
        pageUrl: window.location.href,
        config: runConfig,
        externalTaskMeta: settings.externalTaskMeta || {},
      });
    if (runPayload) {
      const run = await collectionRunStore.createRun(runPayload);
      this.collectionRunId = run.collectionRunId;
      existingRun = run;
    } else if (existingCollectionRunId || existingRun?.collectionRunId) {
      this.collectionRunId = existingCollectionRunId || existingRun.collectionRunId;
    } else {
      this.collectionRunId = '';
    }

    // 1. 启动验证码监控
    this.captchaWatcher = watchCaptcha(async () => {
      this.pause();
      const action = await showCaptchaPauseOverlay();
      if (action === 'resume') {
        this.resume();
      } else {
        this.stop();
      }
    });

    try {
      if (mode === COLLECT_MODE.DETAIL && !this.surfaceOnly) {
        await this._captureCurrentDetailTask();
        return;
      }

      // 2. 发现笔记列表（含滚动加载）
      this._emitProgress({
        status: 'discovering',
        total: 0,
        current: 0,
        message: '正在扫描页面笔记...',
      });
      if (this.surfaceOnly && mode === COLLECT_MODE.SEARCH) {
        const searchApiNotes = await discoverSearchSurfaceNotesFromApi({
          expectedCount: this.targetNoteId ? 1 : count,
          currentUrl: window.location.href,
        }).catch(() => []);
        this.noteList = searchApiNotes.length > 0
          ? searchApiNotes
          : await discoverWithScroll(this._containerSelector, 10, {
            expectedCount: this.targetNoteId ? 1 : count,
          });
      } else if (this.surfaceOnly && mode === COLLECT_MODE.PROFILE) {
        const profileApiNotes = await discoverProfileSurfaceNotesFromApi({
          expectedCount: this.targetNoteId ? 1 : count,
          currentUrl: window.location.href,
        }).catch(() => []);
        this.noteList = profileApiNotes.length > 0
          ? profileApiNotes
          : await discoverWithScroll(this._containerSelector, 10, {
            expectedCount: this.targetNoteId ? 1 : count,
          });
      } else {
        this.noteList = await discoverWithScroll(this._containerSelector, 10, {
          expectedCount: this.targetNoteId ? 1 : count,
        });
      }
      this.noteList = filterTargetedXhsNoteList(this.noteList, this.targetNoteId);
      if (this.targetNoteId && this.noteList.length === 0) {
        throw new Error(`目标作品未在当前作者页找到：${this.targetNoteId}`);
      }

    // 3. 如果按点赞排序，取 Top N
    if (topByLikes) {
      this.noteList = this.noteList
        .map((note) => ({ ...note, __likesParsed: parseCount(note.likes) }))
        .sort((a, b) => {
          const likesDiff = (b.__likesParsed || 0) - (a.__likesParsed || 0);
          if (likesDiff !== 0) return likesDiff;
          return Number(a._top || 0) - Number(b._top || 0);
        });
    }

    // 4. 限制篇数
    const maxNotes = Math.min(this.noteList.length, this.targetNoteId ? 1 : count);
    this.noteList = this.noteList.slice(0, maxNotes);
    if (topByLikes) {
      this.noteList = this.noteList.map((note, idx) => ({ ...note, __topRank: idx + 1 }));
    }
    // TopN 只决定"选哪些笔记"，执行顺序按页面位置，减少来回滚动
    if (topByLikes) {
      this.noteList.sort((a, b) => Number(a._top || 0) - Number(b._top || 0));
    }
    const resumeState = resolveBatchResumeState({
      runRecord: existingRun,
      targets: this.noteList,
      getTargetId: (item) => item.noteId,
    });
    this.noteList = resumeState.targets;
    if (resumeState.resumed) {
      this.currentIndex = resumeState.nextIndex;
      const hydrated = hydrateXhsNoteResumeState(existingRun, resumeState.completedTargetIds);
      this.collected = hydrated.collected;
      this.failed = hydrated.failed;
      this.commentResults = hydrated.commentResults;
      this._totalCommentsCollected = hydrated.totalComments;
      this._emitProgress({
        status: 'resuming',
        total: this.noteList.length,
        current: this.currentIndex,
        message: `已从本地记录恢复，前 ${this.currentIndex}/${this.noteList.length} 篇不重复采集`,
      });
    }

    const topLikeText = topByLikes && this.noteList.length > 0
      ? `，最高点赞约 ${formatCompactCount(Math.max(...this.noteList.map((n) => Number(n.__likesParsed || 0))))}`
      : '';
    if (this.surfaceOnly) {
      await this._completeSurfaceScan({
        mode,
        sourcePageUrl: window.location.href,
      });
      return;
    }

    this._emitProgress({
      status: 'started',
      total: maxNotes,
      current: 0,
      message: `发现 ${maxNotes} 篇笔记，开始采集${topByLikes ? `（已按点赞筛选 Top ${maxNotes}${topLikeText}）` : ''}${mode === COLLECT_MODE.SEARCH && hasExplicitXhsSearchFilters(this._searchFilters) ? `（页面筛选：${searchFilterSummary}）` : ''}`,
    });

    // 5. 逐篇采集
      await this._captureLoop();
    } catch (error) {
      await this._cleanupAfterLoop();
      await this._markRunFailed(error);
      throw error;
    }
  }

  async _completeSurfaceScan({ mode = '', sourcePageUrl = '' } = {}) {
    const records = buildXhsSurfaceNoteRecords(this.noteList, {
      monitorMeta: this.monitorMeta,
      collectionRunId: this.collectionRunId,
      mode: this.monitorMeta?.surfaceMode,
      limit: this.noteList.length,
      sourcePageUrl,
      searchKeyword: mode === COLLECT_MODE.SEARCH ? this.monitorMeta?.keyword : '',
      searchPageUrl: mode === COLLECT_MODE.SEARCH ? sourcePageUrl : '',
      searchFilters: mode === COLLECT_MODE.SEARCH ? this._searchFilters : undefined,
      searchFilterSnapshot: mode === COLLECT_MODE.SEARCH ? this._searchFilterSnapshot : undefined,
    });
    this.collected = records;

    if (records.length > 0) {
      await noteStore.bulkUpsert(records);
      records.forEach((record) => this._reportCollectedNote(record));
    }

    if (this.collectionRunId) {
      await this._finalizeCollectionRun('done', buildXhsBatchNotesRunPatch({
        noteList: this.noteList,
        collected: this.collected,
        failed: this.failed,
        commentResults: this.commentResults,
      }));
    }
    await this._cleanupAfterLoop();
    this._setState(TASK_STATE.DONE, 'done');
    this._emitProgress({
      status: 'done',
      total: this.noteList.length,
      current: records.length,
      failed: 0,
      message: `表层扫描完成：发现 ${records.length} 条内容`,
    });
    reportDone('batchNotes', records.length, {
      taskType: this.type,
      taskState: this.state,
      phase: 'done',
    });
  }

  async _finalizeCollectionRun(status, patch = {}) {
    if (!this.collectionRunId) return null;
    const finalizer = status === 'stopped'
      ? collectionRunStore.markStopped
      : collectionRunStore.markDone;
    const updated = await finalizer.call(collectionRunStore, this.collectionRunId, patch);
    if (!updated) {
      throw new Error(`采集运行记录最终状态写入失败：${this.collectionRunId}`);
    }
    return updated;
  }

  async _captureLoop() {
    while (this.currentIndex < this.noteList.length && this.isRunning) {
      await this._waitIfPaused();
      if (!this.isRunning) break;
      if (await this._pauseForRiskControl()) {
        continue;
      }
      const noteInfo = this.noteList[this.currentIndex];
      this.currentIndex++;
      const rankHint = this._topByLikes
        ? `｜Top ${noteInfo.__topRank || this.currentIndex}/${this.noteList.length}（赞 ${formatCompactCount(noteInfo.__likesParsed ?? parseCount(noteInfo.likes))}）`
        : '';

      this._emitProgress({
        status: 'collecting',
        total: this.noteList.length,
        current: this.currentIndex,
        message: `正在采集第 ${this.currentIndex}/${this.noteList.length} 篇：${noteInfo.title || noteInfo.noteId}${rankHint}`,
      });
      reportProgress(this.currentIndex, this.noteList.length, '采集中', {
        taskType: this.type,
        taskState: this.state,
        phase: 'collect',
      });
      void this.reportHeartbeat.report(this.collectionRunId, {
        taskState: this.state,
        stage: 'collecting',
        current: this.currentIndex,
        total: this.noteList.length,
        message: `正在采集第 ${this.currentIndex}/${this.noteList.length} 篇`,
      }).catch(() => {});

      try {
        const success = await this._captureOneNote(noteInfo);
        if (!success) {
          if (!this.isRunning || this.isPaused) continue;
          // 弹窗方式失败，尝试 URL 导航 fallback
          if (this._allowNavigationFallback) {
            console.log('[灵感爆爆爆] 弹窗方式失败，尝试 URL 导航:', noteInfo.noteId);
            await randomDelay(250, 450);
            const fallbackSuccess = await this._captureByNavigation(noteInfo);
            if (!fallbackSuccess) {
              this.failed.push({ noteId: noteInfo.noteId, error: '弹窗和导航方式均失败' });
            }
          } else {
            this.failed.push({ noteId: noteInfo.noteId, error: '弹窗采集失败（已禁用导航回退）' });
          }
        }
      } catch (err) {
        console.warn(`[灵感爆爆爆] 采集异常: ${noteInfo.noteId}`, err);
        this.failed.push({ noteId: noteInfo.noteId, error: err.message });
        await this._closeNotePopup();
      } finally {
        await this._syncRunProgress();
      }

      // 分级节流
      await this._throttleAfterOne();
    }

    const stopped = this._stoppedByUser;
    const finalRunPatch = buildXhsBatchNotesRunPatch({
      noteList: this.noteList,
      collected: this.collected,
      failed: this.failed,
      commentResults: this.commentResults,
    });
    if (stopped) {
      await this._finalizeCollectionRun('stopped', finalRunPatch);
    } else if (this.collectionRunId) {
      await this._finalizeCollectionRun('done', finalRunPatch);
    }
    await this._cleanupAfterLoop();
    this._setState(TASK_STATE.DONE, stopped ? 'stopped' : 'done');
    this._emitProgress({
      status: 'done',
      total: this.noteList.length,
      current: this.collected.length,
      failed: this.failed.length,
      message: stopped
        ? `批量笔记已停止：成功 ${this.collected.length}，评论 ${this._totalCommentsCollected} 条，失败 ${this.failed.length}`
        : `采集完成：成功 ${this.collected.length}，评论 ${this._totalCommentsCollected} 条，失败 ${this.failed.length}`,
    });
    reportDone('batchNotes', this.collected.length, {
      taskType: this.type,
      taskState: this.state,
      phase: 'done',
    });
  }

  _resolveCurrentDetailNoteInfo() {
    const currentUrl = String(window.location.href || '').trim();
    const noteId = String(this.targetNoteId || extractNoteId(currentUrl) || '').trim().replace(/^xhs_/, '');
    if (!noteId) {
      throw new Error('当前详情页缺少目标作品 ID');
    }
    return {
      noteId,
      url: currentUrl,
      canonicalUrl: currentUrl,
      rawUrl: currentUrl,
    };
  }

  async _captureCurrentDetailTask() {
    const noteInfo = this._resolveCurrentDetailNoteInfo();
    this.noteList = [noteInfo];
    this.currentIndex = 1;

    this._emitProgress({
      status: 'collecting',
      total: 1,
      current: 1,
      message: `正在采集当前作品详情：${noteInfo.noteId}`,
    });
    reportProgress(1, 1, '正在采集当前作品详情', {
      taskType: this.type,
      taskState: this.state,
      phase: 'collect',
    });
    void this.reportHeartbeat.report(this.collectionRunId, {
      taskState: this.state,
      stage: 'collecting',
      current: 1,
      total: 1,
      message: '正在采集当前作品详情',
    }).catch(() => {});

    try {
      if (await this._pauseForRiskControl()) {
        await this._waitIfPaused();
      }
      if (!this.isRunning) {
        await this._cleanupAfterLoop();
        return;
      }

      const pageReady = await this._waitForNoteLoad(noteInfo.noteId, 10000);
      if (!pageReady) {
        throw new Error(`目标作品详情页未加载完成：${noteInfo.noteId}`);
      }

      const stateReady = await waitForNoteState(noteInfo.noteId, 10000);
      if (!stateReady) {
        console.warn('[灵感爆爆爆] __INITIAL_STATE__ 未就绪，尝试直接采集当前详情页:', noteInfo.noteId);
      }

      await this._settleAfterDetailReady();
      const warmedUp = await this._waitForNoteDataStable(noteInfo.noteId, 6000);
      if (!warmedUp) {
        console.warn('[灵感爆爆爆] 当前详情页目标笔记数据仍未稳定，进入保守重试模式:', noteInfo.noteId);
      }

      const collectedNote = await this._collectCurrentDetailNote(noteInfo);

      await this._collectAttachedComments(
        noteInfo,
        collectedNote?.url || collectedNote?.canonicalUrl || noteInfo.url || window.location.href,
        collectedNote,
      );

      await this._syncRunProgress();
      if (this.collectionRunId) {
        await this._finalizeCollectionRun('done', buildXhsBatchNotesRunPatch({
          noteList: this.noteList,
          collected: this.collected,
          failed: this.failed,
          commentResults: this.commentResults,
        }));
      }
      await this._cleanupAfterLoop();
      this._setState(TASK_STATE.DONE, 'done');
      this._emitProgress({
        status: 'done',
        total: 1,
        current: this.collected.length,
        failed: this.failed.length,
        message: `采集完成：成功 ${this.collected.length}，评论 ${this._totalCommentsCollected} 条，失败 ${this.failed.length}`,
      });
      reportDone('batchNotes', this.collected.length, {
        taskType: this.type,
        taskState: this.state,
        phase: 'done',
      });
    } catch (error) {
      this.failed.push({ noteId: noteInfo.noteId, error: String(error?.message || error) });
      await this._syncRunProgress();
      throw error;
    }
  }

  async _collectCurrentDetailNote(noteInfo) {
    let collectedNote = null;
    for (let attempt = 0; attempt < 3; attempt++) {
      try {
        await this._waitIfPaused();
        if (!this.isRunning) break;
        if (attempt > 0) {
          await this._waitForNoteDataStable(noteInfo.noteId, 2600 + (attempt * 1200));
          await randomDelay(420, 760);
        }
        const result = await collectNote(window, {
          collectionRunId: this.collectionRunId,
          expectedNoteId: noteInfo.noteId,
          monitorMeta: this.monitorMeta,
        });
        if (String(result?.noteId || '').trim() !== String(noteInfo.noteId || '').trim()) {
          throw new Error(`采集到的笔记与目标不一致: expected=${noteInfo.noteId} actual=${result?.noteId || ''}`);
        }
        this.collected.push(result);
        await noteStore.upsert(result);
        this._reportCollectedNote(result);
        collectedNote = result;
        console.log(`[灵感爆爆爆] 当前详情页采集成功: ${noteInfo.noteId} (${result.title})`);
        break;
      } catch (err) {
        console.warn(`[灵感爆爆爆] 当前详情页采集第 ${attempt + 1} 次失败: ${noteInfo.noteId}`, err.message);
      }
    }

    if (!collectedNote) {
      // 2026-07-08 归因修复：重试耗尽后不能只报通用文案——服务端靠关键词识别
      // "内容已失效"，通用文案匹配不上，会被误判成"稍后再试"而不停重试。
      // 这里复用 capabilityCheck.js 派发前用过的同一套失效 title 特征词
      // （见 shared/deadPageSignals.js），命中才升级成明确的失效文案；不命中
      // 说明是别的原因（页面卡顿、字段缺失等），保留原通用文案，不误伤。
      const pageTitle = String(document.title || '').trim();
      if (looksLikeDeadPageTitle(pageTitle)) {
        throw new Error(`笔记不存在或暂时无法浏览：${noteInfo.noteId}（页面标题：${pageTitle}）`);
      }
      throw new Error(`目标作品详情采集失败：${noteInfo.noteId}`);
    }
    return collectedNote;
  }

  async _throttleAfterOne() {
    // TopN 在搜索页使用更快节奏；博主页保持保守节奏以降低风控概率
    if (this._topByLikes && this._mode !== COLLECT_MODE.PROFILE) {
      await randomDelay(300, 600);
      return;
    }
    await throttle(this.collected.length);
  }

  _emitProgress(payload) {
    this.onStateChange?.({
      taskType: this.type,
      taskState: this.state,
      phase: payload.status || 'running',
      ...payload,
    });
  }

  _setState(state, phase = 'running') {
    this.state = state;
    this._emitProgress({
      status: state,
      phase,
      current: this.currentIndex,
      total: this.noteList.length,
      message: '',
    });
  }

  async _syncRunProgress() {
    if (!this.collectionRunId) return;
    const runPatch = buildXhsBatchNotesProgressPatch({
      noteList: this.noteList,
      collected: this.collected,
      failed: this.failed,
      commentResults: this.commentResults,
      processedCount: this.currentIndex,
    });
    await collectionRunStore.updateById(this.collectionRunId, runPatch).catch(() => {});
  }

  _buildPartialRunPatch() {
    return buildXhsBatchNotesProgressPatch({
      noteList: this.noteList,
      collected: this.collected,
      failed: this.failed,
      commentResults: this.commentResults,
      processedCount: this.collected.length + this.failed.length,
    });
  }

  async _persistPausedState() {
    if (!this.collectionRunId || !collectionRunStore?.markPaused) return;
    await collectionRunStore.markPaused(this.collectionRunId, this._buildPartialRunPatch()).catch(() => {});
  }

  async _persistRunningState() {
    if (!this.collectionRunId) return;
    await collectionRunStore.updateById(this.collectionRunId, {
      ...this._buildPartialRunPatch(),
      status: 'running',
      finishedAt: undefined,
    }).catch(() => {});
  }

  async _persistStoppedState() {
    if (!this.collectionRunId) return;
    await collectionRunStore.markStopped(this.collectionRunId, this._buildPartialRunPatch()).catch(() => {});
  }

  async _captureOneNote(noteInfo) {
    const urlBefore = window.location.href;
    let element = null;

    try {
      // 1. 滚动到笔记大致位置，让虚拟列表渲染出卡片
      element = await this._scrollToAndFindNote(noteInfo);
      if (!element) {
        console.warn('[灵感爆爆爆] 无法在 DOM 中找到笔记卡片:', noteInfo.noteId);
        return false;
      }

      await randomDelay(this._topByLikes ? 40 : 50, this._topByLikes ? 70 : 100);

      await this._waitIfPaused();
      if (!this.isRunning) return false;

      // 3. 点击打开笔记
      const coverLink = element.querySelector('a.cover');
      if (!coverLink) return false;
      coverLink.click();

      // 4. 等待页面响应：URL 变化（SPA 路由）或弹窗出现
      const pageReady = await this._waitForNoteLoad(noteInfo.noteId, 10000);
      if (!pageReady) {
        if (await this._pauseForRiskControl()) return false;
        console.warn('[灵感爆爆爆] 笔记页面未加载，跳过:', noteInfo.noteId);
        await this._goBackToList(urlBefore);
        return false;
      }

      await this._waitIfPaused();
      if (!this.isRunning) return false;

      // 5. 等待 __INITIAL_STATE__ 中出现该笔记的数据
      const stateReady = await waitForNoteState(noteInfo.noteId, 10000);
      if (!stateReady) {
        console.warn('[灵感爆爆爆] __INITIAL_STATE__ 未就绪，尝试直接采集:', noteInfo.noteId);
      }

      // 6. 额外等待渲染稳定，并确认目标笔记数据至少连续两次可用
      await this._settleAfterDetailReady();
      const warmedUp = await this._waitForNoteDataStable(noteInfo.noteId, 2500);
      if (!warmedUp) {
        console.warn('[灵感爆爆爆] 目标笔记数据仍未稳定，进入保守重试模式:', noteInfo.noteId);
      }

      // 7. 采集笔记数据（含重试）
      let collected = false;
      let collectedNote = null;
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          await this._waitIfPaused();
          if (!this.isRunning) break;
          if (attempt > 0) {
            await this._waitForNoteDataStable(noteInfo.noteId, 2600 + (attempt * 1200));
            await randomDelay(420, 760);
          }
          const result = mergeSurfaceCoverFallback(await collectNote(window, {
            collectionRunId: this.collectionRunId,
            expectedNoteId: noteInfo.noteId,
            monitorMeta: this.monitorMeta,
          }), noteInfo);
          if (String(result?.noteId || '').trim() !== String(noteInfo.noteId || '').trim()) {
            throw new Error(`采集到的笔记与目标不一致: expected=${noteInfo.noteId} actual=${result?.noteId || ''}`);
          }
          this.collected.push(result);
          await noteStore.upsert(result);
          this._reportCollectedNote(result);
          collectedNote = result;
          collected = true;
          console.log(`[灵感爆爆爆] 采集成功: ${noteInfo.noteId} (${result.title})`);
          break;
        } catch (err) {
          console.warn(`[灵感爆爆爆] 采集第 ${attempt + 1} 次失败: ${noteInfo.noteId}`, err.message);
        }
      }

      if (collected) {
        await this._collectAttachedComments(
          noteInfo,
          collectedNote?.url || collectedNote?.canonicalUrl || noteInfo.url || window.location.href,
          collectedNote,
        );
      }

      // 8. 返回列表页
      await this._goBackToList(urlBefore);
      await randomDelay(120, 200);

      return collected;
    } finally {
      this._cleanupCollectingMarks();
    }
  }

  async _captureByNavigation(noteInfo) {
    if (await this._pauseForRiskControl()) return false;
    const noteUrl = noteInfo.url || `/explore/${noteInfo.noteId}`;
    const fullUrl = noteUrl.startsWith('http') ? noteUrl : `https://www.xiaohongshu.com${noteUrl}`;

    try {
      const urlBefore = window.location.href;

      window.location.href = fullUrl;

      await this._waitForPageLoad(15000);

      await waitForNoteState(noteInfo.noteId, 10000);
      if (await this._pauseForRiskControl()) return false;
      await this._settleAfterDetailReady();
      await this._waitForNoteDataStable(noteInfo.noteId, 2500);

      const result = mergeSurfaceCoverFallback(await collectNote(window, {
        collectionRunId: this.collectionRunId,
        expectedNoteId: noteInfo.noteId,
        monitorMeta: this.monitorMeta,
      }), noteInfo);
      if (String(result?.noteId || '').trim() !== String(noteInfo.noteId || '').trim()) {
        throw new Error(`采集到的笔记与目标不一致: expected=${noteInfo.noteId} actual=${result?.noteId || ''}`);
      }
      this.collected.push(result);
      await noteStore.upsert(result);
      this._reportCollectedNote(result);
      console.log(`[灵感爆爆爆] Fallback 采集成功: ${noteInfo.noteId} (${result.title})`);

      await this._collectAttachedComments(
        noteInfo,
        result?.url || result?.canonicalUrl || fullUrl,
        result,
      );

      await this._goBackToList(urlBefore);

      return true;
    } catch (err) {
      console.warn(`[灵感爆爆爆] URL 导航采集失败: ${noteInfo.noteId}`, err.message);
      try {
        window.history.back();
        await randomDelay(700, 1100);
      } catch { /* ignore */ }
      return false;
    }
  }

  _reportCollectedNote(result = {}) {
    if (!this.collectionRunId) return;
    if (this._shouldUseFinalResultPackageOnly()) return;
    const record = withMonitorRecordMeta(
      buildWorkbenchNoteRecord(result),
      this.monitorMeta || result.monitorMeta,
      result.monitorMode,
    );
    const externalRecordId = String(record.noteId || record.platformContentId || record.url || '').trim();
    if (!externalRecordId && !record.title && !record.content) return;
    reportWorkbenchRecord({
      recordType: WORKBENCH_RECORD_TYPE.NOTE,
      externalRecordId,
      record,
      collectionRunId: this.collectionRunId,
      externalTaskId: this.externalTaskId,
      sequence: Date.now(),
      collectedAt: new Date().toISOString(),
    });
  }

  _shouldUseFinalResultPackageOnly() {
    return Boolean(this._includeComments && this._commentLimit > 0 && !this.surfaceOnly);
  }

  async _collectAttachedComments(noteInfo = {}, noteUrl = '', collectedNote = {}) {
    if (!this._includeComments || this._commentLimit <= 0) return { total: 0, comments: [] };
    if (!this.isRunning) return { total: 0, comments: [] };

    const noteId = String(noteInfo?.noteId || '').trim().replace(/^xhs_/, '');
    if (!noteId) return { total: 0, comments: [] };
    const publicCommentCount = publicCommentCountFromXhsNote(collectedNote);
    if (publicCommentCount === 0) {
      const result = buildXhsAttachedCommentResult({
        noteId,
        total: 0,
        publicCommentCount,
        requestedCommentLimit: this._commentLimit,
      });
      this.commentResults.push(result);
      return { total: 0, comments: [], publicCommentCount };
    }

    try {
      this._emitProgress({
        status: 'collecting_comments',
        total: this.noteList.length,
        current: this.currentIndex,
        message: `正在顺手采集第 ${this.currentIndex}/${this.noteList.length} 篇前 ${this._commentLimit} 条评论`,
      });
      const ready = await this._waitForAttachedCommentContext(noteId, 8000);
      if (!ready) {
        this._emitProgress({
          status: 'collecting_comments',
          total: this.noteList.length,
          current: this.currentIndex,
          message: `第 ${this.currentIndex}/${this.noteList.length} 篇评论区未完全稳定，继续尝试从页面接口同步评论`,
        });
      }

      await randomDelay(ready ? 120 : 220, ready ? 220 : 420);
      const result = await collectComments({
        noteId,
        noteUrl: noteUrl || noteInfo.url || window.location.href,
        maxTotal: this._commentLimit,
        maxSubComments: this._commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : BATCH_CONFIG.maxSubComments,
        commentDepthMode: this._commentDepthMode,
        shouldStop: () => !this.isRunning,
        waitIfPaused: () => this._waitIfPaused(),
        collectionRunId: this.collectionRunId,
        onProgress: (progress) => {
          const message = progress?.message || `正在顺手采集第 ${this.currentIndex}/${this.noteList.length} 篇评论`;
          void this.reportHeartbeat.report(this.collectionRunId, {
            taskState: this.state,
            stage: 'collecting_comments',
            current: this.currentIndex,
            total: this.noteList.length,
            message,
          }).catch(() => {});
        },
      });
      const comments = Array.isArray(result?.comments) ? result.comments : [];
      const total = Number(result?.total ?? comments.length) || 0;
      const commentResult = buildXhsAttachedCommentResult({
        noteId,
        total,
        publicCommentCount,
        requestedCommentLimit: this._commentLimit,
      });
      this._totalCommentsCollected += total;
      this.commentResults.push(commentResult);
      comments.forEach((comment, index) => this._reportCollectedComment(comment, noteInfo, index));
      return {
        total,
        comments,
        ...(publicCommentCount !== null ? { publicCommentCount } : {}),
        ...(commentResult.error ? { error: commentResult.error } : {}),
      };
    } catch (error) {
      const result = buildXhsAttachedCommentResult({
        noteId,
        total: 0,
        error: String(error?.message || error || 'comments_failed'),
        publicCommentCount,
        requestedCommentLimit: this._commentLimit,
      });
      this.commentResults.push(result);
      console.warn(`[灵感爆爆爆] 附带评论采集失败: ${noteId}`, error);
      return result;
    }
  }

  async _waitForAttachedCommentContext(noteId, timeout = 8000) {
    const startedAt = Date.now();
    let stableRounds = 0;
    while (Date.now() - startedAt < timeout) {
      await this._waitIfPaused();
      if (!this.isRunning) return false;
      if (await this._pauseForRiskControl()) {
        await this._waitIfPaused();
        if (!this.isRunning) return false;
        continue;
      }

      const pathname = window.location.pathname || '';
      const inDetail = pathname.includes(noteId) || isPopupOpen() || isNoteDetailReady();
      const context = getActiveCommentsContext();
      const hasContainer = Boolean(context.container);
      const hasCommentItems = Boolean(context.hasCommentItems);
      const hasCommentMeta = Boolean(context.hasCommentMeta || context.hasExplicitEmptyState);

      if (inDetail && hasContainer && (hasCommentItems || hasCommentMeta)) {
        stableRounds += 1;
        if (stableRounds >= 2) return true;
      } else {
        stableRounds = 0;
      }

      void this.reportHeartbeat.report(this.collectionRunId, {
        taskState: this.state,
        stage: 'context_check',
        current: this.currentIndex,
        total: this.noteList.length,
        message: `正在等待第 ${this.currentIndex}/${this.noteList.length} 篇评论区稳定`,
      }).catch(() => {});
      await randomDelay(140, 220);
    }
    return false;
  }

  _reportCollectedComment(comment = {}, noteInfo = {}, offset = 0) {
    if (!this.collectionRunId) return;
    if (this._shouldUseFinalResultPackageOnly()) return;
    const noteId = String(comment.noteId || noteInfo.noteId || '').trim().replace(/^xhs_/, '');
    const record = withMonitorRecordMeta({
      ...comment,
      platform: String(comment.platform || 'xhs').trim() || 'xhs',
      noteId,
      platformContentId: String(comment.platformContentId || noteId || '').trim(),
      contentId: String(comment.contentId || (noteId ? `xhs_${noteId}` : '')).trim(),
    }, this.monitorMeta || comment.monitorMeta, comment.monitorMode);
    const externalRecordId = String(record.commentId || record.id || record.commentEntityId || '').trim();
    if (!externalRecordId || !String(record.text || record.content || record.commentText || '').trim()) return;
    reportWorkbenchRecord({
      recordType: WORKBENCH_RECORD_TYPE.COMMENT,
      externalRecordId,
      record,
      collectionRunId: this.collectionRunId,
      externalTaskId: this.externalTaskId,
      sequence: Date.now() + offset,
      collectedAt: new Date().toISOString(),
    });
  }

  _waitForPageLoad(timeout) {
    return new Promise((resolve) => {
      if (document.readyState === 'complete') {
        resolve();
        return;
      }
      const timer = setTimeout(resolve, timeout);
      window.addEventListener('load', () => {
        clearTimeout(timer);
        resolve();
      }, { once: true });
    });
  }

  async _settleAfterDetailReady() {
    const startedAt = Date.now();
    while (Date.now() - startedAt < 250) {
      if (isNoteDetailReady() || isPopupOpen()) {
        const text = String(document.body?.innerText || '').slice(0, 2000);
        if (/#/.test(text) || /赞|评论|收藏/.test(text)) return;
      }
      const text = String(document.body?.innerText || '').slice(0, 2000);
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    await randomDelay(80, 150);
  }

  _readExpectedNoteSnapshot(noteId) {
    const noteMap = window.__INITIAL_STATE__?.note?.noteDetailMap || {};
    return resolveExpectedNoteFromMap(noteMap, noteId, window.location.href);
  }

  _isNoteDataComplete(snapshot) {
    if (!snapshot.exactMatch || !snapshot.usable) return false;
    const { note } = snapshot;
    if (!note) return false;

    // 检查关键互动数据是否全部到位（避免 AJAX 分批填充时过早采集）
    const interactInfo = note.interactInfo;
    const hasFullStats = Boolean(
      interactInfo
      && (interactInfo.likedCount != null || interactInfo.likeCount != null)
      && (interactInfo.collectedCount != null || interactInfo.collectCount != null)
      && (interactInfo.commentCount != null || interactInfo.comments != null)
    );

    // 检查媒体数据是否完整（图片必须有有效 URL）
    const hasValidMedia = Boolean(
      (Array.isArray(note.imageList) && note.imageList.length > 0
        && (note.imageList[0]?.url || note.imageList[0]?.urlDefault))
      || note.video?.media?.stream
      || note.video?.consumer
    );

    return hasFullStats && hasValidMedia;
  }

  async _waitForNoteDataStable(noteId, timeout = 6000) {
    const startedAt = Date.now();
    let stableRounds = 0;

    while (Date.now() - startedAt < timeout) {
      if (await this._pauseForRiskControl()) return false;
      const snapshot = this._readExpectedNoteSnapshot(noteId);
      // detail_probe 场景要求数据完整，不能只用 isCollectedNoteUsable 的宽松判定
      const isComplete = this._isNoteDataComplete(snapshot);
      if (isComplete) {
        stableRounds += 1;
        if (stableRounds >= 2) return true;
      } else {
        stableRounds = 0;
      }

      void this.reportHeartbeat.report(this.collectionRunId, {
        taskState: this.state,
        stage: 'context_check',
        current: this.currentIndex,
        total: this.noteList.length,
        message: `正在等待第 ${this.currentIndex}/${this.noteList.length} 篇目标笔记数据稳定`,
      }).catch(() => {});

      await new Promise((resolve) => setTimeout(resolve, 120));
    }

    return false;
  }

  async _scrollToAndFindNote(noteInfo) {
    let element = findNoteElementById(noteInfo.noteId, this._containerSelector);
    const focusDelayMin = this._topByLikes ? 180 : 300;
    const focusDelayMax = this._topByLikes ? 280 : 520;
    if (element && document.body.contains(element)) {
      element.scrollIntoView({ behavior: 'auto', block: 'center' });
      await randomDelay(this._topByLikes ? 70 : 90, this._topByLikes ? 110 : 150);
      return element;
    }

    const targetY = Math.max(Math.round((noteInfo._top || 0) - window.innerHeight * 0.8), 0);
    if (Math.abs(window.scrollY - targetY) > window.innerHeight) {
      window.scrollTo({ top: targetY, behavior: 'auto' });
      await randomDelay(90, 160);
    }

    const maxScrollAttempts = this._topByLikes ? 12 : 18;
    const scrollStep = Math.round(window.innerHeight * 0.52);

    for (let i = 0; i < maxScrollAttempts; i++) {
      element = findNoteElementById(noteInfo.noteId, this._containerSelector);
      if (element && document.body.contains(element)) {
        element.scrollIntoView({ behavior: 'auto', block: 'center' });
        await randomDelay(this._topByLikes ? 70 : 90, this._topByLikes ? 110 : 150);
        return element;
      }

      window.scrollBy({ top: scrollStep, behavior: 'auto' });
      await randomDelay(this._topByLikes ? 90 : 120, this._topByLikes ? 160 : 200);

      if (window.scrollY > targetY + window.innerHeight * 3) break;
    }

    element = findNoteElementById(noteInfo.noteId, this._containerSelector);
    return element && document.body.contains(element) ? element : null;
  }

  async _waitForNoteLoad(noteId, timeout = 10000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      if (await this._pauseForRiskControl()) {
        return false;
      }
      const path = window.location.pathname;
      if (noteId && (path.includes(`/explore/${noteId}`) || path.includes(`/discovery/item/${noteId}`) || path.includes(`/${noteId}`))) {
        console.log('[灵感爆爆爆] URL 已跳转到笔记详情:', path);
        return true;
      }
      if (isNoteDetailReady()) {
        console.log('[灵感爆爆爆] 笔记详情 DOM 已就绪');
        return true;
      }
      await new Promise(r => setTimeout(r, 180));
    }
    return false;
  }

  async _pauseForRiskControl() {
    if (!isRiskControlPage()) return false;
    const is300017 = isErrorCode300017();
    this.pause();
    this._emitProgress({
      status: TASK_STATE.PAUSED,
      total: this.noteList.length,
      current: this.currentIndex,
      message: is300017
        ? '检测到风控限制（300017），正在自动切换账号...'
        : '检测到安全验证页面，已自动暂停，请完成验证后点击"继续采集"。',
    });
    reportProgress(this.currentIndex, this.noteList.length, is300017 ? '风控300017' : '暂停', {
      taskType: this.type,
      taskState: this.state,
      phase: 'risk_control',
      riskControlCode: is300017 ? '300017' : '',
    });
    return true;
  }

  async _goBackToList(originalUrl) {
    const currentUrl = window.location.href;

    if (currentUrl !== originalUrl) {
      window.history.back();
      for (let i = 0; i < 15; i++) {
        await new Promise(r => setTimeout(r, 150));
        if (!(/\/explore\/[a-z0-9]+/i.test(window.location.pathname)) &&
            !(/\/discovery\/item\/[a-z0-9]+/i.test(window.location.pathname))) {
          await randomDelay(80, 150);
          return;
        }
      }
      console.warn('[灵感爆爆爆] history.back() 超时，直接导航回列表页');
      window.location.href = originalUrl;
      await new Promise(r => setTimeout(r, 700));
    } else {
      await this._closeNotePopup();
    }
  }

  _showCollectingMark(element, noteInfo = {}) {
    if (!element) return;
    element.querySelector('.lgboom-collecting-mark')?.remove();

    const mark = document.createElement('div');
    mark.className = 'lgboom-collecting-mark';
    Object.assign(mark.style, {
      position: 'absolute',
      top: '0',
      left: '0',
      right: '0',
      bottom: '0',
      background: 'rgba(255, 71, 87, 0.28)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      zIndex: '100',
      borderRadius: '8px',
      pointerEvents: 'none',
    });
    const rankText = this._topByLikes
      ? `Top ${noteInfo.__topRank || this.currentIndex}/${this.noteList.length} `
      : '';
    const likesText = this._topByLikes
      ? ` 赞 ${formatCompactCount(noteInfo.__likesParsed ?? parseCount(noteInfo.likes))}`
      : '';
    const span = document.createElement('span');
    span.style.cssText = 'background:#ff4757;color:#fff;padding:6px 16px;border-radius:20px;font-size:14px;font-weight:800;box-shadow:0 2px 8px rgba(255,71,87,0.3);';
    span.textContent = `${rankText}采集中...${likesText}`;
    mark.appendChild(span);

    const currentPosition = window.getComputedStyle(element).position;
    if (currentPosition === 'static') {
      element.style.position = 'relative';
      element.dataset.lgboomMarkAdjusted = '1';
    }
    element.appendChild(mark);
  }

  _removeCollectingMark(element) {
    if (!element) return;
    element.querySelector('.lgboom-collecting-mark')?.remove();
    if (element.dataset.lgboomMarkAdjusted === '1') {
      element.style.position = '';
      delete element.dataset.lgboomMarkAdjusted;
    }
  }

  _cleanupCollectingMarks() {
    document.querySelectorAll('.lgboom-collecting-mark').forEach((el) => el.remove());
  }

  _upsertTopSelectionMark(element, noteInfo = {}) {
    if (!this._topByLikes || !element) return;
    const rank = noteInfo.__topRank || this.currentIndex;
    let badge = element.querySelector('.lgboom-topn-badge');
    if (!badge) {
      badge = document.createElement('div');
      badge.className = 'lgboom-topn-badge';
      Object.assign(badge.style, {
        position: 'absolute',
        top: '8px',
        right: '8px',
        zIndex: '101',
        background: '#3bb8d8',
        border: '2px solid #121212',
        borderRadius: '999px',
        padding: '2px 10px',
        fontSize: '12px',
        fontWeight: '900',
        color: '#121212',
        boxShadow: '2px 2px 0 #121212',
        pointerEvents: 'none',
      });
      const currentPosition = window.getComputedStyle(element).position;
      if (currentPosition === 'static') {
        element.style.position = 'relative';
      }
      element.appendChild(badge);
    }
    badge.textContent = `Top ${rank}`;
  }

  _cleanupTopSelectionMarks() {
    document.querySelectorAll('.lgboom-topn-badge').forEach((el) => el.remove());
  }

  async _closeNotePopup() {
    for (const sel of CLOSE_SELECTORS) {
      try {
        const el = document.querySelector(sel);
        if (el && el.offsetWidth > 0 && el.offsetHeight > 0) {
          el.click();
          await randomDelay(80, 150);
          if (!isPopupOpen()) return;
        }
      } catch { /* 忽略无效选择器 */ }
    }

    try {
      await sendToBackground(MSG.DISPATCH_ESC);
      await randomDelay(80, 150);
    } catch (err) {
      console.warn('[灵感爆爆爆] 关闭弹窗失败', err);
    }
  }

  _getContainerSelector(mode) {
    switch (mode) {
      case COLLECT_MODE.SEARCH:
        return '.feeds-container';
      case COLLECT_MODE.PROFILE:
        return '#userPostedFeeds';
      case COLLECT_MODE.FAVORITE:
        return '.feeds-container:not(#userPostedFeeds)';
      default:
        return '.feeds-container';
    }
  }

  async _cleanupAfterLoop() {
    this.captchaWatcher?.disconnect();
    this._cleanupCollectingMarks();
    this._cleanupTopSelectionMarks();
    await this._closeNotePopup().catch(() => {});
    await sendToBackground(MSG.UNBLOCK_MEDIA).catch(() => {});
    this.isRunning = false;
    this.isPaused = false;
    if (this._pauseResolve) {
      this._pauseResolve();
      this._pauseResolve = null;
    }
  }

  async _markRunFailed(error) {
    if (!this.collectionRunId) return;
    await collectionRunStore.markFailed(this.collectionRunId, error, {
      ...buildXhsBatchNotesRunPatch({
        noteList: this.noteList,
        collected: this.collected,
        failed: this.failed,
        commentResults: this.commentResults,
      }),
      error: String(error?.message || error),
    }).catch(() => {});
  }
}
