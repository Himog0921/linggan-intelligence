import { buildBatchResumeCheckpoint } from './batchResume.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeNonNegativeInteger(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return Math.max(0, Math.floor(value));
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value.replace(/[,+]/g, ''));
    if (Number.isFinite(parsed)) return Math.max(0, Math.floor(parsed));
  }
  return null;
}

const MAX_ATTACHED_COMMENTS = 30;

function expectedCommentCountFrom({
  expectedCommentCount = null,
  publicCommentCount = null,
  requestedCommentLimit = null,
  commentLimit = null,
} = {}) {
  const explicitExpected = normalizeNonNegativeInteger(expectedCommentCount);
  if (explicitExpected !== null) return Math.min(explicitExpected, MAX_ATTACHED_COMMENTS);
  const normalizedPublicCommentCount = normalizeNonNegativeInteger(publicCommentCount);
  if (normalizedPublicCommentCount === null) return null;
  const requestedLimit =
    normalizeNonNegativeInteger(requestedCommentLimit) ??
    normalizeNonNegativeInteger(commentLimit) ??
    MAX_ATTACHED_COMMENTS;
  return Math.min(normalizedPublicCommentCount, requestedLimit, MAX_ATTACHED_COMMENTS);
}

export function publicCommentCountFromXhsNote(note = {}) {
  if (!note || typeof note !== 'object') return null;
  const direct = normalizeNonNegativeInteger(note.publicCommentCount);
  if (direct !== null) return direct;
  if (note.publicCommentCountKnown !== true) return null;
  return normalizeNonNegativeInteger(note.comments)
    ?? normalizeNonNegativeInteger(note.commentCount)
    ?? normalizeNonNegativeInteger(note.commentsCount)
    ?? normalizeNonNegativeInteger(note.comment_count)
    ?? normalizeNonNegativeInteger(note.commentNum);
}

export function buildXhsAttachedCommentResult({
  noteId = '',
  total = 0,
  error = '',
  publicCommentCount = null,
  expectedCommentCount = null,
  requestedCommentLimit = null,
  commentLimit = null,
} = {}) {
  const normalizedTotal = Math.max(0, Number(total || 0) || 0);
  const normalizedPublicCommentCount = normalizeNonNegativeInteger(publicCommentCount);
  const normalizedExpectedCommentCount = expectedCommentCountFrom({
    expectedCommentCount,
    publicCommentCount: normalizedPublicCommentCount,
    requestedCommentLimit,
    commentLimit,
  });
  const inferredError =
    normalizedExpectedCommentCount !== null && normalizedTotal < normalizedExpectedCommentCount
      ? (normalizedTotal <= 0 ? 'comments_empty_after_request' : 'comments_under_expected')
      : (normalizedTotal <= 0 && normalizedPublicCommentCount !== 0 ? 'comments_empty_after_request' : '');
  const normalizedError = normalizeText(error) || inferredError;
  return {
    noteId: normalizeText(noteId),
    total: normalizedTotal,
    ...(normalizedPublicCommentCount !== null
      ? { publicCommentCount: normalizedPublicCommentCount }
      : {}),
    ...(normalizedError && normalizedExpectedCommentCount !== null
      ? { expectedCommentCount: normalizedExpectedCommentCount }
      : {}),
    error: normalizedError,
  };
}

function normalizeExternalTaskMeta(meta = {}) {
  return {
    externalTaskId: normalizeText(meta.externalTaskId),
    externalTaskType: normalizeText(meta.externalTaskType),
    executorInstanceId: normalizeText(meta.executorInstanceId),
    protocolVersion: normalizeText(meta.protocolVersion),
  };
}

export function buildRemoteRunCreatePayload({
  platform = 'xhs',
  taskType = '',
  pageType = '',
  triggerSource = '',
  pageUrl = '',
  config = {},
  externalTaskMeta = {},
} = {}) {
  const taskMeta = normalizeExternalTaskMeta(externalTaskMeta);
  if (!taskMeta.externalTaskId) return null;

  return {
    externalTaskId: taskMeta.externalTaskId,
    externalTaskType: taskMeta.externalTaskType,
    executorInstanceId: taskMeta.executorInstanceId,
    protocolVersion: taskMeta.protocolVersion,
    platform: normalizeText(platform),
    taskType: normalizeText(taskType),
    pageType: normalizeText(pageType),
    triggerSource: normalizeText(triggerSource),
    resultUploadStatus: 'pending_upload',
    lastHeartbeatAt: Date.now(),
    config: config && typeof config === 'object' ? config : {},
    meta: {
      pageUrl: normalizeText(pageUrl),
    },
  };
}

function buildContentIdsFromNotes(collected = []) {
  return (Array.isArray(collected) ? collected : [])
    .map((item) => {
      const direct = normalizeText(item?.contentId);
      if (direct.startsWith('xhs_')) return direct;
      const noteId = normalizeText(item?.noteId || direct).replace(/^xhs_/, '');
      return noteId ? `xhs_${noteId}` : '';
    })
    .filter(Boolean);
}

function buildAttachedCommentSummary(commentResults = []) {
  const results = (Array.isArray(commentResults) ? commentResults : [])
    .map((item) => buildXhsAttachedCommentResult(item))
    .filter((item) => item.noteId);
  if (results.length === 0) return {};

  return {
    totalComments: results.reduce((sum, item) => sum + item.total, 0),
    attachedCommentResults: results,
  };
}

export function buildXhsBatchNotesRunPatch({
  noteList = [],
  collected = [],
  failed = [],
  commentResults = [],
} = {}) {
  const targets = (Array.isArray(noteList) ? noteList : [])
    .map((item) => normalizeText(item?.noteId))
    .filter(Boolean);

  return {
    itemsPlanned: targets.length,
    itemsSucceeded: Array.isArray(collected) ? collected.length : 0,
    itemsFailed: Array.isArray(failed) ? failed.length : 0,
    targetIds: targets,
    contentIds: buildContentIdsFromNotes(collected),
    failedTargets: Array.isArray(failed) ? failed : [],
    ...buildAttachedCommentSummary(commentResults),
  };
}

export function buildXhsBatchNotesProgressPatch({
  noteList = [],
  collected = [],
  failed = [],
  commentResults = [],
  processedCount = 0,
} = {}) {
  const allTargets = (Array.isArray(noteList) ? noteList : [])
    .map((item) => normalizeText(item?.noteId))
    .filter(Boolean);
  const processedTargets = allTargets.slice(0, Math.max(0, Number(processedCount || 0) || 0));
  const processedSet = new Set(processedTargets);
  const scopedCollected = (Array.isArray(collected) ? collected : [])
    .filter((item) => processedSet.has(normalizeText(item?.noteId)));
  const scopedFailed = (Array.isArray(failed) ? failed : [])
    .filter((item) => processedSet.has(normalizeText(item?.noteId)));
  const scopedCommentResults = (Array.isArray(commentResults) ? commentResults : [])
    .filter((item) => processedSet.has(normalizeText(item?.noteId)));
  const summary = buildXhsBatchNotesRunPatch({
    noteList: processedTargets.map((noteId) => ({ noteId })),
    collected: scopedCollected,
    failed: scopedFailed,
    commentResults: scopedCommentResults,
  });
  const resume = buildBatchResumeCheckpoint({
    targetIds: allTargets,
    processedCount,
    resultStatuses: [
      ...scopedCollected.map((item) => ({
        targetId: normalizeText(item?.noteId),
        ok: true,
        contentId: normalizeText(item?.contentId || item?.noteId),
      })),
      ...scopedFailed.map((item) => ({
        targetId: normalizeText(item?.noteId),
        ok: false,
        error: normalizeText(item?.error),
      })),
    ],
  });

  return {
    ...summary,
    ...resume,
    itemsPlanned: allTargets.length,
    targetIds: allTargets,
  };
}

export function buildXhsBatchCommentsRunPatch({
  noteList = [],
  results = [],
} = {}) {
  const targets = (Array.isArray(noteList) ? noteList : [])
    .map((item) => normalizeText(item?.noteId))
    .filter(Boolean);
  const resultMap = new Map(
    (Array.isArray(results) ? results : [])
      .map((item) => [normalizeText(item?.noteId), item])
      .filter(([noteId]) => Boolean(noteId)),
  );

  const succeeded = [];
  const failed = [];
  let totalComments = 0;

  for (const noteId of targets) {
    const result = resultMap.get(noteId);
    const total = Number(result?.total || 0) || 0;
    totalComments += total;
    if (total > 0) {
      succeeded.push(`xhs_${noteId}`);
    } else {
      failed.push(result ? { noteId, total } : { noteId, error: 'no_result' });
    }
  }

  return {
    itemsPlanned: targets.length,
    itemsSucceeded: succeeded.length,
    itemsFailed: failed.length,
    totalComments,
    targetIds: targets,
    contentIds: succeeded,
    failedTargets: failed,
  };
}

export function buildXhsBatchCommentsProgressPatch({
  noteList = [],
  results = [],
  processedCount = 0,
} = {}) {
  const allTargets = (Array.isArray(noteList) ? noteList : [])
    .map((item) => normalizeText(item?.noteId))
    .filter(Boolean);
  const processedTargets = allTargets.slice(0, Math.max(0, Number(processedCount || 0) || 0));
  const summary = buildXhsBatchCommentsRunPatch({
    noteList: processedTargets.map((noteId) => ({ noteId })),
    results,
  });
  const processedSet = new Set(processedTargets);
  const resume = buildBatchResumeCheckpoint({
    targetIds: allTargets,
    processedCount,
    resultStatuses: (Array.isArray(results) ? results : [])
      .filter((item) => processedSet.has(normalizeText(item?.noteId)))
      .map((item) => ({
        targetId: normalizeText(item?.noteId),
        ok: Number(item?.total || 0) > 0,
        contentId: normalizeText(item?.noteId),
        totalComments: Number(item?.total || 0) || 0,
        error: Number(item?.total || 0) > 0 ? '' : normalizeText(item?.error || 'no_comments_collected'),
      })),
  });

  return {
    ...summary,
    ...resume,
    itemsPlanned: allTargets.length,
    targetIds: allTargets,
  };
}
