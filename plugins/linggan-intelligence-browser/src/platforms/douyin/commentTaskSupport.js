function toNonNegativeInt(value, fallback = 0) {
  const num = Number(value);
  if (!Number.isFinite(num) || num < 0) return fallback;
  return Math.floor(num);
}

export function buildDouyinSingleCommentRunPatch({
  stopped = false,
  totalComments = 0,
  note = {},
} = {}) {
  const normalizedTotalComments = toNonNegativeInt(totalComments, 0);
  const contentId = String(note?.contentId || '').trim();
  const platformContentId = String(note?.platformContentId || '').trim();

  return {
    itemsPlanned: 1,
    itemsSucceeded: stopped
      ? (normalizedTotalComments > 0 ? 1 : 0)
      : 1,
    itemsFailed: 0,
    totalComments: normalizedTotalComments,
    contentId,
    targetIds: [platformContentId].filter(Boolean),
  };
}

export function resolveDouyinSingleCommentUiTotal({
  maxTotal = 0,
} = {}) {
  const normalizedMaxTotal = toNonNegativeInt(maxTotal, 0);
  return normalizedMaxTotal > 0 ? normalizedMaxTotal : 0;
}
