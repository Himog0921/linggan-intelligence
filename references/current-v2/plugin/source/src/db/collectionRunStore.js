import db from './index.js';
import {
  buildHeartbeatPatchForRun,
  isTerminalCollectionRunStatus,
} from './collectionRunStatus.js';

function now() {
  return Date.now();
}

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function createCollectionRunId(taskType = 'run') {
  const prefix = normalizeText(taskType || 'run') || 'run';
  const randomPart = globalThis.crypto?.randomUUID?.()
    || `${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36)}`;
  return `${prefix}_${randomPart}`;
}

function normalizeRunRecord(run = {}, { preserveCreatedAt = false } = {}) {
  const normalized = {
    ...run,
    collectionRunId: normalizeText(run.collectionRunId || createCollectionRunId(run.taskType)),
    externalTaskId: normalizeText(run.externalTaskId),
    externalTaskType: normalizeText(run.externalTaskType),
    executorInstanceId: normalizeText(run.executorInstanceId),
    protocolVersion: normalizeText(run.protocolVersion),
    collectionProfile: normalizeText(run.collectionProfile),
    platform: normalizeText(run.platform),
    taskType: normalizeText(run.taskType),
    pageType: normalizeText(run.pageType),
    triggerSource: normalizeText(run.triggerSource),
    status: normalizeText(run.status || 'running') || 'running',
    resultUploadStatus: normalizeText(run.resultUploadStatus || 'local_only') || 'local_only',
    config: normalizeObject(run.config),
    meta: normalizeObject(run.meta),
  };

  if (!preserveCreatedAt || !Number.isFinite(Number(normalized.createdAt))) {
    normalized.createdAt = now();
  } else {
    normalized.createdAt = Number(normalized.createdAt);
  }

  normalized.updatedAt = Number.isFinite(Number(normalized.updatedAt))
    ? Number(normalized.updatedAt)
    : normalized.createdAt;
  normalized.startedAt = Number.isFinite(Number(normalized.startedAt))
    ? Number(normalized.startedAt)
    : normalized.createdAt;
  normalized.finishedAt = Number.isFinite(Number(normalized.finishedAt))
    ? Number(normalized.finishedAt)
    : undefined;
  normalized.lastHeartbeatAt = Number.isFinite(Number(normalized.lastHeartbeatAt))
    ? Number(normalized.lastHeartbeatAt)
    : 0;

  return normalized;
}

export const collectionRunStore = {
  async upsert(run) {
    await db.collectionRuns.put(normalizeRunRecord(run, { preserveCreatedAt: true }));
  },

  async createRun({
    collectionRunId = '',
    externalTaskId = '',
    externalTaskType = '',
    executorInstanceId = '',
    protocolVersion = '',
    collectionProfile = '',
    platform = '',
    taskType = '',
    pageType = '',
    triggerSource = '',
    status = 'running',
    resultUploadStatus = 'local_only',
    lastHeartbeatAt = 0,
    config = {},
    meta = {},
  } = {}) {
    const startedAt = now();
    const run = normalizeRunRecord({
      collectionRunId: collectionRunId || createCollectionRunId(taskType),
      externalTaskId,
      externalTaskType,
      executorInstanceId,
      protocolVersion,
      collectionProfile,
      platform,
      taskType,
      pageType,
      triggerSource,
      status,
      resultUploadStatus,
      lastHeartbeatAt,
      config,
      meta,
      startedAt,
      updatedAt: startedAt,
      createdAt: startedAt,
    });
    await this.upsert(run);
    return run;
  },

  async updateById(collectionRunId, patch = {}) {
    const id = String(collectionRunId || '').trim();
    if (!id) return null;
    const existing = await this.getById(id);
    if (!existing) return null;
    const next = normalizeRunRecord({
      ...existing,
      ...patch,
      collectionRunId: id,
      updatedAt: now(),
      createdAt: existing.createdAt,
    }, { preserveCreatedAt: true });
    await this.upsert(next);
    return next;
  },

  async markDone(collectionRunId, patch = {}) {
    return this.updateById(collectionRunId, {
      ...patch,
      status: 'done',
      finishedAt: now(),
    });
  },

  async markFailed(collectionRunId, error, patch = {}) {
    const message = String(error?.message || error || '').trim();
    return this.updateById(collectionRunId, {
      ...patch,
      status: 'failed',
      error: message,
      finishedAt: now(),
    });
  },

  async markPaused(collectionRunId, patch = {}) {
    return this.updateById(collectionRunId, {
      ...patch,
      status: 'paused',
      finishedAt: undefined,
    });
  },

  async markStopped(collectionRunId, patch = {}) {
    return this.updateById(collectionRunId, {
      ...patch,
      status: 'stopped',
      finishedAt: now(),
    });
  },

  async bindExternalTask(collectionRunId, {
    externalTaskId = '',
    externalTaskType = '',
    executorInstanceId = '',
    protocolVersion = '',
  } = {}) {
    return this.updateById(collectionRunId, {
      externalTaskId,
      externalTaskType,
      executorInstanceId,
      protocolVersion,
    });
  },

  async markHeartbeat(collectionRunId, timestamp = now(), patch = {}) {
    const id = String(collectionRunId || '').trim();
    if (!id) return null;
    const existing = await this.getById(id);
    if (!existing) return null;

    const heartbeatPatch = buildHeartbeatPatchForRun(existing, patch, timestamp);
    if (!heartbeatPatch) return existing;

    return this.updateById(id, heartbeatPatch);
  },

  async markResultUploadStatus(collectionRunId, resultUploadStatus = 'pending', patch = {}) {
    return this.updateById(collectionRunId, {
      ...patch,
      resultUploadStatus,
    });
  },

  async getAll() {
    return db.collectionRuns.orderBy('startedAt').reverse().toArray();
  },

  async getById(collectionRunId) {
    return db.collectionRuns.get(collectionRunId);
  },

  async getLatestByExternalTaskId(externalTaskId) {
    const id = normalizeText(externalTaskId);
    if (!id) return null;
    const runs = await db.collectionRuns
      .where('externalTaskId')
      .equals(id)
      .toArray();
    if (!runs.length) return null;
    return runs.sort((a, b) => Number(b.startedAt || 0) - Number(a.startedAt || 0))[0] || null;
  },

  async getLatestResumableByExternalTaskId(externalTaskId, { taskType = '' } = {}) {
    const run = await this.getLatestByExternalTaskId(externalTaskId);
    if (!run) return null;
    if (taskType && normalizeText(run.taskType) !== normalizeText(taskType)) return null;
    if (isTerminalCollectionRunStatus(run.status)) return null;
    return run;
  },

  async deleteById(collectionRunId) {
    await db.collectionRuns.delete(collectionRunId);
  },
};
