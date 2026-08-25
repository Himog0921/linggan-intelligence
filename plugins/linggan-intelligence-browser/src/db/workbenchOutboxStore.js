import Dexie from 'dexie';
import db from './index.js';

const IN_FLIGHT_TIMEOUT_MS = 5 * 60 * 1000;

function now() {
  return Date.now();
}

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function retryDelayMs(attemptCount = 0) {
  const attempt = Math.max(1, Number(attemptCount || 0));
  if (attempt <= 1) return 1000;
  if (attempt === 2) return 2000;
  if (attempt === 3) return 5000;
  if (attempt === 4) return 15000;
  return 60000;
}

async function recoverStaleInFlightRows({ now: nowMs = now(), limit = 50 } = {}) {
  const maxLimit = Math.max(1, Number(limit || 50));
  const staleRows = await db.workbenchOutbox
    .where('[status+nextAttemptAt+createdAt]')
    .between(['in_flight', Dexie.minKey, Dexie.minKey], ['in_flight', nowMs, Dexie.maxKey])
    .limit(maxLimit)
    .toArray();

  await Promise.all(staleRows.map(async (row) => {
    const attemptCount = Number(row.attemptCount || 0) + 1;
    await db.workbenchOutbox.update(row.id, {
      status: 'failed',
      attemptCount,
      errorMessage: 'in_flight_timeout',
      nextAttemptAt: nowMs,
      updatedAt: nowMs,
    });
  }));

  return staleRows.length;
}

function normalizeOutboxRow(item = {}) {
  const idempotencyKey = normalizeText(item.idempotencyKey || item.id);
  return {
    id: normalizeText(item.id || idempotencyKey),
    taskId: normalizeText(item.taskId),
    pluginRunId: normalizeText(item.pluginRunId),
    idempotencyKey,
    kind: normalizeText(item.kind),
    status: normalizeText(item.status || 'pending') || 'pending',
    payload: normalizeObject(item.payload),
    snapshot: item.snapshot && typeof item.snapshot === 'object' && !Array.isArray(item.snapshot)
      ? item.snapshot
      : null,
    // 2026-07-13 事故根因：此前 normalize 重建对象时丢掉 executionContext，
    // 冻结的租约身份（attemptId/leaseToken/leaseEpoch）在真实持久化边界消失，
    // 严格校验随后在发 HTTP 前把记录判成 failed_terminal。该字段必须原样持久化。
    executionContext: item.executionContext
      && typeof item.executionContext === 'object'
      && !Array.isArray(item.executionContext)
      ? { ...item.executionContext }
      : null,
    sequence: Number.isFinite(Number(item.sequence)) ? Math.floor(Number(item.sequence)) : 0,
    attemptCount: Number.isFinite(Number(item.attemptCount)) ? Math.floor(Number(item.attemptCount)) : 0,
    nextAttemptAt: Number.isFinite(Number(item.nextAttemptAt)) ? Number(item.nextAttemptAt) : 0,
    errorMessage: normalizeText(item.errorMessage),
    createdAt: Number.isFinite(Number(item.createdAt)) ? Number(item.createdAt) : now(),
    updatedAt: now(),
  };
}

async function getByKey(idempotencyKey = '') {
  const key = normalizeText(idempotencyKey);
  if (!key) return null;
  return db.workbenchOutbox.where('idempotencyKey').equals(key).first();
}

export const workbenchOutboxStore = {
  async enqueue(item = {}) {
    const row = normalizeOutboxRow(item);
    if (!row.idempotencyKey || !row.taskId || !row.pluginRunId || !row.kind) {
      throw new Error('invalid_workbench_outbox_item');
    }
    try {
      await db.workbenchOutbox.put(row);
      return row;
    } catch (error) {
      if (error?.name === 'ConstraintError' || /constraint/i.test(String(error?.message || error))) {
        const existing = await getByKey(row.idempotencyKey);
        return existing || row;
      }
      throw error;
    }
  },

  async listPending({ limit = 10, now: nowMs = now() } = {}) {
    const maxLimit = Math.max(1, Number(limit || 10));
    await recoverStaleInFlightRows({ now: nowMs, limit: maxLimit });
    const pending = await db.workbenchOutbox
      .where('[status+nextAttemptAt+createdAt]')
      .between(['pending', Dexie.minKey, Dexie.minKey], ['pending', nowMs, Dexie.maxKey])
      .limit(maxLimit)
      .toArray();
    const failed = await db.workbenchOutbox
      .where('[status+nextAttemptAt+createdAt]')
      .between(['failed', Dexie.minKey, Dexie.minKey], ['failed', nowMs, Dexie.maxKey])
      .limit(maxLimit)
      .toArray();
    return pending
      .concat(failed)
      .sort((a, b) => Number(a.createdAt || 0) - Number(b.createdAt || 0))
      .slice(0, maxLimit);
  },

  async recoverStaleInFlight(options = {}) {
    return recoverStaleInFlightRows(options);
  },

  async countUnsent({ now: nowMs = now() } = {}) {
    await recoverStaleInFlightRows({ now: nowMs, limit: 50 });
    // 死信（failed_terminal）不再计入"待发送"：它们永远不会被自动重发，
    // 混进计数会让操作者误以为等网络重试即可（2026-07-13 事故的 2144 计数教训）。
    const statuses = ['pending', 'failed', 'in_flight'];
    const counts = await Promise.all(
      statuses.map((status) => db.workbenchOutbox.where('status').equals(status).count())
    );
    return counts.reduce((sum, count) => sum + Number(count || 0), 0);
  },

  // 分状态账本：传输态（pending/inFlight/retryable）与处置态（deadLetter）分开呈现。
  // retryable 对应存储值 failed，deadLetter 对应存储值 failed_terminal——只做展示语义
  // 映射，不迁移已有行（工位上的存量死信是恢复证据）。
  async countsByStatus({ now: nowMs = now() } = {}) {
    await recoverStaleInFlightRows({ now: nowMs, limit: 50 });
    const [pending, inFlight, retryable, deadLetter, acked] = await Promise.all([
      db.workbenchOutbox.where('status').equals('pending').count(),
      db.workbenchOutbox.where('status').equals('in_flight').count(),
      db.workbenchOutbox.where('status').equals('failed').count(),
      db.workbenchOutbox.where('status').equals('failed_terminal').count(),
      db.workbenchOutbox.where('status').equals('acked').count(),
    ]);
    const oldestCreatedAt = async (statuses) => {
      const rows = await Promise.all(statuses.map((status) => (
        db.workbenchOutbox
          .where('[status+createdAt]')
          .between([status, Dexie.minKey], [status, Dexie.maxKey])
          .first()
      )));
      return rows.reduce((min, row) => {
        if (!row) return min;
        const createdAt = Number(row.createdAt || 0);
        if (!createdAt) return min;
        return min === 0 ? createdAt : Math.min(min, createdAt);
      }, 0);
    };
    return {
      pending: Number(pending || 0),
      inFlight: Number(inFlight || 0),
      retryable: Number(retryable || 0),
      deadLetter: Number(deadLetter || 0),
      acked: Number(acked || 0),
      oldestUnsentCreatedAt: await oldestCreatedAt(['pending', 'failed', 'in_flight']),
      oldestDeadLetterCreatedAt: await oldestCreatedAt(['failed_terminal']),
    };
  },

  async listDeadLetters({ limit = 200 } = {}) {
    const maxLimit = Math.max(1, Number(limit || 200));
    const rows = await db.workbenchOutbox
      .where('status')
      .equals('failed_terminal')
      .limit(maxLimit)
      .toArray();
    return rows.sort((a, b) => Number(a.createdAt || 0) - Number(b.createdAt || 0));
  },

  async markInFlight(ids = [], { timeoutMs = IN_FLIGHT_TIMEOUT_MS } = {}) {
    const nowMs = now();
    const nextAttemptAt = nowMs + Math.max(1000, Number(timeoutMs || IN_FLIGHT_TIMEOUT_MS));
    await Promise.all((Array.isArray(ids) ? ids : [ids]).map(async (id) => {
      const key = normalizeText(id);
      if (!key) return;
      await db.workbenchOutbox.update(key, {
        status: 'in_flight',
        errorMessage: '',
        nextAttemptAt,
        updatedAt: nowMs,
      });
    }));
  },

  async markAcked(keys = []) {
    const nowMs = now();
    await Promise.all((Array.isArray(keys) ? keys : [keys]).map(async (key) => {
      const row = await getByKey(key);
      if (!row) return;
      await db.workbenchOutbox.update(row.id, { status: 'acked', updatedAt: nowMs, errorMessage: '' });
    }));
  },

  async markDuplicate(keys = []) {
    return this.markAcked(keys);
  },

  async markRetry(keysOrIds = [], error = null) {
    const nowMs = now();
    await Promise.all((Array.isArray(keysOrIds) ? keysOrIds : [keysOrIds]).map(async (keyOrId) => {
      const key = normalizeText(keyOrId);
      if (!key) return;
      const row = await getByKey(key) || await db.workbenchOutbox.get(key);
      if (!row) return;
      const attemptCount = Number(row.attemptCount || 0) + 1;
      await db.workbenchOutbox.update(row.id, {
        status: 'failed',
        attemptCount,
        errorMessage: normalizeText(error?.message || error || 'upload_failed'),
        nextAttemptAt: nowMs + retryDelayMs(attemptCount),
        updatedAt: nowMs,
      });
    }));
  },

  async markTerminal(keysOrIds = [], error = null) {
    const nowMs = now();
    await Promise.all((Array.isArray(keysOrIds) ? keysOrIds : [keysOrIds]).map(async (keyOrId) => {
      const key = normalizeText(keyOrId);
      if (!key) return;
      const row = await getByKey(key) || await db.workbenchOutbox.get(key);
      if (!row) return;
      await db.workbenchOutbox.update(row.id, {
        status: 'failed_terminal',
        errorMessage: normalizeText(error?.message || error || 'terminal_upload_failed'),
        updatedAt: nowMs,
      });
    }));
  },

  async getStatusByKey(idempotencyKey = '') {
    const row = await getByKey(idempotencyKey);
    return row ? normalizeText(row.status) : '';
  },

  async getLastSequence(taskId = '', pluginRunId = '') {
    const rows = await db.workbenchOutbox
      .where('taskId')
      .equals(normalizeText(taskId))
      .toArray();
    return rows
      .filter((row) => normalizeText(row.pluginRunId) === normalizeText(pluginRunId))
      .reduce((max, row) => Math.max(max, Number(row.sequence || 0)), 0);
  },
};
