import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';

import {
  createDetailPageSessionStore,
  createDetailPageNavigationGrantStore,
  createDetailPageLanePreparationStore,
  canonicalCaptureTimestamp,
  detailPageSessionExecutionReceipt,
  packageDetailPageSessionLane,
  validateDetailPageSessionPlan,
} from '../src/linggan/detailPageSessionStore.js';
import {
  createTaskSpec,
  decodePageExecutionReceipt,
  detailPageSessionLanePreparationContractFromHealth,
  grantLingganDetailPageSession,
  normalizeDetailPageLanePreparation,
} from '../src/linggan/adapter.js';

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

function lanePreparationTable() {
  const rows = new Map();
  return {
    async get(key) { return rows.get(key) ? structuredClone(rows.get(key)) : undefined; },
    async put(row) { rows.set(row.taskId, structuredClone(row)); },
    size() { return rows.size; },
  };
}

function grantResponse(overrides = {}) {
  return {
    outcome: 'authorized',
    sessionRef: 'session-1',
    pageSessionPlan: plan,
    ...overrides,
  };
}

const grantHealth = {
  routes: {
    dispatch: {
      detailPageSessionGrant: '/api/local/dispatch/detail-page-sessions/grant',
    },
  },
};

const handshakeHealth = {
  routes: {
    dispatch: {
      detailPageSessionGrant: '/api/local/dispatch/detail-page-sessions/grant',
      detailPageSessionLanePreparationContract: 'linggan.detail-page-session.lane-preparation.v1',
    },
  },
};

function lanePreparationReceipt() {
  return {
    contractVersion: 'linggan.detail-page-session.lane-preparation.v1',
    sessionRef: 'session-1',
    lanes: [
      { capability: 'content_detail', taskId: '00000000-0000-4000-8000-000000000001', attemptId: '00000000-0000-4000-8000-000000000005' },
      { capability: 'media_slots', taskId: '00000000-0000-4000-8000-000000000002', attemptId: '00000000-0000-4000-8000-000000000006' },
      { capability: 'comments', taskId: '00000000-0000-4000-8000-000000000003', attemptId: '00000000-0000-4000-8000-000000000007' },
      { capability: 'replies', taskId: '00000000-0000-4000-8000-000000000004', attemptId: '00000000-0000-4000-8000-000000000008' },
    ].map((lane) => ({ ...lane, taskSpec: task(lane.capability, lane.taskId),
      leaseExpiresAt: '2026-09-21T16:00:00Z' })),
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
  return createTaskSpec({
    taskId: /^[0-9a-f-]{36}$/.test(taskId) ? taskId : `00000000-0000-4000-8000-${String(['content_detail','media_slots','comments','replies'].indexOf(capability)+1).padStart(12,'0')}`,
    source: 'scheduled', platform: 'xhs', pageType: 'note_detail',
    target: { contentExternalId: 'note-1' }, capabilitiesRequested: [capability],
    maximumQuota: 1, riskPolicy: 'server_authorized_leased', stopConditions: ['maximum_quota'],
  });
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
  const taskSpec = task('content_detail', '00000000-0000-4000-8000-000000000001');
  const receipt = detailPageSessionExecutionReceipt({
    action: 'lingganCollectNoteFull',
    taskSpec,
    delivery: 'pending',
    submissionId: 'submission-1',
  });
  assert.deepEqual(
    decodePageExecutionReceipt(receipt, {
      action: 'lingganCollectNoteFull', capability: 'content_detail', taskId: '00000000-0000-4000-8000-000000000001',
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

  await store.markTaskQueued(entry.cacheKey, 'comments', task('comments').taskId);
  assert.equal(
    (await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('comments') })).alreadyQueued,
    true,
  );

  now += 121_000;
  assert.ok(await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('media_slots') }),
    'a pending frozen lane remains deliverable after the original lease TTL');
});

test('an epoch-millisecond page observation is canonicalized before every frozen lane packages it', async () => {
  const store = createDetailPageSessionStore(memoryTable(), () => 1_000);
  const observedAt = 1789908628809;
  const entry = await store.put({
    leaseRef: 'lease-1', plan, note: { ...note, observedAt }, commentResult, receipt: {},
  });
  assert.equal(entry.observedAt, '2026-09-20T12:50:28.809Z');
  assert.equal(canonicalCaptureTimestamp(observedAt), entry.observedAt);
  const media = packageDetailPageSessionLane(
    await store.getForTask({ leaseRef: 'lease-1', taskSpec: task('media_slots') }),
    task('media_slots'),
  );
  assert.equal(media.observedAt, entry.observedAt);
});

test('an invalid observedAt falls through to the collector observed timestamp instead of inventing now', async () => {
  const store = createDetailPageSessionStore(memoryTable(), () => 9_999);
  const entry = await store.put({
    leaseRef: 'lease-1', plan,
    note: { ...note, observedAt: 'not-a-time', collectedAt: 1789908628809 },
    commentResult, receipt: {},
  });
  assert.equal(entry.observedAt, '2026-09-20T12:50:28.809Z');
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

test('a damaged persisted page session is rejected instead of becoming a retryable later lane', async () => {
  const table = memoryTable();
  const store = createDetailPageSessionStore(table, () => 1_000);
  const entry = await store.put({ leaseRef: 'lease-1', plan, note, commentResult, receipt: {} });
  await table.put({
    ...entry,
    plan: { ...entry.plan, lanes: ['content_detail', 'comments', 'unknown_lane'] },
  });
  await assert.rejects(
    store.getForTask({ leaseRef: 'lease-1', taskSpec: task('comments') }),
    /detail_page_session_lanes_invalid/,
  );
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

test('a service without preparation cannot authorize new navigation', async () => {
  let sentBody = null;
  const result = await grantLingganDetailPageSession({
    installKey: 'install-1',
    installationCredential: 'credential-1',
    taskId: '00000000-0000-4000-8000-000000000001',
    grantRequestId: 'request-1',
    executionSourceUrl: 'https://www.xiaohongshu.com/explore/note-1?xsec_token=token',
    health: grantHealth,
    fetchImpl: async (url, options) => {
      sentBody = JSON.parse(options.body);
      return { ok: true, async json() { return grantResponse(); } };
    },
  });
  assert.equal(detailPageSessionLanePreparationContractFromHealth(grantHealth), '');
  assert.equal(sentBody, null, 'incompatible services receive no new navigation grant request');
  assert.equal(result.granted, false);
  assert.equal(result.reasonCode, 'grant_lane_preparation_unsupported');
});

test('an announced handshake is requested, and a missing receipt never authorizes a page open', async () => {
  let sentBody = null;
  const fetchImpl = async (url, options) => {
    sentBody = JSON.parse(options.body);
    return { ok: true, async json() { return grantResponse(); } };
  };
  const args = {
    installKey: 'install-1',
    installationCredential: 'credential-1',
    taskId: '00000000-0000-4000-8000-000000000001',
    grantRequestId: 'request-1',
    executionSourceUrl: 'https://www.xiaohongshu.com/explore/note-1?xsec_token=token',
    health: handshakeHealth,
    fetchImpl,
  };
  const refused = await grantLingganDetailPageSession(args);
  assert.equal(sentBody.lanePreparationContract, 'linggan.detail-page-session.lane-preparation.v1');
  assert.equal(refused.granted, false);
  assert.equal(refused.reasonCode, 'grant_lane_preparation_missing');

  const accepted = await grantLingganDetailPageSession({
    ...args,
    fetchImpl: async () => ({
      ok: true,
      async json() { return grantResponse({ lanePreparation: lanePreparationReceipt() }); },
    }),
  });
  assert.equal(accepted.granted, true);
  assert.deepEqual(accepted.lanePreparation.lanes, lanePreparationReceipt().lanes);
});

test('a preparation receipt must answer the exact contract, session and frozen lane set', () => {
  const expected = { contractVersion: 'linggan.detail-page-session.lane-preparation.v1', sessionRef: 'session-1' };
  const receipt = lanePreparationReceipt();
  assert.deepEqual(normalizeDetailPageLanePreparation(receipt, expected), receipt);
  assert.equal(normalizeDetailPageLanePreparation({ ...receipt, contractVersion: 'other' }, expected), null);
  assert.equal(normalizeDetailPageLanePreparation({ ...receipt, sessionRef: 'session-2' }, expected), null);
  assert.equal(normalizeDetailPageLanePreparation({ ...receipt, lanes: [] }, expected), null);
  assert.equal(normalizeDetailPageLanePreparation({
    ...receipt,
    lanes: [{ capability: 'author_profile', taskId: 't', attemptId: 'a' }],
  }, expected), null, 'a lane outside the frozen plan is not a preparation this browser may act on');
  assert.equal(normalizeDetailPageLanePreparation({
    ...receipt,
    lanes: [{ capability: 'content_detail', taskId: 't' }],
  }, expected), null, 'an identity without a server-minted attempt id is not a preparation');
});

test('prepared lane identities are persisted with the navigation grant and cannot be re-pointed', async () => {
  const lanePreparations = createDetailPageLanePreparationStore({
    table: lanePreparationTable(),
    transaction: (work) => work(),
    now: () => 1000,
  });
  const grantStore = createDetailPageNavigationGrantStore({
    table: navigationGrantTable(),
    lanePreparationStore: lanePreparations,
    transaction: serializedTransaction(),
    now: () => 1000,
    createRequestId: () => 'request-1',
  });
  const taskSpec = task('content_detail', '00000000-0000-4000-8000-000000000001');
  const prepared = await grantStore.prepare({ leaseRef: 'lease-1', taskSpec });
  await grantStore.attachServerGrant({
    grantKey: prepared.grantKey, sessionRef: 'session-1', plan,
    lanePreparation: lanePreparationReceipt(),
  });
  const media = await lanePreparations.get('00000000-0000-4000-8000-000000000002');
  assert.equal(media.attemptId, '00000000-0000-4000-8000-000000000006');
  assert.equal(media.leaseRef, 'lease-1');
  assert.equal(media.contentExternalId, 'note-1');

  // 同一身份重放是幂等的；换成另一个身份就是给同一通道改身份，必须拒绝。
  await lanePreparations.recordLanes({
    sessionRef: 'session-1', leaseRef: 'lease-1', contentExternalId: 'note-1',
    lanes: [lanePreparationReceipt().lanes[1]],
  });
  await assert.rejects(
    lanePreparations.recordLanes({
      sessionRef: 'session-1', leaseRef: 'lease-1', contentExternalId: 'note-1',
      lanes: [{ ...lanePreparationReceipt().lanes[1], attemptId: 'attempt-other' }],
    }),
    /lane_preparation_conflict/,
  );
  await assert.rejects(
    lanePreparations.recordLanes({
      sessionRef: 'session-1', leaseRef: 'lease-1', contentExternalId: 'note-1',
      lanes: [{ ...lanePreparationReceipt().lanes[1], capability: 'comments' }],
    }),
    /lane_preparation_(conflict|invalid)/,
  );
});

test('lane delivery uses the server-minted identity and stops without one', async () => {
  const background = await readFile(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const lookup = background.indexOf('const preparedAttemptId = await preparedAttemptIdForTask(taskSpec.taskId)');
  const derived = background.indexOf('await deterministicUuid(`attempt:${instanceId}:${taskSpec.taskId}:${stableKey}`)');
  assert.ok(lookup >= 0 && derived > lookup,
    'a prepared identity must win over the locally derived attempt id');
  assert.match(background, /const attemptId = preparedAttemptId \|\| \(stableKey/);
  assert.match(background, /detailPageLanePreparationStore\.get\(key\)/);
  assert.match(background, /state: 'detail_page_session_lane_preparation_unavailable'/);
  assert.match(background, /'detail_page_session_lane_preparation_unavailable',/);
});
