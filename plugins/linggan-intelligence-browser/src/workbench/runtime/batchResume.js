export const BATCH_RESUME_CHECKPOINT_VERSION = 1;

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeComparableId(value = '') {
  return normalizeText(value)
    .replace(/^(xhs_|dy_|douyin_)/i, '')
    .toLowerCase();
}

function normalizeTargetIds(targetIds = []) {
  return (Array.isArray(targetIds) ? targetIds : [])
    .map((value) => normalizeText(value))
    .filter(Boolean);
}

function normalizeResultStatuses(resultStatuses = []) {
  return (Array.isArray(resultStatuses) ? resultStatuses : [])
    .map((item) => {
      if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
      const targetId = normalizeText(item.targetId || item.noteId || item.awemeId || item.contentId);
      if (!targetId) return null;
      const hasError = Boolean(normalizeText(item.error));
      return {
        targetId,
        ok: item.ok === undefined ? !hasError : item.ok !== false,
        contentId: normalizeText(item.contentId || item.noteId || item.videoId),
        error: normalizeText(item.error),
        totalComments: Number.isFinite(Number(item.totalComments)) ? Number(item.totalComments) : undefined,
      };
    })
    .filter(Boolean);
}

function clampIndex(value, max) {
  const num = Number(value);
  if (!Number.isFinite(num) || num < 0) return 0;
  return Math.min(Math.floor(num), Math.max(0, max));
}

function inferProcessedCountFromRun(runRecord = {}, targetIds = []) {
  const comparableTargets = targetIds.map(normalizeComparableId).filter(Boolean);
  if (!comparableTargets.length) return 0;

  const done = new Set();
  for (const contentId of Array.isArray(runRecord.contentIds) ? runRecord.contentIds : []) {
    const normalized = normalizeComparableId(contentId);
    if (normalized) done.add(normalized);
  }
  for (const failed of Array.isArray(runRecord.failedTargets) ? runRecord.failedTargets : []) {
    const normalized = normalizeComparableId(
      failed?.targetId
      || failed?.noteId
      || failed?.awemeId
      || failed?.contentId,
    );
    if (normalized) done.add(normalized);
  }

  let processed = 0;
  for (const targetId of comparableTargets) {
    if (!done.has(targetId)) break;
    processed += 1;
  }
  return processed;
}

export function buildBatchResumeCheckpoint({
  targetIds = [],
  processedCount = 0,
  resultStatuses = [],
  updatedAt = Date.now(),
} = {}) {
  const normalizedTargets = normalizeTargetIds(targetIds);
  const nextIndex = clampIndex(processedCount, normalizedTargets.length);
  const targetSet = new Set(normalizedTargets.map(normalizeComparableId));
  const statuses = normalizeResultStatuses(resultStatuses)
    .filter((item) => targetSet.has(normalizeComparableId(item.targetId)))
    .slice(0, nextIndex);

  return {
    processedCount: nextIndex,
    nextIndex,
    resumeCheckpoint: {
      version: BATCH_RESUME_CHECKPOINT_VERSION,
      processedCount: nextIndex,
      nextIndex,
      targetIds: normalizedTargets,
      resultStatuses: statuses,
      updatedAt: Number.isFinite(Number(updatedAt)) ? Number(updatedAt) : Date.now(),
    },
  };
}

export function resolveBatchResumeState({
  runRecord = null,
  targets = [],
  getTargetId = (item) => item,
} = {}) {
  const originalTargets = Array.isArray(targets) ? targets : [];
  const targetById = new Map();
  for (const target of originalTargets) {
    const id = normalizeText(getTargetId(target));
    if (!id) continue;
    targetById.set(normalizeComparableId(id), target);
  }

  const checkpoint = runRecord?.resumeCheckpoint && typeof runRecord.resumeCheckpoint === 'object'
    ? runRecord.resumeCheckpoint
    : {};
  const previousTargetIds = normalizeTargetIds(
    Array.isArray(checkpoint.targetIds) && checkpoint.targetIds.length
      ? checkpoint.targetIds
      : runRecord?.targetIds,
  );

  const orderedTargets = [];
  const seen = new Set();
  for (const targetId of previousTargetIds) {
    const comparable = normalizeComparableId(targetId);
    const target = targetById.get(comparable);
    if (!target || seen.has(comparable)) continue;
    orderedTargets.push(target);
    seen.add(comparable);
  }
  for (const target of originalTargets) {
    const comparable = normalizeComparableId(getTargetId(target));
    if (!comparable || seen.has(comparable)) continue;
    orderedTargets.push(target);
    seen.add(comparable);
  }

  const orderedTargetIds = orderedTargets.map(getTargetId).map(normalizeText).filter(Boolean);
  const inferredProcessed = inferProcessedCountFromRun(runRecord || {}, orderedTargetIds);
  const processedCount = Math.max(
    clampIndex(checkpoint.nextIndex, orderedTargetIds.length),
    clampIndex(checkpoint.processedCount, orderedTargetIds.length),
    clampIndex(runRecord?.nextIndex, orderedTargetIds.length),
    clampIndex(runRecord?.processedCount, orderedTargetIds.length),
    inferredProcessed,
  );
  const nextIndex = clampIndex(processedCount, orderedTargetIds.length);

  return {
    targets: orderedTargets,
    targetIds: orderedTargetIds,
    nextIndex,
    processedCount: nextIndex,
    completedTargetIds: orderedTargetIds.slice(0, nextIndex),
    resultStatuses: normalizeResultStatuses(checkpoint.resultStatuses),
    resumed: nextIndex > 0,
  };
}
