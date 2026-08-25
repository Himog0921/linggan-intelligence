import { collectComments } from './commentCollector.js';
import { discoverWithScroll } from './noteCollector.js';
import { watchCaptcha, showCaptchaPauseOverlay } from './antiDetect.js';
import { sendToBackground, reportProgress, reportDone } from '../../shared/messaging.js';
import { BATCH_CONFIG, COLLECT_MODE, COMMENT_DEPTH_MODE, MSG, TASK_STATE } from '../../shared/constants.js';
import { randomDelay, parseCount } from '../../shared/utils.js';
import { collectionRunStore } from '../../db/collectionRunStore.js';
import {
  createCollectionRunHeartbeatReporter,
  createCollectionRunHeartbeatLoop,
} from '../../workbench/runtime/heartbeat.js';
import {
  buildRemoteRunCreatePayload,
  buildXhsBatchCommentsProgressPatch,
  buildXhsBatchCommentsRunPatch,
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
  findNoteElementById,
  formatCompactCount,
  getActiveCommentsContext,
  shouldWaitForNoteState,
  waitForNoteState,
} from './batchShared.js';
import { BaseBatchController } from '../../shared/baseBatchController.js';

const REMOTE_CAPTCHA_ACTION_TIMEOUT_MS = 45_000;
const REMOTE_NOTE_COLLECTION_TIMEOUT_MS = 120_000;
const CAPTCHA_TIMEOUT_ERROR_MESSAGE = '检测到小红书安全验证，等待人工处理超时，已停止本次评论采集。';
const NOTE_COLLECTION_TIMEOUT_ERROR_MESSAGE = '单篇评论采集长时间没有结束，已停止本篇并释放任务。';

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

function hydrateXhsCommentResumeState(runRecord = {}, completedTargetIds = []) {
  const completedSet = new Set((Array.isArray(completedTargetIds) ? completedTargetIds : [])
    .map((value) => String(value || '').trim().replace(/^xhs_/, ''))
    .filter(Boolean));
  const statuses = Array.isArray(runRecord?.resumeCheckpoint?.resultStatuses)
    ? runRecord.resumeCheckpoint.resultStatuses
    : [];
  const results = statuses
    .map((item) => ({
      noteId: String(item?.targetId || item?.noteId || '').trim().replace(/^xhs_/, ''),
      total: Math.max(0, Number(item?.totalComments || 0) || 0),
      error: String(item?.error || '').trim(),
    }))
    .filter((item) => item.noteId && completedSet.has(item.noteId));
  const total = results.reduce((sum, item) => sum + Number(item.total || 0), 0);
  return { results, total };
}

export class BatchCommentController extends BaseBatchController {
  constructor() {
    super();
    this.type = 'batchComments';
    this.results = [];
    this.captchaWatcher = null;
    this._containerSelector = '.feeds-container';
    this._mode = COLLECT_MODE.SEARCH;
    this._originUrl = '';
    this._topByLikes = false;
    this._allowNavigationFallback = true;
    this._commentLimit = 0;
    this._commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL;
    this._totalCommentsCollected = 0;
    this._stoppedByUser = false;
    this._captchaActionTimeoutMs = 0;
    this._noteCollectionTimeoutMs = 0;
    this._blockingError = null;
    this._noteTimedOut = false;
    this._searchFilters = normalizeXhsSearchFilters();
    this._searchFilterSnapshot = null;
    this.reportHeartbeat = createCollectionRunHeartbeatReporter({ collectionRunStore });
    this.heartbeatLoop = createCollectionRunHeartbeatLoop({ reporter: this.reportHeartbeat });
  }

  async start(mode, onProgress, settings = {}) {
    this.isRunning = true;
    this.isPaused = false;
    this._stoppedByUser = false;
    this._setState(TASK_STATE.RUNNING, 'init');
    this.currentIndex = 0;
    this.results = [];
    this._totalCommentsCollected = 0;
    this._originUrl = window.location.href;
    this.onStateChange = typeof onProgress === 'function' ? onProgress : null;
    this._mode = mode;
    this._containerSelector = mode === COLLECT_MODE.PROFILE ? '#userPostedFeeds' : '.feeds-container';
    this._topByLikes = Boolean(settings.topByLikes);
    this._allowNavigationFallback = mode !== COLLECT_MODE.PROFILE;
    this._commentLimit = Math.max(0, Number(settings.commentLimit || 0) || 0);
    this._commentDepthMode = String(settings.commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL).trim() || COMMENT_DEPTH_MODE.TWO_LEVEL;
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
    const safeCount = Math.min(Math.max(1, Number(settings.count || 10) || 10), BATCH_CONFIG.maxPerSession);
    const triggerSource = String(settings.triggerSource || 'popup_manual').trim() || 'popup_manual';
    const externalTaskId = String(settings.externalTaskMeta?.externalTaskId || '').trim();
    const isRemoteDispatch = Boolean(externalTaskId);
    this._captchaActionTimeoutMs = Math.max(
      0,
      Number(settings.captchaActionTimeoutMs ?? (isRemoteDispatch ? REMOTE_CAPTCHA_ACTION_TIMEOUT_MS : 0)) || 0,
    );
    this._noteCollectionTimeoutMs = Math.max(
      0,
      Number(settings.noteCollectionTimeoutMs ?? (isRemoteDispatch ? REMOTE_NOTE_COLLECTION_TIMEOUT_MS : 0)) || 0,
    );
    const existingCollectionRunId = String(settings.collectionRunId || '').trim();
    let existingRun = await resolveExistingBatchRun({
      collectionRunId: existingCollectionRunId,
      externalTaskId,
      taskType: this.type,
    });
    const runPayload = existingCollectionRunId || existingRun?.collectionRunId
      ? null
      : buildRemoteRunCreatePayload({
        platform: 'xhs',
        taskType: 'batchComments',
        pageType: mode,
        triggerSource,
        pageUrl: window.location.href,
        config: {
          count: safeCount,
          topByLikes: this._topByLikes,
          commentLimit: this._commentLimit,
          commentDepthMode: this._commentDepthMode,
          searchFilters: mode === COLLECT_MODE.SEARCH ? this._searchFilters : undefined,
          searchFilterSummary: mode === COLLECT_MODE.SEARCH ? searchFilterSummary : undefined,
          searchFilterSnapshot: mode === COLLECT_MODE.SEARCH ? this._searchFilterSnapshot : undefined,
        },
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
    this.heartbeatLoop.stop();
    this.heartbeatLoop.start(this.collectionRunId, () => ({
      taskState: this.state,
      stage: this.isPaused ? 'paused' : 'collecting',
      current: this.currentIndex,
      total: this.noteList.length,
      message: this.isPaused
        ? `批量评论已暂停：第 ${this.currentIndex}/${this.noteList.length} 篇`
        : `批量评论执行中：第 ${this.currentIndex}/${this.noteList.length} 篇`,
    }));

    this.captchaWatcher = watchCaptcha(async () => {
      this.pause();
      const action = await showCaptchaPauseOverlay({ timeoutMs: this._captchaActionTimeoutMs });
      if (action === 'resume') {
        this.resume();
      } else {
        this._blockingError = new Error(action === 'timeout' ? CAPTCHA_TIMEOUT_ERROR_MESSAGE : '用户停止了安全验证后的评论采集。');
        this.stop();
      }
    });

    let noteList = Array.isArray(settings.noteList) ? settings.noteList.slice() : [];
    if (noteList.length === 0) {
      this._emitProgress({
        status: 'discovering',
        total: 0,
        current: 0,
        message: '正在扫描页面作品并准备评论任务...',
      });
      noteList = await discoverWithScroll(this._containerSelector, mode === COLLECT_MODE.PROFILE ? 14 : 10, {
        expectedCount: safeCount,
      });
    }

    if (this._topByLikes) {
      noteList = noteList
        .map((note) => ({ ...note, __likesParsed: parseCount(note.likes) }))
        .sort((a, b) => {
          const likesDiff = (b.__likesParsed || 0) - (a.__likesParsed || 0);
          if (likesDiff !== 0) return likesDiff;
          return Number(a._top || 0) - Number(b._top || 0);
        })
        .slice(0, safeCount)
        .map((note, idx) => ({ ...note, __topRank: idx + 1 }))
        .sort((a, b) => Number(a._top || 0) - Number(b._top || 0));
    } else {
      noteList = noteList.slice(0, safeCount);
    }

    this.noteList = noteList;
    const resumeState = resolveBatchResumeState({
      runRecord: existingRun,
      targets: this.noteList,
      getTargetId: (item) => item.noteId,
    });
    this.noteList = resumeState.targets;
    if (resumeState.resumed) {
      this.currentIndex = resumeState.nextIndex;
      const hydrated = hydrateXhsCommentResumeState(existingRun, resumeState.completedTargetIds);
      this.results = hydrated.results;
      this._totalCommentsCollected = hydrated.total;
      this._emitProgress({
        status: 'resuming',
        total: this.noteList.length,
        current: this.currentIndex,
        message: `已从本地记录恢复，前 ${this.currentIndex}/${this.noteList.length} 篇评论不重复采集`,
      });
    }

    this._emitProgress({
      status: 'started',
      total: this.noteList.length,
      current: 0,
      message: this._topByLikes
        ? `发现 ${this.noteList.length} 篇作品，按点赞 Top ${this.noteList.length} 依次进入详情页采集评论`
        : `发现 ${this.noteList.length} 篇作品，按当前顺位依次进入详情页采集评论${mode === COLLECT_MODE.SEARCH && hasExplicitXhsSearchFilters(this._searchFilters) ? `（页面筛选：${searchFilterSummary}）` : ''}`,
    });

    try {
      await this._collectLoop();
    } catch (error) {
      await this._cleanupAfterLoop();
      await this._markRunFailed(error);
      throw error;
    }
  }

  stop() {
    this.heartbeatLoop.stop();
    super.stop();
  }

  async _collectLoop() {
    while (this.currentIndex < this.noteList.length && this.isRunning) {
      await this._waitIfPaused();
      this._throwIfBlockingError();
      if (!this.isRunning) break;
      if (await this._pauseForRiskControl()) continue;

      const noteInfo = this.noteList[this.currentIndex];
      this.currentIndex += 1;
      this._noteTimedOut = false;
      const rankHint = this._topByLikes
        ? `｜Top ${noteInfo.__topRank || this.currentIndex}/${this.noteList.length}（赞 ${formatCompactCount(noteInfo.__likesParsed ?? parseCount(noteInfo.likes))}）`
        : '';

      this._emitProgress({
        status: 'collecting',
        total: this.noteList.length,
        current: this.currentIndex,
        message: `正在采集第 ${this.currentIndex}/${this.noteList.length} 篇评论${rankHint}`,
      });
      reportProgress(this.currentIndex, this.noteList.length, '评论采集中', {
        taskType: this.type,
        taskState: this.state,
        phase: 'collect',
      });
      void this.reportHeartbeat.report(this.collectionRunId, {
        taskState: this.state,
        stage: 'collecting',
        current: this.currentIndex,
        total: this.noteList.length,
        message: `正在采集第 ${this.currentIndex}/${this.noteList.length} 篇评论`,
      }).catch(() => {});

      try {
        await this._captureNoteWithTimeout(noteInfo);
      } catch (err) {
        console.warn(`[灵感爆爆爆] 评论采集失败: ${noteInfo.noteId}`, err);
        if (this._blockingError) throw this._blockingError;
        await this._closeNotePopup();
      } finally {
        await this._syncRunProgress();
      }

      await randomDelay(1200, 2200);
    }

    await this._cleanupAfterLoop();
    this._setState(TASK_STATE.DONE, this._stoppedByUser ? 'stopped' : 'done');
    const runPatch = buildXhsBatchCommentsRunPatch({
      noteList: this.noteList,
      results: this.results,
    });
    if (this._stoppedByUser) {
      await this._finalizeCollectionRun('stopped', runPatch);
    } else if (this.collectionRunId) {
      await this._finalizeCollectionRun('done', runPatch);
    }
    this._emitProgress({
      status: 'done',
      total: this.noteList.length,
      current: this._totalCommentsCollected,
      message: this._stoppedByUser
        ? `批量评论已停止：已采集 ${this._totalCommentsCollected} 条评论`
        : `批量评论完成：共采集 ${this._totalCommentsCollected} 条评论`,
    });
    reportDone('batchComments', this._totalCommentsCollected, {
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

  _throwIfBlockingError() {
    if (this._blockingError) {
      throw this._blockingError;
    }
  }

  async _captureNote(noteInfo) {
    if (this._mode === 'detail') {
      const success = await this._captureCurrentDetail(noteInfo);
      if (success) return true;
      return this._captureByNavigation(noteInfo);
    }

    const success = await this._captureByPopup(noteInfo);
    if (!success && this._allowNavigationFallback) {
      return this._captureByNavigation(noteInfo);
    }
    return success;
  }

  async _captureNoteWithTimeout(noteInfo) {
    const timeoutMs = this._noteCollectionTimeoutMs;
    if (!(timeoutMs > 0)) return this._captureNote(noteInfo);

    let timer = null;
    try {
      return await Promise.race([
        this._captureNote(noteInfo),
        new Promise((_, reject) => {
          timer = window.setTimeout(() => {
            this._noteTimedOut = true;
            reject(new Error(NOTE_COLLECTION_TIMEOUT_ERROR_MESSAGE));
          }, timeoutMs);
        }),
      ]);
    } finally {
      if (timer) window.clearTimeout(timer);
    }
  }

  async _captureCurrentDetail(noteInfo) {
    if (!this._isNoteDetailPath(window.location.pathname, noteInfo.noteId)) {
      return false;
    }

    if (shouldWaitForNoteState(window.location.pathname, noteInfo.noteId)) {
      await waitForNoteState(noteInfo.noteId, 10000);
    }
    const commentsReady = await this._waitForCommentPipelineReady(noteInfo.noteId, 12000);
    if (!commentsReady) return false;

    await randomDelay(120, 220);

    const result = await this._collectOneNoteComments(noteInfo, noteInfo.url || window.location.href);
    this._totalCommentsCollected += Number(result?.total || 0);
    this.results.push({ noteId: noteInfo.noteId, ...result });
    return true;
  }

  async _captureByPopup(noteInfo) {
    const urlBefore = window.location.href;
    const element = await this._scrollToAndFindNote(noteInfo);
    const coverLink = element?.querySelector('a.cover');
    if (!coverLink) return false;

    coverLink.click();
    const pageReady = await this._waitForNoteLoad(noteInfo.noteId, 12000);
    if (!pageReady) {
      await this._goBackToList(urlBefore);
      return false;
    }

    if (shouldWaitForNoteState(window.location.pathname, noteInfo.noteId)) {
      await waitForNoteState(noteInfo.noteId, 10000);
    }
    const commentsReady = await this._waitForCommentPipelineReady(noteInfo.noteId, 12000);
    if (!commentsReady) {
      await this._goBackToList(urlBefore);
      await this._closeNotePopup();
      return false;
    }

    await randomDelay(120, 220);

    const result = await this._collectOneNoteComments(noteInfo, noteInfo.url);
    this._totalCommentsCollected += Number(result?.total || 0);
    this.results.push({ noteId: noteInfo.noteId, ...result });
    await this._goBackToList(urlBefore);
    return true;
  }

  async _captureByNavigation(noteInfo) {
    const urlBefore = window.location.href;
    const noteUrl = noteInfo.url || '/explore/' + noteInfo.noteId;
    const fullUrl = noteUrl.startsWith('http') ? noteUrl : 'https://www.xiaohongshu.com' + noteUrl;

    window.location.href = fullUrl;
    await this._waitForPageLoad(15000);
    if (shouldWaitForNoteState(window.location.pathname, noteInfo.noteId)) {
      await waitForNoteState(noteInfo.noteId, 10000);
    }
    const ready = await this._waitForCommentPipelineReady(noteInfo.noteId, 12000);
    if (!ready) {
      await this._goBackToList(urlBefore);
      return false;
    }

    await randomDelay(120, 220);

    const result = await this._collectOneNoteComments(noteInfo, fullUrl);
    this._totalCommentsCollected += Number(result?.total || 0);
    this.results.push({ noteId: noteInfo.noteId, ...result });
    await this._goBackToList(urlBefore);
    await randomDelay(180, 280);
    return true;
  }

  async _cleanupAfterLoop() {
    this.heartbeatLoop.stop();
    this.captchaWatcher?.disconnect();
    await this._closeNotePopup();
    await sendToBackground(MSG.UNBLOCK_MEDIA).catch(() => {});
    this.isRunning = false;
    this.isPaused = false;
    if (this._pauseResolve) {
      this._pauseResolve();
      this._pauseResolve = null;
    }
  }

  async _pauseForRiskControl() {
    if (!isRiskControlPage()) return false;
    this.pause();
    this._emitProgress({
      status: TASK_STATE.PAUSED,
      total: this.noteList.length,
      current: this.currentIndex,
      message: '检测到安全验证页面，已自动暂停，请完成验证后点击“继续采集”。',
    });
    return true;
  }

  _emitProgress(payload) {
    this.onStateChange?.({
      taskType: this.type,
      taskState: this.state,
      phase: payload.status || 'running',
      ...payload,
    });
  }

  _reportHeartbeat(message = '', patch = {}) {
    void this.reportHeartbeat.report(this.collectionRunId, {
      taskState: this.state,
      stage: patch.stage || 'collecting',
      current: Number(patch.current ?? this.currentIndex ?? 0),
      total: Number(patch.total ?? this.noteList.length ?? 0),
      message: String(message || '').trim(),
      force: Boolean(patch.force),
    }).catch(() => {});
  }

  async _syncRunProgress() {
    if (!this.collectionRunId) return;
    const runPatch = buildXhsBatchCommentsProgressPatch({
      noteList: this.noteList,
      results: this.results,
      processedCount: this.currentIndex,
    });
    await collectionRunStore.updateById(this.collectionRunId, runPatch).catch(() => {});
  }

  _buildPartialRunPatch() {
    return buildXhsBatchCommentsProgressPatch({
      noteList: this.noteList,
      results: this.results,
      processedCount: this.results.length,
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

  async _collectOneNoteComments(noteInfo, noteUrl) {
    const makeOptions = () => ({
      noteId: noteInfo.noteId,
      noteUrl,
      maxTotal: this._commentLimit,
      maxSubComments: this._commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : BATCH_CONFIG.maxSubComments,
      commentDepthMode: this._commentDepthMode,
      shouldStop: () => !this.isRunning || this._noteTimedOut,
      waitIfPaused: () => this._waitIfPaused(),
      collectionRunId: this.collectionRunId,
      captchaActionTimeoutMs: this._captchaActionTimeoutMs,
      onProgress: (progress) => {
        this._reportHeartbeat(progress?.message || `第 ${this.currentIndex}/${this.noteList.length} 篇评论采集中`, {
          stage: 'collecting',
          current: this.currentIndex,
          total: this.noteList.length,
        });
      },
    });

    let result = await collectComments(makeOptions());
    if (Number(result?.total || 0) > 0) return result;

    if (this._hasExplicitNoCommentsState()) {
      return result;
    }

    let commentHint = this._readVisibleCommentHint();
    if (commentHint > 0 || this._hasVisibleCommentItems()) {
      await this._waitForCommentPipelineReady(noteInfo.noteId, 3200);
      await randomDelay(260, 420);
      result = await collectComments(makeOptions());
      if (Number(result?.total || 0) > 0 || this._hasExplicitNoCommentsState()) return result;
    }

    await this._waitForCommentPipelineReady(noteInfo.noteId, 4200);
    commentHint = this._readVisibleCommentHint();
    if (commentHint > 0 || this._hasVisibleCommentItems() || !this._hasExplicitNoCommentsState()) {
      await randomDelay(320, 520);
      result = await collectComments(makeOptions());
    }

    return result;
  }

  async _waitForCommentPipelineReady(noteId, timeout = 12000) {
    const startedAt = Date.now();
    let stableRounds = 0;
    while (Date.now() - startedAt < timeout) {
      this._throwIfBlockingError();
      if (this._noteTimedOut) return false;
      if (await this._pauseForRiskControl()) return;
      const pathname = window.location.pathname || '';
      const inDetail = this._isNoteDetailPath(pathname, noteId) || isPopupOpen() || isNoteDetailReady();
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

      this._reportHeartbeat(`正在等待第 ${this.currentIndex}/${this.noteList.length} 篇评论上下文稳定`, {
        stage: 'context_check',
        current: this.currentIndex,
        total: this.noteList.length,
      });
      await randomDelay(140, 220);
    }
    return false;
  }

  async _settleAfterDetailReady() {
    const startedAt = Date.now();
    while (Date.now() - startedAt < 700) {
      const context = getActiveCommentsContext();
      const hasContainer = Boolean(context.container);
      const hasCommentItems = context.hasCommentItems;
      const hasCommentMeta = context.hasCommentMeta || context.hasExplicitEmptyState;
      if (hasContainer || hasCommentItems || hasCommentMeta) return;
      await randomDelay(70, 120);
    }
    await randomDelay(160, 260);
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

  async _waitForNoteLoad(noteId, timeout = 15000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      this._throwIfBlockingError();
      if (this._noteTimedOut) return false;
      if (await this._pauseForRiskControl()) return false;
      this._reportHeartbeat(`正在等待第 ${this.currentIndex}/${this.noteList.length} 篇详情页加载`, {
        stage: 'context_check',
        current: this.currentIndex,
        total: this.noteList.length,
      });
      const path = window.location.pathname || '';
      if (this._isNoteDetailPath(path, noteId)) {
        return true;
      }
      if (isPopupOpen() || isNoteDetailReady()) {
        return true;
      }
      await randomDelay(240, 340);
    }
    return false;
  }

  async _waitForCommentsReady(noteId, timeout = 12000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      this._throwIfBlockingError();
      if (this._noteTimedOut) return false;
      if (await this._pauseForRiskControl()) return false;
      this._reportHeartbeat(`正在等待第 ${this.currentIndex}/${this.noteList.length} 篇评论区加载`, {
        stage: 'context_check',
        current: this.currentIndex,
        total: this.noteList.length,
      });
      const pathname = window.location.pathname || '';
      const inDetail = this._isNoteDetailPath(pathname, noteId);
      const context = getActiveCommentsContext();
      const hasContainer = Boolean(context.container);
      const hasCommentItems = context.hasCommentItems;
      const hasCommentMeta = context.hasCommentMeta || context.hasExplicitEmptyState;
      if (hasContainer && ((inDetail && (hasCommentItems || hasCommentMeta)) || hasCommentItems || hasCommentMeta)) {
        return true;
      }
      await randomDelay(220, 320);
    }
    return false;
  }

  async _scrollToAndFindNote(noteInfo) {
    this._reportHeartbeat(`正在定位第 ${this.currentIndex}/${this.noteList.length} 篇卡片`, {
      stage: 'discovering',
      current: this.currentIndex,
      total: this.noteList.length,
    });
    let element = findNoteElementById(noteInfo.noteId, this._containerSelector);
    if (element && document.body.contains(element)) {
      element.scrollIntoView({ behavior: 'auto', block: 'center' });
      await randomDelay(120, 220);
      return element;
    }

    const targetY = Math.max(Math.round((noteInfo._top || 0) - window.innerHeight * 0.8), 0);
    if (Math.abs(window.scrollY - targetY) > window.innerHeight) {
      window.scrollTo({ top: targetY, behavior: 'auto' });
      await randomDelay(120, 220);
    }

    const maxScrollAttempts = 18;
    const scrollStep = Math.round(window.innerHeight * 0.52);
    for (let i = 0; i < maxScrollAttempts; i += 1) {
      element = findNoteElementById(noteInfo.noteId, this._containerSelector);
      if (element && document.body.contains(element)) {
        element.scrollIntoView({ behavior: 'auto', block: 'center' });
        await randomDelay(120, 220);
        return element;
      }
      window.scrollBy({ top: scrollStep, behavior: 'auto' });
      await randomDelay(140, 240);
      if (window.scrollY > targetY + window.innerHeight * 3) break;
    }

    element = findNoteElementById(noteInfo.noteId, this._containerSelector);
    return element && document.body.contains(element) ? element : null;
  }

  async _goBackToList(originalUrl) {
    const currentUrl = window.location.href;
    if (currentUrl !== originalUrl) {
      window.history.back();
      for (let i = 0; i < 30; i += 1) {
        await randomDelay(180, 260);
        if (!this._isNoteDetailPath(window.location.pathname)) {
          await randomDelay(160, 260);
          return;
        }
      }
      window.location.href = originalUrl;
      await randomDelay(1400, 2000);
    } else {
      await this._closeNotePopup();
    }
  }

  _isNoteDetailPath(pathname = window.location.pathname, noteId = '') {
    const path = String(pathname || '');
    const id = String(noteId || '').trim();
    if (/\/explore\/[a-z0-9]+/i.test(path) || /\/discovery\/item\/[a-z0-9]+/i.test(path) || /\/search_result\/[a-z0-9]+/i.test(path)) {
      return !id || path.includes(id);
    }
    return false;
  }


  _readVisibleCommentHint() {
    const context = getActiveCommentsContext();
    const text = String(context.text || document.body?.innerText || '').slice(0, 3000);
    const match = text.match(/共\s*(\d+)\s*条评论/);
    return match ? Number(match[1] || 0) : 0;
  }

  _hasVisibleCommentItems() {
    return Boolean(getActiveCommentsContext().hasCommentItems);
  }

  _hasExplicitNoCommentsState() {
    return Boolean(getActiveCommentsContext().hasExplicitEmptyState);
  }

  async _closeNotePopup() {
    for (const sel of CLOSE_SELECTORS) {
      try {
        const el = document.querySelector(sel);
        if (el && el.offsetWidth > 0 && el.offsetHeight > 0) {
          el.click();
          await randomDelay(120, 220);
          if (!isPopupOpen()) return;
        }
      } catch {
        // ignore selector failures
      }
    }

    try {
      await sendToBackground(MSG.DISPATCH_ESC);
      await randomDelay(120, 220);
    } catch {
      // ignore
    }
  }

  async _markRunFailed(error) {
    this.heartbeatLoop.stop();
    if (!this.collectionRunId) return;
    await collectionRunStore.markFailed(this.collectionRunId, error, {
      ...buildXhsBatchCommentsRunPatch({
        noteList: this.noteList,
        results: this.results,
      }),
      error: String(error?.message || error),
    }).catch(() => {});
  }
}
