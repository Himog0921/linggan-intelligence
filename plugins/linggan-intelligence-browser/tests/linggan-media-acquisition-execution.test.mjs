import assert from 'node:assert/strict';
import test from 'node:test';

import {
  claimedMediaUpload,
  executeClaimedMediaAcquisition,
  prioritizeMediaUploads,
} from '../src/linggan/mediaAcquisitionExecution.js';

const claim = {
  workRef: '11111111-1111-4111-8111-111111111111',
  mediaObservationRef: '22222222-2222-4222-8222-222222222222',
  claimGeneration: 3,
  componentKind: 'single',
  candidateUris: ['https://sns-webpic-qc.xhscdn.com/cover.webp'],
};

test('claimed media upload keeps the exact server work generation and approved candidates', () => {
  assert.deepEqual(claimedMediaUpload({
    claim,
    installKey: 'installation-1',
    allowCandidate: (value) => value.endsWith('.webp'),
  }), {
    uploadId: `${claim.workRef}:3`,
    serverWorkRef: claim.workRef,
    claimGeneration: 3,
    installKey: 'installation-1',
    mediaObservationRef: claim.mediaObservationRef,
    componentKind: 'single',
    candidateUris: claim.candidateUris,
  });
});

test('claimed media upload upgrades an approved raw HTTP candidate before durable enqueue', () => {
  const rawClaim = {
    ...claim,
    candidateUris: ['http://sns-webpic-qc.xhscdn.com/cover.webp'],
  };
  const upload = claimedMediaUpload({
    claim: rawClaim,
    installKey: 'installation-1',
    normalizeCandidate: (value) => value.replace(/^http:/, 'https:'),
    allowCandidate: (value) => value.startsWith('https://') && value.endsWith('.webp'),
  });
  assert.deepEqual(upload.candidateUris, ['https://sns-webpic-qc.xhscdn.com/cover.webp']);
});

test('the freshly claimed media generation is processed before older browser-local rows', () => {
  const preferred = { uploadId: 'fresh', status: 'pending' };
  const historical = [
    { uploadId: 'old-1', status: 'retryable' },
    { uploadId: 'old-2', status: 'pending' },
  ];
  assert.deepEqual(
    prioritizeMediaUploads(preferred, historical, 2).map((row) => row.uploadId),
    ['fresh', 'old-1'],
  );
  assert.deepEqual(
    prioritizeMediaUploads({ uploadId: 'fresh', status: 'acknowledged' }, historical, 2).map((row) => row.uploadId),
    ['old-1', 'old-2'],
  );
});

test('a post-claim enqueue failure is reported instead of silently expiring its lease', async () => {
  const events = [];
  const result = await executeClaimedMediaAcquisition({
    claim,
    installKey: 'installation-1',
    allowCandidate: () => true,
    scheduleRecovery: async (seconds) => events.push(`alarm:${seconds}`),
    outbox: {
      async enqueue() { events.push('enqueue'); throw new Error('indexed_db_unavailable'); },
      async terminal(uploadId) { events.push(`terminal:${uploadId}`); },
    },
    flush: async (uploadId) => events.push(`flush:${uploadId}`),
    recordFailure: async (upload, error) => {
      events.push(`failure:${upload.serverWorkRef}:${upload.claimGeneration}:${error.message}`);
    },
  });

  assert.equal(result.success, false);
  assert.equal(result.state, 'indexed_db_unavailable');
  assert.equal(result.nextPollAfterSeconds, 60);
  assert.deepEqual(events, [
    'alarm:60',
    'enqueue',
    `failure:${claim.workRef}:3:indexed_db_unavailable`,
    `terminal:${claim.workRef}:3`,
  ]);
});

test('an acknowledged media generation is distinguishable from merely queued work', async () => {
  const rows = new Map();
  const result = await executeClaimedMediaAcquisition({
    claim,
    installKey: 'installation-1',
    allowCandidate: () => true,
    scheduleRecovery: async () => {},
    outbox: {
      async enqueue(upload) { rows.set(upload.uploadId, { ...upload, status: 'pending' }); },
      async get(uploadId) { return rows.get(uploadId); },
    },
    flush: async (uploadId) => {
      assert.equal(uploadId, `${claim.workRef}:3`);
      rows.set(`${claim.workRef}:3`, { status: 'acknowledged' });
    },
    recordFailure: async () => {},
  });

  assert.equal(result.success, true);
  assert.equal(result.state, 'media_generation_acknowledged');
  assert.equal(result.nextPollAfterSeconds, 0);
});
