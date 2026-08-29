import assert from 'node:assert/strict';
import test from 'node:test';
import { IDBKeyRange, indexedDB } from 'fake-indexeddb';

globalThis.indexedDB = indexedDB;
globalThis.IDBKeyRange = IDBKeyRange;

const { default: Dexie } = await import('dexie');
const { createLocalMediaOutbox, createLocalProducerOutbox } = await import('../src/linggan/localProducerOutbox.js');
const { createLocalAttempt, createLocalSubmission, createManualTaskSpec } = await import('../src/linggan/adapter.js');

test('manual TaskSpec remains flat while scheduled control is never fabricated', () => {
  const spec = createManualTaskSpec({ taskId: '11111111-1111-4111-8111-111111111111' });
  assert.deepEqual(spec, {
    contractVersion: 'linggan.producer.task-spec.v1',
    taskId: '11111111-1111-4111-8111-111111111111', source: 'manual', platform: 'xhs',
    pageType: 'search_results', target: { query: '__manual_placeholder__', surface: 'current_visible_search_surface' },
    capabilitiesRequested: ['discovery_search'], maximumQuota: 20,
    commentLimit: 'not_requested', acquireMedia: 'not_requested',
    riskPolicy: 'local_trusted_user_initiated',
    stopConditions: ['current_surface_read_once', 'maximum_quota'],
  });
});

test('outbox preserves one submission across an in-flight timeout and acknowledgement', async () => {
  const database = new Dexie(`linggan-test-${crypto.randomUUID()}`);
  database.version(1).stores({ submissions: '&submissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]' });
  let timestamp = 100;
  const outbox = createLocalProducerOutbox(database.submissions, () => timestamp);
  const taskSpec = createManualTaskSpec({ taskId: '11111111-1111-4111-8111-111111111111' });
  const attempt = createLocalAttempt({
    producerInstanceId: '22222222-2222-4222-8222-222222222222', taskId: taskSpec.taskId,
    attemptId: '33333333-3333-4333-8333-333333333333',
  });
  const submission = createLocalSubmission({
    producerInstanceId: attempt.producerInstanceId, taskId: taskSpec.taskId, attemptId: attempt.attemptId,
    submissionId: '44444444-4444-4444-8444-444444444444', discoveryPackage: { contractVersion: 'synthetic' },
  });
  await outbox.enqueue({ ...submission, taskSpec, attempt });
  await outbox.markInFlight(submission.submissionId, { at: timestamp, timeoutMs: 10 });
  timestamp = 111;
  const afterRestart = await outbox.due({ at: timestamp });
  assert.equal(afterRestart.length, 1);
  assert.equal(afterRestart[0].submissionId, submission.submissionId);
  assert.equal(afterRestart[0].status, 'retryable');
  await outbox.acknowledge(submission.submissionId, { delivery: 'acknowledged' }, { at: timestamp });
  assert.equal(await outbox.pendingCount(), 0);
  await database.delete();
});

test('media outbox restores an interrupted local upload without losing its independent lane', async () => {
  const database = new Dexie(`linggan-media-test-${crypto.randomUUID()}`);
  database.version(1).stores({ mediaUploads: '&uploadId, slotSubmissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]' });
  let timestamp = 100;
  const outbox = createLocalMediaOutbox(database.mediaUploads, () => timestamp);
  await outbox.enqueue({
    uploadId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    slotSubmissionId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
    mediaObservationRef: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
    candidateUris: ['https://example.test/media.jpg'],
  });
  await outbox.markInFlight('aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', { at: timestamp, timeoutMs: 10 });
  timestamp = 111;
  const due = await outbox.due({ at: timestamp });
  assert.equal(due.length, 1);
  assert.equal(due[0].status, 'retryable');
  await database.delete();
});

test('media outbox stops a local-only delivery after three failed attempts', async () => {
  const database = new Dexie(`linggan-media-limit-${crypto.randomUUID()}`);
  database.version(1).stores({ mediaUploads: '&uploadId, slotSubmissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]' });
  const outbox = createLocalMediaOutbox(database.mediaUploads, () => 100);
  await outbox.enqueue({
    uploadId: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
    serverWorkRef: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
    mediaObservationRef: 'ffffffff-ffff-4fff-8fff-ffffffffffff',
    candidateUris: ['https://sns-img-hw.xhscdn.com/cover.jpg'],
  });
  await outbox.retry('dddddddd-dddd-4ddd-8ddd-dddddddddddd', 'one');
  await outbox.retry('dddddddd-dddd-4ddd-8ddd-dddddddddddd', 'two');
  await outbox.retry('dddddddd-dddd-4ddd-8ddd-dddddddddddd', 'three');
  const row = await outbox.get('dddddddd-dddd-4ddd-8ddd-dddddddddddd');
  assert.equal(row.status, 'terminal');
  assert.equal(row.attempts, 3);
  await database.delete();
});
