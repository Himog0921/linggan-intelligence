import { parseCount } from '../shared/utils.js';

// Browser-local execution support only. These records keep pause/resume and progress durable
// inside the installed producer; they are never a second Linggan truth or an old Workbench
// transport. Linggan receives typed packages through the runtime outbox.
export const LOCAL_SURFACE_MODE = Object.freeze({
  KEYWORD_SURFACE: 'keyword_surface',
  AUTHOR_SURFACE: 'author_surface',
  AUTHOR_PROFILE: 'author_profile',
  DETAIL_PROBE: 'detail_probe',
});

export const LOCAL_TASK_STRATEGY = Object.freeze({
  AUTHOR_BASELINE: 'author_baseline',
});

export const LOCAL_READ_RECORD_TYPE = Object.freeze({
  NOTE: 'note',
  COMMENT: 'comment',
  AUTHOR: 'author',
});

const MAX_ATTACHED_COMMENTS = 30;

function text(value = '') { return String(value || '').trim(); }
function nonNegative(value) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? Math.floor(number) : null;
}
function positive(value, fallback = 0) {
  const number = nonNegative(value);
  return number && number > 0 ? number : fallback;
}
function normalizedTargetId(value = '') {
  return text(value).replace(/^(xhs_|dy_|douyin_)/i, '').toLowerCase();
}
function url(value = '', base = '') {
  const valueText = text(value);
  if (!valueText) return '';
  if (/^https?:\/\//i.test(valueText)) return valueText;
  return valueText.startsWith('/') ? `${base}${valueText}` : valueText;
}
function firstMedia(value) {
  if (!Array.isArray(value)) return '';
  for (const item of value) {
    if (typeof item === 'string' && text(item)) return text(item);
    if (item && typeof item === 'object') {
      const candidate = text(item.urlDefault || item.url || item.src);
      if (candidate) return candidate;
    }
  }
  return '';
}

export function withLocalReadMeta(record = {}, localMeta = null, mode = '') {
  if (!localMeta || typeof localMeta !== 'object') return record;
  const surfaceMode = text(mode || record.surfaceMode || localMeta.surfaceMode);
  return {
    ...record,
    surfaceMode,
    localExecutionMeta: { ...localMeta, ...(record.localExecutionMeta || {}), surfaceMode },
  };
}

export function buildXhsSurfaceNoteRecords(cards = [], {
  localMeta = null,
  collectionRunId = '',
  mode = '',
  limit = 0,
  sourcePageUrl = '',
  searchKeyword = '',
  searchPageUrl = '',
  searchFilters = null,
  searchFilterSnapshot = null,
} = {}) {
  const maximum = positive(limit, Array.isArray(cards) ? cards.length : 0);
  return (Array.isArray(cards) ? cards : []).slice(0, maximum).map((card, index) => {
    const noteId = text(card?.noteId || card?.platformContentId || card?.contentId).replace(/^xhs_/, '');
    if (!noteId) return null;
    const images = Array.isArray(card.images) ? card.images.filter(Boolean) : [];
    const imageCandidates = Array.isArray(card.imageCandidates) ? card.imageCandidates.filter(Boolean) : [];
    const cover = text(card.cover || card.coverImg || card.coverUrl || card.thumbnail) || firstMedia(images) || firstMedia(imageCandidates);
    const commentsKnown = card?.comments !== undefined && card?.comments !== null && text(card.comments) !== '';
    const observedAt = new Date().toISOString();
    return withLocalReadMeta({
      dataQuality: 'seed', qualityReason: 'local_surface_seed', sourceTier: 'seed',
      noteId, targetKey: `xhs:note:${noteId}`, platformContentId: noteId, contentId: `xhs_${noteId}`,
      platform: 'xhs', title: text(card.title || card.titleHint), content: text(card.content || card.desc || card.title || card.titleHint),
      bodyText: text(card.bodyText || card.content || card.desc || card.title || card.titleHint),
      url: url(card.rawUrl || card.url || `/explore/${noteId}`, 'https://www.xiaohongshu.com'),
      canonicalUrl: url(card.canonicalUrl || card.rawUrl || card.url || `/explore/${noteId}`, 'https://www.xiaohongshu.com'),
      rawUrl: url(card.rawUrl || card.url || `/explore/${noteId}`, 'https://www.xiaohongshu.com'),
      sourceUrl: text(card.sourceUrl || sourcePageUrl), runnableDetailUrl: url(card.runnableDetailUrl || `/explore/${noteId}`, 'https://www.xiaohongshu.com'),
      cover, coverImg: text(card.coverImg) || cover, coverUrl: text(card.coverUrl) || cover, thumbnail: text(card.thumbnail) || cover,
      images: images.length ? images : (cover ? [cover] : []), imageCandidates,
      likes: parseCount(card.likes), likeCount: parseCount(card.likes), comments: commentsKnown ? parseCount(card.comments) : null,
      commentCount: commentsKnown ? parseCount(card.comments) : null, publicCommentCount: commentsKnown ? parseCount(card.comments) : null,
      publicCommentCountKnown: commentsKnown, collects: parseCount(card.collects), collectCount: parseCount(card.collects), shares: parseCount(card.shares), shareCount: parseCount(card.shares),
      type: text(card.type || 'normal'), authorId: text(card.authorId || card.authorPlatformId || card.userId || localMeta?.authorPlatformId),
      authorPlatformId: text(card.authorPlatformId || card.authorId || card.userId || localMeta?.authorPlatformId), authorName: text(card.authorName || card.authorHint || localMeta?.authorName), authorAvatar: text(card.authorAvatar || card.avatar),
      publishedAt: card.publishedAt ?? card.publishTime ?? card.createTime ?? null, publishedAtText: text(card.publishedAtText || card.publishTimeText || card.timeText),
      sourcePageUrl: text(sourcePageUrl), searchKeyword: text(searchKeyword || localMeta?.keyword), searchPageUrl: text(searchPageUrl),
      searchFilters: searchFilters && typeof searchFilters === 'object' ? searchFilters : undefined,
      searchFilterSnapshot: searchFilterSnapshot && typeof searchFilterSnapshot === 'object' ? searchFilterSnapshot : undefined,
      batchRank: positive(card.rank || card.batchRank || Number(card._discoveryOrder) + 1, index + 1),
      rank: positive(card.rank || card.batchRank || Number(card._discoveryOrder) + 1, index + 1),
      collectionRunId: text(collectionRunId), dataSource: 'local_surface_card', observedAt, collectedAt: observedAt, createdAt: Date.now(), updatedAt: Date.now(),
    }, localMeta, mode || localMeta?.surfaceMode);
  }).filter(Boolean);
}

export function buildDouyinSurfaceNoteRecords(targets = [], {
  localMeta = null, collectionRunId = '', mode = '', limit = 0, searchKeyword = '', searchPageUrl = '',
} = {}) {
  const maximum = positive(limit, Array.isArray(targets) ? targets.length : 0);
  return (Array.isArray(targets) ? targets : []).slice(0, maximum).map((target, index) => {
    const id = text(target?.awemeId || target?.platformContentId || target?.noteId || target?.contentId).replace(/^(dy_|douyin_)/i, '');
    if (!id) return null;
    const images = Array.isArray(target.images) ? target.images.filter(Boolean) : [];
    const imageCandidates = Array.isArray(target.imageCandidates) ? target.imageCandidates.filter(Boolean) : [];
    const cover = text(target.cover || target.coverImg || target.coverUrl || target.thumbnail) || firstMedia(images) || firstMedia(imageCandidates);
    return withLocalReadMeta({
      dataQuality: 'seed', qualityReason: 'local_surface_seed', sourceTier: 'seed', noteId: id, platformContentId: id, contentId: `dy_${id}`,
      platform: 'douyin', title: text(target.titleHint || target.title || id), content: text(target.titleHint || target.title), bodyText: text(target.titleHint || target.title),
      url: url(target.href || target.url || `/video/${id}`, 'https://www.douyin.com'), canonicalUrl: url(target.href || target.url || `/video/${id}`, 'https://www.douyin.com'),
      cover, coverImg: text(target.coverImg) || cover, coverUrl: text(target.coverUrl) || cover, thumbnail: text(target.thumbnail) || cover,
      images: images.length ? images : (cover ? [cover] : []), imageCandidates, likes: parseCount(target.likes), comments: parseCount(target.comments), collects: parseCount(target.collects), shares: parseCount(target.shares), type: 'video',
      authorId: text(target.authorPlatformId || target.platformAuthorId || target.authorId || localMeta?.authorPlatformId), authorPlatformId: text(target.authorPlatformId || target.platformAuthorId || target.authorId || localMeta?.authorPlatformId), authorName: text(target.authorHint || target.authorName || localMeta?.authorName),
      profileUrl: text(target.profileUrl || localMeta?.profileUrl), publishedAt: target.publishedAt ?? target.publishTime ?? target.createTime ?? null, publishedAtText: text(target.timeHint || target.publishedAtText),
      searchKeyword: text(searchKeyword || target.searchKeyword || localMeta?.keyword), searchPageUrl: text(searchPageUrl), batchRank: index + 1,
      collectionRunId: text(collectionRunId), dataSource: 'local_surface_card', collectedAt: Date.now(), createdAt: Date.now(), updatedAt: Date.now(),
    }, localMeta, mode || localMeta?.surfaceMode);
  }).filter(Boolean);
}

export function createLocalExecutionHeartbeatReporter({ localExecutionStore, intervalMs = 15000, now = () => Date.now() } = {}) {
  const lastByRun = new Map();
  return { async report(runId, patch = {}) {
    const id = text(runId);
    if (!id || !localExecutionStore?.markHeartbeat) return false;
    const timestamp = Number.isFinite(Number(patch.heartbeatAt)) ? Number(patch.heartbeatAt) : now();
    if (!patch.force && timestamp - Number(lastByRun.get(id) || 0) < intervalMs) return false;
    lastByRun.set(id, timestamp);
    await localExecutionStore.markHeartbeat(id, timestamp, { ...(patch.status || patch.taskState ? { status: patch.status || patch.taskState } : {}), ...(patch.stage || patch.phase ? { stage: patch.stage || patch.phase } : {}), ...(patch.message ? { message: patch.message } : {}) });
    return true;
  } };
}

export function createLocalExecutionHeartbeatLoop({ reporter, intervalMs = 30000 } = {}) {
  let timer = null;
  return {
    start(runId, patch = () => {}) { if (!text(runId) || timer) return; timer = setInterval(() => void reporter?.report(runId, { ...(patch() || {}), force: true }).catch(() => {}), intervalMs); },
    stop() { if (timer) clearInterval(timer); timer = null; },
  };
}

export function buildLocalExecutionCreatePayload({ platform = '', taskType = '', pageType = '', triggerSource = '', pageUrl = '', config = {} } = {}) {
  return { platform: text(platform), taskType: text(taskType), pageType: text(pageType), triggerSource: text(triggerSource), resultUploadStatus: 'local_staging', lastHeartbeatAt: Date.now(), config: config && typeof config === 'object' ? config : {}, meta: { pageUrl: text(pageUrl) } };
}

function expectedCommentCount({ expectedCommentCount = null, publicCommentCount = null, requestedCommentLimit = null, commentLimit = null } = {}) {
  const explicit = nonNegative(expectedCommentCount);
  if (explicit !== null) return Math.min(explicit, MAX_ATTACHED_COMMENTS);
  const knownPublic = nonNegative(publicCommentCount);
  if (knownPublic === null) return null;
  const requested = nonNegative(requestedCommentLimit) ?? nonNegative(commentLimit) ?? MAX_ATTACHED_COMMENTS;
  return Math.min(knownPublic, requested, MAX_ATTACHED_COMMENTS);
}

export function publicCommentCountFromXhsNote(note = {}) {
  const direct = nonNegative(note?.publicCommentCount);
  if (direct !== null) return direct;
  return note?.publicCommentCountKnown === true ? nonNegative(note.comments ?? note.commentCount ?? note.commentNum) : null;
}

export function buildXhsAttachedCommentResult(input = {}) {
  const total = nonNegative(input.total) ?? 0;
  const publicCount = nonNegative(input.publicCommentCount);
  const expected = expectedCommentCount({ ...input, publicCommentCount: publicCount });
  const inferred = expected !== null && total < expected ? (total === 0 ? 'comments_empty_after_request' : 'comments_under_expected') : '';
  return { noteId: text(input.noteId), total, ...(publicCount !== null ? { publicCommentCount: publicCount } : {}), ...(expected !== null && (text(input.error) || inferred) ? { expectedCommentCount: expected } : {}), error: text(input.error) || inferred };
}

function xhsSummary(noteList = [], collected = [], failed = [], commentResults = []) {
  const targetIds = (Array.isArray(noteList) ? noteList : []).map((item) => text(item?.noteId)).filter(Boolean);
  const attached = (Array.isArray(commentResults) ? commentResults : []).map(buildXhsAttachedCommentResult).filter((result) => result.noteId);
  return { itemsPlanned: targetIds.length, itemsSucceeded: Array.isArray(collected) ? collected.length : 0, itemsFailed: Array.isArray(failed) ? failed.length : 0, targetIds, contentIds: (Array.isArray(collected) ? collected : []).map((item) => text(item?.contentId || item?.noteId)).filter(Boolean), failedTargets: Array.isArray(failed) ? failed : [], totalComments: attached.reduce((sum, item) => sum + item.total, 0), attachedCommentResults: attached };
}

export function buildXhsBatchNotesRunPatch({ noteList = [], collected = [], failed = [], commentResults = [] } = {}) { return xhsSummary(noteList, collected, failed, commentResults); }
export function buildXhsBatchNotesProgressPatch({ noteList = [], collected = [], failed = [], commentResults = [], processedCount = 0 } = {}) {
  const targetIds = (Array.isArray(noteList) ? noteList : []).map((item) => text(item?.noteId)).filter(Boolean);
  const processed = targetIds.slice(0, Math.max(0, Number(processedCount) || 0));
  const summary = xhsSummary(processed.map((noteId) => ({ noteId })), (collected || []).filter((item) => processed.includes(text(item?.noteId))), (failed || []).filter((item) => processed.includes(text(item?.noteId))), (commentResults || []).filter((item) => processed.includes(text(item?.noteId))));
  return { ...summary, ...buildBatchResumeCheckpoint({ targetIds, processedCount, resultStatuses: [...(collected || []).map((item) => ({ targetId: text(item?.noteId), ok: true, contentId: text(item?.contentId || item?.noteId) })), ...(failed || []).map((item) => ({ targetId: text(item?.noteId), ok: false, error: text(item?.error) }))] }), itemsPlanned: targetIds.length, targetIds };
}
export function buildXhsBatchCommentsRunPatch({ noteList = [], results = [] } = {}) {
  const targetIds = (Array.isArray(noteList) ? noteList : []).map((item) => text(item?.noteId)).filter(Boolean);
  const resultMap = new Map((results || []).map((item) => [text(item?.noteId), item]));
  // A page that explicitly has no public comments completed successfully.  Collection success
  // is whether the target was read and reported, never whether a non-zero payload happened.
  const succeeded = targetIds.filter((id) => {
    const result = resultMap.get(id);
    return result && !text(result.error);
  });
  return { itemsPlanned: targetIds.length, itemsSucceeded: succeeded.length, itemsFailed: targetIds.length - succeeded.length, totalComments: (results || []).reduce((sum, item) => sum + (Number(item?.total) || 0), 0), targetIds, contentIds: succeeded.map((id) => `xhs_${id}`), failedTargets: targetIds.filter((id) => !succeeded.includes(id)).map((noteId) => ({ noteId, error: text(resultMap.get(noteId)?.error || 'no_result') })) };
}
export function buildXhsBatchCommentsProgressPatch({ noteList = [], results = [], processedCount = 0 } = {}) {
  const targetIds = (noteList || []).map((item) => text(item?.noteId)).filter(Boolean); const processed = targetIds.slice(0, Math.max(0, Number(processedCount) || 0));
  return { ...buildXhsBatchCommentsRunPatch({ noteList: processed.map((noteId) => ({ noteId })), results }), ...buildBatchResumeCheckpoint({ targetIds, processedCount, resultStatuses: (results || []).filter((item) => processed.includes(text(item?.noteId))).map((item) => ({ targetId: text(item?.noteId), ok: !text(item?.error), totalComments: Number(item?.total || 0), error: text(item?.error) })) }), itemsPlanned: targetIds.length, targetIds };
}

function douyinTarget(item = {}) { return text(item?.awemeId || item?.platformContentId || item?.videoId || item?.noteId || item?.contentId).replace(/^(dy_|douyin_)/i, ''); }
function douyinSummary(targets = [], results = [], totalComments = null) {
  const targetIds = (targets || []).map(douyinTarget).filter(Boolean); const failedTargets = (results || []).filter((item) => item?.ok === false).map((item) => ({ awemeId: douyinTarget(item), error: text(item?.error || 'failed') })); const contentIds = (results || []).filter((item) => item?.ok !== false).map((item) => `dy_${douyinTarget(item)}`).filter(Boolean);
  return { itemsPlanned: targetIds.length, itemsSucceeded: contentIds.length, itemsFailed: failedTargets.length, ...(totalComments === null ? {} : { totalComments: Math.max(0, Number(totalComments) || 0) }), targetIds, contentIds, failedTargets };
}
export function buildDouyinBatchVideosRunPatch({ targets = [], results = [] } = {}) { return douyinSummary(targets, results); }
export function buildDouyinBatchVideosProgressPatch({ targets = [], results = [], processedCount = 0 } = {}) { const targetIds = (targets || []).map(douyinTarget).filter(Boolean); const processed = new Set(targetIds.slice(0, Math.max(0, Number(processedCount) || 0))); const scoped = (results || []).filter((item) => processed.has(douyinTarget(item))); return { ...douyinSummary(targets, scoped), ...buildBatchResumeCheckpoint({ targetIds, processedCount, resultStatuses: scoped.map((item) => ({ targetId: douyinTarget(item), ok: item?.ok !== false, contentId: douyinTarget(item), error: text(item?.error) })) }), itemsPlanned: targetIds.length, targetIds }; }
export function buildDouyinBatchCommentsRunPatch({ targets = [], results = [], totalComments = 0 } = {}) { return douyinSummary(targets, results, totalComments); }
export function buildDouyinBatchCommentsProgressPatch({ targets = [], results = [], totalComments = 0, processedCount = 0 } = {}) { const targetIds = (targets || []).map(douyinTarget).filter(Boolean); const processed = new Set(targetIds.slice(0, Math.max(0, Number(processedCount) || 0))); const scoped = (results || []).filter((item) => processed.has(douyinTarget(item))); return { ...douyinSummary(targets, scoped, totalComments), ...buildBatchResumeCheckpoint({ targetIds, processedCount, resultStatuses: scoped.map((item) => ({ targetId: douyinTarget(item), ok: item?.ok !== false, contentId: douyinTarget(item), totalComments: Number(item?.totalComments || 0), error: text(item?.error) })) }), itemsPlanned: targetIds.length, targetIds }; }

export function buildBatchResumeCheckpoint({ targetIds = [], processedCount = 0, resultStatuses = [], updatedAt = Date.now() } = {}) {
  const ids = (targetIds || []).map(text).filter(Boolean); const nextIndex = Math.min(Math.max(0, Math.floor(Number(processedCount) || 0)), ids.length); const allowed = new Set(ids.map(normalizedTargetId));
  const statuses = (resultStatuses || []).map((item) => ({ targetId: text(item?.targetId || item?.noteId || item?.awemeId || item?.contentId), ok: item?.ok !== false && !text(item?.error), contentId: text(item?.contentId || item?.noteId || item?.videoId), error: text(item?.error), totalComments: Number.isFinite(Number(item?.totalComments)) ? Number(item.totalComments) : undefined })).filter((item) => item.targetId && allowed.has(normalizedTargetId(item.targetId))).slice(0, nextIndex);
  return { processedCount: nextIndex, nextIndex, resumeCheckpoint: { version: 1, processedCount: nextIndex, nextIndex, targetIds: ids, resultStatuses: statuses, updatedAt: Number.isFinite(Number(updatedAt)) ? Number(updatedAt) : Date.now() } };
}

export function resolveBatchResumeState({ runRecord = null, targets = [], getTargetId = (item) => item } = {}) {
  const targetById = new Map((targets || []).map((target) => [normalizedTargetId(getTargetId(target)), target]).filter(([id]) => id)); const checkpoint = runRecord?.resumeCheckpoint && typeof runRecord.resumeCheckpoint === 'object' ? runRecord.resumeCheckpoint : {}; const prior = Array.isArray(checkpoint.targetIds) && checkpoint.targetIds.length ? checkpoint.targetIds : (runRecord?.targetIds || []); const ordered = []; const seen = new Set();
  for (const id of [...prior, ...(targets || []).map(getTargetId)]) { const key = normalizedTargetId(id); const target = targetById.get(key); if (target && !seen.has(key)) { ordered.push(target); seen.add(key); } }
  const targetIds = ordered.map(getTargetId).map(text).filter(Boolean); const nextIndex = Math.min(Math.max(0, Number(checkpoint.nextIndex ?? checkpoint.processedCount ?? runRecord?.nextIndex ?? runRecord?.processedCount) || 0), targetIds.length);
  return { targets: ordered, targetIds, nextIndex, processedCount: nextIndex, completedTargetIds: targetIds.slice(0, nextIndex), resultStatuses: Array.isArray(checkpoint.resultStatuses) ? checkpoint.resultStatuses : [], resumed: nextIndex > 0 };
}
