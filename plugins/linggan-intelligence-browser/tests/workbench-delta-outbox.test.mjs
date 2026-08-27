import test from 'node:test';
import assert from 'node:assert/strict';

import { createDeltaOutbox } from '../src/workbench/runtime/deltaOutbox.js';
import { enrichNoteWithDataFoundationPayload } from '../src/workbench/runtime/dataFoundationPayload.js';
import {
  WORKBENCH_EVENT_SOURCE,
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
} from '../src/workbench/protocol/schema.js';

function createMemoryOutboxStore() {
  const rows = new Map();
  return {
    rows,
    async enqueue(item) {
      if (rows.has(item.idempotencyKey)) return rows.get(item.idempotencyKey);
      const row = {
        id: item.idempotencyKey,
        status: 'pending',
        attemptCount: 0,
        nextAttemptAt: 0,
        createdAt: Date.now(),
        ...item,
      };
      rows.set(row.idempotencyKey, row);
      return row;
    },
    async recoverStaleInFlight({ now: nowMs = Date.now() } = {}) {
      let recovered = 0;
      for (const row of rows.values()) {
        if (row.status !== 'in_flight' || Number(row.nextAttemptAt || 0) > nowMs) continue;
        row.status = 'failed';
        row.errorMessage = 'in_flight_timeout';
        row.attemptCount += 1;
        row.nextAttemptAt = nowMs;
        row.updatedAt = nowMs;
        recovered += 1;
      }
      return recovered;
    },
    async listPending({ now: nowMs = Date.now() } = {}) {
      return [...rows.values()].filter((row) => (
        row.status === 'pending'
        || (row.status === 'failed' && Number(row.nextAttemptAt || 0) <= nowMs)
      ));
    },
    async markInFlight(ids) {
      for (const id of ids) {
        const row = rows.get(id);
        if (row) {
          row.status = 'in_flight';
          row.nextAttemptAt = Date.now() + 5 * 60 * 1000;
          row.updatedAt = Date.now();
        }
      }
    },
    async markAcked(keys) {
      for (const key of keys) {
        const row = rows.get(key);
        if (row) row.status = 'acked';
      }
    },
    async markDuplicate(keys) {
      for (const key of keys) {
        const row = rows.get(key);
        if (row) row.status = 'acked';
      }
    },
    async markRetry(keysOrIds, error) {
      for (const key of keysOrIds) {
        const row = rows.get(key);
        if (!row) continue;
        row.status = 'failed';
        row.errorMessage = String(error?.message || error || '');
        row.attemptCount += 1;
        row.nextAttemptAt = Date.now() + 1000;
      }
    },
    async markTerminal(keysOrIds, error) {
      for (const key of keysOrIds) {
        const row = rows.get(key);
        if (!row) continue;
        row.status = 'failed_terminal';
        row.errorMessage = String(error?.message || error || '');
      }
    },
  };
}

test('delta outbox dedupes idempotency keys and flushes one commit envelope', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: envelope.records.map((record) => record.idempotencyKey),
        duplicateKeys: [],
      };
    },
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 1,
    payload: { message: '采到第 1 条' },
  });
  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 1,
    payload: { message: '重复事件不会多写' },
  });

  assert.equal(store.rows.size, 1);

  const result = await outbox.flush();

  assert.equal(result.success, true);
  assert.equal(envelopes.length, 1);
  assert.equal(envelopes[0][0], 'task_1');
  assert.equal(envelopes[0][1].events.length, 1);
  assert.equal(envelopes[0][1].executorInstanceId, 'plugin_1');
  assert.equal([...store.rows.values()][0].status, 'acked');
});

test('delta outbox waits for an in-flight flush instead of reporting a skipped flush', async () => {
  const store = createMemoryOutboxStore();
  let releaseCommit;
  const commitGate = new Promise((resolve) => {
    releaseCommit = resolve;
  });
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      await commitGate;
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_wait_flush',
    pluginRunId: 'run_wait_flush',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 1,
    payload: { message: 'waiting flush' },
  });

  const firstFlush = outbox.flush();
  await Promise.resolve();
  const secondFlush = outbox.flush();
  releaseCommit();
  const [firstResult, secondResult] = await Promise.all([firstFlush, secondFlush]);

  assert.equal(firstResult.success, true);
  assert.equal(secondResult.success, true);
  assert.equal(secondResult.reason, undefined);
  assert.equal([...store.rows.values()][0].status, 'acked');
});

test('delta outbox resolves executor identity at flush time', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    executorInstanceId: 'startup_random_id',
    getExecutorInstanceId: async () => 'persisted_executor_id',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 3,
    payload: { message: 'flush uses persistent id' },
  });

  await outbox.flush();

  assert.equal(envelopes[0][1].executorInstanceId, 'persisted_executor_id');
});

test('delta outbox attaches lease credentials and page fingerprint to commit envelope', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    getTaskExecutionContext: async (taskId) => ({
      attemptId: `attempt-${taskId}`,
      leaseToken: `lease-${taskId}`,
      leaseEpoch: 4,
      pageFingerprint: {
        platform: 'xhs',
        pageType: 'detail',
        contentId: 'note_1',
      },
    }),
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 3,
    payload: { message: 'flush uses execution truth' },
  });

  await outbox.flush();

  assert.equal(envelopes[0][1].attemptId, 'attempt-task_1');
  assert.equal(envelopes[0][1].leaseToken, 'lease-task_1');
  assert.equal(envelopes[0][1].leaseEpoch, 4);
  assert.deepEqual(envelopes[0][1].pageFingerprint, {
    platform: 'xhs',
    pageType: 'detail',
    contentId: 'note_1',
  });
});

test('delta outbox retains execution identity when a terminal packet flushes after local cleanup', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  let executionContext = {
    attemptId: 'attempt-terminal-identity',
    leaseToken: 'lease-terminal-identity',
    leaseEpoch: 7,
    pageFingerprint: { platform: 'xhs', pageType: 'author' },
  };
  const outbox = createDeltaOutbox({
    store,
    getTaskExecutionContext: async () => executionContext,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_terminal_identity',
    pluginRunId: 'run_terminal_identity',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_COMPLETED,
    sequence: 1,
    payload: { status: 'completed' },
  });

  executionContext = {};
  await outbox.flush();

  assert.equal(envelopes.length, 1);
  assert.equal(envelopes[0][1].attemptId, 'attempt-terminal-identity');
  assert.equal(envelopes[0][1].leaseToken, 'lease-terminal-identity');
  assert.equal(envelopes[0][1].leaseEpoch, 7);
});

test('delta outbox never borrows a newer lease for legacy packets without frozen execution identity', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  await store.enqueue({
    taskId: 'task_legacy_identity',
    pluginRunId: 'run_legacy_identity',
    idempotencyKey: 'legacy-event-1',
    kind: 'event',
    sequence: 1,
    payload: {
      idempotencyKey: 'legacy-event-1',
      eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_COMPLETED,
      type: WORKBENCH_TASK_EVENT_TYPE.TASK_COMPLETED,
    },
  });
  const outbox = createDeltaOutbox({
    store,
    getTaskExecutionContext: async () => ({
      attemptId: 'new-attempt-that-must-not-be-borrowed',
      leaseToken: 'new-lease-that-must-not-be-borrowed',
      leaseEpoch: 9,
    }),
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return { success: true, acceptedEventKeys: ['legacy-event-1'] };
    },
    autoFlush: false,
    requireExecutionIdentity: true,
  });

  await outbox.flush();

  assert.equal(envelopes.length, 0);
  const legacyRow = store.rows.get('legacy-event-1');
  assert.equal(legacyRow.status, 'failed_terminal');
  assert.match(legacyRow.errorMessage, /outbox_execution_context_missing/);
});

test('strict task outbox rejects a missing producer context without consulting a newer lease', async () => {
  const store = createMemoryOutboxStore();
  let lookupCalls = 0;
  const outbox = createDeltaOutbox({
    store,
    requireExecutionIdentity: true,
    getTaskExecutionContext: async () => {
      lookupCalls += 1;
      return {
        attemptId: 'new-attempt-that-must-not-be-read',
        leaseToken: 'new-lease-that-must-not-be-read',
        leaseEpoch: 99,
      };
    },
    commitDelta: async () => ({ success: true }),
    autoFlush: false,
  });

  await assert.rejects(
    outbox.enqueueEvent({
      taskId: 'task_missing_producer_context',
      pluginRunId: 'run_missing_producer_context',
      eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_COMPLETED,
      sequence: 1,
      payload: { status: 'completed' },
    }),
    /task_report_context_missing/,
  );

  assert.equal(lookupCalls, 0);
  assert.equal(store.rows.size, 0);
});

test('delta outbox drains an auto-scheduled terminal packet before flush resolves', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  let outbox;
  outbox = createDeltaOutbox({
    store,
    getTaskExecutionContext: async () => ({
      attemptId: 'attempt-drain',
      leaseToken: 'lease-drain',
      leaseEpoch: 8,
    }),
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      if (envelope.events.some((event) => event.eventType === 'first')) {
        await outbox.enqueueEvent({
          taskId,
          pluginRunId: 'run-drain',
          eventType: 'terminal',
          sequence: 2,
          payload: { status: 'completed' },
        });
      }
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    autoFlush: true,
  });

  await outbox.enqueueEvent({
    taskId: 'task_drain',
    pluginRunId: 'run-drain',
    eventType: 'first',
    sequence: 1,
    payload: { status: 'running' },
  });

  await outbox.flush();

  assert.equal(envelopes.length, 2);
  assert.equal(envelopes[1][1].events[0].eventType, 'terminal');
  assert.equal(envelopes[1][1].attemptId, 'attempt-drain');
});

test('delta outbox preserves explicit event execution identity fields', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_release_identity',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_RELEASED,
    sequence: 5,
    attemptId: 'attempt-release-1',
    leaseId: 'lease-release-1',
    stationId: 'station-release-1',
    accountId: 'account-release-1',
    platform: 'xhs',
    payload: { reasonCode: 'page_context_unavailable' },
  });

  await outbox.flush();

  const event = envelopes[0][1].events[0];
  assert.equal(event.attemptId, 'attempt-release-1');
  assert.equal(event.leaseId, 'lease-release-1');
  assert.equal(event.stationId, 'station-release-1');
  assert.equal(event.accountId, 'account-release-1');
  assert.equal(event.platform, 'xhs');
});

test('strict task outbox commits records with producer-frozen execution identity', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    requireExecutionIdentity: true,
    getTaskExecutionContext: async () => {
      throw new Error('strict task reporting must not do a late context lookup');
    },
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: [],
        acceptedRecordKeys: envelope.records.map((record) => record.idempotencyKey),
        duplicateKeys: [],
      };
    },
    autoFlush: false,
  });

  await outbox.enqueueRecord({
    taskId: 'task_frozen_record_identity',
    pluginRunId: 'run_frozen_record_identity',
    recordType: 'note',
    externalRecordId: 'note_frozen_identity',
    sequence: 7,
    payload: {
      targetKey: 'xhs:note:note_frozen_identity',
      platform: 'xhs',
      noteId: 'note_frozen_identity',
      observedAt: '2026-07-13T08:28:54.128Z',
      title: '冻结身份测试',
    },
    executionContext: {
      attemptId: 'attempt-frozen-record',
      leaseToken: 'lease-frozen-record',
      leaseEpoch: 2,
      platform: 'xhs',
    },
  });

  await outbox.flush();

  assert.equal(envelopes.length, 1);
  assert.equal(envelopes[0][1].attemptId, 'attempt-frozen-record');
  assert.equal(envelopes[0][1].leaseToken, 'lease-frozen-record');
  assert.equal(envelopes[0][1].leaseEpoch, 2);
  assert.equal([...store.rows.values()][0].status, 'acked');
});

test('delta outbox prepares record payloads before storing them', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: [],
        acceptedRecordKeys: envelope.records.map((record) => record.idempotencyKey),
        duplicateKeys: [],
      };
    },
    prepareRecordPayload: async ({ payload }) => ({
      ...payload,
      coverUrl: 'https://blob.example.com/stable-cover.webp',
    }),
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueRecord({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    recordType: 'note',
    externalRecordId: 'note_1',
    sequence: 4,
    payload: {
      targetKey: 'xhs:note:note_1',
      platform: 'xhs',
      noteId: 'note_1',
      observedAt: '2026-07-01T00:00:00.000Z',
      coverUrl: 'https://sns-img.example.com/cover.webp',
    },
  });

  const queued = [...store.rows.values()][0];
  assert.equal(queued.payload.targetKey, 'xhs:note:note_1');
  assert.equal(queued.payload.platform, 'xhs');
  assert.equal(queued.payload.observedAt, '2026-07-01T00:00:00.000Z');
  assert.equal(queued.payload.payload.coverUrl, 'https://blob.example.com/stable-cover.webp');

  await outbox.flush();

  assert.equal(envelopes[0][1].records[0].payload.coverUrl, 'https://blob.example.com/stable-cover.webp');
});

test('delta outbox keeps note full results in one commit envelope', async () => {
  const store = createMemoryOutboxStore();
  const envelopes = [];
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      envelopes.push([taskId, envelope]);
      return {
        success: true,
        acceptedEventKeys: [],
        acceptedRecordKeys: envelope.records.map((record) => record.idempotencyKey),
        duplicateKeys: [],
      };
    },
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });
  const records = [
    {
      taskId: 'task_note_full',
      pluginRunId: 'run_note_full',
      recordType: WORKBENCH_RECORD_TYPE.NOTE,
      externalRecordId: 'note_full_1',
      sequence: 1,
      payload: {
        platform: 'xhs',
        noteId: 'note_full_1',
        title: '完整详情采集',
        content: '正文',
      },
    },
    ...Array.from({ length: 20 }, (_, index) => ({
      taskId: 'task_note_full',
      pluginRunId: 'run_note_full',
      recordType: WORKBENCH_RECORD_TYPE.COMMENT,
      externalRecordId: `comment_${index + 1}`,
      sequence: index + 2,
      payload: {
        platform: 'xhs',
        commentId: `comment_${index + 1}`,
        noteId: 'note_full_1',
        text: `评论 ${index + 1}`,
      },
    })),
  ];

  await outbox.enqueueRecords(records);
  const result = await outbox.flush();

  assert.equal(result.success, true);
  assert.equal(envelopes.length, 1);
  assert.equal(envelopes[0][1].records.length, 21);
});

test('delta outbox can attach data foundation payload before storing note records', async () => {
  const store = createMemoryOutboxStore();
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async () => ({ success: true }),
    prepareRecordPayload: async (record) => enrichNoteWithDataFoundationPayload(record.payload, record),
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueRecord({
    taskId: 'task_data_foundation',
    pluginRunId: 'run_data_foundation',
    recordType: WORKBENCH_RECORD_TYPE.NOTE,
    externalRecordId: 'note_1',
    sequence: 5,
    payload: {
      platform: 'xhs',
      noteId: 'note_1',
      title: '低粉爆文',
      authorId: 'author_1',
      authorFans: 900,
      hashtags: ['#ADHD'],
      coverUrl: 'https://images.example.com/cover.webp',
    },
  });

  const queued = [...store.rows.values()][0];
  assert.equal(queued.payload.payload.standardContentCode, 'cw-content:global:xhs:image_text:note_1');
  assert.equal(queued.payload.payload.standardAuthorCode, 'cw-author:global:xhs:author_1');
  assert.deepEqual(queued.payload.payload.sourceRun, {
    source: 'plugin_task_delta',
    taskId: 'task_data_foundation',
    recordId: 'note_1',
  });
});

test('delta outbox rejects invalid extractor records before they enter the queue', async () => {
  const store = createMemoryOutboxStore();
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async () => ({ success: true }),
    autoFlush: false,
  });

  await assert.rejects(
    outbox.enqueueRecord({
      taskId: 'task_1',
      pluginRunId: 'run_1',
      recordType: WORKBENCH_RECORD_TYPE.COMMENT,
      externalRecordId: 'comment_1',
      payload: {
        commentId: 'comment_1',
        text: '没有父级作品',
      },
    }),
    (error) => {
      assert.equal(error.retryable, false);
      assert.equal(error.code, 'missing_comment_parent');
      assert.equal(error.observability.recordType, WORKBENCH_RECORD_TYPE.COMMENT);
      assert.equal(error.observability.schemaValidationFailureCount, 1);
      return true;
    },
  );
  assert.equal(store.rows.size, 0);
});

test('delta outbox schedules retry on network failure then accepts duplicate ack', async () => {
  const store = createMemoryOutboxStore();
  let calls = 0;
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      calls += 1;
      if (calls === 1) {
        const error = new Error('network down');
        error.retryable = true;
        throw error;
      }
      return {
        success: true,
        acceptedEventKeys: [],
        acceptedRecordKeys: [],
        duplicateKeys: envelope.events.map((event) => event.idempotencyKey),
      };
    },
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 2,
    payload: { status: 'running' },
  });

  const first = await outbox.flush();
  const rowAfterFailure = [...store.rows.values()][0];
  assert.equal(first.success, false);
  assert.equal(rowAfterFailure.status, 'failed');
  assert.equal(rowAfterFailure.attemptCount, 1);
  assert.ok(rowAfterFailure.nextAttemptAt > Date.now());

  rowAfterFailure.nextAttemptAt = 0;
  const second = await outbox.flush();

  assert.equal(second.success, true);
  assert.equal(calls, 2);
  assert.equal(rowAfterFailure.status, 'acked');
});

test('delta outbox marks permanent sync rejection terminal instead of retrying', async () => {
  const store = createMemoryOutboxStore();
  let calls = 0;
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async () => {
      calls += 1;
      const error = new Error('lease_token_mismatch');
      error.retryable = false;
      throw error;
    },
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_HEARTBEAT,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 2,
    payload: { status: 'running' },
  });

  const first = await outbox.flush();
  const rowAfterFailure = [...store.rows.values()][0];
  const second = await outbox.flush();

  assert.equal(first.success, false);
  assert.equal(rowAfterFailure.status, 'failed_terminal');
  assert.equal(rowAfterFailure.errorMessage, 'lease_token_mismatch');
  assert.equal(second.success, true);
  assert.equal(second.idle, true);
  assert.equal(calls, 1);
});

test('delta outbox recovers stale in-flight rows before flushing', async () => {
  const store = createMemoryOutboxStore();
  let calls = 0;
  const outbox = createDeltaOutbox({
    store,
    commitDelta: async (taskId, envelope) => {
      calls += 1;
      return {
        success: true,
        acceptedEventKeys: envelope.events.map((event) => event.idempotencyKey),
        acceptedRecordKeys: [],
        duplicateKeys: [],
      };
    },
    executorInstanceId: 'plugin_1',
    autoFlush: false,
  });

  await outbox.enqueueEvent({
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_PROGRESS,
    source: WORKBENCH_EVENT_SOURCE.PLUGIN,
    sequence: 5,
    payload: { message: 'stale in-flight should retry' },
  });

  const row = [...store.rows.values()][0];
  row.status = 'in_flight';
  row.nextAttemptAt = Date.now() - 1;

  const result = await outbox.flush();

  assert.equal(result.success, true);
  assert.equal(calls, 1);
  assert.equal(row.status, 'acked');
  assert.equal(row.attemptCount, 1);
});
