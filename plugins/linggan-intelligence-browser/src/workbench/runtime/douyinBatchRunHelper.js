import { buildBatchResumeCheckpoint } from './batchResume.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeDouyinTargetId(item = {}) {
  return normalizeText(
    item.awemeId
    || item.platformContentId
    || item.videoId
    || item.noteId
    || item.contentId,
  ).replace(/^(dy_|douyin_)/i, '');
}

function normalizeDouyinContentId(item = {}) {
  const direct = normalizeText(item.contentId || item.noteId);
  if (direct.startsWith('dy_')) return direct;

  const targetId = normalizeDouyinTargetId(item);
  return targetId ? `dy_${targetId}` : '';
}

function buildTargetIds(targets = []) {
  return (Array.isArray(targets) ? targets : [])
    .map((item) => normalizeDouyinTargetId(item))
    .filter(Boolean);
}

function buildFailedTargets(results = []) {
  return (Array.isArray(results) ? results : [])
    .filter((item) => !item?.ok)
    .map((item) => ({
      awemeId: normalizeDouyinTargetId(item),
      error: normalizeText(item?.error) || 'failed',
    }))
    .filter((item) => item.awemeId || item.error);
}

function buildContentIds(results = []) {
  return (Array.isArray(results) ? results : [])
    .filter((item) => item?.ok)
    .map((item) => normalizeDouyinContentId(item))
    .filter(Boolean);
}

export function buildDouyinBatchVideosRunPatch({
  targets = [],
  results = [],
} = {}) {
  const targetIds = buildTargetIds(targets);
  const failedTargets = buildFailedTargets(results);

  return {
    itemsPlanned: targetIds.length,
    itemsSucceeded: buildContentIds(results).length,
    itemsFailed: failedTargets.length,
    targetIds,
    contentIds: buildContentIds(results),
    failedTargets,
  };
}

export function buildDouyinBatchVideosProgressPatch({
  targets = [],
  results = [],
  processedCount = 0,
} = {}) {
  const targetIds = buildTargetIds(targets);
  const processedIds = new Set(targetIds.slice(0, Math.max(0, Number(processedCount || 0) || 0)));
  const scopedResults = (Array.isArray(results) ? results : [])
    .filter((item) => processedIds.has(normalizeDouyinTargetId(item)));
  return {
    ...buildDouyinBatchVideosRunPatch({ targets, results: scopedResults }),
    ...buildBatchResumeCheckpoint({
      targetIds,
      processedCount,
      resultStatuses: scopedResults.map((item) => ({
        targetId: normalizeDouyinTargetId(item),
        ok: item?.ok !== false,
        contentId: normalizeDouyinContentId(item),
        error: normalizeText(item?.error),
      })),
    }),
    itemsPlanned: targetIds.length,
    targetIds,
  };
}

export function buildDouyinBatchCommentsRunPatch({
  targets = [],
  results = [],
  totalComments = 0,
} = {}) {
  const targetIds = buildTargetIds(targets);
  const failedTargets = buildFailedTargets(results);

  return {
    itemsPlanned: targetIds.length,
    itemsSucceeded: buildContentIds(results).length,
    itemsFailed: failedTargets.length,
    totalComments: Math.max(0, Number(totalComments || 0) || 0),
    targetIds,
    contentIds: buildContentIds(results),
    failedTargets,
  };
}

export function buildDouyinBatchCommentsProgressPatch({
  targets = [],
  results = [],
  totalComments = 0,
  processedCount = 0,
} = {}) {
  const targetIds = buildTargetIds(targets);
  const processedIds = new Set(targetIds.slice(0, Math.max(0, Number(processedCount || 0) || 0)));
  const scopedResults = (Array.isArray(results) ? results : [])
    .filter((item) => processedIds.has(normalizeDouyinTargetId(item)));
  return {
    ...buildDouyinBatchCommentsRunPatch({ targets, results: scopedResults, totalComments }),
    ...buildBatchResumeCheckpoint({
      targetIds,
      processedCount,
      resultStatuses: scopedResults.map((item) => ({
        targetId: normalizeDouyinTargetId(item),
        ok: item?.ok !== false,
        contentId: normalizeDouyinContentId(item),
        totalComments: Number(item?.totalComments || 0) || 0,
        error: normalizeText(item?.error),
      })),
    }),
    itemsPlanned: targetIds.length,
    targetIds,
  };
}
