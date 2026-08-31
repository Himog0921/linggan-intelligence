import { parseCount, toHighQualityImageUrl, getHighQualityImageCandidates } from '../../shared/utils.js';
import { commentStore } from '../../db/commentStore.js';
import { BATCH_CONFIG, COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import { reportProgress } from '../../shared/messaging.js';
import { detectCaptcha, waitForPageSettle } from './antiDetect.js';
import { getActiveCommentsContext, isRiskControlPage } from './batchShared.js';
import { createCollectorEvidence, createCollectorQualityMeta, joinRawDomText } from '../../shared/collectorMetadata.js';
import { emitCollectorReceipt } from '../../runtime/collectorReceiptSink.js';
import { buildXhsCommentCollectionReceipt } from './captureReceipt.js';
import {
  buildXhsCommentsFromSnapshot,
  requestXhsCommentSnapshot,
  startFreshXhsCommentSnapshot,
  hydrateXhsCommentSnapshot,
  mergeXhsCommentSnapshots,
  fetchXhsJsonViaBridge,
} from './commentApi.js';

const DEFAULT_DOM_TOP_UP_MAX_NO_NEW = 3;
const DEFAULT_DOM_TOP_UP_SETTLE_MS = 2600;
const DEFAULT_COMMENT_ACTION_COOLDOWN_MS = 1200;
const DEFAULT_COMMENT_SCROLL_DISTANCE = 280;
// CSS layout and device scaling can leave an element a fraction of a pixel beyond the
// scroll-parent edge even after scrollIntoView({ block: 'nearest' }). Treating that as
// offscreen makes the collector reveal the same control forever without ever clicking it.
const REPLY_CONTROL_VISIBILITY_EPSILON_PX = 1;
const TIME_TEXT_RE = /^(刚刚|\d+\s*分钟前|\d+\s*小时前|\d+\s*天前|昨天(?:\s+\d{1,2}:\d{2})?|前天(?:\s+\d{1,2}:\d{2})?|\d{1,2}[-/]\d{1,2}(?:\s+\d{1,2}:\d{2})?|\d{4}[-/年]\d{1,2}(?:[-/月]\d{1,2})?(?:日)?(?:\s+\d{1,2}:\d{2})?)$/;
const INLINE_TIME_TEXT_RE = /(刚刚|\d+\s*分钟前|\d+\s*小时前|\d+\s*天前|昨天\s*\d{0,2}:?\d{0,2}|前天\s*\d{0,2}:?\d{0,2}|\d{1,2}[-/]\d{1,2}(?:\s+\d{1,2}:\d{2})?|\d{4}[-/年]\d{1,2}(?:[-/月]\d{1,2})?(?:日)?(?:\s+\d{1,2}:\d{2})?)/;

function hasXhsCollectionRiskSignal() {
  return detectCaptcha() || isRiskControlPage();
}

async function waitForCondition(predicate, timeout = 500, interval = 80) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeout) {
    if (predicate()) return true;
    await waitForPageSettle(interval);
  }
  return Boolean(predicate());
}

export function createCommentActionGate({
  minimumCooldownMs = DEFAULT_COMMENT_ACTION_COOLDOWN_MS,
  sleep = waitForPageSettle,
  now = () => Date.now(),
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  let lastActionStartedAt = 0;
  return {
    async before() {
      await waitIfPaused();
      if (shouldStop()) return false;
      const remaining = lastActionStartedAt > 0
        ? Math.max(0, Number(minimumCooldownMs || 0) - (now() - lastActionStartedAt))
        : 0;
      if (remaining > 0) await sleep(remaining);
      await waitIfPaused();
      if (shouldStop()) return false;
      lastActionStartedAt = now();
      return true;
    },
    async after() {
      await waitIfPaused();
      return !shouldStop();
    },
  };
}

function withPageCommentCount(payload = {}, container = null) {
  const context = getActiveCommentsContext();
  const signals = readCommentSignalsSafe(context?.container || container);
  const pageCommentCount = context?.publicCommentCount ?? (signals.commentHint || null);
  return {
    ...payload,
    ...(pageCommentCount == null ? {} : { pageCommentCount }),
  };
}

function readCommentSignals(container) {
  const textSource = container?.innerText || document.body?.innerText || '';
  const text = String(textSource || '').slice(0, 6000);
  const match = text.match(/共\s*([\d,.]+(?:万|亿)?)\s*条评论/);
  const commentHint = match ? Math.floor(parseCount(String(match[1] || '').replace(/,/g, ''))) : 0;
  const hasEndMarker = /- THE END -/.test(text);
  const hasExpandableReplies = [...(container?.querySelectorAll?.('div.show-more') || [])]
    .some((el) => isExpandMoreReplyTrigger(el?.textContent));
  return {
    commentHint,
    hasEndMarker,
    hasExpandableReplies,
  };
}

function readCommentSignalsSafe(container) {
  if (!container) {
    return {
      commentHint: 0,
      hasEndMarker: false,
      hasExpandableReplies: false,
    };
  }
  return readCommentSignals(container);
}

function getCommentSurfaceFingerprint(container) {
  const items = [...(container?.querySelectorAll?.('.parent-comment, .comment-item') || [])];
  const ids = items.slice(0, 80).map((item, index) => String(item?.dataset?.commentId || item?.dataset?.id || item?.id || index)).join('|');
  const signals = readCommentSignalsSafe(container);
  return `${items.length}:${ids}:${signals.hasEndMarker ? 'end' : 'open'}`;
}

async function loadMoreCommentSurface(container, distance = DEFAULT_COMMENT_SCROLL_DISTANCE, {
  actionGate = createCommentActionGate(),
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const scrollParent = findScrollParent(container);
  if (!scrollParent || typeof scrollParent.scrollBy !== 'function') return false;
  await waitIfPaused();
  if (shouldStop() || !await actionGate.before({ kind: 'dom_scroll' })) return false;
  const before = getCommentSurfaceFingerprint(container);
  const beforeScrollTop = Number(scrollParent.scrollTop || 0);
  const beforeScrollHeight = Number(scrollParent.scrollHeight || 0);
  scrollParent.scrollBy({ top: distance, behavior: 'auto' });
  const changed = await waitForCondition(() => {
    if (hasXhsCollectionRiskSignal()) return true;
    const current = getActiveCommentsContext().container || container;
    const signals = readCommentSignalsSafe(current);
    const positionAdvanced = Number(scrollParent.scrollTop || 0) > beforeScrollTop;
    const surfaceExpanded = Number(scrollParent.scrollHeight || 0) > beforeScrollHeight;
    return signals.hasEndMarker
      || getCommentSurfaceFingerprint(current) !== before
      || positionAdvanced
      || surfaceExpanded;
  }, DEFAULT_DOM_TOP_UP_SETTLE_MS, 140);
  await actionGate.after({ kind: 'dom_scroll', changed });
  await waitIfPaused();
  return changed;
}

export async function rewindCommentSurface(container, {
  actionGate = createCommentActionGate(),
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const scrollParent = findScrollParent(container);
  if (!scrollParent || Number(scrollParent.scrollTop || 0) <= 0) return false;
  await waitIfPaused();
  if (shouldStop() || !await actionGate.before({ kind: 'dom_rewind' })) return false;
  if (typeof scrollParent.scrollTo === 'function') {
    scrollParent.scrollTo({ top: 0, behavior: 'auto' });
  } else {
    scrollParent.scrollTop = 0;
  }
  const changed = await waitForCondition(
    () => Number(scrollParent.scrollTop || 0) <= 1,
    DEFAULT_DOM_TOP_UP_SETTLE_MS,
    140,
  );
  await actionGate.after({ kind: 'dom_rewind', changed });
  await waitIfPaused();
  return changed;
}

/**
 * 采集单篇笔记的所有评论（含子评论）
 * 技术路径：DOM 解析 + 自动滚动加载 + 子评论展开
 *
 * @param {Object} options
 * @param {string} options.noteId - 笔记 ID
 * @param {string} options.noteUrl - 笔记 URL
 * @param {number} options.maxTotal - 最多采集总评论数（0=不限）
 * @param {number} options.maxSubComments - 每条主评论最多采集的子评论数（0=不限）
 * @param {Function} options.onProgress - 进度回调
 * @param {Function} options.shouldStop - 返回 true 时停止采集
 * @param {string} options.commentDepthMode - 评论深度：twoLevel / allReplies
 * @param {string} options.collectionRunId - 采集批次 ID
 */
export async function collectComments({
  noteId = '',
  noteUrl = '',
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  onProgress = null,
  onSnapshot = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
  commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  collectionRunId = '',
  persist = true,
  emitReceipt = true,
  taskSpec = undefined,
  publicCommentCount = null,
  observedNoteId = '',
  executionPolicy = {},
} = {}) {
  const actionGate = createCommentActionGate({
    minimumCooldownMs: executionPolicy.minimumCooldownMs ?? DEFAULT_COMMENT_ACTION_COOLDOWN_MS,
    shouldStop,
    waitIfPaused,
  });
  const apiResult = await collectCommentsViaApi({
    noteId,
    noteUrl,
    maxTotal,
    maxSubComments,
    onProgress,
    onSnapshot,
    shouldStop,
    waitIfPaused,
    commentDepthMode,
    collectionRunId,
    persist,
    actionGate,
    publicCommentCount,
  });
  if (apiResult.stopReason === 'risk_control' || (!apiResult.needsDomContinuation && (apiResult.apiObserved || apiResult.total > 0))) {
    const result = withCommentCollectionReceipt({
      total: apiResult.total,
      comments: apiResult.comments,
      stopReason: apiResult.stopReason,
    }, { noteId, maxTotal, publicCommentCount, observedNoteId });
    if (emitReceipt) {
      result.lingganDelivery = await emitCollectorReceipt('comments', result, { platform: 'xhs', noteId, options: { maxTotal, maxSubComments, commentDepthMode, taskSpec } });
    }
    return result;
  }

  if (apiResult.needsDomContinuation) {
    onProgress?.({
      status: 'collecting',
      current: apiResult.total || 0,
      message: `页面 API 未完整覆盖更多回复，继续展开页面中的回复链路（当前 ${apiResult.total || 0} 条）`,
    });
  }

  const result = await collectCommentsFromDom({
    noteId,
    noteUrl,
    maxTotal,
    maxSubComments,
    onProgress,
    onSnapshot,
    shouldStop,
    waitIfPaused,
    commentDepthMode,
    collectionRunId,
    initialComments: apiResult.comments,
    persist,
    actionGate,
  });
  const normalizedResult = withCommentCollectionReceipt(result, {
    noteId,
    maxTotal,
    publicCommentCount,
    observedNoteId,
  });
  if (emitReceipt) {
    normalizedResult.lingganDelivery = await emitCollectorReceipt('comments', normalizedResult, { platform: 'xhs', noteId, options: { maxTotal, maxSubComments, commentDepthMode, taskSpec } });
  }
  return normalizedResult;
}

function strictNoteIdFromLocation(value = '') {
  const match = String(value || '').match(/\/(?:explore|discovery\/item|search_result)\/([a-z0-9]+)/i)
    || String(value || '').match(/\/user\/profile\/[a-z0-9]+\/([a-z0-9]+)/i);
  return String(match?.[1] || '').trim();
}

export function resolveXhsCommentTargetIdentity({
  expectedNoteId = '',
  observedNoteId = '',
  currentUrl = '',
} = {}) {
  const expected = String(expectedNoteId || '').trim();
  const observed = String(observedNoteId || strictNoteIdFromLocation(currentUrl)).trim();
  if (!observed) return 'unverified';
  return observed === expected ? 'matched' : 'mismatched';
}

function withCommentCollectionReceipt(result = {}, {
  noteId = '',
  maxTotal = 0,
  publicCommentCount = null,
  observedNoteId = '',
} = {}) {
  const context = getActiveCommentsContext();
  const total = Number(result?.total ?? (Array.isArray(result?.comments) ? result.comments.length : 0)) || 0;
  const explicitEmptyState = Boolean(context?.hasExplicitEmptyState);
  const stopReason = String(result?.stopReason || '').trim()
    || (explicitEmptyState ? 'explicit_empty_state' : 'collector_returned_partial');
  const targetIdentity = resolveXhsCommentTargetIdentity({
    expectedNoteId: noteId,
    observedNoteId: observedNoteId || result?.observedNoteId,
    currentUrl: window.location?.href,
  });
  const receipt = buildXhsCommentCollectionReceipt({
    noteId,
    maxTotal,
    publicCommentCount: publicCommentCount ?? context?.publicCommentCount,
    actual: total,
    explicitEmptyState,
    stopReason,
    targetIdentity,
  });
  return {
    ...result,
    noteId,
    total,
    explicitEmptyState,
    ordering: 'unknown',
    publicCommentCount: receipt.pageCommentCount,
    collectionReceipt: receipt,
    collectionScope: receipt.scope,
    collectionState: receipt.state,
    analysisUsability: receipt.analysisUsability,
    targetIdentity: receipt.targetIdentity,
    stopReason,
  };
}

function buildCommentSeenId(comment) {
  return String(comment?.commentId || '').trim();
}

export function initializeCollectedComments(initialComments = []) {
  const allComments = [];
  const seenIds = new Set();
  const list = Array.isArray(initialComments) ? initialComments : [];
  list.forEach((comment) => {
    const seenId = buildCommentSeenId(comment);
    if (!seenId || seenIds.has(seenId)) return;
    seenIds.add(seenId);
    allComments.push(comment);
  });
  return { allComments, seenIds };
}

export function shouldContinueDomAfterApi({
  depthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  hydrationDegraded = false,
  hasExpandableReplies = false,
  currentTotal = 0,
  maxTotal = 0,
  commentHint = 0,
  hasDomComments = false,
} = {}) {
  const wantsReplyContinuation = String(depthMode || COMMENT_DEPTH_MODE.TWO_LEVEL).trim() === COMMENT_DEPTH_MODE.ALL_REPLIES
    && Boolean(hydrationDegraded)
    && Boolean(hasExpandableReplies);
  const requestedTotal = Number(maxTotal || 0);
  const observedTotal = Number(currentTotal || 0);
  const visibleTotalHint = Number(commentHint || 0);
  // An unlimited run means “collect the current public set”, not “accept the first API
  // window”. XHS can expose the initial comments without a reusable API cursor; in that
  // case the public page count is the concrete target for DOM continuation.
  const expectedTotal = requestedTotal > 0 ? requestedTotal : visibleTotalHint;
  const needsVisibleTopUp = expectedTotal > 0
    && observedTotal < expectedTotal
    && Boolean(hasDomComments)
    && (!visibleTotalHint || visibleTotalHint > observedTotal || requestedTotal > 0);
  return wantsReplyContinuation || needsVisibleTopUp;
}

export function shouldFallbackToDomAfterFreshApiFailure({
  freshAttemptStarted = false,
  freshAttemptReady = false,
  apiObserved = false,
  currentTotal = 0,
} = {}) {
  return Boolean(freshAttemptStarted)
    && !freshAttemptReady
    && !apiObserved
    && Number(currentTotal || 0) === 0;
}

export function resolveCommentContinuationHint(pageCommentCount = 0, publicCommentCount = null) {
  const pageCount = Number(pageCommentCount || 0);
  if (Number.isFinite(pageCount) && pageCount > 0) return Math.floor(pageCount);
  const suppliedCount = Number(publicCommentCount);
  return Number.isFinite(suppliedCount) && suppliedCount >= 0 ? Math.floor(suppliedCount) : 0;
}

async function collectCommentsViaApi({
  noteId = '',
  noteUrl = '',
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  onProgress = null,
  onSnapshot = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
  commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  collectionRunId = '',
  persist = true,
  actionGate = createCommentActionGate({ shouldStop, waitIfPaused }),
  publicCommentCount = null,
} = {}) {
  noteUrl = noteUrl || window.location.href;
  noteId = noteId || noteUrl.split('/').pop()?.split('?')[0] || '';
  const contentId = noteId ? `xhs_${noteId}` : '';

  const resolveContainer = () => getActiveCommentsContext().container;
  let container = resolveContainer();

  const allComments = [];
  const seenIds = new Set();
  const depthMode = String(commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL).trim() || COMMENT_DEPTH_MODE.TWO_LEVEL;
  let noNewCount = 0;
  const maxNoNew = depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 8 : 4;
  const shouldExpandReplies = depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES;
  let apiObserved = false;
  let hydrationDegradedEver = false;
  let consecutiveHydrationFailures = 0;
  let riskStopped = false;
  let retainedSnapshot = null;
  let freshAttemptStarted = false;
  let freshAttemptReady = false;
  let observedPublicCommentCount = Number.isFinite(Number(publicCommentCount))
    ? Math.max(0, Math.floor(Number(publicCommentCount)))
    : null;
  const publishCountedProgress = (payload) => {
    const enriched = withPageCommentCount(payload, container);
    if (enriched.pageCommentCount != null && Number.isFinite(Number(enriched.pageCommentCount))) {
      observedPublicCommentCount = Math.max(0, Math.floor(Number(enriched.pageCommentCount)));
    }
    onProgress?.(enriched);
    return enriched;
  };

  while (!shouldStop()) {
    container = resolveContainer() || container;
    await waitIfPaused();
    if (shouldStop()) break;
    if (maxTotal > 0 && allComments.length >= maxTotal) {
      onProgress?.({ status: 'done', message: `已达到设定上限 ${maxTotal} 条，停止采集` });
      break;
    }

    publishCountedProgress({
      status: 'collecting',
      current: allComments.length,
      message: `正在扫描评论区，当前已采集 ${allComments.length} 条评论`,
    });

    if (hasXhsCollectionRiskSignal()) {
      riskStopped = true;
      publishCountedProgress({
        status: 'blocked',
        current: allComments.length,
        message: `检测到安全验证或访问受限，已停止本次评论采集（当前 ${allComments.length} 条）`,
      });
      break;
    }

    let foundNew = false;
    let hydrationActionsPerformed = 0;
    let hydrationActionAttempted = false;
    let observedSnapshot = null;
    if (!freshAttemptStarted) {
      freshAttemptStarted = true;
      hydrationActionAttempted = true;
      observedSnapshot = await startFreshXhsCommentSnapshot(noteId, {
        fetchJson: fetchXhsJsonViaBridge,
        beforeExternalAction: async () => {
          if (!await actionGate.before({ kind: 'api_page' })) throw new Error('comment_collection_stopped');
        },
        afterExternalAction: () => actionGate.after({ kind: 'api_page' }),
      }).then((snapshot) => {
        hydrationActionsPerformed = 1;
        freshAttemptReady = true;
        return snapshot;
      }).catch(() => {
        hydrationActionsPerformed = 1;
        freshAttemptReady = false;
        hydrationDegradedEver = true;
        consecutiveHydrationFailures += 1;
        return null;
      });
    } else if (freshAttemptReady) {
      observedSnapshot = await requestXhsCommentSnapshot(noteId).catch(() => null);
    }
    if (observedSnapshot) {
      retainedSnapshot = mergeXhsCommentSnapshots(retainedSnapshot || {}, observedSnapshot);
      let hydrationDegraded = false;
      const hydratedSnapshot = hydrationActionsPerformed > 0 || consecutiveHydrationFailures >= 2
        ? retainedSnapshot
        : await hydrateXhsCommentSnapshot(retainedSnapshot, {
          noteId,
          fetchJson: fetchXhsJsonViaBridge,
          shouldStop,
          waitIfPaused,
          maxExternalActions: 1,
          beforeExternalAction: async () => {
            hydrationActionAttempted = true;
            await actionGate.before({ kind: 'api_page' });
          },
          afterExternalAction: async () => {
            await actionGate.after({ kind: 'api_page' });
          },
        }).then((value) => {
          consecutiveHydrationFailures = 0;
          return value;
        }).catch(() => {
          hydrationDegraded = true;
          hydrationDegradedEver = true;
          consecutiveHydrationFailures += 1;
          return retainedSnapshot;
        });
      hydrationActionsPerformed = Number(hydratedSnapshot?.hydrationMeta?.actionsPerformed || 0);
      retainedSnapshot = mergeXhsCommentSnapshots(retainedSnapshot, hydratedSnapshot);
      apiObserved = apiObserved
        || hydratedSnapshot.pages.length > 0
        || hydratedSnapshot.subPages.length > 0;
      const apiComments = buildXhsCommentsFromSnapshot(hydratedSnapshot, {
        noteId,
        noteUrl,
        url: noteUrl,
        contentId,
      }, {
        maxSubComments: depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : maxSubComments,
        sortMode: 'unknown',
        collectionRunId,
        qualityMeta: hydrationDegraded
          ? {
              dataQuality: 'degraded',
              qualityReason: 'api_snapshot_partial',
              sourceTier: 'api',
            }
          : {},
      });

      for (const comment of apiComments) {
        await waitIfPaused();
        if (shouldStop()) break;
        if (maxTotal > 0 && allComments.length >= maxTotal) break;
        const key = `${comment.commentId}|${comment.parentCommentId || ''}|${comment.level || 1}`;
        if (!comment.commentId || seenIds.has(key)) continue;
        seenIds.add(key);
        allComments.push(comment);
        foundNew = true;
      }
      if (foundNew) {
        const context = getActiveCommentsContext();
        onSnapshot?.(withCommentCollectionReceipt({
          total: allComments.length,
          comments: [...allComments],
          stopReason: 'in_progress',
        }, {
          noteId,
          maxTotal,
          publicCommentCount: context?.publicCommentCount ?? observedPublicCommentCount,
        }));
      }
    }

    if (foundNew) {
      publishCountedProgress({
        status: 'collecting',
        current: allComments.length,
        message: `已通过页面 API 同步 ${allComments.length} 条评论${maxTotal > 0 ? `（上限 ${maxTotal}）` : ''}`,
      });
    }

    // A failed fresh-page request means this Attempt has no API state it can advance.
    // In allReplies mode, staying in this loop would repeatedly click the same visible
    // expand control before any DOM comment is parsed, leaving real page comments at 0.
    // Return to the shared DOM collector immediately; it owns parsing, reply expansion,
    // pacing, pause/stop boundaries and immutable snapshots for this fallback path.
    if (shouldFallbackToDomAfterFreshApiFailure({
      freshAttemptStarted,
      freshAttemptReady,
      apiObserved,
      currentTotal: allComments.length,
    })) {
      publishCountedProgress({
        status: 'collecting',
        current: 0,
        message: '本轮页面 API 未返回评论，切换到页面评论树采集',
      });
      break;
    }

    // The API hydrator is deliberately stepwise: after one network page this loop yields,
    // re-reads state and chooses the next single action instead of racing DOM scrolling.
    if (hydrationActionsPerformed > 0 || hydrationActionAttempted) {
      noNewCount = foundNew ? 0 : noNewCount;
      continue;
    }

    if (shouldExpandReplies && hydrationDegradedEver && container) {
      const expandableParent = [...container.querySelectorAll('.parent-comment')]
        .find((parentEl) => [...parentEl.querySelectorAll('div.show-more')]
          .some((el) => isExpandMoreReplyTrigger(el?.textContent)));
      if (expandableParent) {
        const expanded = await expandNextReply(expandableParent, {
          waitIfPaused,
          shouldStop,
          waitBeforeAction: () => actionGate.before({ kind: 'dom_expand_reply' }),
          waitAfterAction: async ({ beforeCount }) => {
            await waitForCondition(() => {
              const signals = readCommentSignalsSafe(resolveContainer() || container);
              return expandableParent.querySelectorAll('.comment-item.comment-item-sub').length > beforeCount
                || signals.hasEndMarker
                || !signals.hasExpandableReplies;
            }, DEFAULT_DOM_TOP_UP_SETTLE_MS, 140);
            await actionGate.after({ kind: 'dom_expand_reply' });
          },
        });
        if (expanded.acted) continue;
      }
    }

    if (!foundNew) {
      noNewCount++;
      publishCountedProgress({
        status: 'collecting',
        current: allComments.length,
        message: `本轮未同步到新评论，准备继续滚动加载（第 ${noNewCount}/${maxNoNew} 次）`,
      });
      if (noNewCount >= maxNoNew) {
        if (!apiObserved && allComments.length === 0) {
          return { total: 0, comments: [], apiObserved: false, stopReason: 'api_unobserved' };
        }
        break;
      }
    } else {
      noNewCount = 0;
    }

    const nextSignals = readCommentSignalsSafe(resolveContainer() || container);
    if (nextSignals.hasEndMarker && noNewCount > 0) {
      onProgress?.({
        status: 'done',
        current: allComments.length,
        message: `检测到评论区已到底，停止采集（当前 ${allComments.length} 条）`,
      });
      break;
    }

    if (foundNew) {
      continue;
    }

    const nextContainer = resolveContainer() || container;
    if (nextContainer) {
      const surfaceProgressed = await loadMoreCommentSurface(nextContainer, DEFAULT_COMMENT_SCROLL_DISTANCE, {
        actionGate,
        shouldStop,
        waitIfPaused,
      });
      if (surfaceProgressed) noNewCount = 0;
    } else {
      await waitForPageSettle(500);
    }
  }

  if (persist && allComments.length > 0) {
    await commentStore.bulkUpsert(allComments);
  }

  const finalContainer = resolveContainer() || container;
  const finalSignals = readCommentSignalsSafe(finalContainer);
  const finalContext = getActiveCommentsContext();
  const hasDomComments = Boolean(finalContainer?.querySelector?.('.parent-comment, .comment-item'));
  return {
    total: allComments.length,
    comments: allComments,
    apiObserved,
    needsDomContinuation: shouldContinueDomAfterApi({
      depthMode,
      hydrationDegraded: hydrationDegradedEver,
      hasExpandableReplies: finalSignals.hasExpandableReplies,
      currentTotal: allComments.length,
      maxTotal,
      commentHint: resolveCommentContinuationHint(
        finalSignals.commentHint,
        publicCommentCount ?? finalContext?.publicCommentCount ?? observedPublicCommentCount,
      ),
      hasDomComments,
    }),
    stopReason: riskStopped
      ? 'risk_control'
      : (shouldStop()
        ? 'manual_stop'
        : (maxTotal > 0 && allComments.length >= maxTotal
        ? 'comment_cap_reached'
        : (finalSignals.hasEndMarker ? 'comment_area_end' : 'no_progress'))),
  };
}

async function collectCommentsFromDom({
  noteId = '',
  noteUrl = '',
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  onProgress = null,
  onSnapshot = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
  commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  collectionRunId = '',
  initialComments = [],
  persist = true,
  actionGate = createCommentActionGate({ shouldStop, waitIfPaused }),
} = {}) {
  noteUrl = noteUrl || window.location.href;
  noteId = noteId || noteUrl.split('/').pop()?.split('?')[0] || '';
  const contentId = noteId ? `xhs_${noteId}` : '';

  const resolveContainer = () => getActiveCommentsContext().container;
  let container = resolveContainer();
  if (!container) {
    throw new Error('未找到评论区域，请确认当前页面有评论');
  }

  await rewindCommentSurface(container, { actionGate, shouldStop, waitIfPaused });

  const seeded = initializeCollectedComments(initialComments);
  const allComments = seeded.allComments;
  const seenIds = seeded.seenIds;
  const depthMode = String(commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL).trim() || COMMENT_DEPTH_MODE.TWO_LEVEL;
  let noNewCount = 0;
  const maxNoNew = depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 8 : DEFAULT_DOM_TOP_UP_MAX_NO_NEW;
  const shouldExpandReplies = depthMode === COMMENT_DEPTH_MODE.ALL_REPLIES;
  let riskStopped = false;

  while (!shouldStop()) {
    container = resolveContainer() || container;
    if (!container) break;
    await waitIfPaused();
    if (shouldStop()) break;
    if (maxTotal > 0 && allComments.length >= maxTotal) {
      onProgress?.({ status: 'done', message: `已达到设定上限 ${maxTotal} 条，停止采集` });
      break;
    }

    onProgress?.(withPageCommentCount({
      status: 'collecting',
      current: allComments.length,
      message: `正在扫描评论区，当前已采集 ${allComments.length} 条评论`,
    }, container));

    if (hasXhsCollectionRiskSignal()) {
      riskStopped = true;
      onProgress?.(withPageCommentCount({
        status: 'blocked',
        current: allComments.length,
        message: `检测到安全验证或访问受限，已停止本次评论采集（当前 ${allComments.length} 条）`,
      }, container));
      break;
    }

    const parentComments = container.querySelectorAll('.parent-comment');
    let foundNew = false;

    for (const parentEl of parentComments) {
      await waitIfPaused();
      if (shouldStop()) break;
      if (maxTotal > 0 && allComments.length >= maxTotal) break;

      const mainItemEl = parentEl.querySelector(':scope > .comment-item:not(.comment-item-sub)')
        || parentEl.querySelector('.comment-item:not(.comment-item-sub)');
      const mainComment = parseCommentNode(mainItemEl);
      const mainIsNew = Boolean(mainComment?.commentId && !seenIds.has(mainComment.commentId));

      if (mainComment) {
        mainComment.noteId = noteId;
        mainComment.noteUrl = noteUrl;
        mainComment.contentId = contentId;
        mainComment.commentEntityId = `xhs_${noteId}_${mainComment.commentId}`;
        mainComment.platformCommentId = mainComment.commentId;
        mainComment.parentCommentId = '';
        mainComment.rootCommentId = mainComment.commentId;
        mainComment.level = 1;
        mainComment.replyToCommentId = '';
        mainComment.replyToUserName = '';
        mainComment.sortMode = 'unknown';
        mainComment.platform = 'xhs';
        mainComment.collectionRunId = collectionRunId;
        mainComment.dataQuality = mainComment.dataQuality || 'degraded';
        mainComment.qualityReason = mainComment.qualityReason || 'api_unobserved_dom_fallback';
        mainComment.sourceTier = mainComment.sourceTier || 'dom';
      }

      if (mainIsNew) {
        foundNew = true;
        seenIds.add(mainComment.commentId);
        allComments.push(mainComment);
      }

      const subItems = parentEl.querySelectorAll('.comment-item.comment-item-sub');
      let subCount = 0;

      for (const subEl of subItems) {
        await waitIfPaused();
        if (maxSubComments > 0 && subCount >= maxSubComments) break;
        if (maxTotal > 0 && allComments.length >= maxTotal) break;

        const subComment = parseCommentNode(subEl);
        if (!subComment || seenIds.has(subComment.commentId)) continue;

        seenIds.add(subComment.commentId);
        foundNew = true;
        subComment.noteId = noteId;
        subComment.noteUrl = noteUrl;
        subComment.contentId = contentId;
        subComment.commentEntityId = `xhs_${noteId}_${subComment.commentId}`;
        subComment.platformCommentId = subComment.commentId;
        subComment.parentCommentId = mainComment?.commentId || '';
        subComment.rootCommentId = mainComment?.commentId || subComment.commentId;
        subComment.level = 2;
        subComment.replyToCommentId = mainComment?.commentId || '';
        subComment.replyToUserName = subComment.replyToUserName || subComment.replyToNickname || mainComment?.author || '';
        subComment.sortMode = 'unknown';
        subComment.platform = 'xhs';
        subComment.collectionRunId = collectionRunId;
        subComment.dataQuality = subComment.dataQuality || 'degraded';
        subComment.qualityReason = subComment.qualityReason || 'api_unobserved_dom_fallback';
        subComment.sourceTier = subComment.sourceTier || 'dom';

        allComments.push(subComment);
        subCount++;
      }

      if (mainIsNew || subCount > 0) {
        onProgress?.(withPageCommentCount({
          status: 'collecting',
          current: allComments.length,
          message: `已采集 ${allComments.length} 条评论${maxTotal > 0 ? `（上限 ${maxTotal}）` : ''}`,
        }, container));
      }
    }

    if (foundNew) {
      const context = getActiveCommentsContext();
      onSnapshot?.(withCommentCollectionReceipt({
        total: allComments.length,
        comments: [...allComments],
        stopReason: 'in_progress',
      }, {
        noteId,
        maxTotal,
        publicCommentCount: context?.publicCommentCount,
      }));
    }

    if (shouldExpandReplies) {
      const expandableParent = [...container.querySelectorAll('.parent-comment')]
        .find((parentEl) => [...parentEl.querySelectorAll('div.show-more')]
          .some((el) => isExpandMoreReplyTrigger(el?.textContent)));
      if (expandableParent) {
        onProgress?.(withPageCommentCount({
          status: 'collecting',
          current: allComments.length,
          message: `正在按页展开回复（当前已取得 ${allComments.length} 条）`,
        }, container));
        const expanded = await expandNextReply(expandableParent, {
          waitIfPaused,
          shouldStop,
          waitBeforeAction: () => actionGate.before({ kind: 'dom_expand_reply' }),
          waitAfterAction: async ({ beforeCount }) => {
            await waitForCondition(() => {
              const current = resolveContainer() || container;
              return expandableParent.querySelectorAll('.comment-item.comment-item-sub').length > beforeCount
                || !readCommentSignalsSafe(current).hasExpandableReplies;
            }, DEFAULT_DOM_TOP_UP_SETTLE_MS, 140);
            await actionGate.after({ kind: 'dom_expand_reply' });
          },
        });
        if (expanded.acted) {
          noNewCount = foundNew ? 0 : noNewCount;
          continue;
        }
      }
    }

    if (!foundNew) {
      noNewCount++;
      onProgress?.(withPageCommentCount({
        status: 'collecting',
        current: allComments.length,
        message: `本轮未发现新评论，准备继续滚动加载（第 ${noNewCount}/${maxNoNew} 次）`,
      }, container));
      if (noNewCount >= maxNoNew) break;
    } else {
      noNewCount = 0;
    }

    const nextSignals = readCommentSignals(resolveContainer() || container);
    if (nextSignals.hasEndMarker && noNewCount > 0) {
      onProgress?.({
        status: 'done',
        current: allComments.length,
        message: `检测到评论区已到底，停止采集（当前 ${allComments.length} 条）`,
      });
      break;
    }

    if (foundNew) {
      continue;
    }

    const nextContainer = resolveContainer() || container;
    const surfaceProgressed = await loadMoreCommentSurface(nextContainer, DEFAULT_COMMENT_SCROLL_DISTANCE, {
      actionGate,
      shouldStop,
      waitIfPaused,
    });
    if (surfaceProgressed) noNewCount = 0;
  }

  if (persist && allComments.length > 0) {
    await commentStore.bulkUpsert(allComments);
  }

  const finalSignals = readCommentSignalsSafe(resolveContainer() || container);
  return {
    total: allComments.length,
    comments: allComments,
    stopReason: riskStopped
      ? 'risk_control'
      : (shouldStop()
        ? 'manual_stop'
        : (maxTotal > 0 && allComments.length >= maxTotal
        ? 'comment_cap_reached'
        : (finalSignals.hasEndMarker ? 'comment_area_end' : 'no_progress'))),
  };
}

/**
 * 采集评论区所有图片 URL（不含文字，仅图片）
 * @param {Object} options
 * @param {string} options.noteId
 * @param {Function} options.onProgress
 * @param {Function} options.shouldStop
 * @returns {Promise<{total: number, images: string[]}>}
 */
export async function collectCommentImages({
  noteId = '',
  onProgress = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const resolveContainer = () => getActiveCommentsContext().container;
  let container = resolveContainer();
  if (!container) {
    throw new Error('未找到评论区域，请确认当前页面有评论');
  }

  const allImages = [];
  const seenUrls = new Set();
  let noNewCount = 0;
  const maxNoNew = 4;
  let riskStopped = false;

  while (!shouldStop()) {
    container = resolveContainer() || container;
    if (!container) break;
    await waitIfPaused();
    if (shouldStop()) break;
    onProgress?.({
      status: 'collecting',
      current: allImages.length,
      message: `正在扫描评论图片区，当前已发现 ${allImages.length} 张`,
    });
    if (hasXhsCollectionRiskSignal()) {
      riskStopped = true;
      onProgress?.({
        status: 'blocked',
        current: allImages.length,
        message: `检测到安全验证或访问受限，已停止本次评论图片采集（当前 ${allImages.length} 张）`,
      });
      break;
    }

    // 展开所有子评论
    const parentComments = container.querySelectorAll('.parent-comment');
    for (const parentEl of parentComments) {
      await waitIfPaused();
      if (shouldStop()) break;
      await expandAllReplies(parentEl);
    }

    // 查找所有评论中的图片
    const images = container.querySelectorAll('.comment-item img, .comment-item-sub img');
    let foundNew = false;

    for (const img of images) {
      await waitIfPaused();
      if (shouldStop()) break;
      const sources = extractImageSources(img);
      if (sources.length === 0) continue;
      // 排除头像等小图（通常评论图片较大）
      // 头像一般在 a.name 或 .avatar 内
      if (img.closest('a.name') || img.closest('.avatar') || img.closest('.author-wrapper')) continue;
      // 排除 emoji/表情图（通常很小或在特定 class 下）
      if (img.width > 0 && img.width < 30) continue;
      if (img.height > 0 && img.height < 30) continue;
      if (img.closest('[class*="emoji"]') || img.closest('[class*="sticker"]')) continue;

      const candidateSet = new Set();
      sources.forEach((sourceUrl) => {
        getHighQualityImageCandidates(sourceUrl).forEach((candidate) => candidateSet.add(candidate));
      });
      const candidates = Array.from(candidateSet);
      const highQualitySrc = candidates[0] || toHighQualityImageUrl(sources[0]);
      if (!highQualitySrc || seenUrls.has(highQualitySrc)) continue;
      seenUrls.add(highQualitySrc);
      allImages.push({ url: highQualitySrc, candidates, originalUrl: sources[0] });
      foundNew = true;
    }

    // 也查找评论中的图片链接（有些评论图片是点击展开的）
    const commentImgContainers = container.querySelectorAll('.comment-image, .note-image, [class*="comment-img"]');
    for (const imgContainer of commentImgContainers) {
      await waitIfPaused();
      if (shouldStop()) break;
      const imgs = imgContainer.querySelectorAll('img');
      for (const img of imgs) {
        await waitIfPaused();
        if (shouldStop()) break;
        const sources = extractImageSources(img);
        if (sources.length === 0) continue;
        const candidateSet = new Set();
        sources.forEach((sourceUrl) => {
          getHighQualityImageCandidates(sourceUrl).forEach((candidate) => candidateSet.add(candidate));
        });
        const candidates = Array.from(candidateSet);
        const highQualitySrc = candidates[0] || toHighQualityImageUrl(sources[0]);
        if (!highQualitySrc || seenUrls.has(highQualitySrc)) continue;
        seenUrls.add(highQualitySrc);
        allImages.push({ url: highQualitySrc, candidates, originalUrl: sources[0] });
        foundNew = true;
      }
    }

    onProgress?.({
      status: 'collecting',
      current: allImages.length,
      message: `已发现 ${allImages.length} 张评论图片`,
    });

    if (!foundNew) {
      noNewCount++;
      onProgress?.({
        status: 'collecting',
        current: allImages.length,
        message: `本轮未发现新图片，准备继续滚动加载（第 ${noNewCount}/${maxNoNew} 次）`,
      });
      if (noNewCount >= maxNoNew) break;
    } else {
      noNewCount = 0;
    }

    const nextContainer = resolveContainer() || container;
    onProgress?.({
      status: 'collecting',
      current: allImages.length,
      message: `正在滚动加载更多评论，当前已发现 ${allImages.length} 张图片`,
    });
    await loadMoreCommentSurface(nextContainer, 600);
  }

  return {
    total: allImages.length,
    images: allImages,
    stopReason: riskStopped ? 'risk_control' : (shouldStop() ? 'manual_stop' : 'collector_complete'),
  };
}

/**
 * 从评论 DOM 节点提取数据（含点赞数）
 */
function normalizeInlineText(value = '') {
  return String(value || '').replace(/\s+/g, ' ').trim();
}

function readElementText(el) {
  return String(el?.innerText || el?.textContent || '').trim();
}

function isTimeText(text = '') {
  return TIME_TEXT_RE.test(normalizeInlineText(text));
}

function isLikeText(text = '') {
  const value = normalizeInlineText(text);
  if (!value || value === '回复') return false;
  if (value === '赞') return true;
  return /^[\d,.]+(?:\.\d+)?(?:万|亿|千|w|W|k|K)?\+?$/.test(value);
}

function isLikelyLocationText(text = '') {
  const value = normalizeInlineText(text);
  if (!value || isTimeText(value) || isLikeText(value)) return false;
  if (/回复|展开|评论|赞|作者|博主/.test(value)) return false;
  return /^[\u4e00-\u9fa5A-Za-z·]{2,16}$/.test(value);
}

function normalizeXhsUrl(url = '') {
  const value = String(url || '').trim();
  if (!value || /^javascript:/i.test(value)) return '';
  if (/^https?:\/\//i.test(value)) return value;
  if (value.startsWith('//')) return `https:${value}`;
  if (value.startsWith('/')) return `https://www.xiaohongshu.com${value}`;
  return value;
}

function extractUserIdFromProfileUrl(url = '') {
  const match = String(url || '').match(/\/user\/profile\/([^/?#]+)/i);
  return match ? match[1] : '';
}

function stripLeadingAuthor(text = '', author = '') {
  let value = normalizeInlineText(text);
  const name = normalizeInlineText(author);
  if (name && value.startsWith(name)) {
    value = normalizeInlineText(value.slice(name.length));
  }
  return value;
}

function splitTailTokens(text = '') {
  return normalizeInlineText(text)
    .split(/\s+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function readTailAfterTime(afterTimeText = '') {
  const tokens = splitTailTokens(afterTimeText);
  let ipLocation = '';
  let likeText = '';
  let replyCount = 0;
  let replyCountText = '';

  for (const token of tokens) {
    if (!ipLocation && isLikelyLocationText(token)) {
      ipLocation = token;
      continue;
    }
    if (!likeText && isLikeText(token)) {
      likeText = token;
      continue;
    }
    const replyMatch = token.match(/^(\d+)\s*条回复$/);
    if (replyMatch) {
      replyCountText = token;
      replyCount = Number(replyMatch[1] || 0) || 0;
    }
  }

  const joined = normalizeInlineText(afterTimeText);
  if (!replyCount) {
    const replyMatch = joined.match(/(\d+)\s*条回复/);
    if (replyMatch) {
      replyCountText = replyMatch[0];
      replyCount = Number(replyMatch[1] || 0) || 0;
    }
  }

  return {
    ipLocation,
    likeText,
    replyCount,
    replyCountText,
  };
}

function extractReplyTarget(contentText = '', author = '') {
  const value = stripLeadingAuthor(contentText, author);
  const directReply = value.match(/^回复\s+(.{1,40}?)\s*[:：]\s*(.+)$/);
  if (directReply) {
    return {
      replyToNickname: normalizeInlineText(directReply[1]),
      text: normalizeInlineText(directReply[2]),
    };
  }
  const namedReply = value.match(/^(.{1,40}?)\s+回复\s+(.{1,40}?)\s*[:：]\s*(.+)$/);
  if (namedReply) {
    return {
      replyToNickname: normalizeInlineText(namedReply[2]),
      text: normalizeInlineText(namedReply[3]),
    };
  }
  return {
    replyToNickname: '',
    text: value,
  };
}

export function parseCommentTextTail(rawText = '', author = '') {
  const lines = String(rawText || '')
    .split(/\n+/)
    .map((line) => normalizeInlineText(line))
    .filter(Boolean);
  const authorText = normalizeInlineText(author);
  let contentText = '';
  let timeText = '';
  let tailText = '';

  const timeLineIndex = lines.findIndex((line) => isTimeText(line));
  if (timeLineIndex >= 0) {
    timeText = lines[timeLineIndex];
    const contentLines = lines
      .slice(0, timeLineIndex)
      .filter((line, index) => !(index === 0 && authorText && line === authorText));
    contentText = normalizeInlineText(contentLines.join(' '));
    tailText = lines.slice(timeLineIndex + 1).join(' ');
  } else {
    const inlineText = normalizeInlineText(rawText);
    const match = inlineText.match(INLINE_TIME_TEXT_RE);
    if (match) {
      timeText = normalizeInlineText(match[1]);
      contentText = stripLeadingAuthor(inlineText.slice(0, match.index), authorText);
      tailText = inlineText.slice((match.index || 0) + match[0].length);
    } else {
      contentText = stripLeadingAuthor(inlineText, authorText);
    }
  }

  const replyParts = extractReplyTarget(contentText, authorText);
  const tail = readTailAfterTime(tailText);
  return {
    contentText: replyParts.text,
    replyToNickname: replyParts.replyToNickname,
    timeText,
    ...tail,
  };
}

function readDomCommentText(el, author = '') {
  const innerContainer = el.querySelector('.comment-inner-container') || el;
  const spans = [...(innerContainer.querySelectorAll?.('span') || [])];
  const authorText = normalizeInlineText(author);
  const candidates = spans
    .map((span) => normalizeInlineText(span.textContent))
    .filter((text) => {
      if (!text || text === authorText) return false;
      if (isTimeText(text) || isLikeText(text) || isLikelyLocationText(text)) return false;
      if (/^(回复|赞|展开|收起)$/.test(text)) return false;
      return text.length > 1;
    });
  return candidates[0] || '';
}

function extractCommentImageUrls(el) {
  const urls = [];
  const images = [...(el.querySelectorAll?.('img') || [])];
  images.forEach((img) => {
    if (img.closest?.('a.name') || img.closest?.('.avatar') || img.closest?.('.author-wrapper')) return;
    if (img.width > 0 && img.width < 30) return;
    if (img.height > 0 && img.height < 30) return;
    extractImageSources(img).forEach((url) => {
      if (!urls.includes(url)) urls.push(url);
    });
  });
  return urls;
}

function extractMentionLinks(el, profileUrl = '') {
  const currentProfile = normalizeXhsUrl(profileUrl);
  return [...(el.querySelectorAll?.('a[href]') || [])]
    .map((link) => normalizeXhsUrl(link.getAttribute('href')))
    .filter((url, index, list) => url && url !== currentProfile && list.indexOf(url) === index);
}

export function parseCommentNode(el) {
  if (!el) return null;

  const rawId = el.dataset?.commentId
    || el.dataset?.id
    || el.getAttribute?.('data-comment-id')
    || el.getAttribute?.('data-id')
    || el.id
    || '';
  const authorEl = el.querySelector('.author-wrapper a.name')
    || el.querySelector('a.name')
    || el.querySelector('a[href*="/user/profile/"]');
  const avatarLinkEl = el.querySelector('.avatar a') || el.querySelector('a.name');
  const author = normalizeInlineText(authorEl?.textContent || authorEl?.innerText || '');
  const rawText = readElementText(el);
  const parsedTail = parseCommentTextTail(rawText, author);
  const domText = readDomCommentText(el, author);
  const replyParts = extractReplyTarget(parsedTail.contentText || domText, author);
  const text = replyParts.text || parsedTail.contentText || domText;
  if (!text && !author) return null;

  const commentId = rawId || `${author}_${text.slice(0, 50)}`;

  // 采集点赞数
  const likesEl = el.querySelector('.like-wrapper .count')
    || el.querySelector('.like .count')
    || el.querySelector('[class*="like"] .count')
    || el.querySelector('.comment-like .count');
  const likesText = normalizeInlineText(likesEl?.textContent || parsedTail.likeText || '0');
  const likes = parseLikeCount(likesText);
  const timeEl = [...(el.querySelectorAll?.('span') || [])].find((span) => isTimeText(span.textContent));
  const timeText = normalizeInlineText(timeEl?.textContent || parsedTail.timeText || '');
  const ipLocation = normalizeInlineText(el.querySelector('.date .location')?.textContent || parsedTail.ipLocation || '');
  const avatarUrl = el.querySelector('.avatar img.avatar-item')?.src
    || el.querySelector('.avatar img')?.src
    || '';
  const profileUrl = normalizeXhsUrl(authorEl?.getAttribute('href') || avatarLinkEl?.getAttribute('href') || '');
  const authorId = avatarLinkEl?.dataset?.userId
    || avatarLinkEl?.getAttribute?.('data-user-id')
    || extractUserIdFromProfileUrl(profileUrl)
    || '';
  const commentImageUrls = extractCommentImageUrls(el);
  const mentionLinks = extractMentionLinks(el, profileUrl);
  const replyToNickname = replyParts.replyToNickname || parsedTail.replyToNickname || '';

  return {
    commentId,
    text,
    author,
    profileUrl,
    location: ipLocation,
    ipLocation,
    avatarUrl,
    authorId,
    time: timeText,
    publishedAt: 0,
    publishedAtText: timeText,
    likes,
    likeText: likesText,
    replyCount: parsedTail.replyCount || 0,
    replyCountText: parsedTail.replyCountText || '',
    replyToNickname,
    replyToUserName: replyToNickname,
    commentImageUrls,
    mentionLinks,
    collectedAt: Date.now(),
    createdAt: Date.now(),
    syncStatus: 'pending',
    ...createCollectorQualityMeta({
      dataQuality: rawId ? 'degraded' : 'degraded',
      qualityReason: rawId ? 'api_unobserved_dom_fallback' : 'synthetic_comment_id',
      sourceTier: 'dom',
    }),
    ...createCollectorEvidence({
      rawPayload: {
        rawId,
        author,
        text,
        rawText,
        time: timeText,
        likesText,
        replyCount: parsedTail.replyCount || 0,
        replyCountText: parsedTail.replyCountText || '',
        replyToNickname,
        ipLocation,
        authorId,
        profileUrl,
        commentImageUrls,
        mentionLinks,
      },
      rawDomText: joinRawDomText([
        author,
        rawText,
        text,
        timeText,
        ipLocation,
      ]),
      rawUrl: window.location.href,
      rawSource: 'xhs.comments.dom',
    }),
  };
}

/**
 * 解析点赞数（支持 "1.2万" 格式）
 */
function parseLikeCount(text) {
  if (!text || text === '赞') return 0;
  return parseCount(text);
}

/**
 * 展开所有子评论回复（递归版）
 * 支持多层级：每次点击展开按钮后重新扫描所有层级的「展开更多回复」
 */
export function isExpandMoreReplyTrigger(text = '') {
  return /展开/.test(String(text || '').trim());
}

function replyControlIsVisible(button) {
  if (typeof button?.getBoundingClientRect !== 'function') return true;
  const rect = button.getBoundingClientRect();
  const scrollParent = findScrollParent(button);
  const parentRect = typeof scrollParent?.getBoundingClientRect === 'function'
    ? scrollParent.getBoundingClientRect()
    : { top: 0, bottom: globalThis.innerHeight || 0 };
  return rect.top >= parentRect.top - REPLY_CONTROL_VISIBILITY_EPSILON_PX
    && rect.bottom <= parentRect.bottom + REPLY_CONTROL_VISIBILITY_EPSILON_PX;
}

export async function expandNextReply(parentCommentEl, {
  waitIfPaused = async () => {},
  shouldStop = () => false,
  waitBeforeAction = () => waitForPageSettle(90),
  waitAfterAction = null,
} = {}) {
  await waitIfPaused();
  if (shouldStop()) return { acted: false, reason: 'stopped' };
  const button = [...parentCommentEl.querySelectorAll('div.show-more')]
    .find((el) => isExpandMoreReplyTrigger(el?.textContent));
  if (!button) return { acted: false, reason: 'no_expand_control' };

  const beforeCount = parentCommentEl.querySelectorAll('.comment-item.comment-item-sub').length;
  await waitBeforeAction({ kind: 'dom_expand_reply' });
  await waitIfPaused();
  if (shouldStop()) return { acted: false, reason: 'stopped' };
  if (!replyControlIsVisible(button)) {
    button.scrollIntoView({ behavior: 'auto', block: 'nearest' });
    if (typeof waitAfterAction === 'function') {
      await waitAfterAction({ kind: 'dom_reveal_reply', beforeCount });
    } else {
      await waitForPageSettle(420);
    }
    await waitIfPaused();
    return { acted: true, reason: 'reply_control_revealed' };
  }
  button.click();
  if (typeof waitAfterAction === 'function') {
    await waitAfterAction({ kind: 'dom_expand_reply', beforeCount });
  } else {
    await waitForCondition(() => {
      const currentCount = parentCommentEl.querySelectorAll('.comment-item.comment-item-sub').length;
      const stillHasExpand = [...parentCommentEl.querySelectorAll('div.show-more')]
        .some((el) => isExpandMoreReplyTrigger(el?.textContent));
      return currentCount > beforeCount || !stillHasExpand;
    }, 800, 70);
  }
  await waitIfPaused();
  return { acted: true, reason: 'reply_expanded' };
}

export async function expandAllReplies(parentCommentEl, maxAttempts = 10, controls = {}) {

  while (maxAttempts > 0) {
    const result = await expandNextReply(parentCommentEl, controls);
    if (!result.acted) break;
    maxAttempts--;
  }
}

function findScrollParent(el) {
  let parent = el.parentElement;
  while (parent) {
    const style = window.getComputedStyle(parent);
    if (style.overflow === 'auto' || style.overflow === 'scroll' ||
        style.overflowY === 'auto' || style.overflowY === 'scroll') {
      return parent;
    }
    parent = parent.parentElement;
  }
  return document.documentElement;
}

function extractImageSources(img) {
  if (!img) return [];
  const attrCandidates = [
    img.currentSrc,
    img.src,
    img.dataset?.src,
    img.dataset?.origin,
    img.dataset?.originSrc,
    img.getAttribute('data-src'),
    img.getAttribute('data-origin'),
    img.getAttribute('data-origin-src'),
    img.getAttribute('src'),
  ].filter(Boolean);

  const srcset = img.getAttribute('srcset') || '';
  if (srcset) {
    srcset.split(',').forEach((item) => {
      const [url] = item.trim().split(/\s+/);
      if (url) attrCandidates.push(url);
    });
  }

  // 采集图片节点及其祖先（最多 5 层）上的 href/data-*/style 背景图
  let cursor = img;
  for (let depth = 0; cursor && depth < 5; depth++) {
    const href = cursor.getAttribute?.('href') || '';
    if (href) attrCandidates.push(href);

    const style = cursor.getAttribute?.('style') || '';
    const bgUrls = extractUrlsFromBackground(style);
    bgUrls.forEach((value) => attrCandidates.push(value));

    const dataset = cursor.dataset || {};
    Object.keys(dataset).forEach((key) => {
      const value = dataset[key];
      if (typeof value !== 'string') return;
      const trimmed = value.trim();
      if (!trimmed) return;
      if (/^https?:\/\//i.test(trimmed) || /^\/\//.test(trimmed) || /^\/[^/]/.test(trimmed)) {
        attrCandidates.push(trimmed);
      }
    });
    cursor = cursor.parentElement;
  }

  const dedup = [];
  attrCandidates.forEach((value) => {
    if (!value) return;
    const normalized = String(value).trim();
    if (!normalized || dedup.includes(normalized)) return;
    if (!isLikelyImageUrl(normalized)) return;
    dedup.push(normalized);
  });
  return dedup;
}

function extractUrlsFromBackground(styleText) {
  if (!styleText || !styleText.includes('url(')) return [];
  const urls = [];
  const re = /url\((['"]?)(.*?)\1\)/g;
  let m;
  while ((m = re.exec(styleText)) !== null) {
    if (m[2]) urls.push(m[2].trim());
  }
  return urls;
}

function isLikelyImageUrl(url) {
  if (!url || /^data:|^javascript:/i.test(url)) return false;
  if (/\.(jpg|jpeg|png|gif|webp|avif)(\?|$)/i.test(url)) return true;
  if (/xhscdn|xhsimg|xhslink/i.test(url)) return true;
  if (/x-oss-process|imageView2|imageslim|thumbnail/i.test(url)) return true;
  return false;
}
