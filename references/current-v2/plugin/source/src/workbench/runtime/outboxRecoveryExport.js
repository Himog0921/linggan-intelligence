// plugin_local_recovery 导出构建器（报告 §9.3）。
//
// 把本地采集事实（captureJournal）与死信出站行（failed_terminal）打包成
// 可审计的恢复包，交给工作台的显式恢复导入通道。导出是只读动作：
// 不改任何行状态，不把旧包绑到新租约。

const RECOVERY_EXPORT_SCHEMA_VERSION = 'plugin-local-recovery/v1';

function normalizeText(value = '') {
  return String(value || '').trim();
}

export function buildOutboxRecoveryExport({
  journalEntries = [],
  deadLetters = [],
  pluginVersion = '',
  stationId = '',
  now = Date.now(),
} = {}) {
  const journal = (Array.isArray(journalEntries) ? journalEntries : []).map((entry) => ({
    entryId: normalizeText(entry.entryId),
    taskId: normalizeText(entry.taskId),
    pluginRunId: normalizeText(entry.pluginRunId),
    kind: normalizeText(entry.kind),
    recordType: normalizeText(entry.recordType),
    externalRecordId: normalizeText(entry.externalRecordId),
    payload: entry.payload ?? null,
    payloadHash: normalizeText(entry.payloadHash),
    capturedAt: Number(entry.capturedAt || 0),
    executionContext: entry.executionContext ?? null,
    pluginVersion: normalizeText(entry.pluginVersion),
  }));

  const deadLetterRows = (Array.isArray(deadLetters) ? deadLetters : []).map((row) => ({
    idempotencyKey: normalizeText(row.idempotencyKey),
    taskId: normalizeText(row.taskId),
    pluginRunId: normalizeText(row.pluginRunId),
    kind: normalizeText(row.kind),
    status: normalizeText(row.status),
    errorMessage: normalizeText(row.errorMessage),
    attemptCount: Number(row.attemptCount || 0),
    payload: row.payload ?? null,
    snapshot: row.snapshot ?? null,
    executionContext: row.executionContext ?? null,
    createdAt: Number(row.createdAt || 0),
    updatedAt: Number(row.updatedAt || 0),
  }));

  return {
    schemaVersion: RECOVERY_EXPORT_SCHEMA_VERSION,
    exportedAt: new Date(now).toISOString(),
    pluginVersion: normalizeText(pluginVersion),
    stationId: normalizeText(stationId),
    counts: {
      journal: journal.length,
      deadLetters: deadLetterRows.length,
    },
    journal,
    deadLetters: deadLetterRows,
  };
}

export { RECOVERY_EXPORT_SCHEMA_VERSION };
