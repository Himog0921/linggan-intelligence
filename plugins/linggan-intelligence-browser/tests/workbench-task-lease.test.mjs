import test from 'node:test';
import assert from 'node:assert/strict';

import { createTaskPoller } from '../src/workbench/runtime/taskPoller.js';
import {
  claimCollectionTaskLease,
  commitCollectionTaskDeltaThroughSync,
  createTaskLeaseMemoryStore,
  createTaskLeaseIdleSnapshot,
  formatTaskLeaseIdleNotice,
  createTaskLeaseStorageStore,
  reconcileExecutionStationLease,
  renewCollectionTaskLease,
  syncCollectionTaskStatusThroughSync,
} from '../src/workbench/runtime/taskLeaseClient.js';

function createMemoryStorage(initial = {}) {
  const values = { ...initial };
  return {
    values,
    async get(key) {
      return { [key]: values[key] };
    },
    async set(next) {
      Object.assign(values, next);
    },
    async remove(key) {
      delete values[key];
    },
  };
}

test('task lease client starts a V1.1 reservation immediately after claim sync', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    if (body.operations?.[0]?.type === 'start_job') {
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            mailboxVersions: { station: 12, 'xhs.monitor_patrol': 3 },
            operationResults: {
              [body.operations[0].operationId]: {
                status: 'accepted',
                attemptId: 'attempt-v11',
                leaseToken: 'lease-v11',
                leaseEpoch: 4,
                leaseExpiresAt: '2026-04-17T12:05:00.000Z',
              },
            },
            reservations: [],
            controlCommands: [],
            nextSync: { afterMs: 30000, reason: 'running' },
          };
        },
      };
    }
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 11, 'xhs.monitor_patrol': 2 },
          operationResults: {},
          reservations: [{
            jobId: 'job-v11',
            reserveToken: 'reserve-v11',
            reservationEpoch: 3,
            lane: 'xhs.monitor_patrol',
            platformAccountId: 'account-xhs-1',
            startBefore: '2026-04-17T12:03:00.000Z',
            taskSpec: {
              platform: 'xhs',
              platformAccountId: 'account-xhs-1',
              lane: 'monitor_patrol',
              taskType: 'xhs.collectAuthor',
              collectionProfile: 'list_scan',
              taskStrategy: 'author_patrol',
              target: 'https://www.xiaohongshu.com/user/profile/abc',
              targetKey: 'xhs:author:abc',
              payload: {},
            },
          }],
          controlCommands: [],
          nextSync: { afterMs: 1000, reason: 'reservations_granted' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore();

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    capabilities: ['xhs.list_scan'],
    platformAccounts: [{ platform: 'xhs', purpose: 'author_monitor', healthStatus: 'healthy' }],
    pluginVersion: '2.0.56',
    fetchFn,
    store,
  });
  const firstBody = JSON.parse(requests[0][1].body);
  const secondBody = JSON.parse(requests[1][1].body);
  const stored = await store.read();

  assert.equal(requests.length, 2);
  assert.equal(firstBody.capacity['xhs.monitor_patrol'].maxReservedTasks, 1);
  assert.deepEqual(firstBody.operations, []);
  assert.equal(secondBody.operations[0].type, 'start_job');
  assert.equal(secondBody.operations[0].jobId, 'job-v11');
  assert.equal(secondBody.operations[0].platformAccountId, 'account-xhs-1');
  assert.equal(claim.task.id, 'job-v11');
  assert.equal(claim.task.taskType, 'xhs.list_scan');
  assert.equal(claim.task.collectionProfile, 'list_scan');
  assert.equal(claim.task.accountId, 'account-xhs-1');
  assert.equal(claim.task.platformAccountId, 'account-xhs-1');
  assert.equal(claim.task.payload.platformAccountId, 'account-xhs-1');
  assert.equal(claim.lease.leaseToken, 'lease-v11');
  assert.equal(stored.taskId, 'job-v11');
  assert.equal(stored.leaseToken, 'lease-v11');
  assert.equal(stored.mailboxLaneVersions['xhs.monitor_patrol'], 3);
});

test('task lease client maps douyin note_detail reservations to batch note collection', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    if (body.operations?.[0]?.type === 'start_job') {
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            mailboxVersions: { station: 22, 'douyin.governance': 4 },
            operationResults: {
              [body.operations[0].operationId]: {
                status: 'accepted',
                attemptId: 'attempt-dy',
                leaseToken: 'lease-dy',
                leaseEpoch: 2,
                leaseExpiresAt: '2026-04-17T12:05:00.000Z',
              },
            },
            reservations: [],
            controlCommands: [],
            nextSync: { afterMs: 30000, reason: 'running' },
          };
        },
      };
    }
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 21, 'douyin.governance': 3 },
          operationResults: {},
          reservations: [{
            jobId: 'job-dy-note',
            reserveToken: 'reserve-dy',
            reservationEpoch: 1,
            lane: 'douyin.governance',
            startBefore: '2026-04-17T12:03:00.000Z',
            taskSpec: {
              platform: 'douyin',
              lane: 'governance',
              jobType: 'collect_detail',
              collectionProfile: 'note_detail',
              target: 'https://www.douyin.com/video/7341234567890123456',
              targetKey: 'douyin:note:7341234567890123456',
              payload: {},
            },
          }],
          controlCommands: [],
          nextSync: { afterMs: 1000, reason: 'reservations_granted' },
        };
      },
    };
  };

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    capabilities: ['douyin.note_full'],
    platformAccounts: [{ platform: 'douyin', purpose: 'execution', healthStatus: 'healthy' }],
    pluginVersion: '2.0.56',
    fetchFn,
    store: createTaskLeaseMemoryStore(),
  });

  const firstBody = JSON.parse(requests[0][1].body);

  assert.equal(firstBody.capacity['douyin.governance'].maxReservedTasks, 1);
  assert.equal(claim.task.taskType, 'douyin.batchNotes');
  assert.equal(claim.task.collectionProfile, 'note_detail');
  assert.equal(claim.task.target, 'https://www.douyin.com/video/7341234567890123456');
});

test('task lease client maps xhs note_full reservations to detail collection with comments', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    if (body.operations?.[0]?.type === 'start_job') {
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            mailboxVersions: { station: 32, 'xhs.manual_hot': 8 },
            operationResults: {
              [body.operations[0].operationId]: {
                status: 'accepted',
                attemptId: 'attempt-xhs-full',
                leaseToken: 'lease-xhs-full',
                leaseEpoch: 3,
                leaseExpiresAt: '2026-04-17T12:05:00.000Z',
              },
            },
            reservations: [],
            controlCommands: [],
            nextSync: { afterMs: 30000, reason: 'running' },
          };
        },
      };
    }
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 31, 'xhs.manual_hot': 7 },
          operationResults: {},
          reservations: [{
            jobId: 'job-xhs-note-full',
            reserveToken: 'reserve-xhs-full',
            reservationEpoch: 1,
            lane: 'xhs.manual_hot',
            platformAccountId: 'account-xhs-1',
            startBefore: '2026-04-17T12:03:00.000Z',
            taskSpec: {
              platform: 'xhs',
              platformAccountId: 'account-xhs-1',
              lane: 'manual_hot',
              jobType: 'collect_detail',
              collectionProfile: 'note_full',
              target: 'https://www.xiaohongshu.com/discovery/item/6986ceb7000000000c03587f?source=webshare&xhsshare=pc_web&xsec_token=ABC&xsec_source=pc_share',
              targetKey: 'xhs:note:6986ceb7000000000c03587f',
              payload: {},
            },
          }],
          controlCommands: [],
          nextSync: { afterMs: 1000, reason: 'reservations_granted' },
        };
      },
    };
  };

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    capabilities: ['xhs.note_full'],
    platformAccounts: [{ platform: 'xhs', purpose: 'execution', healthStatus: 'healthy' }],
    pluginVersion: '2.0.66',
    fetchFn,
    store: createTaskLeaseMemoryStore(),
  });

  assert.equal(claim.task.taskType, 'xhs.note_full');
  assert.equal(claim.task.collectionProfile, 'note_full');
  assert.equal(claim.task.target, 'https://www.xiaohongshu.com/discovery/item/6986ceb7000000000c03587f?source=webshare&xhsshare=pc_web&xsec_token=ABC&xsec_source=pc_share');
  assert.equal(claim.task.payload.targetPageType, 'detail');
  assert.equal(claim.task.payload.includeComments, true);
  assert.equal(claim.task.payload.commentLimit, 30);
  assert.equal(claim.task.payload.commentsLimit, 30);
  assert.equal(claim.task.payload.collectMode, 'detailsWithComments');
  assert.equal(claim.task.payload.noteId, '6986ceb7000000000c03587f');
  assert.equal(claim.task.payload.platformContentId, '6986ceb7000000000c03587f');
});

test('task lease client commits outbox records through V1.1 commit_raw_snapshot', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 14 },
          operationResults: {
            [op.operationId]: {
              status: 'accepted',
              rawSnapshotId: 'raw-1',
              captureId: op.captureId,
              checksumAlgorithm: 'sha256',
              checksumValue: 'abc',
              contentLength: 123,
              snapshotStatus: 'committed',
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-commit-1',
    leaseToken: 'lease-commit-1',
    leaseEpoch: 6,
    attemptId: 'attempt-commit-1',
  });

  const response = await commitCollectionTaskDeltaThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-commit-1',
    envelope: {
      taskId: 'job-commit-1',
      pluginRunId: 'run-commit-1',
      cursor: 'cursor-1',
      attemptId: 'attempt-commit-1',
      leaseToken: 'lease-commit-1',
      leaseEpoch: 6,
      executionContext: { platform: 'xhs', expectedTargetKey: 'xhs:note:1' },
      snapshot: {
        latestSummary: {
          requestedCount: 200,
          discoveredCount: 37,
          discoverySummary: {
            stopReason: 'no_new_cards_after_scroll',
            scrollRounds: 12,
            canLoadMore: true,
          },
        },
      },
      records: [{
        recordType: 'note',
        externalRecordId: 'note-1',
        sequence: 1,
        collectedAt: '2026-04-17T12:00:00.000Z',
        idempotencyKey: 'record-key-1',
        payload: { noteId: 'note-1', title: 'hello' },
      }],
      events: [{ idempotencyKey: 'event-key-1' }],
    },
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    capabilities: ['xhs.list_scan'],
    pluginVersion: '2.0.58',
    fetchFn,
    store,
  });
  const body = JSON.parse(requests[0][1].body);
  const op = body.operations[0];

  assert.equal(requests[0][0], 'http://localhost:3000/api/execution-stations/sync');
  assert.equal(Object.prototype.hasOwnProperty.call(body, 'mode'), false);
  assert.equal(body.capacity, undefined);
  assert.equal(op.type, 'commit_raw_snapshot');
  assert.equal(op.jobId, 'job-commit-1');
  assert.equal(op.leaseToken, 'lease-commit-1');
  assert.equal(op.leaseEpoch, 6);
  assert.deepEqual(op.resultSummary.discoverySummary, {
    stopReason: 'no_new_cards_after_scroll',
    scrollRounds: 12,
    canLoadMore: true,
  });
  assert.equal(op.records[0].recordType, 'note');
  assert.equal(op.records[0].idempotencyKey, 'record-key-1');
  assert.deepEqual(response.acceptedRecordKeys, ['record-key-1']);
  assert.deepEqual(response.acceptedEventKeys, ['event-key-1']);
});

test('task lease client treats stale lease operation rejection as permanent', async () => {
  const fetchFn = async (url, options = {}) => {
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 14 },
          operationResults: {
            [op.operationId]: {
              status: 'rejected',
              reason: 'lease_token_mismatch',
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-stale-1',
    leaseToken: 'lease-stale-1',
    leaseEpoch: 2,
    attemptId: 'attempt-stale-1',
  });

  await assert.rejects(
    () => commitCollectionTaskDeltaThroughSync({
      serverUrl: 'http://localhost:3000',
      taskId: 'job-stale-1',
      envelope: {
        taskId: 'job-stale-1',
        pluginRunId: 'run-stale-1',
        attemptId: 'attempt-stale-1',
        leaseToken: 'lease-stale-1',
        leaseEpoch: 2,
        records: [{
          recordType: 'note',
          externalRecordId: 'note-stale',
          sequence: 1,
          idempotencyKey: 'record-stale-1',
          payload: { noteId: 'note-stale', title: 'stale' },
        }],
        events: [],
      },
      stationId: 'station-1',
      stationToken: 'station-token',
      authorizationToken: 'auth_token_1',
      pluginVersion: '2.0.58',
      fetchFn,
      store,
    }),
    (error) => {
      assert.equal(error.status, 410);
      assert.equal(error.retryable, false);
      assert.equal(error.reasonCode, 'lease_token_mismatch');
      return true;
    },
  );
});

test('task lease client commits a terminal empty result as a raw snapshot', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 16 },
          operationResults: {
            [op.operationId]: {
              status: 'accepted',
              rawSnapshotId: 'raw-empty-1',
              captureId: op.captureId,
              snapshotStatus: 'committed',
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-empty-1',
    leaseToken: 'lease-empty-1',
    leaseEpoch: 3,
    attemptId: 'attempt-empty-1',
  });

  await commitCollectionTaskDeltaThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-empty-1',
    envelope: {
      taskId: 'job-empty-1',
      pluginRunId: 'run-empty-1',
      attemptId: 'attempt-empty-1',
      leaseToken: 'lease-empty-1',
      leaseEpoch: 3,
      snapshot: { status: 'completed', progress: 100 },
      records: [],
      events: [{ idempotencyKey: 'event-empty-1' }],
    },
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.58',
    fetchFn,
    store,
  });
  const body = JSON.parse(requests[0][1].body);
  const op = body.operations[0];

  assert.equal(op.type, 'commit_raw_snapshot');
  assert.equal(op.jobId, 'job-empty-1');
  assert.equal(op.leaseToken, 'lease-empty-1');
  assert.deepEqual(op.records, []);
});

test('task lease client commits a terminal failed result even when invalid records are filtered out', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 18 },
          operationResults: {
            [op.operationId]: {
              status: 'accepted',
              rawSnapshotId: 'raw-filtered-empty-1',
              captureId: op.captureId,
              snapshotStatus: 'committed',
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-filtered-empty-1',
    leaseToken: 'lease-filtered-empty-1',
    leaseEpoch: 8,
    attemptId: 'attempt-filtered-empty-1',
  });

  const response = await commitCollectionTaskDeltaThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-filtered-empty-1',
    envelope: {
      taskId: 'job-filtered-empty-1',
      pluginRunId: 'run-filtered-empty-1',
      attemptId: 'attempt-filtered-empty-1',
      leaseToken: 'lease-filtered-empty-1',
      leaseEpoch: 8,
      snapshot: { status: 'failed', progress: 100 },
      records: [{
        recordType: 'note',
        externalRecordId: 'note-without-key',
        payload: { noteId: 'note-without-key' },
      }],
      events: [{ idempotencyKey: 'event-filtered-empty-1' }],
    },
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.59',
    fetchFn,
    store,
  });
  const body = JSON.parse(requests[0][1].body);
  const op = body.operations[0];

  assert.equal(op.type, 'commit_raw_snapshot');
  assert.deepEqual(op.records, []);
  assert.equal(response.clientRecordStats.inputRecordCount, 1);
  assert.equal(response.clientRecordStats.committedRecordCount, 0);
  assert.equal(response.clientRecordStats.droppedRecordCount, 1);
  assert.equal(response.clientRecordStats.dropReason, 'missing_idempotency_key_or_invalid_record');
});

test('task lease client syncs status updates without the retired collection task PATCH route', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 15 },
          operationResults: {
            [op.operationId]: {
              status: 'accepted',
              leaseExpiresAt: '2026-04-17T12:10:00.000Z',
              leaseEpoch: op.leaseEpoch,
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-status-1',
    leaseToken: 'lease-status-1',
    leaseEpoch: 2,
  });

  await syncCollectionTaskStatusThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-status-1',
    patch: { status: 'running', progress: 50 },
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.58',
    fetchFn,
    store,
  });
  const body = JSON.parse(requests[0][1].body);

  assert.equal(requests[0][0], 'http://localhost:3000/api/execution-stations/sync');
  assert.equal(Object.prototype.hasOwnProperty.call(body, 'mode'), false);
  assert.equal(body.capacity, undefined);
  assert.equal(body.operations[0].type, 'progress_update');
  assert.equal(body.operations[0].jobId, 'job-status-1');
});

test('task lease client can defer terminal release while raw result is still being committed', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 17 },
          operationResults: {
            [op.operationId]: {
              status: 'accepted',
              leaseEpoch: op.leaseEpoch,
            },
          },
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 30000, reason: 'running' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'job-terminal-1',
    leaseToken: 'lease-terminal-1',
    leaseEpoch: 5,
  });

  await syncCollectionTaskStatusThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-terminal-1',
    patch: { status: 'failed', progress: 100, deferRelease: true },
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.58',
    fetchFn,
    store,
  });
  const body = JSON.parse(requests[0][1].body);

  assert.equal(body.operations[0].type, 'progress_update');
  assert.equal(body.operations[0].stage, 'failed');
});

test('task lease client claims through station sync and renews through progress_update', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    if (url.endsWith('/api/execution-stations/sync')) {
      const body = JSON.parse(options.body || '{}');
      if (body.operations?.[0]?.type === 'progress_update') {
        return {
          ok: true,
          status: 200,
          async json() {
            return {
              mailboxVersions: { station: 13 },
              operationResults: {
                [body.operations[0].operationId]: {
                  status: 'accepted',
                  leaseToken: 'lease-token-1',
                  leaseEpoch: 4,
                  leaseExpiresAt: '2026-04-17T12:10:00.000Z',
                },
              },
              reservations: [],
              controlCommands: [],
              nextSync: { afterMs: 30000, reason: 'running' },
            };
          },
        };
      }
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            heartbeat: { success: true },
            mailbox: { version: 12 },
            reconcile: { action: 'idle', serverLease: null },
            claim: {
              task: {
                id: 'task-lease-1',
                taskStrategy: 'author_patrol',
                leaseEpoch: 3,
              },
              lease: {
                leaseToken: 'lease-token-1',
                expiresAt: '2026-04-17T12:05:00.000Z',
              },
              attempt: {
                attemptId: 'attempt-1',
                attemptNumber: 2,
              },
            },
          };
        },
      };
    }
    return {
      ok: true,
      status: 200,
      async json() {
        return { success: true, expiresAt: '2026-04-17T12:10:00.000Z' };
      },
    };
  };
  const store = createTaskLeaseMemoryStore();

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    capabilities: ['xhs.list_scan'],
    platformAccounts: [{ platform: 'xhs', purpose: 'author_monitor', healthStatus: 'healthy' }],
    pluginVersion: '2.0.35',
    fetchFn,
    store,
  });
  const claimedLease = await store.read();
  const renewal = await renewCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    taskId: 'task-lease-1',
    stationId: 'station-1',
    stationToken: 'station-token',
    leaseToken: claimedLease.leaseToken,
    attemptId: claimedLease.attemptId,
    attemptNumber: claimedLease.attemptNumber,
    leaseEpoch: claimedLease.leaseEpoch,
    authorizationToken: 'auth_token_1',
    status: 'running',
    pluginVersion: '2.0.55',
    fetchFn,
    store,
  });

  assert.equal(claim.task.id, 'task-lease-1');
  assert.equal((await store.read()).leaseToken, 'lease-token-1');
  assert.equal((await store.read()).attemptId, 'attempt-1');
  assert.equal((await store.read()).attemptNumber, 2);
  assert.equal((await store.read()).leaseEpoch, 4);
  assert.equal(renewal.expiresAt, '2026-04-17T12:10:00.000Z');
  assert.equal((await store.read()).expiresAt, '2026-04-17T12:10:00.000Z');
  assert.equal((await store.read()).mailboxVersion, 13);
  assert.equal(requests.length, 2);
  assert.ok(requests[0][0].endsWith('/api/execution-stations/sync'));
  assert.equal(requests[0][0].includes('/api/execution-stations/dispatch'), false);
  assert.equal(requests[0][0].includes('/api/collection-tasks/claim'), false);
  assert.equal(requests[0][1].headers.Authorization, 'Bearer auth_token_1');
  const claimBody = JSON.parse(requests[0][1].body);
  assert.equal(claimBody.pluginVersion, '2.0.35');
  assert.equal(claimBody.capacity['xhs.monitor_patrol'].maxReservedTasks, 1);
  assert.deepEqual(claimBody.operations, []);
  assert.ok(requests[1][0].endsWith('/api/execution-stations/sync'));
  assert.equal(requests[1][0].includes('/api/collection-tasks/'), false);
  assert.equal(requests[1][1].headers.Authorization, 'Bearer auth_token_1');
  const renewBody = JSON.parse(requests[1][1].body);
  assert.equal(renewBody.operations[0].type, 'progress_update');
  assert.equal(renewBody.operations[0].jobId, 'task-lease-1');
  assert.equal(renewBody.operations[0].leaseToken, 'lease-token-1');
  assert.equal(renewBody.operations[0].leaseEpoch, 3);
});

test('task lease client keeps mailbox cursor in V1.1 sync request', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 12 },
          operationResults: {},
          reservations: [],
          controlCommands: [],
          nextSync: { afterMs: 60000, reason: 'idle' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: '',
    leaseToken: '',
    expiresAt: '',
    mailboxVersion: 12,
  });

  await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.42',
    fetchFn,
    store,
  });

  const body = JSON.parse(requests[0][1].body);
  assert.equal(body.mailboxCursors.station, 12);
  assert.equal(Object.prototype.hasOwnProperty.call(body, 'mode'), false);
  assert.equal(Object.prototype.hasOwnProperty.call(body, 'mailboxVersion'), false);
});

test('task lease client exposes renewal conflict status', async () => {
  const fetchFn = async () => ({
    ok: false,
    status: 409,
    async text() {
      return JSON.stringify({ error: 'Task lease is held by another station' });
    },
  });

  await assert.rejects(
    () => renewCollectionTaskLease({
      serverUrl: 'http://localhost:3000',
      taskId: 'task-lease-conflict',
      stationId: 'station-1',
      stationToken: 'station-token',
      leaseToken: 'stale-lease-token',
      authorizationToken: 'auth_token_1',
      fetchFn,
    }),
    (error) => {
      assert.equal(error.status, 409);
      assert.equal(error.retryable, false);
      assert.match(error.message, /another station/);
      return true;
    },
  );
});

test('task lease client exposes server backpressure retry delay', async () => {
  const fetchFn = async () => ({
    ok: false,
    status: 503,
    headers: {
      get(name) {
        return String(name || '').toLowerCase() === 'retry-after' ? '60' : null;
      },
    },
    async text() {
      return JSON.stringify({
        error: '执行设备通道正在保护数据库，请稍后重试。',
        code: 'plugin_protocol_backpressure',
        retryAfterSeconds: 60,
      });
    },
  });

  await assert.rejects(
    () => claimCollectionTaskLease({
      serverUrl: 'http://localhost:3000',
      stationId: 'station-1',
      stationToken: 'station-token',
      authorizationToken: 'auth_token_1',
      fetchFn,
    }),
    (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.retryable, true);
      assert.equal(error.reasonCode, 'plugin_protocol_backpressure');
      assert.equal(error.nextPollAfterMs, 60_000);
      assert.match(error.message, /保护数据库/);
      return true;
    },
  );
});

test('task lease client preserves claim reason and writes an idle snapshot', async () => {
  const fetchFn = async () => ({
    ok: true,
    status: 200,
    async json() {
      return {
        heartbeat: { success: true },
        reconcile: { action: 'idle', serverLease: null },
        claim: {
          task: null,
          reason: {
            code: 'no_available_account',
            message: '没有可用账号',
          },
          nextPollAfterMs: 45000,
        },
      };
    },
  });
  const store = createTaskLeaseMemoryStore();

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    capabilities: ['xhs.list_scan'],
    platformAccounts: [],
    fetchFn,
    store,
  });

  assert.deepEqual(claim.reason, {
    code: 'no_available_account',
    message: '没有可用账号',
  });
  assert.equal(claim.nextPollAfterMs, 45000);
  assert.deepEqual(await store.read(), {
    taskId: '',
    leaseToken: '',
    expiresAt: '',
    idleReasonCode: 'no_available_account',
    idleReasonMessage: '没有可用账号',
    nextPollAfterMs: 45000,
    reason: {
      code: 'no_available_account',
      message: '没有可用账号',
    },
  });
  assert.deepEqual(createTaskLeaseIdleSnapshot(claim), await store.read());
  assert.deepEqual(
    formatTaskLeaseIdleNotice(await store.read()),
    {
      message: '最近一次不接单原因：没有可用账号（no_available_account），约 45 秒后重试',
      type: 'warning',
      visible: true,
    },
  );
});

test('task lease client stores server resume lease from station sync without a second claim', async () => {
  const store = createTaskLeaseMemoryStore();
  const fetchFn = async () => ({
    ok: true,
    status: 200,
    async json() {
      return {
        heartbeat: { success: true },
        reconcile: {
          action: 'resume',
          task: { id: 'task-resume-1' },
          serverLease: {
            taskId: 'task-resume-1',
            leaseToken: 'lease-resume-1',
            expiresAt: '2026-04-17T12:05:00.000Z',
          },
        },
        claim: null,
      };
    },
  });

  const claim = await claimCollectionTaskLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    pluginVersion: '2.0.35',
    fetchFn,
    store,
  });

  assert.equal(claim.task, null);
  assert.equal(claim.reason.code, 'server_task_resume_required');
  assert.deepEqual(await store.read(), {
    taskId: 'task-resume-1',
    leaseToken: 'lease-resume-1',
    expiresAt: '2026-04-17T12:05:00.000Z',
  });
});

test('task lease client reconciles a server lease into local storage', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push([url, options]);
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          mailboxVersions: { station: 4 },
          operationResults: {},
          reservations: [],
          controlCommands: [],
          reconcile: {
            action: 'resume',
            task: { id: 'task-reconcile-1' },
            lease: {
              taskId: 'task-reconcile-1',
              leaseToken: 'lease-reconcile-1',
              expiresAt: '2026-05-10T01:05:00.000Z',
            },
          },
          nextSync: { afterMs: 60000, reason: 'idle' },
        };
      },
    };
  };
  const store = createTaskLeaseMemoryStore({
    taskId: 'local-task',
    leaseToken: 'local-token',
  });

  const result = await reconcileExecutionStationLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    authorizationToken: 'auth_token_1',
    localLease: { taskId: 'local-task', leaseToken: 'local-token' },
    capabilities: ['xhs.list_scan'],
    platformAccounts: [{ platform: 'xhs', purpose: 'author_monitor', healthStatus: 'healthy' }],
    pluginVersion: '2.0.7',
    fetchFn,
    store,
  });

  assert.equal(result.action, 'resume');
  assert.equal(result.lease.leaseToken, 'lease-reconcile-1');
  assert.deepEqual(await store.read(), {
    taskId: 'task-reconcile-1',
    leaseToken: 'lease-reconcile-1',
    expiresAt: '2026-05-10T01:05:00.000Z',
    mailboxVersion: 4,
  });
  assert.equal(requests[0][0], 'http://localhost:3000/api/execution-stations/sync');
  assert.equal(requests[0][1].headers.Authorization, 'Bearer auth_token_1');
  const body = JSON.parse(requests[0][1].body);
  assert.equal(body.stationId, 'station-1');
  assert.equal(body.activeLeases[0].leaseToken, 'local-token');
  assert.equal(body.pluginVersion, '2.0.7');
  assert.deepEqual(body.capabilities, ['xhs.list_scan']);
  assert.equal(body.capacity, undefined);
  assert.deepEqual(Object.keys(body).sort(), [
    'accountReports',
    'activeLeases',
    'capabilities',
    'operations',
    'pluginVersion',
    'protocolVersion',
    'stationId',
    'stationSessionId',
    'stationToken',
  ]);
});

test('task lease client clears local lease when reconcile says idle', async () => {
  const fetchFn = async () => ({
    ok: true,
    status: 200,
    async json() {
      return {
        mailboxVersions: { station: 5 },
        operationResults: {},
        reservations: [],
        controlCommands: [],
        reconcile: { action: 'clear_local' },
        nextSync: { afterMs: 60000, reason: 'idle' },
      };
    },
  });
  const store = createTaskLeaseMemoryStore({
    taskId: 'stale-task',
    leaseToken: 'stale-token',
  });

  const result = await reconcileExecutionStationLease({
    serverUrl: 'http://localhost:3000',
    stationId: 'station-1',
    stationToken: 'station-token',
    localLease: { taskId: 'stale-task', leaseToken: 'stale-token' },
    fetchFn,
    store,
  });

  assert.equal(result.action, 'clear_local');
  assert.deepEqual(await store.read(), {
    taskId: '',
    leaseToken: '',
    expiresAt: '',
    idleReasonCode: 'server_cleared_local_task',
    idleReasonMessage: '服务端没有当前任务，本地任务状态已清理。',
    nextPollAfterMs: 0,
    reason: {
      code: 'server_cleared_local_task',
      message: '服务端没有当前任务，本地任务状态已清理。',
    },
    mailboxVersion: 5,
  });
});

test('task lease client reconciles only through V1.1 sync', async () => {
  const paths = [];
  const fetchFn = async (url) => {
    paths.push(new URL(url).pathname);
    return {
      ok: false,
      status: 404,
      async text() {
        return 'not found';
      },
    };
  };

  await assert.rejects(
    () => reconcileExecutionStationLease({
      serverUrl: 'http://localhost:3000',
      stationId: 'station-1',
      stationToken: 'station-token',
      fetchFn,
    }),
    (error) => {
      assert.equal(error.status, 404);
      return true;
    },
  );

  assert.deepEqual(paths, ['/api/execution-stations/sync']);
});

test('task poller keeps one active lease and does not claim another task while running', async () => {
  let claimCalls = 0;
  const renewals = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => {
      claimCalls += 1;
      return {
        task: {
          id: 'task-lease-2',
          taskType: 'xhs.collectAuthor',
          platform: 'xhs',
          taskStrategy: 'author_patrol',
        },
        lease: {
          leaseToken: 'lease-token-2',
          expiresAt: '2026-04-17T12:05:00.000Z',
        },
      };
    },
    renewTaskLease: async (taskId, lease) => {
      renewals.push([taskId, lease.leaseToken]);
      return { success: true, expiresAt: '2026-04-17T12:10:00.000Z' };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task-lease-2',
      resultLookup: { externalTaskId: 'task-lease-2' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run-lease-2',
        status: 'running',
        resultSummary: { itemsPlanned: 10, itemsSucceeded: 1, failedItems: 0 },
        records: { notes: [], comments: [], authors: [], mediaAssets: [] },
      },
    }),
  });

  const first = await poller.tick();
  const second = await poller.tick();

  assert.equal(first.accepted, true);
  assert.equal(second.status, 'running');
  assert.equal(claimCalls, 1);
  assert.deepEqual(renewals, [['task-lease-2', 'lease-token-2']]);
  assert.equal(poller.getState().activeLease.leaseToken, 'lease-token-2');
});

test('task poller stays idle when station is not paired yet', async () => {
  let dispatchCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: null,
      reason: {
        code: 'station_not_registered',
        message: '请先绑定执行设备',
      },
      nextPollAfterMs: 30000,
    }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run without a leased task');
    },
  });

  const result = await poller.tick();

  assert.deepEqual(result, {
    success: true,
    idle: true,
    nextPollAfterMs: 30000,
    idleReasonCode: 'station_not_registered',
    idleReasonMessage: '请先绑定执行设备',
    reason: {
      code: 'station_not_registered',
      message: '请先绑定执行设备',
    },
  });
  assert.equal(dispatchCalls, 0);
  assert.equal(poller.getState().activeLease, null);
});

test('task lease storage store survives service worker memory loss', async () => {
  const storageArea = createMemoryStorage();
  const store = createTaskLeaseStorageStore({ storageArea, storageKey: 'lease' });

  await store.write({
    taskId: 'task-storage-1',
    leaseToken: 'lease-storage-token',
    expiresAt: '2026-04-17T12:05:00.000Z',
  });

  assert.equal((await store.read()).leaseToken, 'lease-storage-token');
  await store.clear();
  assert.equal(await store.read(), null);
});

test('采集阶段进度封顶 95：progress_update 不允许自报 100（报告 §9.1）', async () => {
  const requests = [];
  const fetchFn = async (url, options = {}) => {
    requests.push(JSON.parse(options.body || '{}'));
    const body = JSON.parse(options.body || '{}');
    const op = body.operations?.[0] || {};
    return {
      ok: true,
      status: 200,
      async json() {
        return {
          operationResults: { [op.operationId]: { status: 'accepted' } },
          reservations: [],
          controlCommands: [],
        };
      },
    };
  };

  await commitCollectionTaskDeltaThroughSync({
    serverUrl: 'http://localhost:3000',
    taskId: 'job-progress-1',
    envelope: {
      taskId: 'job-progress-1',
      pluginRunId: 'run-progress-1',
      attemptId: 'attempt-p1',
      leaseToken: 'lease-p1',
      leaseEpoch: 1,
      snapshot: { status: 'running', progress: 100 },
      records: [],
      events: [{ idempotencyKey: 'event-p1' }],
    },
    stationId: 'station-1',
    stationToken: 'station-token',
    fetchFn,
    storageArea: createMemoryStorage(),
  });

  const op = requests[0]?.operations?.[0];
  assert.equal(op?.type, 'progress_update');
  assert.equal(op?.progress, 95, '采集阶段自报 100 必须被封顶到 95');
});
