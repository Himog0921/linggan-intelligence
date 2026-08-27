import {
  buildIngestEnvelope as buildCommitEnvelope,
  buildTaskEvent,
  buildTaskRecord,
} from '../protocol/deltaEnvelope.js';
import {
  WORKBENCH_EVENT_SOURCE,
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
} from '../protocol/schema.js';
import {
  createRecordPayloadValidationError,
  validateRecordPayload,
} from '../protocol/recordPayloadValidator.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function normalizeSequence(value, fallback = Date.now()) {
  const num = Number(value);
  return Number.isFinite(num) && num >= 0 ? Math.floor(num) : fallback;
}

function createCursor(rows = []) {
  const maxSequence = rows.reduce((max, row) => Math.max(max, Number(row.sequence || 0)), 0);
  return maxSequence ? `local-outbox-seq-${maxSequence}` : '';
}

function executionContextKey(row = {}) {
  const context = normalizeObject(row.executionContext);
  return [
    normalizeText(context.attemptId),
    normalizeText(context.leaseToken),
    Number.isFinite(Number(context.leaseEpoch)) ? Math.floor(Number(context.leaseEpoch)) : '',
  ].join('\u0001');
}

function groupRows(rows = []) {
  const grouped = new Map();
  for (const row of rows) {
    const key = [
      normalizeText(row.taskId),
      normalizeText(row.pluginRunId),
      executionContextKey(row),
    ].join('\u0000');
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key).push(row);
  }
  return [...grouped.values()];
}

function extractAckKeys(response = {}) {
  return [
    ...(Array.isArray(response.acceptedEventKeys) ? response.acceptedEventKeys : []),
    ...(Array.isArray(response.acceptedRecordKeys) ? response.acceptedRecordKeys : []),
  ];
}

function rowToEvent(row = {}) {
  return normalizeObject(row.payload);
}

function rowToRecord(row = {}) {
  return normalizeObject(row.payload);
}

function snapshotExecutionContext(value = {}) {
  const source = normalizeObject(value);
  const context = {};
  const attemptId = normalizeText(source.attemptId);
  const leaseToken = normalizeText(source.leaseToken);
  const leaseEpoch = Number(source.leaseEpoch);
  const stationId = normalizeText(source.stationId);
  const accountId = normalizeText(source.accountId);
  const platform = normalizeText(source.platform);
  if (attemptId) context.attemptId = attemptId;
  if (leaseToken) context.leaseToken = leaseToken;
  if (Number.isFinite(leaseEpoch)) context.leaseEpoch = Math.floor(leaseEpoch);
  if (stationId) context.stationId = stationId;
  if (accountId) context.accountId = accountId;
  if (platform) context.platform = platform;
  if (source.pageFingerprint && typeof source.pageFingerprint === 'object' && !Array.isArray(source.pageFingerprint)) {
    context.pageFingerprint = { ...source.pageFingerprint };
  }
  return context;
}

function hasFrozenExecutionIdentity(context = {}) {
  return Boolean(
    normalizeText(context.attemptId)
    && normalizeText(context.leaseToken)
    && Number.isFinite(Number(context.leaseEpoch)),
  );
}

function explicitExecutionContext({
  executionContext = null,
  attemptId = '',
  leaseToken = '',
  leaseId = '',
  leaseEpoch,
  stationId = '',
  accountId = '',
  platform = '',
} = {}) {
  const provided = snapshotExecutionContext(executionContext);
  return snapshotExecutionContext({
    ...provided,
    attemptId: provided.attemptId || attemptId,
    leaseToken: provided.leaseToken || leaseToken || leaseId,
    leaseEpoch: provided.leaseEpoch ?? leaseEpoch,
    stationId: provided.stationId || stationId,
    accountId: provided.accountId || accountId,
    platform: provided.platform || platform,
  });
}

export function createDeltaOutbox({
  store,
  commitDelta,
  executorInstanceId = '',
  getExecutorInstanceId = null,
  getTaskExecutionContext = null,
  prepareRecordPayload = null,
  batchLimit = 250,
  autoFlush = true,
  requireExecutionIdentity = false,
  captureJournal = null,
  pluginVersion = '',
} = {}) {
  const state = {
    flushing: false,
    scheduled: false,
    currentFlush: null,
  };

  async function runFlush() {
    if (!store || typeof store.listPending !== 'function' || typeof commitDelta !== 'function') {
      return { success: true, skipped: true, reason: 'missing_dependencies' };
    }
    state.flushing = true;
    try {
      const nowMs = Date.now();
      await store.recoverStaleInFlight?.({ limit: batchLimit, now: nowMs });
      const rows = await store.listPending({ limit: batchLimit, now: nowMs });
      if (!rows.length) return { success: true, idle: true };

      let success = true;
      let sent = 0;
      for (const group of groupRows(rows)) {
        const ids = group.map((row) => normalizeText(row.id || row.idempotencyKey)).filter(Boolean);
        await store.markInFlight?.(ids);
        const first = group[0] || {};
        const resolvedExecutorInstanceId = typeof getExecutorInstanceId === 'function'
          ? normalizeText(await getExecutorInstanceId())
          : normalizeText(executorInstanceId);
        const persistedExecutionContext = snapshotExecutionContext(first.executionContext);
        // A queued row belongs to the lease that existed when it was produced.
        // Task reporting cannot legally recover a legacy packet by borrowing a
        // later attempt's execution right.
        if (requireExecutionIdentity && !hasFrozenExecutionIdentity(persistedExecutionContext)) {
          const error = new Error('outbox_execution_context_missing');
          error.retryable = false;
          await store.markTerminal?.(ids, error);
          continue;
        }
        const executionContext = persistedExecutionContext;
        const leaseEpoch = Number(executionContext.leaseEpoch);
        const envelope = buildCommitEnvelope({
          taskId: first.taskId,
          pluginRunId: first.pluginRunId,
          executorInstanceId: resolvedExecutorInstanceId,
          cursor: createCursor(group),
          events: group.filter((row) => row.kind === 'event').map(rowToEvent),
          records: group.filter((row) => row.kind === 'record').map(rowToRecord),
          snapshot: group.find((row) => row.snapshot)?.snapshot || null,
          attemptId: executionContext.attemptId,
          leaseToken: executionContext.leaseToken,
          leaseEpoch: Number.isFinite(leaseEpoch) ? leaseEpoch : undefined,
          pageFingerprint: executionContext.pageFingerprint,
          executionContext,
        });
        try {
          const response = await commitDelta(first.taskId, envelope);
          const ackedKeys = extractAckKeys(response);
          const duplicateKeys = Array.isArray(response?.duplicateKeys) ? response.duplicateKeys : [];
          if (ackedKeys.length) await store.markAcked?.(ackedKeys);
          if (duplicateKeys.length) await store.markDuplicate?.(duplicateKeys);

          const knownKeys = new Set([...ackedKeys, ...duplicateKeys]);
          const unclassified = group
            .map((row) => row.idempotencyKey)
            .filter((key) => key && !knownKeys.has(key));
          if (unclassified.length && response?.success) {
            await store.markAcked?.(unclassified);
          }
          sent += group.length;
        } catch (error) {
          success = false;
          if (error?.retryable === false && typeof store.markTerminal === 'function') {
            await store.markTerminal(group.map((row) => row.idempotencyKey || row.id), error);
          } else {
            await store.markRetry?.(group.map((row) => row.idempotencyKey || row.id), error);
          }
        }
      }
      return { success, sent };
    } finally {
      state.flushing = false;
    }
  }

  async function flush() {
    if (state.currentFlush) {
      state.scheduled = true;
      return state.currentFlush;
    }
    const flushPromise = (async () => {
      let aggregate = null;
      do {
        state.scheduled = false;
        const result = await runFlush();
        if (!aggregate) {
          aggregate = { ...result };
        } else {
          aggregate = {
            ...result,
            success: aggregate.success !== false && result.success !== false,
            sent: Number(aggregate.sent || 0) + Number(result.sent || 0),
          };
        }
      } while (state.scheduled);
      return aggregate || { success: true, idle: true };
    })();
    state.currentFlush = flushPromise;
    try {
      return await flushPromise;
    } finally {
      if (state.currentFlush === flushPromise) {
        state.currentFlush = null;
      }
    }
  }

  // Capture Journal（报告 §9.2）：record 类采集事实先落不可变账本，与租约
  // 是否有效、出站行是否被判死信解耦。账本写入失败不阻塞出站主链路。
  async function appendCaptureJournal(row, executionContext) {
    if (!captureJournal || typeof captureJournal.append !== 'function') return;
    if (normalizeText(row.kind) !== 'record') return;
    try {
      await captureJournal.append({
        entryId: row.idempotencyKey || row.id,
        taskId: row.taskId,
        pluginRunId: row.pluginRunId,
        kind: row.kind,
        recordType: row.recordType || '',
        externalRecordId: row.externalRecordId || '',
        payload: normalizeObject(row.payload),
        capturedAt: Date.now(),
        executionContext,
        pluginVersion,
      });
    } catch {
      // 账本失败不能挡住出站；出站行本身仍带 executionContext。
    }
  }

  async function enqueueRow(row, { deferFlush = false } = {}) {
    if (!store || typeof store.enqueue !== 'function') {
      return null;
    }
    let executionContext = snapshotExecutionContext(row.executionContext);
    // Generic, non-task callers may still enrich their messages from a lookup.
    // Task reporting must carry the lease identity from the producer instead:
    // looking it up later can accidentally attach a result to a newer attempt.
    if (!requireExecutionIdentity
      && Object.keys(executionContext).length === 0
      && typeof getTaskExecutionContext === 'function') {
      try {
        executionContext = snapshotExecutionContext(await getTaskExecutionContext(row.taskId));
      } catch {
        executionContext = {};
      }
    }
    // 账本先于身份校验：即使下面严格校验拒绝入队，采集事实也已经保住。
    await appendCaptureJournal(row, executionContext);
    if (requireExecutionIdentity && !hasFrozenExecutionIdentity(executionContext)) {
      const error = new Error('task_report_context_missing');
      error.retryable = false;
      throw error;
    }
    const stored = await store.enqueue({
      ...row,
      ...(Object.keys(executionContext).length > 0 ? { executionContext } : {}),
    });
    if (autoFlush && !deferFlush) {
      queueMicrotask(() => {
        void flush();
      });
    }
    return stored;
  }

  async function enqueueEvent({
    taskId = '',
    pluginRunId = '',
    eventType = WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT,
    source = WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence = Date.now(),
    payload = {},
    controlRequestId = '',
    snapshot = null,
    occurredAt = '',
    attemptId = '',
    leaseToken = '',
    leaseId = '',
    leaseEpoch,
    stationId = '',
    accountId = '',
    platform = '',
    executionContext = null,
  } = {}) {
    const normalizedSequence = normalizeSequence(sequence);
    const event = buildTaskEvent({
      taskId,
      pluginRunId,
      eventType,
      source,
      sequence: normalizedSequence,
      payload,
      controlRequestId,
      occurredAt,
      attemptId,
      leaseId,
      stationId,
      accountId,
      platform,
    });
    return enqueueRow({
      taskId,
      pluginRunId,
      idempotencyKey: event.idempotencyKey,
      kind: 'event',
      sequence: normalizedSequence,
      payload: event,
      snapshot,
      executionContext: explicitExecutionContext({
        executionContext,
        attemptId,
        leaseToken,
        leaseId,
        leaseEpoch,
        stationId,
        accountId,
        platform,
      }),
    });
  }

  async function enqueueRecord({
    taskId = '',
    pluginRunId = '',
    recordType = WORKBENCH_RECORD_TYPE.NOTE,
    externalRecordId = '',
    sequence = Date.now(),
    payload = {},
    collectedAt = '',
    snapshot = null,
    executionContext = null,
  } = {}, { deferFlush = false } = {}) {
    const normalizedSequence = normalizeSequence(sequence);
    const preparedPayload = typeof prepareRecordPayload === 'function'
      ? await prepareRecordPayload({
        taskId,
        pluginRunId,
        recordType,
        externalRecordId,
        payload,
      })
      : payload;
    const validation = validateRecordPayload(recordType, preparedPayload);
    if (!validation.valid) {
      throw createRecordPayloadValidationError({ recordType, validation });
    }
    const record = buildTaskRecord({
      taskId,
      pluginRunId,
      recordType,
      externalRecordId,
      sequence: normalizedSequence,
      payload: preparedPayload,
      collectedAt,
    });
    return enqueueRow({
      taskId,
      pluginRunId,
      idempotencyKey: record.idempotencyKey,
      kind: 'record',
      recordType,
      externalRecordId,
      sequence: normalizedSequence,
      payload: record,
      snapshot,
      executionContext: explicitExecutionContext({ executionContext }),
    }, { deferFlush });
  }

  async function enqueueRecords(records = []) {
    const list = Array.isArray(records) ? records : [records];
    const results = [];
    for (const record of list) {
      results.push(await enqueueRecord(record, { deferFlush: true }));
    }
    if (autoFlush && results.length > 0) {
      queueMicrotask(() => {
        void flush();
      });
    }
    return results;
  }

  return {
    enqueueEvent,
    enqueueRecord,
    enqueueRecords,
    flush,
  };
}
