// 2026-07-13 事故回归测试：executionContext 必须在真实 IndexedDB 持久化边界存活。
//
// 此前 tests/workbench-delta-outbox.test.mjs 只用内存 Map store（`...item` 保留全部字段），
// 从未经过 normalizeOutboxRow + Dexie round-trip，导致 2.0.83 的冻结身份修复在
// 真实持久化层丢字段（enqueue 后读回无 executionContext），严格校验在发 HTTP 前
// 把记录判成 failed_terminal。本文件用 fake-indexeddb 走真实 Dexie 路径堵住这个测试边界。
import 'fake-indexeddb/auto';
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';

const { workbenchOutboxStore } = await import('../src/db/workbenchOutboxStore.js');
const { default: db } = await import('../src/db/index.js');
const { createDeltaOutbox } = await import('../src/workbench/runtime/deltaOutbox.js');

const FROZEN_CONTEXT = {
  attemptId: 'attempt-001',
  leaseToken: 'lease-token-abc',
  leaseEpoch: 3,
  stationId: 'station-zs-1',
  accountId: 'acct-9',
  platform: 'xiaohongshu',
  pageFingerprint: { url: 'https://www.xiaohongshu.com/user/profile/6897f2e8', title: '希希妈妈聊 ADHD' },
};

function buildRow(overrides = {}) {
  return {
    id: overrides.idempotencyKey || 'evt-ctx-1',
    idempotencyKey: overrides.idempotencyKey || 'evt-ctx-1',
    taskId: 'task-1',
    pluginRunId: 'run-1',
    kind: 'event',
    sequence: Date.now(),
    payload: { idempotencyKey: overrides.idempotencyKey || 'evt-ctx-1', eventType: 'task.progress' },
    executionContext: { ...FROZEN_CONTEXT },
    ...overrides,
  };
}

test.beforeEach(async () => {
  await db.workbenchOutbox.clear();
});

test('enqueue -> IndexedDB -> 读回：executionContext 完整存活', async () => {
  await workbenchOutboxStore.enqueue(buildRow({ idempotencyKey: 'evt-roundtrip-1' }));

  const persisted = await db.workbenchOutbox.get('evt-roundtrip-1');
  assert.ok(persisted, '行必须已落库');
  assert.ok(persisted.executionContext, 'executionContext 不允许在持久化边界被丢弃');
  assert.equal(persisted.executionContext.attemptId, FROZEN_CONTEXT.attemptId);
  assert.equal(persisted.executionContext.leaseToken, FROZEN_CONTEXT.leaseToken);
  assert.equal(persisted.executionContext.leaseEpoch, FROZEN_CONTEXT.leaseEpoch);
  assert.deepEqual(persisted.executionContext.pageFingerprint, FROZEN_CONTEXT.pageFingerprint);
});

test('状态分账：countUnsent 排除死信；countsByStatus 分桶；listDeadLetters 可导出', async () => {
  const baseMs = Date.now() - 60_000;
  await workbenchOutboxStore.enqueue(buildRow({ idempotencyKey: 'row-pending', createdAt: baseMs }));
  await workbenchOutboxStore.enqueue(buildRow({ idempotencyKey: 'row-retryable', createdAt: baseMs + 1 }));
  await workbenchOutboxStore.enqueue(buildRow({ idempotencyKey: 'row-dead', createdAt: baseMs + 2 }));
  await workbenchOutboxStore.enqueue(buildRow({ idempotencyKey: 'row-acked', createdAt: baseMs + 3 }));

  await workbenchOutboxStore.markRetry(['row-retryable'], new Error('network_flap'));
  await workbenchOutboxStore.markTerminal(['row-dead'], new Error('outbox_execution_context_missing'));
  await workbenchOutboxStore.markAcked(['row-acked']);

  // 死信不允许再计入"待发送"（此前 2144 计数把 failed_terminal 一并相加）
  const unsent = await workbenchOutboxStore.countUnsent();
  assert.equal(unsent, 2, `待发送只含 pending+retryable，实际 ${unsent}`);

  const counts = await workbenchOutboxStore.countsByStatus();
  assert.equal(counts.pending, 1);
  assert.equal(counts.retryable, 1);
  assert.equal(counts.deadLetter, 1);
  assert.equal(counts.acked, 1);
  assert.equal(counts.inFlight, 0);
  assert.equal(counts.oldestUnsentCreatedAt, baseMs, '必须从索引返回最老未发送行时间');
  assert.equal(counts.oldestDeadLetterCreatedAt, baseMs + 2, '必须从索引返回最老死信时间');

  const deadLetters = await workbenchOutboxStore.listDeadLetters({ limit: 10 });
  assert.equal(deadLetters.length, 1);
  assert.equal(deadLetters[0].idempotencyKey, 'row-dead');
  assert.equal(deadLetters[0].errorMessage, 'outbox_execution_context_missing');
  assert.ok(deadLetters[0].executionContext, '死信行必须携带执行身份以便恢复归档');
  assert.equal(deadLetters[0].taskId, 'task-1');
});

test('countsByStatus 通过 status+createdAt 索引取最老行，不加载全量 payload', () => {
  assert.ok(
    db.workbenchOutbox.schema.idxByName['[status+createdAt]'],
    'workbenchOutbox 必须提供 [status+createdAt] 索引'
  );

  const source = fs.readFileSync(new URL('../src/db/workbenchOutboxStore.js', import.meta.url), 'utf8');
  const start = source.indexOf('async countsByStatus');
  const end = source.indexOf('async listDeadLetters', start);
  const countsByStatusSource = source.slice(start, end);
  assert.match(countsByStatusSource, /\[status\+createdAt\]/);
  assert.doesNotMatch(
    countsByStatusSource,
    /\.toArray\(/,
    '诊断轮询不得为了最老时间戳加载全部 outbox 大对象'
  );
});

test('真实 store flush：严格校验放行，HTTP envelope 带冻结身份，行最终 acked', async () => {
  const committed = [];
  const outbox = createDeltaOutbox({
    store: workbenchOutboxStore,
    requireExecutionIdentity: true,
    autoFlush: false,
    commitDelta: async (taskId, envelope) => {
      committed.push({ taskId, envelope });
      return { success: true, acceptedEventKeys: envelope.events.map((e) => e.idempotencyKey) };
    },
  });

  await outbox.enqueueEvent({
    taskId: 'task-1',
    pluginRunId: 'run-1',
    eventType: 'task.progress',
    sequence: 1001,
    payload: { progress: 50 },
    executionContext: { ...FROZEN_CONTEXT },
  });

  const result = await outbox.flush();
  assert.equal(result.success, true, `flush 必须成功，实际：${JSON.stringify(result)}`);
  assert.equal(committed.length, 1, '必须发出一次 HTTP 提交（此前故障：发 HTTP 前被判 failed_terminal）');
  assert.equal(committed[0].envelope.attemptId, FROZEN_CONTEXT.attemptId);
  assert.equal(committed[0].envelope.leaseToken, FROZEN_CONTEXT.leaseToken);
  assert.equal(committed[0].envelope.leaseEpoch, FROZEN_CONTEXT.leaseEpoch);

  const rows = await db.workbenchOutbox.toArray();
  assert.equal(rows.length, 1);
  assert.equal(rows[0].status, 'acked', `行应 acked，实际 ${rows[0].status}（errorMessage=${rows[0].errorMessage}）`);
  assert.notEqual(rows[0].status, 'failed_terminal');
});
