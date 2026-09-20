import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

import {
  createDetailPageSessionStore,
  createDetailPageNavigationGrantStore,
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

function navigationGrantTable() {
  const rows = new Map();
  return {
    async get(key) { return rows.get(key) ? structuredClone(rows.get(key)) : undefined; },
    async add(row) {
      if (rows.has(row.grantKey)) {
        const error = new Error('duplicate'); error.name = 'ConstraintError'; throw error;
      }
      rows.set(row.grantKey, structuredClone(row));
    },
    async put(row) { rows.set(row.grantKey, structuredClone(row)); },
    async update(key, patch) { rows.set(key, { ...rows.get(key), ...structuredClone(patch) }); },
  };
}

function serializedTransaction() {
  let tail = Promise.resolve();
  return (work) => {
    const next = tail.then(work);
    tail = next.catch(() => {});
    return next;
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
  assert.ok(await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('media_slots') }),
    'a pending frozen lane remains deliverable after the original lease TTL');
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

test('a page payload outlives its lease but cannot be adopted by a new lease', async () => {
  let now = 1_000;
  const table = memoryTable();
  const store = createDetailPageSessionStore(table, () => now);
  await store.put({ leaseRef: 'lease-old', plan: { ...plan, cacheTtlSeconds: 1 }, note, commentResult, receipt: {} });
  now += 2_000;
  assert.ok(await store.getForTask({ leaseRef: 'lease-old', taskSpec: task('comments') }),
    'already-read comments remain available to their original lease after its expiry');
  assert.equal(await store.getForTask({ leaseRef: 'lease-new', taskSpec: task('comments') }), null,
    'a new lease must not silently attach an old page payload to a new task');
  const entry = await store.getForTask({ leaseRef: 'lease-old', taskSpec: task('content_detail') });
  for (const capability of plan.lanes) {
    await store.markTaskQueued(entry.cacheKey, capability, `${capability}-task`);
  }
  assert.equal(await store.pruneExpired(), 1,
    'the local page payload is removable only after every frozen lane is durably queued');
  assert.equal(await store.getForTask({ leaseRef: 'lease-old', taskSpec: task('comments') }), null);
});

test('two replayed dispatch wakeups consume one persisted navigation grant only once', async () => {
  let requestNo = 0;
  const ledger = createDetailPageNavigationGrantStore({
    table: navigationGrantTable(),
    transaction: serializedTransaction(),
    now: () => 1_000,
    createRequestId: () => `request-${++requestNo}`,
  });
  const taskSpec = task('content_detail', 'detail-task-atomic');
  const [first, second] = await Promise.all([
    ledger.prepare({ leaseRef: 'lease-atomic', taskSpec }),
    ledger.prepare({ leaseRef: 'lease-atomic', taskSpec }),
  ]);
  assert.equal(first.grantRequestId, 'request-1');
  assert.equal(second.grantRequestId, 'request-1', 'the same task retry reuses its request id');
  const accepted = await ledger.attachServerGrant({ grantKey: first.grantKey, sessionRef: 'session-1', plan });
  assert.equal(accepted.state, 'grant_accepted', 'prepared alone must not consume a navigation budget');
  const consumed = await Promise.all([
    ledger.consume({ grantKey: first.grantKey, sessionRef: 'session-1' }),
    ledger.consume({ grantKey: first.grantKey, sessionRef: 'session-1' }),
  ]);
  assert.equal(consumed.filter((entry) => entry.shouldNavigate).length, 1);
  assert.equal(consumed.filter((entry) => !entry.shouldNavigate).length, 1);
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
  assert.match(background, /detailPageNavigationGrantStore\.consume/);
  assert.match(background, /capability !== 'content_detail'/);
  assert.ok(
    content.indexOf('runtime.submitContentDetail') < content.indexOf('STORE_DETAIL_PAGE_SESSION'),
    'a cache hand-off failure must not discard the first durable content delivery',
  );
});

test('the detail body is handed off before comments, and a timeout is not reported as navigation', async () => {
  const [collector, content, background] = await Promise.all([
    readFile(new URL('../src/platforms/xhs/detailPackageCollector.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/content/index.js', import.meta.url), 'utf8'),
    readFile(new URL('../src/linggan/background.js', import.meta.url), 'utf8'),
  ]);
  const readNote = collector.indexOf('const note = await collectNote');
  const handoff = collector.indexOf('await options.onDetailReady(note)');
  const comments = collector.indexOf('await collectComments');
  assert.ok(readNote >= 0 && handoff > readNote && comments > handoff,
    'the content body must reach its durable handoff before the slow comment crawl');
  assert.match(content, /onDetailReady: queueDetailBeforeComments/);
  assert.match(content, /contentDelivery \|\| await runtime\.submitContentDetail/);

  const ready = background.indexOf('const ready = await waitForTabReady(tabId)');
  const timeout = background.indexOf("stopReason: 'page_unavailable'", ready);
  const observed = background.indexOf("kind: 'navigation_observed'", ready);
  assert.ok(ready >= 0 && timeout > ready && observed > timeout,
    'a readiness timeout records a stop; navigation is observed only after readiness succeeds');
  assert.match(background, /tabUrl\.includes\(contentExternalId\).*tabUrl\.includes\(encodedContentId\)/s,
    'a recovered tab must still identify the expected content, not only the XHS domain');
});
