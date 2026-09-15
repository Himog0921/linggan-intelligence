import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

import {
  createDetailPageSessionStore,
  detailPageSessionExecutionReceipt,
  packageDetailPageSessionLane,
  validateDetailPageSessionPlan,
} from '../src/linggan/detailPageSessionStore.js';
import { decodePageExecutionReceipt } from '../src/linggan/adapter.js';

function memoryTable() {
  const rows = new Map();
  return {
    async get(key) { return rows.get(key); },
    async put(row) { rows.set(row.cacheKey, structuredClone(row)); },
    async update(key, patch) { rows.set(key, { ...rows.get(key), ...structuredClone(patch) }); },
    async delete(key) { rows.delete(key); },
    where(field) {
      return {
        belowOrEqual(value) {
          return {
            async primaryKeys() {
              return [...rows.values()].filter((row) => row[field] <= value).map((row) => row.cacheKey);
            },
          };
        },
      };
    },
    async bulkDelete(keys) { keys.forEach((key) => rows.delete(key)); },
    async count() { return rows.size; },
    orderBy(field) {
      return {
        limit(size) {
          return {
            async primaryKeys() {
              return [...rows.values()]
                .sort((left, right) => Number(left[field]) - Number(right[field]))
                .slice(0, size)
                .map((row) => row.cacheKey);
            },
          };
        },
      };
    },
    size() { return rows.size; },
  };
}

function task(capability, taskId = `${capability}-task`) {
  return {
    taskId,
    source: 'scheduled',
    platform: 'xhs',
    target: { contentExternalId: 'note-1' },
    capabilitiesRequested: [capability],
  };
}

const plan = {
  contractVersion: 'linggan.detail-page-session.v1',
  contentExternalId: 'note-1',
  lanes: ['content_detail', 'media_slots', 'comments', 'replies'],
  commentLimit: 30,
  replyExpandLimit: 2,
  cacheTtlSeconds: 120,
};

const note = {
  noteId: 'note-1',
  title: '真实标题',
  coverUrl: 'https://ci.xiaohongshu.com/cover.jpg',
  images: [{ url: 'https://ci.xiaohongshu.com/image.jpg' }],
};

const commentResult = {
  total: 2,
  stopReason: 'maximum_quota',
  comments: [
    { commentId: 'comment-1', content: '主评论' },
    { commentId: 'reply-1', rootCommentId: 'comment-1', content: '回复' },
  ],
};

test('detail page plan is bounded to one content and the approved closed lane set', () => {
  assert.deepEqual(validateDetailPageSessionPlan(plan, 'note-1').lanes, plan.lanes);
  assert.throws(
    () => validateDetailPageSessionPlan({ ...plan, contentExternalId: 'other' }, 'note-1'),
    /target_mismatch/,
  );
  assert.throws(
    () => validateDetailPageSessionPlan({ ...plan, lanes: [...plan.lanes, 'author_profile'] }, 'note-1'),
    /lanes_invalid/,
  );
});

test('the first detail lane returns the exact dispatched page identity', () => {
  const taskSpec = task('content_detail', 'detail-task-1');
  const receipt = detailPageSessionExecutionReceipt({
    action: 'lingganCollectNoteFull',
    taskSpec,
    delivery: 'pending',
    submissionId: 'submission-1',
  });
  assert.deepEqual(
    decodePageExecutionReceipt(receipt, {
      action: 'lingganCollectNoteFull', capability: 'content_detail', taskId: 'detail-task-1',
    }),
    { ok: true, state: 'detail_page_session_queued', message: receipt.message },
  );
  assert.throws(
    () => detailPageSessionExecutionReceipt({ action: 'lingganCollectNoteFull', taskSpec: task('comments') }),
    /receipt_identity_invalid/,
  );
});

test('one persistent page result produces separate packages only for separately claimed tasks', async () => {
  let now = 1_000;
  const store = createDetailPageSessionStore(memoryTable(), () => now);
  const entry = await store.put({ leaseRef: 'lease-1', plan, note, commentResult, receipt: {} });

  for (const capability of plan.lanes) {
    const taskSpec = task(capability);
    const cached = await store.getForTask({ leaseRef: 'lease-1', taskSpec });
    assert.equal(cached.alreadyQueued, false);
    const capturePackage = packageDetailPageSessionLane(cached, taskSpec);
    assert.equal(capturePackage.packageKind, capability);
    assert.equal(capturePackage.coverage.target.contentExternalId, 'note-1');
    assert.ok(capturePackage.records.every((record) => record.sourceObject.externalId === 'note-1'));
  }

  await store.markTaskQueued(entry.cacheKey, 'comments', 'comments-task');
  assert.equal(
    (await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('comments') })).alreadyQueued,
    true,
  );

  now += 121_000;
  assert.equal(await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('media_slots') }), null);
});

test('cached comment and reply lanes keep their content source identity', async () => {
  const store = createDetailPageSessionStore(memoryTable(), () => 1_000);
  await store.put({ leaseRef: 'lease-1', plan, note, commentResult, receipt: {} });
  const commentsTask = task('comments');
  const repliesTask = task('replies');
  const commentsEntry = await store.getForTask({ leaseRef: 'lease-1', taskSpec: commentsTask });
  const repliesEntry = await store.getForTask({ leaseRef: 'lease-1', taskSpec: repliesTask });
  assert.equal(packageDetailPageSessionLane(commentsEntry, commentsTask).records[0].kind, 'comment');
  assert.equal(packageDetailPageSessionLane(repliesEntry, repliesTask).records[0].kind, 'reply');
});

test('put prunes expired detail sessions without a separate maintenance action', async () => {
  let now = 1_000;
  const table = memoryTable();
  const store = createDetailPageSessionStore(table, () => now);
  await store.put({ leaseRef: 'lease-old', plan: { ...plan, cacheTtlSeconds: 1 }, note, commentResult, receipt: {} });
  now += 2_000;
  await store.put({ leaseRef: 'lease-new', plan, note, commentResult, receipt: {} });
  assert.equal(table.size(), 1);
  assert.equal(await store.getForTask({ leaseRef: 'lease-old', taskSpec: task('content_detail') }), null);
  assert.ok(await store.getForTask({ leaseRef: 'lease-new', taskSpec: task('content_detail') }));
});

test('the XHS content context hands a completed session to the background-owned cache', async () => {
  const [content, background] = await Promise.all([
    readFile(new URL('../src/content/index.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/linggan/background.js', import.meta.url), 'utf8'),
  ]);
  assert.doesNotMatch(content, /detailPageSessionStore\.put/);
  assert.match(content, /STORE_DETAIL_PAGE_SESSION/);
  assert.match(content, /MARK_DETAIL_PAGE_SESSION_TASK_QUEUED/);
  assert.match(background, /storeDetailPageSessionFromPage/);
  assert.match(background, /detailPageSessionStore\.put/);
});
