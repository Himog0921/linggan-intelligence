import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeAuthorRecord } from '../src/db/recordNormalization.js';
import { createContentMessageHandlers } from '../src/content/messageHandlers.js';
import { MSG } from '../src/shared/constants.js';
import { mapTaskControlToInternalCommand } from '../src/workbench/runtime/taskControlMapper.js';
import { REMOTE_TASK_CONTROL_ACTION, WORKBENCH_MESSAGE_TYPE } from '../src/workbench/protocol/schema.js';

test('normalizeAuthorRecord preserves collectionRunId for author records', () => {
  const normalized = normalizeAuthorRecord({
    platform: 'xhs',
    userId: 'user_1',
    name: '作者A',
    collectionRunId: 'run_author_1',
  });

  assert.equal(normalized.collectionRunId, 'run_author_1');
});

test('collect author task controls route to the active content page', () => {
  const command = mapTaskControlToInternalCommand({
    type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
    taskId: 'wb_task_author_control_1',
    taskType: 'douyin.collectAuthor',
    action: REMOTE_TASK_CONTROL_ACTION.STOP,
    protocolVersion: 'v1',
  }, { tabId: 42 });

  assert.equal(command.dispatchTarget, 'content');
  assert.equal(command.action, MSG.WORKBENCH_TASK_CONTROL);
  assert.equal(command.payload.taskControl.taskId, 'wb_task_author_control_1');
  assert.equal(command.payload.taskControl.action, REMOTE_TASK_CONTROL_ACTION.STOP);
});

test('remote author collection creates a collection run and passes collectionRunId into xhs collector', async () => {
  const calls = [];
  const collectionRunStore = {
    async createRun(input) {
      calls.push(['createRun', input]);
      return { collectionRunId: 'run_author_1' };
    },
    async markDone(runId, payload) {
      calls.push(['markDone', runId, payload]);
      return { collectionRunId: runId, ...payload };
    },
    async markFailed(runId, error, payload) {
      calls.push(['markFailed', runId, error, payload]);
      return { collectionRunId: runId, error, ...payload };
    },
  };

  let collectorOptions = null;
  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => false,
    collectAuthor: async (options = {}) => {
      collectorOptions = options;
      return {
        userId: 'xhs_1',
        name: '作者A',
        collectionRunId: options.collectionRunId,
      };
    },
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
    collectionRunStore,
  });

  const result = await handlers[MSG.COLLECT_AUTHOR]({
    triggerSource: 'workbench_dispatch',
    externalTaskMeta: {
      externalTaskId: 'wb_task_author_1',
      externalTaskType: 'xhs.collectAuthor',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.collectionRunId, 'run_author_1');
  assert.equal(collectorOptions.collectionRunId, 'run_author_1');
  assert.equal(collectorOptions.triggerSource, 'workbench_dispatch');
  assert.equal(calls[0][0], 'createRun');
  assert.equal(calls[0][1].externalTaskId, 'wb_task_author_1');
  assert.equal(calls[0][1].externalTaskType, 'xhs.collectAuthor');
  assert.equal(calls[0][1].protocolVersion, 'v1');
  assert.equal(calls[0][1].platform, 'xhs');
  assert.equal(calls[0][1].taskType, 'collectAuthor');
  assert.equal(calls[0][1].pageType, 'profile');
  assert.equal(calls[0][1].triggerSource, 'workbench_dispatch');
  assert.equal(calls[0][1].resultUploadStatus, 'pending_upload');
  assert.equal(typeof calls[0][1].lastHeartbeatAt, 'number');
  assert.deepEqual(calls[0][1].config, {});
  assert.deepEqual(calls[0][1].meta, { pageUrl: '' });
  assert.equal(calls[1][0], 'markDone');
  assert.equal(calls[1][1], 'run_author_1');
});

test('remote author collection can acknowledge async dispatch before collector finishes', async () => {
  const calls = [];
  let releaseCollector;
  let collectorStarted = false;
  const collectorDone = new Promise((resolve) => {
    releaseCollector = resolve;
  });

  const collectionRunStore = {
    async createRun(input) {
      calls.push(['createRun', input]);
      return { collectionRunId: 'run_author_async_1' };
    },
    async markDone(runId, payload) {
      calls.push(['markDone', runId, payload]);
      return { collectionRunId: runId, ...payload };
    },
    async markFailed(runId, error, payload) {
      calls.push(['markFailed', runId, error, payload]);
      return { collectionRunId: runId, error, ...payload };
    },
  };

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => false,
    collectAuthor: async (options = {}) => {
      collectorStarted = true;
      await collectorDone;
      return {
        userId: 'xhs_async_1',
        name: '作者B',
        collectionRunId: options.collectionRunId,
      };
    },
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
    collectionRunStore,
  });

  const result = await handlers[MSG.COLLECT_AUTHOR]({
    triggerSource: 'workbench_dispatch',
    asyncDispatch: true,
    externalTaskMeta: {
      externalTaskId: 'wb_task_author_async_1',
      externalTaskType: 'xhs.collectAuthor',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_author_async_1');
  assert.equal(calls.length, 1);
  assert.equal(calls[0][0], 'createRun');

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(collectorStarted, true);
  assert.equal(calls.length, 1);

  releaseCollector();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(calls[1][0], 'markDone');
  assert.equal(calls[1][1], 'run_author_async_1');
});

test('remote douyin author collection stops and packages the partial author result', async () => {
  const calls = [];
  let collectorStarted = false;
  let collectorOptions = null;
  let releaseCollector;
  let resolveStopped;
  const collectorDone = new Promise((resolve) => {
    releaseCollector = resolve;
  });
  const stoppedDone = new Promise((resolve) => {
    resolveStopped = resolve;
  });

  const collectionRunStore = {
    async createRun(input) {
      calls.push(['createRun', input]);
      return { collectionRunId: 'run_douyin_author_stop_1' };
    },
    async markDone() {
      throw new Error('markDone should not be called after stop');
    },
    async markStopped(runId, payload) {
      calls.push(['markStopped', runId, payload]);
      resolveStopped(payload);
      return { collectionRunId: runId, ...payload };
    },
    async markFailed() {
      throw new Error('markFailed should not be called after stop');
    },
  };

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async (options = {}) => {
      collectorStarted = true;
      collectorOptions = options;
      await collectorDone;
      return {
        ok: true,
        data: {
          userId: 'dy_author_stop_1',
          platformAuthorId: 'dy_author_stop_1',
          name: '抖音作者',
        },
      };
    },
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {
      throw new Error('reportDone should not be called after stop');
    },
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'profile' }),
    collectionRunStore,
  });

  const accepted = await handlers[MSG.COLLECT_AUTHOR]({
    triggerSource: 'workbench_dispatch',
    asyncDispatch: true,
    externalTaskMeta: {
      externalTaskId: 'wb_task_douyin_author_stop_1',
      externalTaskType: 'douyin.collectAuthor',
      protocolVersion: 'v1',
    },
  });

  assert.equal(accepted.success, true);
  assert.equal(accepted.pending, true);
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(collectorStarted, true);

  const stopped = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    taskControl: {
      taskId: 'wb_task_douyin_author_stop_1',
      taskType: 'douyin.collectAuthor',
      collectionRunId: 'run_douyin_author_stop_1',
      action: REMOTE_TASK_CONTROL_ACTION.STOP,
      protocolVersion: 'v1',
    },
  });

  assert.equal(stopped.success, true);
  assert.equal(stopped.status, 'stopping');
  assert.equal(collectorOptions.shouldStop(), true);

  releaseCollector();
  const stoppedPayload = await stoppedDone;
  assert.deepEqual(calls[1], ['markStopped', 'run_douyin_author_stop_1', stoppedPayload]);
  assert.equal(stoppedPayload.itemsSucceeded, 1);
  assert.deepEqual(stoppedPayload.targetIds, ['dy_author_stop_1']);
});

test('remote xhs author baseline waits for profile batch notes and reuses the same collection run', async () => {
  const calls = [];
  const previousWindow = globalThis.window;
  let batchStarted = false;
  let releaseBatch;
  const batchDone = new Promise((resolve) => {
    releaseBatch = resolve;
  });

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/xhs_author_1',
      pathname: '/user/profile/xhs_author_1',
      origin: 'https://www.xiaohongshu.com',
    },
  };

  try {
    class FakeBatchNoteController {
      constructor() {
        this.noteList = [];
        this.collected = [];
        this.failed = [];
        this.collectionRunId = '';
        this._stoppedByUser = false;
      }

      async start(mode, onProgress, options = {}) {
        batchStarted = true;
        calls.push(['batchStart', mode, options]);
        this.collectionRunId = options.collectionRunId;
        this.noteList = [
          { noteId: 'note_1' },
          { noteId: 'note_2' },
        ];
        this.collected = [
          { noteId: 'note_1', contentId: 'xhs_note_1' },
          { noteId: 'note_2', contentId: 'xhs_note_2' },
        ];
        await batchDone;
      }
    }

    const collectionRunStore = {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_author_baseline_1' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
        return { collectionRunId: runId, ...payload };
      },
      async markStopped() {
        throw new Error('markStopped should not be called');
      },
      async markFailed() {
        throw new Error('markFailed should not be called');
      },
    };

    const handlers = createContentMessageHandlers({
      MSG,
      isDouyinPage: () => false,
      collectAuthor: async (options = {}) => ({
        userId: 'xhs_author_1',
        platformAuthorId: 'xhs_author_1',
        name: '作者C',
        collectionRunId: options.collectionRunId,
      }),
      collectDouyinAuthor: async () => ({ ok: true, data: {} }),
      BatchNoteController: FakeBatchNoteController,
      noteStore: { count: async () => 0, getAll: async () => [] },
      commentStore: { count: async () => 0, getAll: async () => [] },
      authorStore: { count: async () => 0, getAll: async () => [] },
      reportDone: () => {},
      batchMessageHandlers: {},
      extractNoteId: () => '',
      downloadNoteMediaFromRecord: async () => ({}),
      generateCsv: () => '',
      downloadFile: () => {},
      backfillLegacyAiReadyFields: async () => ({}),
      getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
      collectionRunStore,
    });

    const pending = handlers[MSG.COLLECT_AUTHOR]({
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'wb_task_author_baseline_1',
        externalTaskType: 'xhs.collectAuthor',
        protocolVersion: 'v1',
        monitorMeta: {
          monitorId: 'monitor_author_1',
          taskStrategy: 'author_baseline',
          scanLimit: 50,
          surfaceOnly: true,
          targetUrl: 'https://www.xiaohongshu.com/user/profile/xhs_author_1',
        },
      },
    });

    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(batchStarted, true);
    assert.equal(calls.findIndex(([type]) => type === 'markDone'), -1);

    releaseBatch();
    const result = await pending;

    assert.equal(result.success, true);
    assert.equal(result.collectionRunId, 'run_author_baseline_1');
    assert.deepEqual(calls[1], ['batchStart', 'profile', {
      count: 50,
      topByLikes: false,
      triggerSource: 'workbench_dispatch',
      collectionRunId: 'run_author_baseline_1',
      monitorMeta: {
        monitorId: 'monitor_author_1',
        taskStrategy: 'author_baseline',
        scanLimit: 50,
        surfaceOnly: true,
        targetUrl: 'https://www.xiaohongshu.com/user/profile/xhs_author_1',
      },
      surfaceOnly: false,
    }]);
    assert.deepEqual(calls[2], ['markDone', 'run_author_baseline_1', {
      itemsPlanned: 3,
      itemsSucceeded: 3,
      itemsFailed: 0,
      targetIds: ['xhs_author_1', 'note_1', 'note_2'],
      contentIds: ['xhs_note_1', 'xhs_note_2'],
      completionNote: '这轮原计划建档 50 条，但当前主页最终只发现 2 条可采作品，所以先按现有作品完成建档。',
      requestedCount: 50,
      discoveredCount: 2,
      shortfallCount: 48,
    }]);
  } finally {
    globalThis.window = previousWindow;
  }
});

test('content workbench result handler proxies to page-side result packager', async () => {
  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => false,
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
    collectionRunStore: {},
    packageWorkbenchResult: async ({ externalTaskId }) => ({
      externalTaskId,
      resultSummary: { authors: 1 },
    }),
  });

  const result = await handlers[MSG.WORKBENCH_GET_RESULT_PACKAGE]({
    externalTaskId: 'wb_result_1',
  });

  assert.equal(result.success, true);
  assert.equal(result.result.externalTaskId, 'wb_result_1');
  assert.equal(result.result.resultSummary.authors, 1);
});
