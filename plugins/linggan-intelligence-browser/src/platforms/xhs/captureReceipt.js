export const XHS_DETAIL_COMMENT_CAP = 30;

function text(value = '') {
  return String(value || '').trim();
}

function nonNegative(value) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? Math.floor(number) : null;
}

function knownPublicCommentCount(note = {}) {
  const direct = nonNegative(note?.publicCommentCount);
  if (direct !== null) return direct;
  if (note?.publicCommentCountKnown !== true) return null;
  return nonNegative(note?.comments ?? note?.commentCount);
}

export function normalizeXhsDetailCommentLimit(value, fallback = XHS_DETAIL_COMMENT_CAP) {
  const requested = nonNegative(value);
  const defaultValue = nonNegative(fallback) ?? XHS_DETAIL_COMMENT_CAP;
  return Math.min(requested === null || requested === 0 ? defaultValue : requested, XHS_DETAIL_COMMENT_CAP);
}

export function buildXhsDetailCaptureReceipt({
  note = {},
  commentResult = null,
  requestedCommentLimit = XHS_DETAIL_COMMENT_CAP,
} = {}) {
  const comments = commentResult && typeof commentResult === 'object' ? commentResult : {};
  const actual = nonNegative(comments.total) ?? (Array.isArray(comments.comments) ? comments.comments.length : 0);
  const publicCount = knownPublicCommentCount(note);
  const requested = normalizeXhsDetailCommentLimit(requestedCommentLimit);
  const explicitEmptyState = comments.explicitEmptyState === true;
  const targetReached = actual >= requested || (publicCount !== null && actual >= Math.min(publicCount, requested));
  const state = explicitEmptyState
    ? 'explicit_empty_state'
    : (targetReached ? 'target_reached' : 'partial');
  const stopReason = text(comments.stopReason)
    || (explicitEmptyState ? 'explicit_empty_state' : (targetReached ? 'comment_cap_reached' : 'unknown'));
  const mediaSlots = [
    ...(Array.isArray(note.images) ? note.images : []),
    ...(note.video ? [note.video] : []),
    ...(Array.isArray(note.livePhotoStreams) ? note.livePhotoStreams : []),
  ].filter(Boolean);

  return {
    kind: 'xhs_note_detail_package',
    noteId: text(note.noteId || note.platformContentId || note.contentId),
    detailSource: text(note.dataSource) || 'unknown',
    media: {
      observedSlots: mediaSlots.length,
      acquisition: 'not_requested',
    },
    comments: {
      cap: XHS_DETAIL_COMMENT_CAP,
      requested,
      actual,
      publicCount,
      state,
      stopReason,
      ordering: text(comments.ordering) || 'unknown',
      repliesPreserved: true,
    },
  };
}

export function buildXhsSearchSurfaceReceipt({
  requestedLimit = 0,
  loadedCount = 0,
  stopReason = 'unknown',
  activeFilters = {},
  suggestionCount = 0,
} = {}) {
  return {
    kind: 'xhs_search_surface',
    requestedLimit: nonNegative(requestedLimit) ?? 0,
    loadedCount: nonNegative(loadedCount) ?? 0,
    stopReason: text(stopReason) || 'unknown',
    activeFilters: activeFilters && typeof activeFilters === 'object' && !Array.isArray(activeFilters)
      ? activeFilters
      : {},
    suggestionCount: nonNegative(suggestionCount) ?? 0,
    // A rendered surface and a task quota never prove platform search exhaustion.
    resultSetComplete: false,
  };
}
