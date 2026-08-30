export const XHS_DETAIL_COMMENT_CAP = 30;

export const XHS_COMMENT_COLLECTION_SCOPE = Object.freeze({
  DETAIL_WINDOW: 'detail_window',
  ALL_PUBLIC_COMMENTS: 'all_public_comments',
});

export const XHS_COMMENT_COLLECTION_STATE = Object.freeze({
  COMPLETE: 'complete',
  PARTIAL: 'partial',
  INVALID_TARGET: 'invalid_target',
});

export const XHS_COMMENT_ANALYSIS_USABILITY = Object.freeze({
  USABLE: 'usable',
  EMPTY: 'empty',
  NOT_USABLE: 'not_usable',
});

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

function collectionScope(maxTotal = 0) {
  return nonNegative(maxTotal) > 0
    ? XHS_COMMENT_COLLECTION_SCOPE.DETAIL_WINDOW
    : XHS_COMMENT_COLLECTION_SCOPE.ALL_PUBLIC_COMMENTS;
}

function terminalCollectionStop(stopReason = '') {
  return new Set([
    'risk_control',
    'manual_stop',
    'comment_collection_failed',
    'target_identity_mismatch',
    'wrong_content',
    'api_unobserved',
  ]).has(text(stopReason));
}

// This is a producer fact, not an analysis gate.  It answers the deliberately mundane
// question the operator needs answered: for the requested comment scope, how many did the
// page show and how many unique comments did this Attempt actually bring back?
export function buildXhsCommentCollectionReceipt({
  noteId = '',
  maxTotal = 0,
  requestedLimit = null,
  publicCommentCount = null,
  actual = 0,
  explicitEmptyState = false,
  stopReason = '',
  targetIdentity = 'matched',
} = {}) {
  const scope = collectionScope(maxTotal);
  const received = nonNegative(actual) ?? 0;
  const pageCount = nonNegative(publicCommentCount);
  const requested = scope === XHS_COMMENT_COLLECTION_SCOPE.DETAIL_WINDOW
    ? normalizeXhsDetailCommentLimit(requestedLimit ?? maxTotal)
    : null;
  const expected = scope === XHS_COMMENT_COLLECTION_SCOPE.DETAIL_WINDOW
    ? (pageCount === null ? requested : Math.min(pageCount, requested))
    : (pageCount === null && explicitEmptyState ? 0 : pageCount);
  const reason = text(stopReason)
    || (explicitEmptyState ? 'explicit_empty_state' : 'unknown');
  const identityMatched = targetIdentity === 'matched';
  const identityMismatched = targetIdentity === 'mismatched';
  const complete = identityMatched
    && !terminalCollectionStop(reason)
    && expected !== null
    && received === expected;
  const state = identityMismatched
    ? XHS_COMMENT_COLLECTION_STATE.INVALID_TARGET
    : (complete ? XHS_COMMENT_COLLECTION_STATE.COMPLETE : XHS_COMMENT_COLLECTION_STATE.PARTIAL);
  const analysisUsability = !identityMatched
    ? XHS_COMMENT_ANALYSIS_USABILITY.NOT_USABLE
    : (received > 0 ? XHS_COMMENT_ANALYSIS_USABILITY.USABLE : XHS_COMMENT_ANALYSIS_USABILITY.EMPTY);

  return {
    version: 1,
    noteId: text(noteId),
    scope,
    ...(requested === null ? {} : { requestedLimit: requested }),
    pageCommentCount: pageCount,
    expectedCount: expected,
    uniqueCollectedCount: received,
    state,
    analysisUsability,
    targetIdentity,
    stopReason: reason,
  };
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
  const commentCollection = buildXhsCommentCollectionReceipt({
    noteId: note.noteId || note.platformContentId || note.contentId,
    maxTotal: requested,
    requestedLimit: requested,
    publicCommentCount: publicCount,
    actual,
    explicitEmptyState,
    stopReason: comments.stopReason,
    targetIdentity: comments.targetIdentity || 'matched',
  });
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
      ...commentCollection,
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
