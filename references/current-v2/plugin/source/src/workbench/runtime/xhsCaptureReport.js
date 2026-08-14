const PROFILE_SLOTS = Object.freeze({
  list_scan: Object.freeze(['note_list']),
  note_detail: Object.freeze(['note', 'comments']),
  note_full: Object.freeze(['note', 'comments']),
  comment_probe: Object.freeze(['comments']),
  author_profile: Object.freeze(['author', 'note_list']),
  author_links: Object.freeze(['note_links']),
});

const REQUIRED_SLOTS = Object.freeze({
  list_scan: Object.freeze(['note_list']),
  note_detail: Object.freeze(['note']),
  note_full: Object.freeze(['note', 'comments']),
  comment_probe: Object.freeze(['comments']),
  author_profile: Object.freeze(['author']),
  author_links: Object.freeze(['note_links']),
});

const DISCOVERY_SUCCESS_REASON = Object.freeze({
  target_reached: 'limit_reached',
  captured_partial: 'source_exhausted',
  api_partial: 'source_exhausted',
  max_rounds_reached: 'source_exhausted',
  bottom_confirmed: 'source_exhausted',
  stable_no_new: 'source_exhausted',
  no_cards_found: 'source_exhausted',
});

const FAILURE_REASON = Object.freeze({
  target_missing: Object.freeze({ state: 'error', reason: 'target_missing', retryable: false }),
  target_mismatch: Object.freeze({ state: 'error', reason: 'target_missing', retryable: false }),
  login_required: Object.freeze({ state: 'blocked', reason: 'login_required', retryable: false }),
  account_or_platform_blocked: Object.freeze({ state: 'blocked', reason: 'platform_blocked', retryable: false }),
  platform_blocked: Object.freeze({ state: 'blocked', reason: 'platform_blocked', retryable: false }),
  page_data_not_ready: Object.freeze({ state: 'error', reason: 'parser_failed', retryable: false }),
  parser_failed: Object.freeze({ state: 'error', reason: 'parser_failed', retryable: false }),
  network_failed: Object.freeze({ state: 'error', reason: 'network_failed', retryable: true }),
});

function nonNegativeInteger(value) {
  return Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function normalizeReason(value) {
  return String(value || '').trim();
}

function terminalFor({ status, producerReason, failureReason }) {
  if (status === 'done') {
    const reason = DISCOVERY_SUCCESS_REASON[producerReason];
    if (!reason) return null;
    return { state: 'completed', reason, retryable: false };
  }
  if (status === 'stopped') {
    return { state: 'cancelled', reason: 'user_cancelled', retryable: false };
  }
  if (status === 'failed') {
    return FAILURE_REASON[failureReason] ? { ...FAILURE_REASON[failureReason] } : null;
  }
  return null;
}

/**
 * Builds the persisted XHS terminal facts at the workflow boundary.
 * Callers must supply the producer's own reason and reconciled counters. This
 * function deliberately has no record-array input and cannot infer a slot from
 * whether a record happened to be emitted.
 */
export function buildPersistedXhsCaptureReport({
  collectionProfile = '',
  status = '',
  producerReason = '',
  failureReason = '',
  counters = {},
  slotReports = null,
} = {}) {
  const profile = normalizeReason(collectionProfile);
  const slots = PROFILE_SLOTS[profile];
  if (!slots) return null;
  const terminal = terminalFor({
    status: normalizeReason(status),
    producerReason: normalizeReason(producerReason),
    failureReason: normalizeReason(failureReason),
  });
  if (!terminal) return null;

  const captureCounters = {};
  for (const key of ['requested', 'discovered', 'emitted', 'deduplicated', 'failed']) {
    const value = nonNegativeInteger(counters[key]);
    if (value === null) return null;
    captureCounters[key] = value;
  }
  if (captureCounters.discovered !== captureCounters.emitted + captureCounters.deduplicated) {
    return null;
  }

  let persistedSlots;
  if (status === 'done') {
    const byId = new Map(
      (Array.isArray(slotReports) ? slotReports : [])
        .map((slot) => [String(slot?.slotId || '').trim(), slot]),
    );
    if (byId.size !== slots.length || slots.some((slotId) => !byId.has(slotId))) return null;
    persistedSlots = slots.map((slotId) => {
      const slot = byId.get(slotId);
      const slotStatus = String(slot?.status || '').trim();
      const slotReason = slot?.reason === null ? null : String(slot?.reason || '').trim();
      if (!['observed', 'not_applicable'].includes(slotStatus)) return null;
      if ((slotStatus === 'observed' && slotReason !== null)
        || (slotStatus === 'not_applicable' && !slotReason)) return null;
      return { slotId, status: slotStatus, reason: slotReason };
    });
    if (persistedSlots.some((slot) => !slot)) return null;
    if (REQUIRED_SLOTS[profile].some((slotId) => byId.get(slotId)?.status !== 'observed')) return null;
  } else {
    persistedSlots = slots.map((slotId) => ({ slotId, status: 'invalid', reason: terminal.reason }));
  }
  return {
    producer: {
      collectionProfile: profile,
      status: normalizeReason(status),
      reason: normalizeReason(producerReason || failureReason || (status === 'stopped' ? 'user_cancelled' : '')),
    },
    captureTerminal: terminal,
    slotReports: persistedSlots,
    captureCounters,
  };
}

export const XHS_CAPTURE_REPORT_PROFILE_SLOTS = PROFILE_SLOTS;
export const XHS_DISCOVERY_SUCCESS_REASONS = Object.freeze(Object.keys(DISCOVERY_SUCCESS_REASON));

export function buildXhsBatchNoteCaptureReport({
  collectionProfile = '',
  status = '',
  producerReason = '',
  includeComments = false,
  patch = {},
} = {}) {
  const emitted = Math.max(0, Number(patch.itemsSucceeded || 0) || 0)
    + Math.max(0, Number(patch.totalComments || 0) || 0);
  const slotReports = collectionProfile === 'list_scan'
    ? [{ slotId: 'note_list', status: 'observed', reason: null }]
    : [
        { slotId: 'note', status: 'observed', reason: null },
        includeComments
          ? { slotId: 'comments', status: 'observed', reason: null }
          : { slotId: 'comments', status: 'not_applicable', reason: 'not_requested_by_plan' },
      ];
  return buildPersistedXhsCaptureReport({
    collectionProfile,
    status,
    producerReason,
    slotReports,
    counters: {
      requested: Math.max(0, Number(patch.itemsPlanned || 0) || 0),
      discovered: emitted,
      emitted,
      deduplicated: 0,
      failed: Math.max(0, Number(patch.itemsFailed || 0) || 0),
    },
  });
}

export function buildXhsBatchCommentCaptureReport({
  collectionProfile = '',
  status = '',
  patch = {},
} = {}) {
  const emitted = Math.max(0, Number(patch.totalComments || 0) || 0);
  return buildPersistedXhsCaptureReport({
    collectionProfile,
    status,
    producerReason: 'target_reached',
    slotReports: [{ slotId: 'comments', status: 'observed', reason: null }],
    counters: {
      requested: Math.max(0, Number(patch.itemsPlanned || 0) || 0),
      discovered: emitted,
      emitted,
      deduplicated: 0,
      failed: Math.max(0, Number(patch.itemsFailed || 0) || 0),
    },
  });
}
