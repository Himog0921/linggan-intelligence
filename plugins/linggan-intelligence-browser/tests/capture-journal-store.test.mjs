// Capture Journal（采集事实账本）：不可变、append-only、与租约有效性解耦。
// 报告 §9.2：Capture Journal 先落本地，是恢复数据的源头；修剪只允许清理已确认送达且超龄的条目。
import 'fake-indexeddb/auto';
import test from 'node:test';
import assert from 'node:assert/strict';

const { captureJournalStore } = await import('../src/db/captureJournalStore.js');
const { default: db } = await import('../src/db/index.js');
const { createDeltaOutbox } = await import('../src/workbench/runtime/deltaOutbox.js');
const { workbenchOutboxStore } = await import('../src/db/workbenchOutboxStore.js');

function buildEntry(overrides = {}) {
  return {
    entryId: 'entry-1',
    taskId: 'task-1',
    pluginRunId: 'run-1',
    kind: 'record',
    recordType: 'note',
    externalRecordId: 'note-abc',
    payload: { title: '希希妈妈聊 ADHD 第 3 期', likes: 120 },
    capturedAt: Date.now(),
    executionContext: { attemptId: 'attempt-001', leaseToken: 'lt-1', leaseEpoch: 2 },
    pluginVersion: '2.0.84',
    ...overrides,
  };
}

test.beforeEach(async () => {
  await db.captureJournal.clear();
});

test('append 落库并自动计算 payloadHash；重复 entryId 不覆盖（append-only）', async () => {
  const first = await captureJournalStore.append(buildEntry());
  assert.ok(first.payloadHash && first.payloadHash.length === 64, 'payloadHash 应为 sha-256 hex');

  const overwritten = await captureJournalStore.append(
    buildEntry({ payload: { title: '被篡改的内容' } }),
  );
  assert.equal(overwritten.payloadHash, first.payloadHash, '同 entryId 再次 append 必须返回原条目，不得覆盖');

  const rows = await db.captureJournal.toArray();
  assert.equal(rows.length, 1);
  assert.equal(rows[0].payload.title, '希希妈妈聊 ADHD 第 3 期');
});

test('listByTask / listAll 返回采集事实', async () => {
  await captureJournalStore.append(buildEntry({ entryId: 'e-1', taskId: 'task-1' }));
  await captureJournalStore.append(buildEntry({ entryId: 'e-2', taskId: 'task-2' }));

  const taskRows = await captureJournalStore.listByTask('task-1');
  assert.equal(taskRows.length, 1);
  assert.equal(taskRows[0].entryId, 'e-1');

  const all = await captureJournalStore.listAll({ limit: 10 });
  assert.equal(all.length, 2);
});

test('出站链路集成：record 入队先落账本；严格校验拒绝时采集事实也不丢', async () => {
  await db.workbenchOutbox.clear();
  const outbox = createDeltaOutbox({
    store: workbenchOutboxStore,
    captureJournal: captureJournalStore,
    pluginVersion: '2.0.84-test',
    requireExecutionIdentity: true,
    autoFlush: false,
    commitDelta: async () => ({ success: true }),
  });

  const notePayload = { noteId: 'note-ok', title: '正常入队的笔记', url: 'https://www.xiaohongshu.com/explore/note-ok' };

  // 1) 带冻结身份：入队成功且账本有事实
  await outbox.enqueueRecord({
    taskId: 'task-j1',
    pluginRunId: 'run-j1',
    recordType: 'note',
    externalRecordId: 'note-ok',
    payload: notePayload,
    executionContext: { attemptId: 'a-1', leaseToken: 'lt-1', leaseEpoch: 1 },
  });
  let journal = await captureJournalStore.listByTask('task-j1');
  assert.equal(journal.length, 1, 'record 入队必须落账本');
  assert.equal(journal[0].recordType, 'note');
  assert.equal(journal[0].pluginVersion, '2.0.84-test');

  // 2) 无冻结身份：严格校验拒绝入队，但账本必须已保住采集事实
  await assert.rejects(
    outbox.enqueueRecord({
      taskId: 'task-j2',
      pluginRunId: 'run-j2',
      recordType: 'note',
      externalRecordId: 'note-quarantined',
      payload: { noteId: 'note-quarantined', title: '身份缺失但事实要保住', url: 'https://www.xiaohongshu.com/explore/note-q' },
    }),
    /task_report_context_missing/,
  );
  journal = await captureJournalStore.listByTask('task-j2');
  assert.equal(journal.length, 1, '被拒绝入队的采集事实也必须留在账本里');
  const outboxRows = await db.workbenchOutbox.where('taskId').equals('task-j2').toArray();
  assert.equal(outboxRows.length, 0, '出站层不应有被拒绝的行');
});

test('pruneAcked 只清理已确认送达且超龄的条目', async () => {
  const oldMs = Date.now() - 20 * 24 * 60 * 60 * 1000;
  await captureJournalStore.append(buildEntry({ entryId: 'old-acked', capturedAt: oldMs }));
  await captureJournalStore.append(buildEntry({ entryId: 'old-unacked', capturedAt: oldMs }));
  await captureJournalStore.append(buildEntry({ entryId: 'fresh-acked', capturedAt: Date.now() }));

  const ackedSet = new Set(['old-acked', 'fresh-acked']);
  const pruned = await captureJournalStore.pruneAcked({
    olderThanMs: 14 * 24 * 60 * 60 * 1000,
    isAcked: (entryId) => ackedSet.has(entryId),
  });

  assert.equal(pruned, 1, '只有 old-acked 满足「已确认 + 超龄」');
  const remaining = (await db.captureJournal.toArray()).map((row) => row.entryId).sort();
  assert.deepEqual(remaining, ['fresh-acked', 'old-unacked']);
});
