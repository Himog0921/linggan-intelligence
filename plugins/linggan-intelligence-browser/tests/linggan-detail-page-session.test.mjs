import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createDetailPageSessionStore,
  packageDetailPageSessionLane,
  validateDetailPageSessionPlan,
} from '../src/linggan/detailPageSessionStore.js';

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
