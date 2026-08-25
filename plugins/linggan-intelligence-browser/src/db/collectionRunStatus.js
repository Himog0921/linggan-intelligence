function normalizeCollectionRunStatus(value = '') {
  return String(value || '').trim().toLowerCase();
}

const TERMINAL_COLLECTION_RUN_STATUSES = new Set(['done', 'stopped', 'failed']);

export function isTerminalCollectionRunStatus(value = '') {
  return TERMINAL_COLLECTION_RUN_STATUSES.has(normalizeCollectionRunStatus(value));
}

export function buildHeartbeatPatchForRun(existingRun = {}, patch = {}, timestamp = Date.now()) {
  if (isTerminalCollectionRunStatus(existingRun?.status)) {
    return null;
  }

  const heartbeatPatch = patch && typeof patch === 'object' && !Array.isArray(patch)
    ? { ...patch }
    : {};
  delete heartbeatPatch.current;
  delete heartbeatPatch.total;

  const normalizedTimestamp = Number.isFinite(Number(timestamp)) ? Number(timestamp) : Date.now();
  return {
    ...heartbeatPatch,
    lastHeartbeatAt: normalizedTimestamp,
  };
}
