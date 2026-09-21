import assert from 'node:assert/strict';
import test from 'node:test';
import { indexedDB, IDBKeyRange } from 'fake-indexeddb';

globalThis.indexedDB = indexedDB;
globalThis.IDBKeyRange = IDBKeyRange;
const { default: Dexie } = await import('dexie');
let messageHandler;
globalThis.fetch = async () => { throw new Error('network_disabled_in_fixture'); };
globalThis.chrome = {
  runtime: { onMessage: { addListener: (fn) => { messageHandler = fn; } }, getManifest: () => ({ version: 'fixture' }) },
  tabs: { query: async () => [], create: async () => { throw new Error('navigation_forbidden_in_fixture'); } },
  storage: { local: { get: async (key) => ({ [key]: '10000000-0000-4000-8000-000000000001' }), set: async () => {} } },
};
const { recoverCachedDetailPageSessions, flushLocalOutboxOnce } = await import('../src/linggan/background.js');
const { createDetailPageSessionStore, createDetailPageLanePreparationStore, detailPageLanePreparationStore } = await import('../src/linggan/detailPageSessionStore.js');
const { createLocalProducerOutbox, localProducerOutbox } = await import('../src/linggan/localProducerOutbox.js');
const { createTaskSpec, createLocalAttempt, createLocalSubmission } = await import('../src/linggan/adapter.js');
const instance = '10000000-0000-4000-8000-000000000001';
const lanes = ['content_detail', 'media_slots', 'comments', 'replies'];
function spec(capability, index = lanes.indexOf(capability)) {
  return createTaskSpec({ taskId: `20000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
    source: 'scheduled', platform: 'xhs', pageType: 'note_detail', target: { contentExternalId: 'synthetic-note' },
    capabilitiesRequested: [capability], maximumQuota: 1, riskPolicy: 'server_authorized_leased', stopConditions: ['maximum_quota'] });
}
function database(name) {
  const db = new Dexie(name);
  db.version(1).stores({
    sessions: '&cacheKey, leaseRef, contentExternalId, prunableAt, updatedAt, [prunableAt+updatedAt]',
    preparations: '&taskId, leaseRef',
    submissions: '&submissionId, &idempotencyKey, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  });
  return db;
}
function stores(db, clock) {
  const sessions = createDetailPageSessionStore(db.sessions, clock);
  const preparations = createDetailPageLanePreparationStore({ table: db.preparations, transaction: (fn) => db.transaction('rw', db.preparations, fn), now: clock });
  const outbox = createLocalProducerOutbox(db.submissions, clock);
  const queue = async ({ taskSpec, capturePackage, idempotencyKey, deliveryNotBefore }) => {
    const lane = await preparations.get(taskSpec.taskId);
    const attempt = createLocalAttempt({ producerInstanceId: instance, taskId: taskSpec.taskId, attemptId: lane.attemptId });
    return outbox.enqueue({ ...createLocalSubmission({ ...attempt, capturePackage }), taskSpec, attempt, idempotencyKey, deliveryNotBefore });
  };
  return { sessions, preparations, outbox, queue, queueMedia: queue };
}

test('cached frozen lanes recover across worker restart and lease expiry without another claim or navigation', async () => {
  const name = `session-recovery-${crypto.randomUUID()}`;
  let now = 1000;
  let db = database(name);
  let ports = stores(db, () => now);
  const expiry = new Date(2000).toISOString();
  await ports.preparations.recordLanes({ sessionRef: 'session', leaseRef: 'original-lease', contentExternalId: 'synthetic-note',
    lanes: lanes.map((capability, index) => ({ capability, taskId: spec(capability).taskId, taskSpec: spec(capability),
      attemptId: `30000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`, leaseExpiresAt: expiry })) });
  const entry = await ports.sessions.put({ leaseRef: 'original-lease',
    plan: { contractVersion: 'linggan.detail-page-session.v1', contentExternalId: 'synthetic-note', lanes, commentLimit: 30, replyExpandLimit: 2, cacheTtlSeconds: 1 },
    note: { noteId: 'synthetic-note', title: 'synthetic', observedAt: '2026-09-21T00:00:00Z', images: [{ url: 'https://example.test/a.jpg' }] },
    commentResult: { total: 0, comments: [], stopReason: 'surface_ended' } });
  // The body was already queued; a lost queue-marker response must replay its exact immutable package.
  const body = await ports.sessions.freezePackage({ cacheKey: entry.cacheKey, taskSpec: spec('content_detail') });
  const original = await ports.queue({ taskSpec: spec('content_detail'), capturePackage: body,
    idempotencyKey: `detail-session:${spec('content_detail').taskId}:content_detail` });
  const realMark = ports.sessions.markTaskQueued;
  let interrupted = false;
  ports.sessions.markTaskQueued = async (...args) => {
    if (args[1] === 'comments' && !interrupted) { interrupted = true; throw new Error('worker interrupted after enqueue'); }
    return realMark(...args);
  };
  const first = await recoverCachedDetailPageSessions(ports);
  assert.equal(first.needsAttention, 1);
  assert.equal(await db.submissions.count(), 4, 'one failing marker does not discard other lanes');
  assert.equal((await ports.outbox.due()).length, 1, 'unclaimed live lanes wait for their claim or lease end');
  const frozen = await db.submissions.toArray();
  db.close();
  db = database(name); ports = stores(db, () => now);
  const recovered = await recoverCachedDetailPageSessions(ports);
  assert.equal(recovered.needsAttention, 0);
  assert.equal(await db.submissions.count(), 4);
  assert.deepEqual((await db.submissions.toArray()).map((row) => row.capturePackage), frozen.map((row) => row.capturePackage));
  assert.equal((await ports.outbox.get(original.submissionId)).submissionId, original.submissionId);
  now = 3000;
  const posted = [];
  await flushLocalOutboxOnce({ outbox: ports.outbox, mediaOutbox: { pendingCount: async () => 0 },
    recoverSessions: () => recoverCachedDetailPageSessions(ports), flushMedia: async () => {},
    readReadiness: async () => ({ deliveryReady: true, producerRoutes: { taskCreation: 'task', attemptStart: 'attempt', submission: 'submission' } }),
    post: async (route, value) => {
      posted.push([route, structuredClone(value)]);
      return { ok: true, status: 200, payload: route === 'submission'
        ? { delivery: 'acknowledged', material_admission: 'ACCEPTED', execution_effect: 'LOST_AUTHORITY' }
        : { outcome: route === 'task' ? 'replay' : 'started' } };
    } });
  assert.equal(await ports.outbox.pendingCount(), 0);
  assert.equal(posted.filter(([route]) => route === 'submission').length, 4);
  assert.deepEqual(new Set(posted.filter(([route]) => route === 'attempt').map(([, value]) => value.attemptId)),
    new Set(frozen.map((row) => row.attemptId)));
  await db.delete();
});

test('reading a prepared identity unsuccessfully never acknowledges a replacement envelope', async () => {
  const previousGet = detailPageLanePreparationStore.get;
  const previousEnqueue = localProducerOutbox.enqueue;
  let writes = 0;
  detailPageLanePreparationStore.get = async () => { throw new Error('storage_unavailable'); };
  localProducerOutbox.enqueue = async () => { writes += 1; };
  try {
    const result = await new Promise((resolve) => messageHandler({ action: 'lingganSubmitCapturePackage',
      taskSpec: spec('content_detail'), capturePackage: { packageRef: crypto.randomUUID(), records: [] }, idempotencyKey: 'frozen' }, {}, resolve));
    assert.equal(result.success, false);
    assert.equal(writes, 0);
  } finally { detailPageLanePreparationStore.get = previousGet; localProducerOutbox.enqueue = previousEnqueue; }
});

test('different frozen content under one identity conflicts in serial and concurrent enqueue', async () => {
  const db = database(`outbox-conflict-${crypto.randomUUID()}`);
  const outbox = createLocalProducerOutbox(db.submissions);
  const base = { producerInstanceId: instance, taskId: spec('comments').taskId, attemptId: crypto.randomUUID(), submissionId: crypto.randomUUID(), idempotencyKey: 'frozen' };
  const results = await Promise.allSettled([
    outbox.enqueue({ ...base, capturePackage: { records: [{ value: 'one' }] } }),
    outbox.enqueue({ ...base, capturePackage: { records: [{ value: 'two' }] } }),
  ]);
  assert.equal(results.filter((row) => row.status === 'fulfilled').length, 1);
  assert.match(String(results.find((row) => row.status === 'rejected').reason), /identity_content_conflict/);
  const retained = await outbox.get(base.submissionId);
  await assert.rejects(outbox.enqueue({ ...base, capturePackage: { records: [{ value: 'three' }] } }), /identity_content_conflict/);
  assert.deepEqual(await outbox.get(base.submissionId), retained);
  await db.delete();
});


test('concurrent different session facts cannot overwrite the first durable observation', async () => {
  const db = database(`session-conflict-${crypto.randomUUID()}`);
  const { sessions } = stores(db, () => 1000);
  const base = { leaseRef: 'lease', plan: { contractVersion: 'linggan.detail-page-session.v1',
    contentExternalId: 'synthetic-note', lanes, commentLimit: 30, replyExpandLimit: 2, cacheTtlSeconds: 1 } };
  const results = await Promise.allSettled(['first', 'second'].map((title) => sessions.put({ ...base,
    note: { noteId: 'synthetic-note', title } })));
  assert.deepEqual(results.map((r) => r.status), ['fulfilled', 'rejected']);
  assert.match(results[1].reason.message, /detail_page_session_content_conflict/);
  assert.equal((await db.sessions.toArray())[0].note.title, 'first');
  await db.delete();
});
