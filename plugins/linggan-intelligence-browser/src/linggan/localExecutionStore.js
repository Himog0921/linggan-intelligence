import db from '../db/index.js';

// Durable browser-local staging for pause/resume. It is deliberately not a
// Linggan Evidence store and is never uploaded as an alternative result path.
function text(value = '') { return String(value || '').trim(); }
function now() { return Date.now(); }
function createId(kind = 'local_execution') {
  const random = globalThis.crypto?.randomUUID?.() || `${Math.random().toString(36).slice(2)}${now().toString(36)}`;
  return `${text(kind) || 'local_execution'}_${random}`;
}
function terminal(status = '') { return new Set(['done', 'stopped', 'failed']).has(text(status).toLowerCase()); }
function normalize(run = {}, preserveCreatedAt = false) {
  const createdAt = preserveCreatedAt && Number.isFinite(Number(run.createdAt)) ? Number(run.createdAt) : now();
  return {
    ...run,
    collectionRunId: text(run.collectionRunId || createId(run.taskType)),
    platform: text(run.platform), taskType: text(run.taskType), pageType: text(run.pageType), triggerSource: text(run.triggerSource),
    status: text(run.status || 'running') || 'running', resultUploadStatus: text(run.resultUploadStatus || 'local_staging') || 'local_staging',
    config: run.config && typeof run.config === 'object' && !Array.isArray(run.config) ? run.config : {},
    meta: run.meta && typeof run.meta === 'object' && !Array.isArray(run.meta) ? run.meta : {},
    createdAt, updatedAt: now(), startedAt: Number.isFinite(Number(run.startedAt)) ? Number(run.startedAt) : createdAt,
    finishedAt: Number.isFinite(Number(run.finishedAt)) ? Number(run.finishedAt) : undefined,
    lastHeartbeatAt: Number.isFinite(Number(run.lastHeartbeatAt)) ? Number(run.lastHeartbeatAt) : 0,
  };
}

export const localExecutionStore = {
  async createRun(input = {}) {
    const run = normalize({ ...input, collectionRunId: input.collectionRunId || createId(input.taskType), status: input.status || 'running' });
    await db.collectionRuns.put(run);
    return run;
  },
  async getById(id = '') { return db.collectionRuns.get(text(id)); },
  async getLatestByExternalTaskId(externalTaskId = '') {
    const id = text(externalTaskId); if (!id) return null;
    const runs = await db.collectionRuns.where('externalTaskId').equals(id).toArray();
    return runs.sort((a, b) => Number(b.startedAt || 0) - Number(a.startedAt || 0))[0] || null;
  },
  async getLatestResumableByExternalTaskId(externalTaskId = '', { taskType = '' } = {}) {
    const run = await this.getLatestByExternalTaskId(externalTaskId);
    return run && (!taskType || text(run.taskType) === text(taskType)) && !terminal(run.status) ? run : null;
  },
  async updateById(id = '', patch = {}) {
    const current = await this.getById(id); if (!current) return null;
    const next = normalize({ ...current, ...patch, collectionRunId: text(id), createdAt: current.createdAt }, true);
    await db.collectionRuns.put(next); return next;
  },
  async markDone(id, patch = {}) { return this.updateById(id, { ...patch, status: 'done', finishedAt: now() }); },
  async markStopped(id, patch = {}) { return this.updateById(id, { ...patch, status: 'stopped', finishedAt: now() }); },
  async markPaused(id, patch = {}) { return this.updateById(id, { ...patch, status: 'paused', finishedAt: undefined }); },
  async markFailed(id, error, patch = {}) { return this.updateById(id, { ...patch, status: 'failed', error: text(error?.message || error), finishedAt: now() }); },
  async markHeartbeat(id, timestamp = now(), patch = {}) {
    const current = await this.getById(id); if (!current || terminal(current.status)) return current;
    return this.updateById(id, { ...patch, lastHeartbeatAt: Number.isFinite(Number(timestamp)) ? Number(timestamp) : now() });
  },
};

export function isTerminalLocalExecutionStatus(status = '') { return terminal(status); }
