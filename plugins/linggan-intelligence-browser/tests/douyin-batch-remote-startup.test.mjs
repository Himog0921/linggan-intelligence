import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createBatchMessageHandlers } from '../src/content/douyinBatchMessageHandlers.js';

function createManagedTaskController(runTask) {
  return {
    start() {
      Promise.resolve().then(() => runTask({
        shouldStop: () => false,
        waitIfPaused: async () => {},
      }));
    },
    stop() {},
    pause() {},
    resume() {},
  };
}

function createDouyinHandlers({
  batchCollectDouyinProfileVideos = async () => ({ ok: true, success: 0, total: 0 }),
  batchCollectDouyinProfileComments = async () => ({
    ok: true,
    successVideos: 0,
    totalVideos: 0,
    totalComments: 0,
  }),
  reportProgress = () => {},
  syncTaskUI = () => {},
} = {}) {
  let batchNoteController = null;
  let batchCommentController = null;

  const handlers = createBatchMessageHandlers({
    isDouyinPage: () => true,
    createManagedTaskController,
    batchCollectDouyinProfileVideos,
    batchCollectDouyinProfileComments,
    BatchNoteController: class {},
    BatchCommentController: class {},
    reportProgress,
    reportDone: () => {},
    syncTaskUI,
    startBatchTask: () => {},
    toggleStopButton: () => {},
    hideTaskControlBar: () => {},
    setActiveTaskType: () => {},
    pauseActiveTask: () => {},
    resumeActiveTask: () => {},
    getBatchNoteCtrl: () => batchNoteController,
    setBatchNoteCtrl: (value) => {
      batchNoteController = value;
    },
    getBatchCommentCtrl: () => batchCommentController,
    setBatchCommentCtrl: (value) => {
      batchCommentController = value;
    },
    getDouyinAdapter: () => null,
  });

  return { handlers };
}

test('remote douyin batch note start waits for collectionRunId before acknowledging success', async () => {
  let releaseStartup = null;
  const { handlers } = createDouyinHandlers({
    batchCollectDouyinProfileVideos: async ({ onCollectionRun } = {}) => {
      await new Promise((resolve) => {
        releaseStartup = () => {
          onCollectionRun?.('run_dy_batch_note_1');
          resolve();
        };
      });
      await new Promise(() => {});
    },
  });

  let settled = false;
  const resultPromise = handlers[MSG.START_BATCH_NOTES]({
    count: 5,
    externalTaskMeta: {
      externalTaskId: 'wb_dy_batch_note_1',
    },
  }).then((result) => {
    settled = true;
    return result;
  });

  await Promise.resolve();
  assert.equal(settled, false);

  releaseStartup();
  const result = await resultPromise;

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_dy_batch_note_1');
});

test('remote douyin batch comment start waits for collectionRunId before acknowledging success', async () => {
  let releaseStartup = null;
  const { handlers } = createDouyinHandlers({
    batchCollectDouyinProfileComments: async ({ onCollectionRun } = {}) => {
      await new Promise((resolve) => {
        releaseStartup = () => {
          onCollectionRun?.('run_dy_batch_comment_1');
          resolve();
        };
      });
      await new Promise(() => {});
    },
  });

  let settled = false;
  const resultPromise = handlers[MSG.START_BATCH_COMMENTS]({
    count: 3,
    externalTaskMeta: {
      externalTaskId: 'wb_dy_batch_comment_1',
    },
  }).then((result) => {
    settled = true;
    return result;
  });

  await Promise.resolve();
  assert.equal(settled, false);

  releaseStartup();
  const result = await resultPromise;

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_dy_batch_comment_1');
});

test('remote douyin batch start throws startup errors instead of faking dispatch success', async () => {
  const progressCalls = [];
  const syncCalls = [];
  const { handlers } = createDouyinHandlers({
    batchCollectDouyinProfileVideos: async () => {
      throw new Error('抖音页面没有真正开始采集');
    },
    reportProgress: (...args) => {
      progressCalls.push(args);
    },
    syncTaskUI: (payload) => {
      syncCalls.push(payload);
    },
  });

  await assert.rejects(
    () =>
      handlers[MSG.START_BATCH_NOTES]({
        count: 4,
        externalTaskMeta: {
          externalTaskId: 'wb_dy_batch_fail_1',
        },
      }),
    /抖音页面没有真正开始采集/,
  );

  assert.equal(progressCalls.length, 1);
  assert.equal(progressCalls[0][2], '抖音页面没有真正开始采集');
  assert.equal(syncCalls.length, 1);
  assert.equal(syncCalls[0].taskState, 'error');
  assert.equal(syncCalls[0].message, '批量视频失败：抖音页面没有真正开始采集');
});
