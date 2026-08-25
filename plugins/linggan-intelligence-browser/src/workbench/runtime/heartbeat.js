function normalizeText(value = '') {
  return String(value || '').trim();
}

function buildHeartbeatPatch(patch = {}) {
  const next = {};
  const status = normalizeText(patch.taskState || patch.status);
  const stage = normalizeText(patch.stage || patch.phase);
  const message = normalizeText(patch.message);

  if (status) next.status = status;
  if (stage) next.stage = stage;
  if (message) next.message = message;

  return next;
}

export function createCollectionRunHeartbeatReporter({
  collectionRunStore,
  intervalMs = 15000,
  now = () => Date.now(),
} = {}) {
  const lastHeartbeatByRun = new Map();

  return {
    async report(collectionRunId, patch = {}) {
      const runId = normalizeText(collectionRunId);
      if (!runId || !collectionRunStore?.markHeartbeat) return false;

      const timestamp = Number.isFinite(Number(patch.heartbeatAt))
        ? Number(patch.heartbeatAt)
        : Number(now());
      const force = Boolean(patch.force);
      const lastHeartbeatAt = Number(lastHeartbeatByRun.get(runId) || 0);

      if (!force && lastHeartbeatAt > 0 && timestamp - lastHeartbeatAt < Number(intervalMs || 0)) {
        return false;
      }

      lastHeartbeatByRun.set(runId, timestamp);
      await collectionRunStore.markHeartbeat(runId, timestamp, buildHeartbeatPatch(patch));
      return true;
    },
  };
}

export function createCollectionRunHeartbeatLoop({
  reporter,
  intervalMs = 30000,
  setIntervalFn = (handler, delay) => setInterval(handler, delay),
  clearIntervalFn = (timerId) => clearInterval(timerId),
} = {}) {
  let timerId = null;

  return {
    start(collectionRunId, getPatch = () => ({})) {
      const runId = normalizeText(collectionRunId);
      if (!runId || !reporter?.report || timerId) return;

      timerId = setIntervalFn(() => Promise.resolve(
        reporter.report(runId, {
          ...(typeof getPatch === 'function' ? (getPatch() || {}) : {}),
          force: true,
        }),
      ).catch(() => {}), Number(intervalMs || 3000));
    },

    stop() {
      if (!timerId) return;
      clearIntervalFn(timerId);
      timerId = null;
    },
  };
}
