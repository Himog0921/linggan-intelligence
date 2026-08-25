export const WORKBENCH_EXECUTOR_INSTANCE_STORAGE_KEY = 'workbenchExecutorInstanceId';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function resolveDefaultStorageArea() {
  return globalThis.chrome?.storage?.local || null;
}

function createGeneratedExecutorId(randomUUID) {
  const rawId = typeof randomUUID === 'function'
    ? randomUUID()
    : globalThis.crypto?.randomUUID?.() || `${Date.now().toString(36)}_${Math.random().toString(36).slice(2)}`;
  const normalized = normalizeText(rawId);
  return normalized.startsWith('plugin_') ? normalized : `plugin_${normalized}`;
}

export function createExecutorIdentity({
  storageArea = resolveDefaultStorageArea(),
  storageKey = WORKBENCH_EXECUTOR_INSTANCE_STORAGE_KEY,
  randomUUID = globalThis.crypto?.randomUUID?.bind(globalThis.crypto),
} = {}) {
  let cachedExecutorInstanceId = '';
  let pendingExecutorInstanceId = null;

  async function readStoredExecutorId() {
    if (!storageArea || typeof storageArea.get !== 'function') return '';
    try {
      const result = await storageArea.get(storageKey);
      return normalizeText(result?.[storageKey]);
    } catch {
      return '';
    }
  }

  async function writeStoredExecutorId(executorInstanceId) {
    if (!storageArea || typeof storageArea.set !== 'function') return;
    try {
      await storageArea.set({ [storageKey]: executorInstanceId });
    } catch {
      // Storage errors should not block task execution; keep the in-memory value.
    }
  }

  async function getExecutorInstanceId() {
    if (cachedExecutorInstanceId) return cachedExecutorInstanceId;
    if (pendingExecutorInstanceId) return pendingExecutorInstanceId;

    pendingExecutorInstanceId = (async () => {
      const stored = await readStoredExecutorId();
      if (stored) {
        cachedExecutorInstanceId = stored;
        return stored;
      }

      const generated = createGeneratedExecutorId(randomUUID);
      cachedExecutorInstanceId = generated;
      await writeStoredExecutorId(generated);
      return generated;
    })();

    try {
      return await pendingExecutorInstanceId;
    } finally {
      pendingExecutorInstanceId = null;
    }
  }

  return {
    getExecutorInstanceId,
  };
}

export const executorIdentity = createExecutorIdentity();
export const getPersistentExecutorInstanceId = executorIdentity.getExecutorInstanceId;
