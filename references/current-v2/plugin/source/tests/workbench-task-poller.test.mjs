import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildWorkbenchResultSummary,
  createTaskPoller as createRuntimeTaskPoller,
} from '../src/workbench/runtime/taskPoller.js';

// These legacy poller tests exercise lease, cleanup, and writeback behavior. The
// dedicated V2 mapper suite owns fail-closed source-contract cases, so this
// harness replaces only that deep boundary with a deterministic validated-body
// result. Production never receives this injection.
function createTaskPoller(deps = {}) {
  return createRuntimeTaskPoller({
    ...deps,
    collectorVersion: deps.collectorVersion || 'test-plugin/v2',
    mapRuntimeTerminalToCaptureSubmissionV2:
      deps.mapRuntimeTerminalToCaptureSubmissionV2
      || (() => ({
        ok: true,
        body: {
          protocolVersion: 'capture-submission/v2',
          testBoundary: 'workbench-task-poller',
        },
      })),
  });
}

function claimTask(tasksOrFactory) {
  return async () => {
    const tasks = typeof tasksOrFactory === 'function' ? await tasksOrFactory() : tasksOrFactory;
    const task = Array.isArray(tasks) ? tasks[0] : tasks;
    return { task: task || null };
  };
}

test('XHS run authority prevents record aliases from synthesizing V2 identities or type', () => {
  const result = buildWorkbenchResultSummary({
    platform: 'xhs',
    records: {
      notes: [{
        noteId: 'note-1',
        contentId: 'note-1',
        contentType: 'video',
        title: 'source boundary',
      }],
      comments: [],
      authors: [{
        authorId: 'author-1',
        authorPlatformId: 'author-1',
        name: 'source boundary',
      }],
      mediaAssets: [],
    },
  });

  assert.equal(result.records.notes[0].noteId, 'note-1');
  assert.equal(result.records.notes[0].platformContentId, '');
  assert.equal(result.records.notes[0].type, '');
  assert.equal(result.records.authors[0].authorId, 'author-1');
  assert.equal(result.records.authors[0].platformAuthorId, '');
});

test('task poller starts a fresh tick when the previous tick is stale', async () => {
  let now = 1_000;
  let claimCalls = 0;
  const poller = createTaskPoller({
    now: () => now,
    tickStaleTimeoutMs: 1_000,
    claimTaskLease: async () => {
      claimCalls += 1;
      if (claimCalls === 1) {
        return new Promise(() => {});
      }
      return {
        task: null,
        nextPollAfterMs: 30_000,
        reason: {
          code: 'NO_PENDING_TASK',
          message: '暂无可接任务',
        },
      };
    },
  });

  void poller.tick();
  await Promise.resolve();
  await Promise.resolve();
  const overlapping = await poller.tick();
  assert.equal(overlapping.skipped, true);
  assert.equal(overlapping.reason, 'tick_in_progress');
  assert.equal(claimCalls, 1);

  now += 1_500;
  const recovered = await poller.tick();
  assert.equal(recovered.idle, true);
  assert.equal(recovered.idleReasonCode, 'NO_PENDING_TASK');
  assert.equal(claimCalls, 2);
  assert.equal(poller.getState().ticking, false);
});

test('task poller claims a pending task and patches completion state', async () => {
  const patches = [];
  const recordBatches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
        payload: { limit: 3 },
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: true,
    }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_1',
      resultLookup: { externalTaskId: 'task_1' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_1',
        status: 'done',
        resultSummary: {
          notes: 3,
          itemsPlanned: 1,
          itemsSucceeded: 1,
          failedItems: 0,
        },
        records: {
          notes: [
            {
              noteId: 'note_1',
              title: '标题 1',
              content: '内容 1',
              url: 'https://example.com/1',
              canonicalUrl: 'https://example.com/1?xsec_token=abc123',
              rawUrl: 'https://example.com/1?xsec_token=abc123',
              rawShareText: '分享文案',
              images: [{ url: 'https://images.example.com/cover.jpg' }],
              likes: 12,
            },
          ],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async (records) => {
      recordBatches.push(records);
      return records;
    },
  });

  const firstTick = await poller.tick();
  assert.equal(firstTick.accepted, true);
  assert.equal(patches[0][0], 'task_1');
  assert.equal(patches[0][1].status, 'dispatched');
  assert.equal(patches[0][1].progress, 5);
  assert.equal(patches[0][1].activeExecutor, null);
  assert.ok(Number.isFinite(Date.parse(patches[0][1].latestHeartbeatAt)));

  const secondTick = await poller.tick();
  assert.equal(secondTick.status, 'completed');
  assert.deepEqual(patches[1], [
    'task_1',
    {
      status: 'completed',
      progress: 100,
      pluginRunId: 'run_1',
      resultSummary: {
        notes: 3,
        itemsPlanned: 1,
        itemsSucceeded: 1,
        failedItems: 0,
        records: {
          notes: [
            {
              platform: '',
              targetKey: '',
              noteId: 'note_1',
              platformContentId: 'note_1',
              title: '标题 1',
              content: '内容 1',
              url: 'https://example.com/1',
              canonicalUrl: 'https://example.com/1?xsec_token=abc123',
              rawUrl: 'https://example.com/1?xsec_token=abc123',
              sourceUrl: '',
              runnableDetailUrl: '',
              rawShareText: '分享文案',
              cover: 'https://images.example.com/cover.jpg',
              coverImg: 'https://images.example.com/cover.jpg',
              coverUrl: 'https://images.example.com/cover.jpg',
              images: [{ url: 'https://images.example.com/cover.jpg' }],
              imageCandidates: [],
              imageCandidateSlots: [],
              videoUrl: '',
              likes: 12,
              likeCount: 12,
              collects: 0,
              collectCount: 0,
              comments: 0,
              commentCount: 0,
              shares: 0,
              shareCount: 0,
              authorId: '',
              authorPlatformId: '',
              authorEntityId: '',
              authorName: '',
              authorAvatar: '',
              publishedAt: null,
              publishedAtText: '',
              type: '',
              lastUpdateTime: null,
              rank: 0,
              batchRank: 0,
              isPinned: false,
              observedAt: '',
              collectionRunId: '',
              monitorMode: '',
              monitorId: '',
              taskStrategy: '',
              monitorMeta: {},
            },
          ],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
      errorMessage: null,
    },
  ]);
  assert.deepEqual(secondTick.cleanupTask, {
    taskId: 'task_1',
    externalTaskId: 'task_1',
    pluginRunId: 'run_1',
  });
  assert.equal(poller.getState().activeTask, null);
  assert.equal(recordBatches.length, 1);
  assert.equal(recordBatches[0][0].taskId, 'task_1');
  assert.equal(recordBatches[0][0].pluginRunId, 'run_1');
  assert.equal(recordBatches[0][0].recordType, 'note');
  assert.equal(recordBatches[0][0].externalRecordId, 'note_1');
  assert.equal(recordBatches[0][0].payload.cover, 'https://images.example.com/cover.jpg');
  assert.deepEqual(recordBatches[0][0].payload.images, [{ url: 'https://images.example.com/cover.jpg' }]);
});

test('task poller carries plugin-opened tab ownership into terminal cleanup', async () => {
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_plugin_tab_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/explore/note_1',
      },
    ]),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({
      success: true,
      accepted: true,
      pluginOpenedTabId: 701,
    }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_plugin_tab_1',
      tabId: 701,
      resultLookup: { externalTaskId: 'task_plugin_tab_1' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_plugin_tab_1',
        status: 'done',
        resultSummary: {
          notes: 1,
          itemsPlanned: 1,
          itemsSucceeded: 1,
          failedItems: 0,
        },
        records: {
          notes: [],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
  });

  await poller.tick();
  const terminalTick = await poller.tick();

  assert.deepEqual(terminalTick.cleanupTask, {
    taskId: 'task_plugin_tab_1',
    externalTaskId: 'task_plugin_tab_1',
    pluginRunId: 'run_plugin_tab_1',
    pluginOpenedTabId: 701,
  });
});

test('task poller marks task running immediately when dispatch already returns a local run id', async () => {
  const patches = [];
  const events = [];
  const lookups = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_started_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: true,
    }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_started_1',
      collectionRunId: 'run_started_1',
      resultLookup: {
        externalTaskId: 'task_started_1',
        collectionRunId: 'run_started_1',
      },
    }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return {
        success: true,
        result: {
          collectionRunId: 'run_started_1',
          status: 'running',
          resultSummary: {
            itemsPlanned: 2,
            itemsSucceeded: 1,
            failedItems: 0,
          },
          records: {
            notes: [],
            comments: [],
            authors: [],
            mediaAssets: [],
          },
        },
      };
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
  });

  const firstTick = await poller.tick();
  assert.equal(firstTick.accepted, true);
  assert.deepEqual(patches[0], [
    'task_started_1',
    {
      status: 'running',
      progress: 10,
      pluginRunId: 'run_started_1',
      activeExecutor: null,
      latestHeartbeatAt: patches[0][1].latestHeartbeatAt,
      errorMessage: null,
    },
  ]);
  assert.ok(Number.isFinite(Date.parse(patches[0][1].latestHeartbeatAt)));
  assert.equal(events[0].eventType, 'task.claimed');
  assert.equal(events[0].payload.status, 'running');
  assert.equal(events[0].payload.collectionRunId, 'run_started_1');
  assert.equal(events[1].eventType, 'task.page_opened');
  assert.equal(events[1].payload.collectionRunId, 'run_started_1');
  assert.equal(events[2].eventType, 'task.running');
  assert.equal(events[2].payload.status, 'running');
  assert.equal(poller.getState().activeTask?.workbenchStatus, 'running');
  assert.equal(poller.getState().activeTask?.pluginRunId, 'run_started_1');

  const secondTick = await poller.tick();
  assert.equal(secondTick.status, 'running');
  assert.deepEqual(lookups[0], {
    collectionRunId: 'run_started_1',
    externalTaskId: 'task_started_1',
  });
  assert.equal(patches[1][1].status, 'running');
  assert.equal(patches[1][1].pluginRunId, 'run_started_1');
});

test('task poller attaches runtime observability to terminal workbench events', async () => {
  let now = 1_000;
  const events = [];
  const poller = createTaskPoller({
    now: () => now,
    claimTaskLease: claimTask([
      {
        id: 'task_runtime_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_baseline',
      },
    ]),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_runtime_1',
      collectionRunId: 'run_runtime_1',
      resultLookup: {
        externalTaskId: 'task_runtime_1',
        collectionRunId: 'run_runtime_1',
      },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_runtime_1',
        status: 'done',
        resultSummary: {
          itemsPlanned: 4,
          itemsSucceeded: 3,
          failedItems: 1,
        },
        records: {
          notes: [],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
  });

  await poller.tick();
  now = 3_500;
  await poller.tick();

  const terminalEvent = events.find((event) => event.eventType === 'task.succeeded');
  assert.equal(terminalEvent.payload.observability.taskType, 'xhs.batchNotes');
  assert.equal(terminalEvent.payload.observability.source, 'monitor');
  assert.equal(terminalEvent.payload.observability.taskStrategy, 'author_baseline');
  assert.equal(terminalEvent.payload.observability.durationMs, 2500);
  assert.equal(terminalEvent.payload.observability.itemAttemptCount, 4);
  assert.equal(terminalEvent.payload.observability.itemFailureCount, 1);
  assert.equal(terminalEvent.payload.observability.report, true);
});

test('task poller fails the task with schema health when extractor note records are invalid', async () => {
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_schema_1',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_schema_1',
      collectionRunId: 'run_schema_1',
      resultLookup: {
        externalTaskId: 'task_schema_1',
        collectionRunId: 'run_schema_1',
      },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_schema_1',
        status: 'done',
        resultSummary: { itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: {
          notes: [{ noteId: 'n1' }],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async () => {
      const error = new Error('note payload must include visible content or media');
      error.code = 'missing_note_body';
      error.reasonCode = 'missing_note_body';
      error.retryable = false;
      error.validationErrors = [{
        field: 'payload',
        code: 'missing_note_body',
        message: 'note payload must include visible content or media',
      }];
      error.observability = {
        recordType: 'note',
        schemaValidationAttemptCount: 1,
        schemaValidationFailureCount: 1,
        schemaValidationFailureRate: 1,
        recordSchemaFailed: true,
        invalidRecordField: 'payload',
        reasonCode: 'missing_note_body',
      };
      throw error;
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
  });

  await poller.tick();
  const result = await poller.tick();

  assert.equal(result.failed, true);
  assert.equal(patches.at(-1)[1].status, 'failed');
  assert.equal(patches.at(-1)[1].errorMessage, 'note payload must include visible content or media');
  const failedEvent = events.find((event) => event.eventType === 'task.failed');
  assert.equal(failedEvent.payload.reasonCode, 'missing_note_body');
  assert.equal(failedEvent.payload.observability.recordType, 'note');
  assert.equal(failedEvent.payload.observability.schemaValidationFailureCount, 1);
  assert.equal(failedEvent.payload.observability.recordSchemaFailed, true);
});

test('task poller drops invalid comment records before remote writeback', async () => {
  const patches = [];
  const recordBatches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_comment_filter_1',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_comment_filter_1',
      collectionRunId: 'run_comment_filter_1',
      resultLookup: {
        externalTaskId: 'task_comment_filter_1',
        collectionRunId: 'run_comment_filter_1',
      },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_comment_filter_1',
        status: 'done',
        resultSummary: { itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: {
          notes: [],
          comments: [
            { commentId: 'c-valid', noteId: 'note-1', text: '有效评论' },
            { commentId: 'c-empty-text', noteId: 'note-1', text: '' },
            { commentId: 'c-missing-parent', text: '缺少父级作品' },
          ],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async (records) => {
      recordBatches.push(records);
      return records;
    },
  });

  await poller.tick();
  const result = await poller.tick();

  assert.equal(result.status, 'completed');
  assert.equal(patches.at(-1)[1].status, 'completed');
  assert.equal(recordBatches.length, 1);
  assert.equal(recordBatches[0].length, 1);
  assert.equal(recordBatches[0][0].payload.commentId, 'c-valid');
  assert.equal(recordBatches[0][0].payload.text, '有效评论');
});

test('task poller exposes lease credentials and page fingerprint for server ingest', async () => {
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_with_truth',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        leaseEpoch: 7,
      },
      lease: {
        leaseToken: 'lease-token-7',
        expiresAt: '2026-04-17T12:05:00.000Z',
        attemptId: 'attempt-7',
        attemptNumber: 3,
        leaseEpoch: 7,
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_with_truth',
      collectionRunId: 'run_truth',
      capabilityReport: {
        platform: 'xhs',
        mode: 'detail',
        pageType: 'noteDetail',
        url: 'https://www.xiaohongshu.com/explore/note_truth',
        readiness: { ready: true },
      },
      resultLookup: {
        externalTaskId: 'task_with_truth',
        collectionRunId: 'run_truth',
      },
    }),
  });

  await poller.tick();
  const context = poller.getExecutionContext('task_with_truth');

  assert.equal(context.leaseToken, 'lease-token-7');
  assert.equal(context.attemptId, 'attempt-7');
  assert.equal(context.leaseEpoch, 7);
  assert.deepEqual(context.pageFingerprint, {
    platform: 'xhs',
    pageType: 'detail',
    rawPageType: 'noteDetail',
    url: 'https://www.xiaohongshu.com/explore/note_truth',
    contentId: 'note_truth',
    routeKey: 'detail:note_truth',
    ready: true,
    readinessReasonCode: '',
  });
});

test('task poller persists active task context after dispatch starts', async () => {
  const persisted = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_persist_context',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-persist-context',
        attemptId: 'attempt-persist-context',
        leaseEpoch: 3,
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_persist_context',
      tabId: 789,
      collectionRunId: 'run_persist_context',
      resultLookup: {
        externalTaskId: 'task_persist_context',
        collectionRunId: 'run_persist_context',
      },
    }),
    writeActiveTaskContext: async (snapshot) => {
      persisted.push(snapshot);
    },
  });

  await poller.tick();

  assert.equal(persisted.length, 1);
  assert.equal(persisted[0].taskId, 'task_persist_context');
  assert.equal(persisted[0].pluginRunId, 'run_persist_context');
  assert.equal(persisted[0].platform, 'xhs');
  assert.equal(persisted[0].tabId, 789);
  assert.equal(persisted[0].lease.leaseToken, 'lease-persist-context');
  assert.equal(persisted[0].lease.attemptId, 'attempt-persist-context');
});

test('task poller recovers persisted context after worker restart', async () => {
  const lookups = [];
  const poller = createTaskPoller({
    readTaskLease: async () => ({
      taskId: 'task_recover_context',
      leaseToken: 'lease-recover-context',
      attemptId: 'attempt-recover-context',
    }),
    readActiveTaskContext: async () => ({
      taskId: 'task_recover_context',
      externalTaskId: 'task_recover_context',
      pluginRunId: 'run_recover_context',
      platform: 'douyin',
      accountId: 'douyin_account_1',
      tabId: 456,
      workbenchStatus: 'running',
      dispatchedAtMs: 1000,
    }),
    reconcileTaskLease: async () => ({
      success: true,
      action: 'resume',
      lease: {
        taskId: 'task_recover_context',
        leaseToken: 'lease-recover-context',
        attemptId: 'attempt-recover-context',
      },
      task: {
        id: 'task_recover_context',
        taskType: 'xhs.authorNoteLinks',
        platform: 'xhs',
        collectionProfile: 'author_links',
        status: 'running',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-17T12:05:00.000Z' }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return {
        success: true,
        result: {
          collectionRunId: 'run_recover_context',
          status: 'running',
          resultSummary: { itemsPlanned: 2, itemsSucceeded: 1, failedItems: 0 },
          records: { notes: [], comments: [], authors: [], mediaAssets: [] },
        },
      };
    },
    patchTask: async () => ({ success: true }),
  });

  await poller.tick();

  assert.deepEqual(lookups[0], {
    collectionRunId: 'run_recover_context',
    externalTaskId: 'task_recover_context',
    tabId: 456,
  });
  assert.equal(poller.getState().activeTask.tabId, 456);
  assert.equal(poller.getState().activeTask.accountId, 'douyin_account_1');
  assert.equal(poller.getState().activeTask.collectionProfile, 'author_links');
});

test('task poller ignores persisted context from a stale attempt', async () => {
  const lookups = [];
  const poller = createTaskPoller({
    readTaskLease: async () => ({
      taskId: 'task_stale_context',
      leaseToken: 'lease-new-context',
      attemptId: 'attempt-new-context',
    }),
    readActiveTaskContext: async () => ({
      taskId: 'task_stale_context',
      externalTaskId: 'task_stale_context',
      pluginRunId: 'run_previous_context',
      attemptId: 'attempt-previous-context',
      workbenchStatus: 'running',
    }),
    reconcileTaskLease: async () => ({
      success: true,
      action: 'resume',
      lease: {
        taskId: 'task_stale_context',
        leaseToken: 'lease-new-context',
        attemptId: 'attempt-new-context',
      },
      task: {
        id: 'task_stale_context',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        status: 'running',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-17T12:05:00.000Z' }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return { success: false, error: 'collectionRun not found for externalTaskId: task_stale_context' };
    },
    patchTask: async () => ({ success: true }),
  });

  await poller.tick();

  assert.deepEqual(lookups[0], {
    collectionRunId: '',
    externalTaskId: 'task_stale_context',
  });
  assert.equal(poller.getState().activeTask.pluginRunId, '');
});

test('task poller keeps persisted execution page when the resumed server task has the same local run id', async () => {
  const lookups = [];
  const persistedSnapshots = [];
  const poller = createTaskPoller({
    readTaskLease: async () => ({
      taskId: 'task_resume_same_run',
      leaseToken: 'lease-new-context',
      attemptId: 'attempt-new-context',
    }),
    readActiveTaskContext: async () => ({
      taskId: 'task_resume_same_run',
      externalTaskId: 'task_resume_same_run',
      pluginRunId: 'run_same_context',
      attemptId: 'attempt-previous-context',
      platform: 'douyin',
      accountId: 'douyin_account_1',
      tabId: 987,
      pageFingerprint: {
        platform: 'douyin',
        pageType: 'profile',
        routeKey: 'profile:author-1',
      },
      workbenchStatus: 'running',
      dispatchedAtMs: 1000,
    }),
    writeActiveTaskContext: async (snapshot) => {
      persistedSnapshots.push(snapshot);
    },
    reconcileTaskLease: async () => ({
      success: true,
      action: 'resume',
      lease: {
        taskId: 'task_resume_same_run',
        leaseToken: 'lease-new-context',
        attemptId: 'attempt-new-context',
      },
      task: {
        id: 'task_resume_same_run',
        taskType: 'douyin.collectAuthor',
        platform: 'douyin',
        status: 'running',
        pluginRunId: 'run_same_context',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-17T12:05:00.000Z' }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return {
        success: true,
        result: {
          collectionRunId: 'run_same_context',
          status: 'running',
          resultSummary: { itemsPlanned: 2, itemsSucceeded: 1, failedItems: 0 },
          records: { notes: [], comments: [], authors: [], mediaAssets: [] },
        },
      };
    },
    patchTask: async () => ({ success: true }),
  });

  await poller.tick();

  assert.deepEqual(lookups[0], {
    collectionRunId: 'run_same_context',
    externalTaskId: 'task_resume_same_run',
    tabId: 987,
  });
  assert.equal(poller.getState().activeTask.accountId, 'douyin_account_1');
  assert.equal(poller.getState().activeTask.pageFingerprint.routeKey, 'profile:author-1');
  assert.equal(persistedSnapshots.at(-1).tabId, 987);
});

test('task poller surfaces idle claim reason details from the lease endpoint', async () => {
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: null,
      reason: {
        code: 'no_available_account',
        message: '没有可用账号',
      },
      nextPollAfterMs: 30000,
    }),
  });

  const result = await poller.tick();

  assert.deepEqual(result, {
    success: true,
    idle: true,
    nextPollAfterMs: 30000,
    idleReasonCode: 'no_available_account',
    idleReasonMessage: '没有可用账号',
    reason: {
      code: 'no_available_account',
      message: '没有可用账号',
    },
  });
  assert.deepEqual(poller.getState().lastIdleReason, {
    taskId: '',
    leaseToken: '',
    expiresAt: '',
    idleReasonCode: 'no_available_account',
    idleReasonMessage: '没有可用账号',
    nextPollAfterMs: 30000,
    reason: {
      code: 'no_available_account',
      message: '没有可用账号',
    },
  });
});

test('task poller turns retryable claim backpressure into an idle wait', async () => {
  const error = new Error('执行设备通道正在保护数据库，请稍后重试。');
  error.status = 503;
  error.retryable = true;
  error.reasonCode = 'plugin_protocol_backpressure';
  error.nextPollAfterMs = 60_000;
  let dispatchCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => {
      throw error;
    },
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run during backpressure');
    },
  });

  const result = await poller.tick();

  assert.deepEqual(result, {
    success: true,
    idle: true,
    nextPollAfterMs: 60_000,
    idleReasonCode: 'plugin_protocol_backpressure',
    idleReasonMessage: '执行设备通道正在保护数据库，请稍后重试。',
    reason: {
      code: 'plugin_protocol_backpressure',
      message: '执行设备通道正在保护数据库，请稍后重试。',
    },
  });
  assert.equal(dispatchCalls, 0);
  assert.equal(poller.getState().lastIdleReason.nextPollAfterMs, 60_000);
});

test('task poller turns authorization failures into a long idle pause', async () => {
  const error = new Error('authorization expired');
  error.status = 401;
  error.retryable = false;
  let dispatchCalls = 0;
  let persistedBackoff = null;
  const poller = createTaskPoller({
    claimTaskLease: async () => {
      throw error;
    },
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run when authorization is invalid');
    },
    writeAuthorizationFailureBackoff: async (snapshot) => {
      persistedBackoff = snapshot;
    },
  });

  const result = await poller.tick();

  assert.equal(result.success, true);
  assert.equal(result.idle, true);
  assert.equal(result.idleReasonCode, 'authorization_invalid');
  assert.equal(result.idleReasonMessage, 'authorization expired');
  assert.equal(result.nextPollAfterMs, 15 * 60_000);
  assert.equal(dispatchCalls, 0);
  assert.equal(poller.getState().lastIdleReason.nextPollAfterMs, 15 * 60_000);
  assert.equal(persistedBackoff.reason.code, 'authorization_invalid');
  assert.ok(persistedBackoff.retryAtMs > Date.now());
});

test('task poller honors persisted authorization backoff before claiming work', async () => {
  const retryAtMs = 1_000_000 + 15 * 60_000;
  let claimCalls = 0;
  const poller = createTaskPoller({
    now: () => 1_000_000,
    readAuthorizationFailureBackoff: async () => ({
      retryAtMs,
      reason: {
        code: 'authorization_invalid',
        message: 'authorization expired',
      },
    }),
    claimTaskLease: async () => {
      claimCalls += 1;
      throw new Error('claim should not run during persisted authorization backoff');
    },
  });

  const result = await poller.tick();

  assert.equal(result.success, true);
  assert.equal(result.idle, true);
  assert.equal(result.idleReasonCode, 'authorization_invalid');
  assert.equal(result.idleReasonMessage, 'authorization expired');
  assert.equal(result.nextPollAfterMs, 15 * 60_000);
  assert.equal(claimCalls, 0);
});

test('task poller persists outdated plugin idle backoff', async () => {
  let persistedBackoff = null;
  const poller = createTaskPoller({
    claimTaskLease: async () => {
      return {
        task: null,
        nextPollAfterMs: 300_000,
        reason: {
          code: 'PLUGIN_VERSION_OUTDATED',
          message: '请先更新插件到最新版后再接单',
        },
      };
    },
    writeAuthorizationFailureBackoff: async (snapshot) => {
      persistedBackoff = snapshot;
    },
  });

  const result = await poller.tick();

  assert.equal(result.success, true);
  assert.equal(result.idle, true);
  assert.equal(result.idleReasonCode, 'PLUGIN_VERSION_OUTDATED');
  assert.equal(result.nextPollAfterMs, 300_000);
  assert.equal(persistedBackoff.reason.code, 'PLUGIN_VERSION_OUTDATED');
});

test('task poller clears local active task when it has no valid lease for too long', async () => {
  let now = 1_000;
  const patches = [];
  const events = [];
  const leases = [];
  const poller = createTaskPoller({
    now: () => now,
    claimTaskLease: claimTask([
      {
        id: 'task_no_lease_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_no_lease_1',
      resultLookup: { externalTaskId: 'task_no_lease_1' },
    }),
    getResultPackage: async () => ({ success: false, error: 'run_not_found' }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      leases.push('cleared');
    },
  });

  const first = await poller.tick();
  assert.equal(first.accepted, true);

  now += 6 * 60_000;
  const second = await poller.tick();

  assert.equal(second.released, true);
  assert.equal(second.reason, 'local_lease_missing_timeout');
  assert.equal(poller.getState().activeTask, null);
  assert.equal(leases.length, 1);
  assert.equal(patches.at(-1)[1].status, 'pending');
  assert.equal(patches.at(-1)[1].errorMessage, '插件本地任务没有有效租约，已自动释放重试。');
  assert.equal(events.at(-1).payload.reason, 'local_lease_missing_timeout');
});

test('task poller keeps monitor tasks running when the local lease is missing', async () => {
  let now = 1_000;
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    now: () => now,
    claimTaskLease: claimTask([
      {
        id: 'monitor_no_lease_1',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_baseline',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'monitor_no_lease_1',
      collectionRunId: 'run_monitor_no_lease_1',
      resultLookup: { externalTaskId: 'monitor_no_lease_1' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_monitor_no_lease_1',
        status: 'running',
        resultSummary: {
          itemsPlanned: 50,
          itemsSucceeded: 1,
          failedItems: 0,
        },
        records: {
          notes: [],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
  });

  const first = await poller.tick();
  assert.equal(first.accepted, true);

  now += 6 * 60_000;
  const second = await poller.tick();

  assert.equal(second.released, undefined);
  assert.equal(poller.getState().activeTask.taskId, 'monitor_no_lease_1');
  assert.equal(
    patches.some(([, patch]) => patch.status === 'pending' && patch.errorMessage === '插件本地任务没有有效租约，已自动释放重试。'),
    false
  );
  assert.equal(events.some((event) => event.payload?.reason === 'local_lease_missing_timeout'), false);
});

test('task poller preserves author profile fields when reporting monitor results', async () => {
  const patches = [];
  const recordBatches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_author_profile',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_baseline',
        payload: { monitorId: 'monitor_author_1' },
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_author_profile',
      resultLookup: { externalTaskId: 'task_author_profile' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_author_profile',
        status: 'done',
        resultSummary: { authors: 1, itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: {
          notes: [],
          comments: [],
          authors: [
            {
              authorId: '6926d8f4000000003702c666',
              platformAuthorId: '6926d8f4000000003702c666',
              authorEntityId: 'xhs_6926d8f4000000003702c666',
              platform: 'xhs',
              name: '孙爸养A娃（成长版）',
              avatar: 'https://images.example.com/avatar.jpg',
              profileUrl: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666',
              description: '混合型 ADHD 清华笨爸',
              follows: 12,
              fans: 16282,
              interactions: 200000,
              notes: 46,
              monitorMode: 'author_profile',
              monitorId: 'monitor_author_1',
              taskStrategy: 'author_baseline',
              monitorMeta: {
                monitorId: 'monitor_author_1',
                monitorMode: 'author_profile',
              },
            },
          ],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async (records) => {
      recordBatches.push(records);
      return records;
    },
  });

  await poller.tick();
  await poller.tick();

  const author = patches[1][1].resultSummary.records.authors[0];
  assert.deepEqual(author, {
    authorId: '6926d8f4000000003702c666',
    platformAuthorId: '6926d8f4000000003702c666',
    authorEntityId: 'xhs_6926d8f4000000003702c666',
    userId: '',
    platform: 'xhs',
    name: '孙爸养A娃（成长版）',
    profileUrl: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666',
    avatar: 'https://images.example.com/avatar.jpg',
    description: '混合型 ADHD 清华笨爸',
    bio: '混合型 ADHD 清华笨爸',
    ipLocation: '',
    location: '',
    handle: '',
    redId: '',
    douyinId: '',
    fans: 16282,
    followers: 16282,
    follows: 12,
    following: 12,
    interactions: 200000,
    likesAndCollects: 200000,
    works: 46,
    notes: 46,
    monitorMode: 'author_profile',
    monitorId: 'monitor_author_1',
    taskStrategy: 'author_baseline',
    monitorMeta: {
      monitorId: 'monitor_author_1',
      monitorMode: 'author_profile',
    },
  });
  assert.equal(recordBatches[0][0].recordType, 'author');
  assert.equal(recordBatches[0][0].payload.monitorMode, 'author_profile');
  assert.equal(recordBatches[0][0].payload.avatar, 'https://images.example.com/avatar.jpg');
});

test('task poller preserves note author and publish-time fields when reporting monitor note results', async () => {
  const patches = [];
  const recordBatches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_author_notes',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_baseline',
        payload: { monitorId: 'monitor_author_2' },
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_author_notes',
      resultLookup: { externalTaskId: 'task_author_notes' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_author_notes',
        status: 'done',
        resultSummary: { notes: 1, itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: {
          notes: [
            {
              platform: 'xhs',
              noteId: 'note_50',
              platformContentId: 'note_50',
              title: '最近一条作品',
              content: '正文内容',
              url: 'https://www.xiaohongshu.com/explore/note_50',
              canonicalUrl: 'https://www.xiaohongshu.com/explore/note_50',
              rawUrl: 'https://www.xiaohongshu.com/explore/note_50?xsec_token=abc123',
              likes: 520,
              collects: 88,
              comments: 34,
              shares: 12,
              authorId: 'author_target_1',
              authorPlatformId: 'author_target_1',
              authorEntityId: 'xhs_author_target_1',
              authorName: '目标博主',
              authorAvatar: 'https://images.example.com/author.jpg',
              publishedAt: 1776766122,
              publishedAtText: '4月21日 18:08',
              type: 'video',
              lastUpdateTime: 1776766999,
              monitorMode: 'author_surface',
              monitorId: 'monitor_author_2',
              taskStrategy: 'author_baseline',
              monitorMeta: {
                monitorId: 'monitor_author_2',
                taskStrategy: 'author_baseline',
                targetUrl: 'https://www.xiaohongshu.com/user/profile/author_target_1',
              },
            },
          ],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async (records) => {
      recordBatches.push(records);
      return records;
    },
  });

  await poller.tick();
  await poller.tick();

  const note = patches[1][1].resultSummary.records.notes[0];
  assert.equal(note.authorId, 'author_target_1');
  assert.equal(note.authorPlatformId, 'author_target_1');
  assert.equal(note.publishedAt, 1776766122);
  assert.equal(note.publishedAtText, '4月21日 18:08');
  assert.equal(note.type, 'video');
  assert.equal(note.monitorMeta.targetUrl, 'https://www.xiaohongshu.com/user/profile/author_target_1');
  assert.equal(recordBatches[0][0].payload.authorId, 'author_target_1');
  assert.equal(recordBatches[0][0].payload.publishedAt, 1776766122);
  assert.equal(recordBatches[0][0].payload.type, 'video');
});

test('task poller leaves task pending when no executable context is available', async () => {
  const patches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      { id: 'task_2', taskType: 'douyin.collectAuthor', platform: 'douyin' },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: false,
      accepted: false,
      error: 'no_matching_tab',
    }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
  });

  const tick = await poller.tick();
  assert.equal(tick.skipped, true);
  assert.equal(patches.length, 0);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller releases a leased task when the content script is unavailable', async () => {
  const patches = [];
  const events = [];
  const leases = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_missing_content_script',
        taskType: 'douyin.batchComments',
        platform: 'douyin',
        target: 'https://www.douyin.com/search/%E5%92%96%E5%95%A1',
      },
      lease: {
        leaseToken: 'lease-missing-content',
        attemptId: 'attempt-missing-content',
        leaseEpoch: 2,
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: false,
      reasonCode: 'page_context_unavailable',
      reasonMessage: '当前页面没有加载插件内容脚本',
      recommendedAction: 'reload_supported_page_with_plugin',
    }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      leases.push('cleared');
    },
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.equal(result.reason, '当前页面没有加载插件内容脚本');
  assert.deepEqual(patches[0], [
    'task_missing_content_script',
    {
      status: 'pending',
      progress: 0,
      errorMessage: '当前页面没有加载插件内容脚本',
    },
  ]);
  assert.equal(events.at(-1).eventType, 'task.released');
  assert.equal(events.at(-1).attemptId, 'attempt-missing-content');
  assert.equal(events.at(-1).leaseId, 'lease-missing-content');
  assert.equal(events.at(-1).platform, 'douyin');
  assert.equal(events.at(-1).payload.reasonCode, 'page_context_unavailable');
  assert.equal(events.at(-1).payload.recommendedAction, 'reload_supported_page_with_plugin');
  assert.equal(leases.length, 1);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller keeps the navigated tab available while xhs requires login verification', async () => {
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_xhs_login_verification',
        taskType: 'xhs.authorNoteLinks',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666',
      },
      lease: {
        leaseToken: 'lease-login-verification',
        attemptId: 'attempt-login-verification',
        leaseEpoch: 2,
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: false,
      reasonCode: 'login_required',
      reasonMessage: '小红书要求使用已登录账号的 APP 扫码验证身份',
      recommendedAction: 'scan_xhs_login_qr',
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.equal(result.preserveTaskExecutionContext, true);
  assert.deepEqual(patches[0], [
    'task_xhs_login_verification',
    {
      status: 'pending',
      progress: 0,
      errorMessage: '小红书要求使用已登录账号的 APP 扫码验证身份',
    },
  ]);
  assert.equal(events.at(-1).eventType, 'task.released');
  assert.equal(events.at(-1).payload.reasonCode, 'login_required');
});

test('task poller fails unavailable detail pages instead of returning them to pending', async () => {
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_deleted_note',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/explore/deleted-note',
      },
      lease: {
        leaseToken: 'lease-deleted-note',
        attemptId: 'attempt-deleted-note',
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: false,
      reasonCode: 'content_not_found',
      reasonMessage: '当前笔记已删除或不可访问',
      report: {
        mode: 'unknown',
        pageType: 'error',
        url: 'https://www.xiaohongshu.com/explore/deleted-note',
        readiness: {
          ready: false,
          reasonCode: 'content_not_found',
          reasonMessage: '当前笔记已删除或不可访问',
        },
        capabilities: {
          canRunTaskTypes: [],
        },
      },
    }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.deepEqual(patches[0], [
    'task_deleted_note',
    {
      status: 'failed',
      progress: 100,
      errorMessage: '当前笔记已删除或不可访问',
    },
  ]);
  const mismatchEvent = events.find((event) => event.eventType === 'task.capability_mismatch');
  assert.equal(mismatchEvent.payload.reasonCode, 'content_not_found');
  assert.equal(mismatchEvent.payload.reportPageType, 'error');
  assert.equal(mismatchEvent.payload.reportUrl, 'https://www.xiaohongshu.com/explore/deleted-note');
  assert.equal(mismatchEvent.payload.status, 'failed');
  assert.equal(events.some((event) => event.eventType === 'task.released'), false);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller fails comment detail tasks when capability only reports unsupported task type', async () => {
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_comment_unsupported',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/explore/note-1',
        taskStrategy: 'detail_probe',
        payload: {
          noteId: 'note-1',
          taskStrategy: 'detail_probe',
        },
      },
      lease: {
        leaseToken: 'lease-comment-unsupported',
        attemptId: 'attempt-comment-unsupported',
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({
      success: true,
      accepted: false,
      reasonCode: 'unsupported_task_type',
      reasonMessage: '当前页面能力报告未声明支持该任务类型',
      report: {
        mode: 'detail',
        pageType: 'detail',
        url: 'https://www.xiaohongshu.com/explore/note-1',
        readiness: {
          ready: false,
          reasonCode: 'content_not_found',
          reasonMessage: '当前笔记已删除或不可访问',
        },
        capabilities: {
          canRunTaskTypes: [],
        },
      },
    }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.deepEqual(patches[0], [
    'task_comment_unsupported',
    {
      status: 'failed',
      progress: 100,
      errorMessage: '当前页面能力报告未声明支持该任务类型',
    },
  ]);
  const mismatchEvent = events.find((event) => event.eventType === 'task.capability_mismatch');
  assert.equal(mismatchEvent.payload.reasonCode, 'unsupported_task_type');
  assert.equal(mismatchEvent.payload.status, 'failed');
  assert.equal(events.some((event) => event.eventType === 'task.released'), false);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller releases target mismatch with an explicit target mismatch code', async () => {
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_target_mismatch',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/expected_author',
      },
      lease: {
        leaseToken: 'lease-target-mismatch',
        attemptId: 'attempt-target-mismatch',
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({
      success: true,
      accepted: false,
      reasonCode: 'page_target_mismatch',
      reasonMessage: '当前页面不是任务目标博主',
    }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  const releaseEvent = events.find((event) => event.eventType === 'task.released');
  assert.equal(releaseEvent.payload.reasonCode, 'page_target_mismatch');
  assert.equal(releaseEvent.payload.errorCode, 'TARGET_MISMATCH');
  assert.equal(releaseEvent.payload.userMessage, '当前页面不是任务目标博主');
});

test('task poller emits a failed event when dispatch throws before startup', async () => {
  const patches = [];
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_dispatch_throw',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-dispatch-throw',
        attemptId: 'attempt-dispatch-throw',
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      throw new Error('content handler crashed during startup');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.success, false);
  assert.equal(result.reason, 'content handler crashed during startup');
  assert.equal(patches[0][1].status, 'failed');
  const failedEvent = events.find((event) => event.eventType === 'task.failed');
  assert.equal(failedEvent.attemptId, 'attempt-dispatch-throw');
  assert.equal(failedEvent.leaseId, 'lease-dispatch-throw');
  assert.equal(failedEvent.payload.reason, 'dispatch_failed');
  assert.equal(failedEvent.payload.errorMessage, 'content handler crashed during startup');
});

test('task poller emits a failed event when dispatch is rejected', async () => {
  const events = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_dispatch_rejected',
        taskType: 'douyin.batchNotes',
        platform: 'douyin',
      },
      lease: {
        leaseToken: 'lease-dispatch-rejected',
        attemptId: 'attempt-dispatch-rejected',
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: false,
      accepted: false,
      error: 'dispatch_not_accepted',
      reasonCode: 'page_context_unavailable',
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.success, false);
  const failedEvent = events.find((event) => event.eventType === 'task.failed');
  assert.equal(failedEvent.attemptId, 'attempt-dispatch-rejected');
  assert.equal(failedEvent.payload.reasonCode, 'page_context_unavailable');
  assert.equal(failedEvent.payload.errorMessage, 'dispatch_not_accepted');
});

test('task poller stores selected account and consumes quota only after dispatch', async () => {
  const quotaUpdates = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_account_1',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'account_xhs_1',
    }),
    afterDispatchSuccess: async (task, preCheck) => {
      quotaUpdates.push([task.id, preCheck.accountId]);
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_account_1',
      resultLookup: { externalTaskId: 'task_account_1' },
    }),
  });

  const result = await poller.tick();

  assert.equal(result.accepted, true);
  assert.deepEqual(quotaUpdates, [['task_account_1', 'account_xhs_1']]);
  assert.equal(poller.getState().activeTask?.accountId, 'account_xhs_1');
});

test('task poller releases a leased task when the selected account is busy', async () => {
  const events = [];
  let dispatchCalls = 0;
  let clearLeaseCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_account_busy',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-account-busy',
        attemptId: 'attempt-account-busy',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'xhs_account_busy',
    }),
    acquireExecutionLock: async () => ({
      acquired: false,
      reasonCode: 'account_busy',
      reasonMessage: '同一账号正在执行另一个采集任务',
      retryAfterMs: 60000,
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run while account is busy');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.equal(result.reason, 'account_busy');
  assert.equal(dispatchCalls, 0);
  assert.equal(clearLeaseCalls, 1);
  const releaseEvent = events.find((event) => event.eventType === 'task.released');
  assert.equal(releaseEvent.payload.reasonCode, 'account_busy');
  assert.equal(releaseEvent.payload.accountId, 'xhs_account_busy');
  assert.equal(releaseEvent.payload.retryAfterMs, 60000);
});

test('task poller clears a stale workbench account lock before dispatching a fresh task', async () => {
  const acquiredLocks = [];
  const releasedLocks = [];
  let dispatchCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'new_douyin_task',
        taskType: 'douyin.collectAuthor',
        platform: 'douyin',
      },
      lease: {
        leaseToken: 'lease-new-douyin-task',
        attemptId: 'attempt-new-douyin-task',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'douyin_account_1',
    }),
    readTaskLease: async () => ({
      taskId: 'new_douyin_task',
      leaseToken: 'lease-new-douyin-task',
      expiresAt: '2099-01-01T00:00:00.000Z',
    }),
    acquireExecutionLock: async (lock) => {
      acquiredLocks.push(lock);
      if (acquiredLocks.length === 1) {
        return {
          acquired: false,
          reasonCode: 'account_busy',
          reasonMessage: '同一账号正在执行另一个采集任务',
          existingTaskId: 'previous_douyin_task',
          retryAfterMs: 60000,
        };
      }
      return { acquired: true };
    },
    releaseExecutionLock: async (lock) => {
      releasedLocks.push(lock);
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      dispatchCalls += 1;
      return {
        success: true,
        accepted: true,
        taskId: 'new_douyin_task',
        resultLookup: { externalTaskId: 'new_douyin_task' },
      };
    },
  });

  const result = await poller.tick();

  assert.equal(result.accepted, true);
  assert.equal(dispatchCalls, 1);
  assert.equal(acquiredLocks.length, 2);
  assert.deepEqual(releasedLocks, [
    {
      platform: 'douyin',
      accountId: 'douyin_account_1',
      taskId: 'previous_douyin_task',
    },
  ]);
  assert.equal(poller.getState().activeTask?.taskId, 'new_douyin_task');
  assert.equal(poller.getState().activeTask?.accountId, 'douyin_account_1');
});

test('task poller keeps manual account locks reserved for local sync work', async () => {
  const releasedLocks = [];
  let dispatchCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'new_douyin_task_manual_busy',
        taskType: 'douyin.collectAuthor',
        platform: 'douyin',
      },
      lease: {
        leaseToken: 'lease-manual-busy',
        attemptId: 'attempt-manual-busy',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'douyin_account_1',
    }),
    acquireExecutionLock: async () => ({
      acquired: false,
      reasonCode: 'account_busy',
      reasonMessage: '同一账号正在执行另一个采集任务',
      existingTaskId: 'manual:startBatchNotes:1',
      retryAfterMs: 60000,
    }),
    releaseExecutionLock: async (lock) => {
      releasedLocks.push(lock);
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run while a manual lock is active');
    },
    clearTaskLease: async () => {},
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.equal(result.reason, 'account_busy');
  assert.equal(dispatchCalls, 0);
  assert.deepEqual(releasedLocks, []);
});

test('task poller releases a leased task when no local account is available before dispatch', async () => {
  const patches = [];
  const events = [];
  let dispatchCalls = 0;
  let clearLeaseCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_no_account',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/explore/note-1',
      },
      lease: {
        leaseToken: 'lease-no-account',
        attemptId: 'attempt-no-account',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: true,
      reason: 'no_available_account',
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => {
      dispatchCalls += 1;
      throw new Error('dispatch should not run without account');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.equal(result.reason, 'no_available_account');
  assert.equal(dispatchCalls, 0);
  assert.equal(clearLeaseCalls, 1);
  assert.deepEqual(patches[0], [
    'task_no_account',
    {
      status: 'pending',
      progress: 0,
      errorMessage: 'no_available_account',
    },
  ]);
  const releaseEvent = events.find((event) => event.eventType === 'task.released');
  assert.equal(releaseEvent.payload.reasonCode, 'no_available_account');
  assert.equal(releaseEvent.payload.status, 'pending');
  assert.equal(poller.getState().activeTask, null);
});

test('task poller releases the account execution lock when a task finishes', async () => {
  const releasedLocks = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_account_lock_done',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-account-lock-done',
        attemptId: 'attempt-account-lock-done',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'xhs_account_lock_done',
    }),
    acquireExecutionLock: async () => ({ acquired: true }),
    releaseExecutionLock: async (lock) => {
      releasedLocks.push(lock);
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_account_lock_done',
      collectionRunId: 'run_account_lock_done',
      resultLookup: {
        externalTaskId: 'task_account_lock_done',
        collectionRunId: 'run_account_lock_done',
      },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_account_lock_done',
        status: 'done',
        resultSummary: { itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: { notes: [], comments: [], authors: [], mediaAssets: [] },
      },
    }),
  });

  await poller.tick();
  await poller.tick();

  assert.deepEqual(releasedLocks, [
    {
      platform: 'xhs',
      accountId: 'xhs_account_lock_done',
      taskId: 'task_account_lock_done',
    },
  ]);
});

test('task poller flushes final result deltas before clearing the active lease', async () => {
  let poller;
  let contextAtFlush = null;
  let clearLeaseCalls = 0;
  poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_flush_before_clear',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-flush-before-clear',
        attemptId: 'attempt-flush-before-clear',
        leaseEpoch: 4,
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_flush_before_clear',
      collectionRunId: 'run_flush_before_clear',
      resultLookup: {
        externalTaskId: 'task_flush_before_clear',
        collectionRunId: 'run_flush_before_clear',
      },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_flush_before_clear',
        status: 'done',
        resultSummary: { notes: 1, itemsPlanned: 1, itemsSucceeded: 1, failedItems: 0 },
        records: {
          notes: [{ noteId: 'note_flush_1', title: '已采笔记' }],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
    enqueueRecords: async (records) => records,
    enqueueEvent: async (event) => event,
    flushDeltas: async () => {
      contextAtFlush = poller.getExecutionContext('task_flush_before_clear');
      return { success: true };
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  await poller.tick();
  const result = await poller.tick();

  assert.equal(result.final, true);
  assert.equal(contextAtFlush.leaseToken, 'lease-flush-before-clear');
  assert.equal(contextAtFlush.attemptId, 'attempt-flush-before-clear');
  assert.equal(contextAtFlush.leaseEpoch, 4);
  assert.equal(clearLeaseCalls, 1);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller waits for terminal result package before enqueueing records', async () => {
  const recordBatches = [];
  let resultPackageCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_terminal_records_only',
        taskType: 'xhs.authorNoteLinks',
        platform: 'xhs',
        collectionProfile: 'author_links',
        payload: {},
      },
      lease: {
        leaseToken: 'lease-terminal-records-only',
        attemptId: 'attempt-terminal-records-only',
        leaseEpoch: 2,
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_terminal_records_only',
      collectionRunId: 'run_terminal_records_only',
      resultLookup: {
        externalTaskId: 'task_terminal_records_only',
        collectionRunId: 'run_terminal_records_only',
      },
    }),
    getResultPackage: async () => {
      resultPackageCalls += 1;
      if (resultPackageCalls === 1) {
        return {
          success: true,
          result: {
            collectionRunId: 'run_terminal_records_only',
            status: 'running',
            resultSummary: { notes: 1, itemsPlanned: 3, itemsSucceeded: 1, failedItems: 0 },
            records: {
              notes: [{ noteId: 'author_link_1', title: '先发现的链接' }],
              comments: [],
              authors: [],
              mediaAssets: [],
            },
          },
        };
      }
      return {
        success: true,
        result: {
          collectionRunId: 'run_terminal_records_only',
          status: 'done',
          resultSummary: { notes: 2, itemsPlanned: 3, itemsSucceeded: 2, failedItems: 0 },
          records: {
            notes: [
              { noteId: 'author_link_1', title: '先发现的链接' },
              { noteId: 'author_link_2', title: '最终一起交付的链接' },
            ],
            comments: [],
            authors: [],
            mediaAssets: [],
          },
        },
      };
    },
    enqueueRecords: async (records) => {
      recordBatches.push(records);
      return records;
    },
    enqueueEvent: async (event) => event,
    flushDeltas: async () => ({ success: true }),
    clearTaskLease: async () => {},
  });

  await poller.tick();
  assert.equal(poller.getState().activeTask?.collectionProfile, 'author_links');
  const runningResult = await poller.tick();

  assert.equal(runningResult.final, false);
  assert.equal(recordBatches.length, 0);

  const finalResult = await poller.tick();

  assert.equal(finalResult.final, true);
  assert.equal(recordBatches.length, 1);
  assert.equal(recordBatches[0].length, 2);
  assert.deepEqual(recordBatches[0].map((record) => record.executionContext), [
    {
      attemptId: 'attempt-terminal-records-only',
      leaseToken: 'lease-terminal-records-only',
      leaseEpoch: 2,
      pageFingerprint: null,
      stationId: '',
      accountId: '',
      platform: 'xhs',
    },
    {
      attemptId: 'attempt-terminal-records-only',
      leaseToken: 'lease-terminal-records-only',
      leaseEpoch: 2,
      pageFingerprint: null,
      stationId: '',
      accountId: '',
      platform: 'xhs',
    },
  ]);
});

test('task poller refreshes the account execution lock while a task is running', async () => {
  const acquiredLocks = [];
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_account_lock_running',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
      lease: {
        leaseToken: 'lease-account-lock-running',
        attemptId: 'attempt-account-lock-running',
      },
    }),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'xhs_account_lock_running',
    }),
    acquireExecutionLock: async (lock) => {
      acquiredLocks.push(lock);
      return { acquired: true };
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_account_lock_running',
      collectionRunId: 'run_account_lock_running',
      resultLookup: {
        externalTaskId: 'task_account_lock_running',
        collectionRunId: 'run_account_lock_running',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-05-24T01:12:00.000Z' }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_account_lock_running',
        status: 'running',
        resultSummary: { itemsPlanned: 2, itemsSucceeded: 1, failedItems: 0 },
        records: { notes: [], comments: [], authors: [], mediaAssets: [] },
      },
    }),
  });

  await poller.tick();
  await poller.tick();

  assert.equal(acquiredLocks.length, 2);
  assert.deepEqual(acquiredLocks[1], {
    platform: 'xhs',
    accountId: 'xhs_account_lock_running',
    taskId: 'task_account_lock_running',
    leaseToken: 'lease-account-lock-running',
    attemptId: 'attempt-account-lock-running',
  });
});

test('task poller does not consume quota when dispatch never starts', async () => {
  const quotaUpdates = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_account_rejected',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
      },
    ]),
    beforeDispatch: async () => ({
      shouldPause: false,
      accountId: 'account_xhs_2',
    }),
    afterDispatchSuccess: async () => {
      quotaUpdates.push('updated');
    },
    capabilityCheck: async () => ({
      success: false,
      accepted: false,
      error: 'no_matching_tab',
    }),
    patchTask: async () => ({ success: true }),
    dispatchTask: async () => {
      throw new Error('dispatch should not run');
    },
  });

  const result = await poller.tick();

  assert.equal(result.skipped, true);
  assert.deepEqual(quotaUpdates, []);
});

test('task poller maps paused status and keeps polling active task', async () => {
  const patches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_3',
        taskType: 'xhs.batchComments',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_3',
      resultLookup: { externalTaskId: 'task_3' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_3',
        status: 'paused',
        resultSummary: {
          comments: 12,
          itemsPlanned: 2,
          itemsSucceeded: 1,
          failedItems: 0,
        },
        records: {
          notes: [],
          comments: [
            { commentId: 'comment_1', noteId: 'note_1', text: '已采到的评论' },
          ],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
  });

  await poller.tick();
  const secondTick = await poller.tick();

  assert.equal(secondTick.status, 'paused');
  assert.deepEqual(patches[1], [
    'task_3',
    {
      status: 'paused',
      progress: 50,
      pluginRunId: 'run_3',
      resultSummary: {
        comments: 12,
        itemsPlanned: 2,
        itemsSucceeded: 1,
        failedItems: 0,
        records: {
          notes: [],
          comments: [
            {
              commentId: 'comment_1',
              noteId: 'note_1',
              text: '已采到的评论',
              author: '',
              authorId: '',
              likes: 0,
              level: 1,
              url: '',
            },
          ],
          authors: [],
          mediaAssets: [],
        },
      },
      errorMessage: null,
    },
  ]);
  assert.notEqual(poller.getState().activeTask, null);
});

test('task poller maps stopped status to final stopped patch with partial results', async () => {
  const patches = [];
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_4',
        taskType: 'douyin.batchComments',
        platform: 'douyin',
        target: 'https://www.douyin.com/user/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_4',
      resultLookup: { externalTaskId: 'task_4' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_4',
        status: 'stopped',
        resultSummary: {
          comments: 6,
          itemsPlanned: 3,
          itemsSucceeded: 1,
          failedItems: 0,
          totalComments: 6,
          targetIds: ['7001', '7002', '7003'],
          contentIds: ['dy_7001'],
          failedTargets: [{ awemeId: '7002', error: 'comment api blocked' }],
        },
        records: {
          notes: [],
          comments: [
            { commentId: 'comment_9', noteId: '7001', text: '停止前已采评论' },
          ],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
  });

  await poller.tick();
  const secondTick = await poller.tick();

  assert.equal(secondTick.status, 'stopped');
  assert.deepEqual(patches[1], [
    'task_4',
    {
      status: 'stopped',
      progress: 33,
      pluginRunId: 'run_4',
      resultSummary: {
        comments: 6,
        itemsPlanned: 3,
        itemsSucceeded: 1,
        failedItems: 0,
        totalComments: 6,
        targetIds: ['7001', '7002', '7003'],
        contentIds: ['dy_7001'],
        failedTargets: [{ awemeId: '7002', error: 'comment api blocked' }],
        records: {
          notes: [],
          comments: [
            {
              commentId: 'comment_9',
              noteId: '7001',
              text: '停止前已采评论',
              author: '',
              authorId: '',
              likes: 0,
              level: 1,
              url: '',
            },
          ],
          authors: [],
          mediaAssets: [],
        },
      },
      errorMessage: null,
      deferRelease: true,
    },
  ]);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller fails monitor tasks on recoverable tab connection errors and releases the lease', async () => {
  const patches = [];
  const events = [];
  let clearLeaseCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'monitor_connection_task',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_patrol',
      },
      lease: {
        leaseToken: 'lease-monitor-1',
        expiresAt: '2026-04-19T01:11:00.000Z',
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'monitor_connection_task',
      resultLookup: { externalTaskId: 'monitor_connection_task' },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-19T01:12:00.000Z' }),
    getResultPackage: async () => ({
      success: false,
      error: 'Could not establish connection. Receiving end does not exist.',
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  await poller.tick();
  const secondTick = await poller.tick();

  assert.equal(secondTick.failed, true);
  assert.deepEqual(patches[0], [
    'monitor_connection_task',
    {
      status: 'failed',
      progress: 100,
      pluginRunId: null,
      errorMessage: 'Could not establish connection. Receiving end does not exist.',
      deferRelease: true,
      leaseToken: 'lease-monitor-1',
    },
  ]);
  assert.equal(events[0].eventType, 'task.failed');
  assert.equal(events[0].payload.status, 'failed');
  assert.equal(clearLeaseCalls, 1);
  assert.equal(poller.getState().activeTask, null);
  assert.equal(poller.getState().activeLease, null);
});

test('task poller preserves failed run diagnostics when the local run only stored error', async () => {
  const patches = [];
  const events = [];
  let clearLeaseCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'detail_probe_task',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'detail_probe',
        payload: {
          targetNoteId: '69fd330a',
        },
      },
      lease: {
        leaseToken: 'lease-detail-1',
        expiresAt: '2026-05-09T01:11:00.000Z',
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'detail_probe_task',
      collectionRunId: 'run_failed_detail',
      resultLookup: {
        collectionRunId: 'run_failed_detail',
        externalTaskId: 'detail_probe_task',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-05-09T01:12:00.000Z' }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_failed_detail',
        status: 'failed',
        resultSummary: {
          itemsPlanned: 1,
          itemsSucceeded: 0,
          failedItems: 1,
        },
        records: {
          notes: [],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
        runRecord: {
          error: '笔记数据未稳定就绪: expected=69fd330a actual=',
          diagnostic: {
            stage: 'collecting',
            failureCategory: 'retry_wait',
            reasonCode: 'page_data_not_ready',
            userMessage: '目标笔记页面没有加载出可采数据',
            technicalMessage: '笔记数据未稳定就绪: expected=69fd330a actual=',
            recommendedAction: '稍后自动重试，或改用作者页重新定位该笔记',
            evidence: {
              expectedNoteId: '69fd330a',
              currentNoteId: '',
            },
          },
        },
      },
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  await poller.tick();
  const secondTick = await poller.tick();

  assert.equal(secondTick.status, 'failed');
  assert.equal(patches[0][0], 'detail_probe_task');
  assert.equal(patches[0][1].status, 'running');
  assert.equal(patches[1][0], 'detail_probe_task');
  assert.equal(patches[1][1].status, 'failed');
  assert.equal(patches[1][1].errorMessage, '笔记数据未稳定就绪: expected=69fd330a actual=');
  const failedEvent = events.find((event) => event.eventType === 'task.failed');
  assert.ok(failedEvent);
  assert.equal(failedEvent.payload.userMessage, '目标笔记页面没有加载出可采数据');
  assert.equal(failedEvent.payload.stage, 'collecting');
  assert.equal(failedEvent.payload.failureCategory, 'retry_wait');
  assert.equal(failedEvent.payload.reasonCode, 'page_data_not_ready');
  assert.equal(failedEvent.payload.recommendedAction, '稍后自动重试，或改用作者页重新定位该笔记');
  assert.deepEqual(failedEvent.payload.evidence, {
    expectedNoteId: '69fd330a',
    currentNoteId: '',
  });
  assert.equal(clearLeaseCalls, 1);
});

test('task poller releases stale active task when lease renewal conflicts', async () => {
  const events = [];
  let clearLeaseCalls = 0;
  let claimCalls = 0;
  const poller = createTaskPoller({
    claimTaskLease: async () => {
      claimCalls += 1;
      return {
        task: {
          id: 'stale_lease_task',
          taskType: 'xhs.collectAuthor',
          platform: 'xhs',
          source: 'monitor',
          taskStrategy: 'author_baseline',
        },
        lease: {
          leaseToken: 'lease-stale',
          expiresAt: '2026-04-19T01:11:00.000Z',
        },
      };
    },
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'stale_lease_task',
      collectionRunId: 'run-stale-lease',
      resultLookup: { externalTaskId: 'stale_lease_task' },
    }),
    renewTaskLease: async () => {
      const error = new Error('Task lease is held by another station');
      error.status = 409;
      error.retryable = false;
      throw error;
    },
    getResultPackage: async () => {
      throw new Error('result lookup should stop after lease conflict');
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  await poller.tick();
  const secondTick = await poller.tick();

  assert.equal(secondTick.released, true);
  assert.equal(secondTick.reason, 'lease_conflict');
  assert.equal(secondTick.nextPollAfterMs, 120_000);
  assert.equal(claimCalls, 1);
  assert.equal(clearLeaseCalls, 1);
  assert.equal(poller.getState().activeTask, null);
  assert.equal(poller.getState().activeLease, null);
  assert.equal(events.at(-1).payload.reason, 'lease_conflict');
  assert.equal(events.at(-1).payload.retryAfterMs, 120_000);
});

test('task poller releases dispatched tasks that never produce a startup run', async () => {
  const patches = [];
  const events = [];
  let nowMs = Date.parse('2026-04-20T15:00:00.000Z');
  const poller = createTaskPoller({
    now: () => nowMs,
    claimTaskLease: claimTask([
      {
        id: 'task_startup_timeout',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'detail_probe',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_startup_timeout',
      resultLookup: { externalTaskId: 'task_startup_timeout' },
    }),
    getResultPackage: async () => ({
      success: false,
      error: 'collectionRun not found for externalTaskId: task_startup_timeout',
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
  });

  const firstTick = await poller.tick();
  assert.equal(firstTick.accepted, true);
  nowMs += 46_000;
  const retryAt = new Date(nowMs + 2 * 60 * 1000).toISOString();

  const secondTick = await poller.tick();

  assert.equal(secondTick.released, true);
  assert.deepEqual(secondTick.cleanupTask, {
    taskId: 'task_startup_timeout',
    externalTaskId: 'task_startup_timeout',
    pluginRunId: '',
  });
  assert.deepEqual(patches[1], [
    'task_startup_timeout',
    {
      status: 'pending',
      progress: 0,
      pluginRunId: null,
      errorMessage: '任务已派出，但页面没有真正启动，已自动释放重试。',
      notBeforeAt: retryAt,
    },
  ]);
  assert.equal(events.at(-1).eventType, 'task.page_open_failed');
  assert.equal(events.at(-1).payload.reason, 'dispatch_startup_timeout');
  assert.equal(events.at(-1).payload.userMessage, '任务已派出，但页面没有真正启动，已自动释放重试。');
  assert.equal(events.at(-1).payload.notBeforeAt, retryAt);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller fails running task when the result package handoff is lost', async () => {
  const patches = [];
  const events = [];
  const lookups = [];
  let nowMs = Date.parse('2026-04-20T16:00:00.000Z');
  const poller = createTaskPoller({
    now: () => nowMs,
    claimTaskLease: claimTask([
      {
        id: 'task_handoff_lost',
        taskType: 'douyin.collectAuthor',
        platform: 'douyin',
        source: 'monitor',
        taskStrategy: 'author_baseline',
        target: 'https://www.douyin.com/user/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_handoff_lost',
      tabId: 789,
      collectionRunId: 'run_handoff_lost',
      resultLookup: {
        externalTaskId: 'task_handoff_lost',
        collectionRunId: 'run_handoff_lost',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-20T16:20:00.000Z' }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return {
        success: false,
        error: 'collectionRun not found for collectionRunId: run_handoff_lost',
      };
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  await poller.tick();
  nowMs += 13 * 60 * 1000;
  const result = await poller.tick();

  assert.equal(result.failed, true);
  assert.equal(result.reason, 'result_package_handoff_lost');
  assert.deepEqual(lookups[0], {
    collectionRunId: 'run_handoff_lost',
    externalTaskId: 'task_handoff_lost',
    tabId: 789,
  });
  assert.deepEqual(patches.at(-1), [
    'task_handoff_lost',
    {
      status: 'failed',
      progress: 100,
      pluginRunId: 'run_handoff_lost',
      errorMessage: '采集页结果包没有交回工作台：插件没有找到本轮执行页，已停止这条卡住的任务。',
    },
  ]);
  assert.equal(events.at(-1).eventType, 'task.failed');
  assert.equal(events.at(-1).payload.reason, 'result_package_handoff_lost');
  assert.equal(poller.getState().activeTask, null);
});

test('task poller does NOT fail handoff-lost when attempt started recently even if dispatchedAt is stale (congestion-after-claim)', async () => {
  // 回归测试（本轮修复）：任务派出后拥堵 ~3.5h 无人接单 → 工作台 dispatchedAt 停在首次派出时间（老）；
  // 刚被工位接单（attemptStartedAtMs 新），采集页还没产出结果包（getResultPackage 报 not found）。
  // 修复前：超时用 dispatchedAtMs（hydrate 时被工作台老值覆盖）→ 第一次轮询就判 result_package_handoff_lost 误杀。
  // 修复后：超时用 attemptStartedAtMs（本轮接单时间）→ now - attemptStartedAtMs < 12min → 不误杀，给满启动窗口。
  const patches = [];
  const events = [];
  let nowMs = Date.parse('2026-04-20T16:00:00.000Z');
  const poller = createTaskPoller({
    now: () => nowMs,
    claimTaskLease: claimTask([
      {
        id: 'task_handoff_congested',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_patrol',
        target: 'https://www.xiaohongshu.com/user/content/congested-author',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_handoff_congested',
      tabId: 321,
      collectionRunId: 'run_handoff_congested',
      resultLookup: {
        externalTaskId: 'task_handoff_congested',
        collectionRunId: 'run_handoff_congested',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-20T16:20:00.000Z' }),
    getResultPackage: async () => ({
      success: false,
      error: 'collectionRun not found for collectionRunId: run_handoff_congested',
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  await poller.tick(); // claim：attemptStartedAtMs = nowMs(16:00)，dispatchedAtMs = nowMs(16:00)

  // 模拟 service worker 重启后 hydrate：dispatchedAtMs 被工作台 task.dispatchedAt 老值覆盖（拥堵 3.5h）
  const activeTask = poller.getState().activeTask;
  assert.ok(activeTask, 'claim 后应存在 activeTask');
  assert.ok(activeTask.attemptStartedAtMs > 0, 'claim 后 attemptStartedAtMs 应已设值');
  activeTask.dispatchedAtMs = nowMs - 3.5 * 60 * 60 * 1000; // 11:00（3.5h 前），模拟工作台老 dispatchedAt 覆盖
  // attemptStartedAtMs 保持 claim 时的 16:00（本轮接单时间，未被工作台覆盖）

  nowMs += 3 * 60 * 1000; // 推进 3 分钟（复现今天 10:09 → 10:12 场景）
  const result = await poller.tick();

  // 关键断言：不应被 result_package_handoff_lost 误杀
  assert.notEqual(result?.reason, 'result_package_handoff_lost');
  assert.notEqual(result?.failed, true);
  assert.equal(
    patches.find(([, patch]) => patch.status === 'failed'),
    undefined,
    '不应产生 failed 状态的 patch',
  );
  assert.equal(
    events.find((event) => event.eventType === 'task.failed'),
    undefined,
    '不应产生 task.failed 事件',
  );
  assert.equal(poller.getState().activeTask?.taskId, 'task_handoff_congested', '任务应仍存活');
});

test('task poller does not complete from streamed records when final result package handoff is lost', async () => {
  const patches = [];
  const events = [];
  const lookups = [];
  let nowMs = Date.parse('2026-04-20T16:00:00.000Z');
  const poller = createTaskPoller({
    now: () => nowMs,
    claimTaskLease: claimTask([
      {
        id: 'task_handoff_streamed',
        taskType: 'douyin.collectAuthor',
        platform: 'douyin',
        source: 'monitor',
        taskStrategy: 'author_baseline',
        target: 'https://www.douyin.com/user/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_handoff_streamed',
      tabId: 790,
      collectionRunId: 'run_handoff_streamed',
      resultLookup: {
        externalTaskId: 'task_handoff_streamed',
        collectionRunId: 'run_handoff_streamed',
      },
    }),
    renewTaskLease: async () => ({ success: true, expiresAt: '2026-04-20T16:20:00.000Z' }),
    getResultPackage: async (lookup) => {
      lookups.push(lookup);
      return {
        success: false,
        error: 'collectionRun not found for collectionRunId: run_handoff_streamed',
      };
    },
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    clearTaskLease: async () => {},
  });

  await poller.tick();
  poller.updateActiveTask({
    firstRecordSeen: true,
    streamedRecordCounts: {
      note: 2,
      author: 1,
    },
  });
  nowMs += 13 * 60 * 1000;
  const result = await poller.tick();

  assert.equal(result.failed, true);
  assert.equal(result.reason, 'result_package_handoff_lost');
  assert.deepEqual(lookups[0], {
    collectionRunId: 'run_handoff_streamed',
    externalTaskId: 'task_handoff_streamed',
    tabId: 790,
  });
  assert.equal(patches.at(-1)[0], 'task_handoff_streamed');
  assert.equal(patches.at(-1)[1].status, 'failed');
  assert.equal(patches.at(-1)[1].progress, 100);
  assert.equal(patches.at(-1)[1].pluginRunId, 'run_handoff_streamed');
  assert.match(patches.at(-1)[1].errorMessage, /结果包没有交回工作台/);
  assert.equal(events.at(-1).eventType, 'task.failed');
  assert.equal(events.at(-1).payload.reason, 'result_package_handoff_lost');
  assert.equal(poller.getState().activeTask, null);
});

test('task poller flushes a terminal stopped snapshot before clearing the active lease', async () => {
  const patches = [];
  const events = [];
  const flushContexts = [];
  let clearLeaseCalls = 0;
  let poller;

  poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_stop_snapshot',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
      lease: {
        leaseToken: 'lease-stop-snapshot',
        attemptId: 'attempt-stop-snapshot',
        leaseEpoch: 9,
      },
    }),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_stop_snapshot',
      resultLookup: { externalTaskId: 'task_stop_snapshot' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_stop_snapshot',
        status: 'running',
        resultSummary: { itemsPlanned: 1, itemsSucceeded: 0, failedItems: 0 },
        records: { notes: [], comments: [], authors: [], mediaAssets: [] },
      },
    }),
    enqueueEvent: async (event) => {
      events.push(event);
      return event;
    },
    flushDeltas: async () => {
      flushContexts.push(poller.getExecutionContext('task_stop_snapshot'));
      return { success: true };
    },
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
  });

  await poller.tick();
  poller.updateActiveTask({
    workbenchStatus: 'stopping',
    pluginRunId: 'run_stop_snapshot',
  });

  const result = await poller.tick();
  const stoppedEvent = events.find((event) => event.eventType === 'task.stopped');

  assert.equal(result.status, 'stopped');
  assert.equal(stoppedEvent.snapshot.status, 'stopped');
  assert.equal(flushContexts[0].leaseToken, 'lease-stop-snapshot');
  assert.equal(flushContexts[0].attemptId, 'attempt-stop-snapshot');
  assert.equal(clearLeaseCalls, 1);
  assert.equal(patches.at(-2)[1].deferRelease, true);
  assert.equal(patches.at(-1)[1].deferRelease, undefined);
  assert.equal(poller.getState().activeTask, null);
});

test('task poller keeps the active lease when a terminal snapshot flush fails', async () => {
  let poller;

  poller = createTaskPoller({
    claimTaskLease: async () => ({
      task: {
        id: 'task_failed_snapshot_retry',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
      lease: {
        leaseToken: 'lease-failed-snapshot',
        attemptId: 'attempt-failed-snapshot',
        leaseEpoch: 4,
      },
    }),
    patchTask: async () => ({ success: true }),
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_failed_snapshot_retry',
      resultLookup: { externalTaskId: 'task_failed_snapshot_retry' },
    }),
    getResultPackage: async () => ({
      success: true,
      result: {
        collectionRunId: 'run_failed_snapshot_retry',
        status: 'failed',
        errorMessage: '页面采集失败',
        resultSummary: { itemsPlanned: 1, itemsSucceeded: 0, failedItems: 1 },
        records: { notes: [], comments: [], authors: [], mediaAssets: [] },
      },
    }),
    enqueueEvent: async (event) => event,
    flushDeltas: async () => ({ success: false, reason: 'network_down' }),
  });

  await poller.tick();
  const result = await poller.tick();

  assert.equal(result.flushPending, true);
  assert.equal(result.final, false);
  assert.equal(poller.getState().activeTask?.taskId, 'task_failed_snapshot_retry');
  assert.equal(poller.getExecutionContext('task_failed_snapshot_retry').leaseToken, 'lease-failed-snapshot');
});

test('task poller does not startup-timeout tasks that were locally marked paused', async () => {
  const patches = [];
  let nowMs = Date.parse('2026-04-20T16:00:00.000Z');
  const poller = createTaskPoller({
    now: () => nowMs,
    claimTaskLease: claimTask([
      {
        id: 'task_risk_pause',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'detail_probe',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_risk_pause',
      resultLookup: { externalTaskId: 'task_risk_pause' },
    }),
    getResultPackage: async () => ({
      success: false,
      error: 'collectionRun not found for externalTaskId: task_risk_pause',
    }),
  });

  const firstTick = await poller.tick();
  assert.equal(firstTick.accepted, true);

  poller.updateActiveTask({
    workbenchStatus: 'paused',
    accountId: 'xhs_account_2',
    pendingAccountUsageId: 'xhs_account_2',
    errorMessage: '风控(300017)，已切换账号，等待恢复',
  });

  nowMs += 46_000;
  const secondTick = await poller.tick();

  assert.equal(secondTick.waiting, true);
  assert.equal(patches.length, 1);
  assert.equal(poller.getState().activeTask?.workbenchStatus, 'paused');
  assert.equal(poller.getState().activeTask?.pendingAccountUsageId, 'xhs_account_2');
});

test('task poller only consumes deferred replacement account usage after a run starts', async () => {
  const patches = [];
  const consumedAccountIds = [];
  let resultLookupCount = 0;
  const poller = createTaskPoller({
    claimTaskLease: claimTask([
      {
        id: 'task_risk_resume',
        taskType: 'xhs.batchNotes',
        platform: 'xhs',
        target: 'https://www.xiaohongshu.com/user/profile/demo',
      },
    ]),
    patchTask: async (taskId, patch) => {
      patches.push([taskId, patch]);
      return { success: true };
    },
    capabilityCheck: async () => ({ success: true, accepted: true }),
    dispatchTask: async () => ({
      success: true,
      accepted: true,
      taskId: 'task_risk_resume',
      resultLookup: { externalTaskId: 'task_risk_resume' },
    }),
    getResultPackage: async () => {
      resultLookupCount += 1;
      if (resultLookupCount === 1) {
        return {
          success: true,
          result: {
            collectionRunId: 'run_risk_resume',
            status: 'running',
            resultSummary: {
              itemsPlanned: 2,
              itemsSucceeded: 1,
              failedItems: 0,
            },
            records: {
              notes: [],
              comments: [],
              authors: [],
              mediaAssets: [],
            },
          },
        };
      }
      return {
        success: true,
        result: {
          collectionRunId: 'run_risk_resume',
          status: 'done',
          resultSummary: {
            itemsPlanned: 2,
            itemsSucceeded: 2,
            failedItems: 0,
          },
          records: {
            notes: [],
            comments: [],
            authors: [],
            mediaAssets: [],
          },
        },
      };
    },
    consumePendingAccountUsage: async (accountId) => {
      consumedAccountIds.push(accountId);
    },
  });

  await poller.tick();
  poller.updateActiveTask({
    workbenchStatus: 'paused',
    accountId: 'xhs_account_3',
    pendingAccountUsageId: 'xhs_account_3',
  });
  assert.deepEqual(consumedAccountIds, []);

  await poller.tick();

  assert.deepEqual(consumedAccountIds, ['xhs_account_3']);
  assert.equal(poller.getState().activeTask?.pendingAccountUsageId, '');
  assert.equal(patches[1][1].status, 'running');
});

test('task poller reconciles a stale local lease before claiming fresh work', async () => {
  let reconcileCalls = 0;
  let clearLeaseCalls = 0;
  let claimCalls = 0;
  const poller = createTaskPoller({
    now: () => 1_000_000,
    readTaskLease: async () => ({ taskId: 'stale-task', leaseToken: 'stale-token' }),
    clearTaskLease: async () => {
      clearLeaseCalls += 1;
    },
    reconcileTaskLease: async ({ localLease }) => {
      reconcileCalls += 1;
      assert.equal(localLease.leaseToken, 'stale-token');
      return { success: true, action: 'clear_local' };
    },
    claimTaskLease: async () => {
      claimCalls += 1;
      return { task: null, nextPollAfterMs: 120_000 };
    },
  });

  const result = await poller.tick();

  assert.equal(result.idle, true);
  assert.equal(reconcileCalls, 1);
  assert.equal(clearLeaseCalls, 1);
  assert.equal(claimCalls, 1);
});

test('task poller resumes a server lease returned by reconcile', async () => {
  let claimCalls = 0;
  const renewals = [];
  const poller = createTaskPoller({
    now: () => 1_000_000,
    readTaskLease: async () => null,
    reconcileTaskLease: async () => ({
      success: true,
      action: 'resume',
      task: {
        id: 'resume-task',
        taskType: 'xhs.collectAuthor',
        platform: 'xhs',
        source: 'monitor',
        taskStrategy: 'author_patrol',
        status: 'running',
      },
      lease: {
        taskId: 'resume-task',
        leaseToken: 'resume-token',
        expiresAt: '2026-05-10T01:10:00.000Z',
      },
    }),
    renewTaskLease: async (taskId, lease) => {
      renewals.push([taskId, lease.leaseToken]);
      return { success: true, expiresAt: '2026-05-10T01:15:00.000Z' };
    },
    claimTaskLease: async () => {
      claimCalls += 1;
      return { task: null };
    },
    getResultPackage: async () => ({
      success: true,
      result: {
        status: 'running',
        resultSummary: {
          itemsPlanned: 3,
          itemsSucceeded: 1,
          failedItems: 0,
        },
        records: {
          notes: [],
          comments: [],
          authors: [],
          mediaAssets: [],
        },
      },
    }),
  });

  const result = await poller.tick();

  assert.equal(result.status, 'running');
  assert.deepEqual(renewals, [['resume-task', 'resume-token']]);
  assert.equal(claimCalls, 0);
  assert.equal(poller.getState().activeTask.taskId, 'resume-task');
  assert.equal(poller.getState().activeLease.leaseToken, 'resume-token');
});

test('出站积压熔断：积压超阈值时不接新任务并返回 OUTBOX_BACKLOG_BLOCKED', async () => {
  let claimCalls = 0;
  let flushCalls = 0;
  let backlog = { pending: 10, inFlight: 0, retryable: 250, deadLetter: 30, oldestUnsentCreatedAt: Date.now() - 60 * 60 * 1000 };
  const poller = createTaskPoller({
    getOutboxBacklog: async () => backlog,
    flushOutbox: async () => { flushCalls += 1; },
    claimTaskLease: async () => {
      claimCalls += 1;
      return { task: null, reason: { code: 'NO_PENDING_TASK', message: '暂无可接任务' } };
    },
  });

  const blocked = await poller.tick();
  assert.equal(blocked.idle, true);
  assert.equal(blocked.idleReasonCode, 'OUTBOX_BACKLOG_BLOCKED');
  assert.equal(claimCalls, 0, '积压熔断期间不得尝试领取新任务');
  assert.equal(flushCalls, 1, '熔断 tick 应触发一次补发自愈');

  // 积压恢复后正常接单
  backlog = { pending: 1, inFlight: 0, retryable: 0, deadLetter: 30, oldestUnsentCreatedAt: Date.now() - 1000 };
  const recovered = await poller.tick();
  assert.equal(recovered.idleReasonCode, 'NO_PENDING_TASK');
  assert.equal(claimCalls, 1);
});

test('出站积压熔断：历史死信只告警不阻塞接单，也不触发无法自愈的补发', async () => {
  let claimCalls = 0;
  let flushCalls = 0;
  const poller = createTaskPoller({
    getOutboxBacklog: async () => ({
      pending: 0,
      inFlight: 0,
      retryable: 0,
      deadLetter: 2144,
      oldestUnsentCreatedAt: 0,
      oldestDeadLetterCreatedAt: Date.now() - 24 * 60 * 60 * 1000,
    }),
    flushOutbox: async () => { flushCalls += 1; },
    claimTaskLease: async () => {
      claimCalls += 1;
      return { task: null, reason: { code: 'NO_PENDING_TASK', message: '暂无可接任务' } };
    },
  });

  const result = await poller.tick();

  assert.equal(result.idleReasonCode, 'NO_PENDING_TASK');
  assert.equal(claimCalls, 1, '只存在死信时必须继续尝试领取新任务');
  assert.equal(flushCalls, 0, '死信不会被自动补发，不应触发无效的自愈 flush');
});

test('出站积压熔断：最老未发送记录超龄同样熔断', async () => {
  let claimCalls = 0;
  const poller = createTaskPoller({
    getOutboxBacklog: async () => ({
      pending: 3, inFlight: 0, retryable: 2, deadLetter: 0,
      oldestUnsentCreatedAt: Date.now() - 45 * 60 * 1000,
    }),
    claimTaskLease: async () => {
      claimCalls += 1;
      return { task: null };
    },
  });

  const blocked = await poller.tick();
  assert.equal(blocked.idleReasonCode, 'OUTBOX_BACKLOG_BLOCKED');
  assert.equal(claimCalls, 0);
});
