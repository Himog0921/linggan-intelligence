const DEFAULT_EXECUTION_ACCOUNT_LOCK_STORAGE_KEY = 'workbenchExecutionAccountLocks';
const DEFAULT_EXECUTION_ACCOUNT_LOCK_TTL_MS = 30 * 60 * 1000;

function normalizeString(value = '') {
  return String(value || '').trim();
}

function toFiniteNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function deepClone(value) {
  if (!value || typeof value !== 'object') return value;
  try {
    return structuredClone(value);
  } catch {
    return JSON.parse(JSON.stringify(value));
  }
}

function normalizeLockSnapshot(value = {}) {
  const source = value && typeof value === 'object' && !Array.isArray(value) ? value : {};
  const locks = source.locks && typeof source.locks === 'object' && !Array.isArray(source.locks)
    ? source.locks
    : {};
  return { locks: deepClone(locks) || {} };
}

function buildLockKey(platform = '', accountId = '') {
  const normalizedPlatform = normalizeString(platform);
  const normalizedAccountId = normalizeString(accountId);
  return normalizedPlatform && normalizedAccountId ? `${normalizedPlatform}:${normalizedAccountId}` : '';
}

function pruneExpiredLocks(snapshot = {}, now = Date.now()) {
  const normalized = normalizeLockSnapshot(snapshot);
  for (const [key, lock] of Object.entries(normalized.locks)) {
    const expiresAtMs = toFiniteNumber(lock?.expiresAtMs, 0);
    if (!expiresAtMs || expiresAtMs <= now) {
      delete normalized.locks[key];
    }
  }
  return normalized;
}

async function readSnapshot(store) {
  if (!store?.read) return { locks: {} };
  return normalizeLockSnapshot(await store.read());
}

async function writeSnapshot(store, snapshot) {
  if (!store?.write) return normalizeLockSnapshot(snapshot);
  return store.write(normalizeLockSnapshot(snapshot));
}

export function createExecutionAccountLockMemoryStore(initial = null) {
  let snapshot = normalizeLockSnapshot(initial);
  return {
    async read() {
      return deepClone(snapshot);
    },
    async write(next) {
      snapshot = normalizeLockSnapshot(next);
      return deepClone(snapshot);
    },
    async clear() {
      snapshot = { locks: {} };
    },
  };
}

export function createExecutionAccountLockStorageStore({
  storageArea = globalThis.chrome?.storage?.session || globalThis.chrome?.storage?.local,
  storageKey = DEFAULT_EXECUTION_ACCOUNT_LOCK_STORAGE_KEY,
} = {}) {
  return {
    async read() {
      if (!storageArea?.get) return { locks: {} };
      const data = await storageArea.get(storageKey);
      return normalizeLockSnapshot(data?.[storageKey]);
    },
    async write(next) {
      const snapshot = normalizeLockSnapshot(next);
      if (storageArea?.set) {
        await storageArea.set({ [storageKey]: snapshot });
      }
      return deepClone(snapshot);
    },
    async clear() {
      if (storageArea?.remove) {
        await storageArea.remove(storageKey);
        return;
      }
      if (storageArea?.set) {
        await storageArea.set({ [storageKey]: { locks: {} } });
      }
    },
  };
}

export function createExecutionAccountLockManager({
  store = createExecutionAccountLockMemoryStore(),
  now = Date.now,
  ttlMs = DEFAULT_EXECUTION_ACCOUNT_LOCK_TTL_MS,
} = {}) {
  let operationQueue = Promise.resolve();

  function runExclusive(operation) {
    const next = operationQueue.then(operation, operation);
    operationQueue = next.catch(() => {});
    return next;
  }

  function getNow() {
    return typeof now === 'function' ? now() : Date.now();
  }

  async function acquire({
    platform = '',
    accountId = '',
    taskId = '',
    leaseToken = '',
    attemptId = '',
    ttlMs: overrideTtlMs,
  } = {}) {
    const normalizedPlatform = normalizeString(platform);
    const normalizedAccountId = normalizeString(accountId);
    const normalizedTaskId = normalizeString(taskId);
    const key = buildLockKey(normalizedPlatform, normalizedAccountId);
    if (!key || !normalizedTaskId) {
      return { acquired: true, skipped: true };
    }

    return runExclusive(async () => {
      const currentTime = getNow();
      const snapshot = pruneExpiredLocks(await readSnapshot(store), currentTime);
      const existing = snapshot.locks[key] || null;
      if (existing && normalizeString(existing.taskId) !== normalizedTaskId) {
        return {
          acquired: false,
          reasonCode: 'account_busy',
          reasonMessage: '同一账号正在执行另一个采集任务',
          retryAfterMs: Math.max(0, toFiniteNumber(existing.expiresAtMs, currentTime) - currentTime),
          existingTaskId: normalizeString(existing.taskId),
        };
      }

      const resolvedTtlMs = Math.max(1000, toFiniteNumber(overrideTtlMs, ttlMs));
      snapshot.locks[key] = {
        key,
        platform: normalizedPlatform,
        accountId: normalizedAccountId,
        taskId: normalizedTaskId,
        leaseToken: normalizeString(leaseToken),
        attemptId: normalizeString(attemptId),
        acquiredAtMs: currentTime,
        expiresAtMs: currentTime + resolvedTtlMs,
      };
      await writeSnapshot(store, snapshot);
      return { acquired: true, expiresAtMs: snapshot.locks[key].expiresAtMs };
    });
  }

  async function release({ platform = '', accountId = '', taskId = '' } = {}) {
    const key = buildLockKey(platform, accountId);
    if (!key) return { released: false };
    return runExclusive(async () => {
      const snapshot = pruneExpiredLocks(await readSnapshot(store), getNow());
      const existing = snapshot.locks[key] || null;
      if (!existing) {
        await writeSnapshot(store, snapshot);
        return { released: false };
      }
      const normalizedTaskId = normalizeString(taskId);
      if (normalizedTaskId && normalizeString(existing.taskId) !== normalizedTaskId) {
        return { released: false, existingTaskId: normalizeString(existing.taskId) };
      }
      delete snapshot.locks[key];
      await writeSnapshot(store, snapshot);
      return { released: true };
    });
  }

  async function snapshot() {
    return runExclusive(async () => pruneExpiredLocks(await readSnapshot(store), getNow()));
  }

  return {
    acquire,
    release,
    snapshot,
  };
}
