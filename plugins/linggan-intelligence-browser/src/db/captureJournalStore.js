import db from './index.js';

// Capture Journal —— 采集事实账本（2026-07-13 事故架构升级，报告 §9.2）。
//
// 定位：记录"插件确实采到了什么"这一不可变事实，与租约是否仍有效、出站行
// 是否被判死信完全解耦。出站层（workbenchOutbox）只负责传输；账本层保证
// 采集内容永不无声丢失，是 plugin_local_recovery 恢复导入的数据源头。
//
// 约束：append-only。同 entryId 重复 append 返回既有条目，不覆盖；不提供
// update；修剪只允许清理「已确认送达 + 超过保留期」的条目。

function normalizeText(value = '') {
  return String(value || '').trim();
}

function canonicalJson(value) {
  const sortValue = (input) => {
    if (Array.isArray(input)) return input.map(sortValue);
    if (input && typeof input === 'object') {
      return Object.fromEntries(
        Object.keys(input).sort().map((key) => [key, sortValue(input[key])]),
      );
    }
    return input;
  };
  return JSON.stringify(sortValue(value ?? null));
}

async function sha256Hex(text = '') {
  const bytes = new TextEncoder().encode(String(text));
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

export const captureJournalStore = {
  async append(entry = {}) {
    const entryId = normalizeText(entry.entryId);
    if (!entryId) throw new Error('invalid_capture_journal_entry');

    const existing = await db.captureJournal.get(entryId);
    if (existing) return existing;

    const payload = entry.payload && typeof entry.payload === 'object' ? entry.payload : {};
    const row = {
      entryId,
      taskId: normalizeText(entry.taskId),
      pluginRunId: normalizeText(entry.pluginRunId),
      kind: normalizeText(entry.kind),
      recordType: normalizeText(entry.recordType),
      externalRecordId: normalizeText(entry.externalRecordId),
      payload,
      payloadHash: await sha256Hex(canonicalJson(payload)),
      capturedAt: Number.isFinite(Number(entry.capturedAt)) ? Number(entry.capturedAt) : Date.now(),
      executionContext: entry.executionContext
        && typeof entry.executionContext === 'object'
        && !Array.isArray(entry.executionContext)
        ? { ...entry.executionContext }
        : null,
      pluginVersion: normalizeText(entry.pluginVersion),
      createdAt: Date.now(),
    };

    try {
      await db.captureJournal.add(row);
    } catch (error) {
      // add 撞主键 = 并发下已有同 entryId 条目：账本不可变，返回既有条目。
      if (error?.name === 'ConstraintError' || /constraint|key already exists/i.test(String(error?.message || error))) {
        const raced = await db.captureJournal.get(entryId);
        if (raced) return raced;
      }
      throw error;
    }
    return row;
  },

  async listByTask(taskId = '') {
    return db.captureJournal.where('taskId').equals(normalizeText(taskId)).toArray();
  },

  async listAll({ limit = 1000, sinceMs = 0 } = {}) {
    const maxLimit = Math.max(1, Number(limit || 1000));
    let collection = db.captureJournal.orderBy('capturedAt');
    if (Number(sinceMs) > 0) {
      collection = db.captureJournal.where('capturedAt').aboveOrEqual(Number(sinceMs));
    }
    return collection.limit(maxLimit).toArray();
  },

  async count() {
    return db.captureJournal.count();
  },

  async pruneAcked({ olderThanMs = 14 * 24 * 60 * 60 * 1000, isAcked, now = Date.now() } = {}) {
    if (typeof isAcked !== 'function') return 0;
    const cutoff = now - Math.max(0, Number(olderThanMs || 0));
    const staleRows = await db.captureJournal.where('capturedAt').below(cutoff).toArray();
    const prunable = [];
    for (const row of staleRows) {
      if (await isAcked(row.entryId)) prunable.push(row.entryId);
    }
    if (prunable.length > 0) {
      await db.captureJournal.bulkDelete(prunable);
    }
    return prunable.length;
  },
};
