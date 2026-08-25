// 恢复导出（plugin_local_recovery 的插件侧）：只读、结构可审计、身份与哈希齐全。
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildOutboxRecoveryExport,
  RECOVERY_EXPORT_SCHEMA_VERSION,
} from '../src/workbench/runtime/outboxRecoveryExport.js';

test('导出包结构完整：schema 版本、计数、账本与死信条目', () => {
  const exported = buildOutboxRecoveryExport({
    pluginVersion: '2.0.84',
    stationId: 'station-zs-1',
    now: 1760000000000,
    journalEntries: [{
      entryId: 'e-1',
      taskId: 'task-1',
      pluginRunId: 'run-1',
      kind: 'record',
      recordType: 'note',
      externalRecordId: 'note-1',
      payload: { title: '希希妈妈聊 ADHD' },
      payloadHash: 'a'.repeat(64),
      capturedAt: 1759999990000,
      executionContext: { attemptId: 'at-1', leaseToken: 'lt-1', leaseEpoch: 1 },
      pluginVersion: '2.0.83',
    }],
    deadLetters: [{
      idempotencyKey: 'row-dead',
      taskId: 'task-1',
      pluginRunId: 'run-1',
      kind: 'record',
      status: 'failed_terminal',
      errorMessage: 'outbox_execution_context_missing',
      attemptCount: 0,
      payload: { recordType: 'note' },
      executionContext: null,
      createdAt: 1759999991000,
      updatedAt: 1759999992000,
    }],
  });

  assert.equal(exported.schemaVersion, RECOVERY_EXPORT_SCHEMA_VERSION);
  assert.equal(exported.exportedAt, new Date(1760000000000).toISOString());
  assert.equal(exported.pluginVersion, '2.0.84');
  assert.equal(exported.stationId, 'station-zs-1');
  assert.deepEqual(exported.counts, { journal: 1, deadLetters: 1 });
  assert.equal(exported.journal[0].payloadHash, 'a'.repeat(64));
  assert.equal(exported.journal[0].executionContext.attemptId, 'at-1');
  assert.equal(exported.deadLetters[0].errorMessage, 'outbox_execution_context_missing');
});

test('空数据导出仍是合法包（计数为 0）', () => {
  const exported = buildOutboxRecoveryExport({ pluginVersion: '2.0.84' });
  assert.deepEqual(exported.counts, { journal: 0, deadLetters: 0 });
  assert.deepEqual(exported.journal, []);
  assert.deepEqual(exported.deadLetters, []);
});
