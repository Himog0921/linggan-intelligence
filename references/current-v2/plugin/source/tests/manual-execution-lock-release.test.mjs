import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createBatchMessageHandlers } from '../src/content/douyinBatchMessageHandlers.js';

function baseDeps(overrides = {}) {
  return {
    isDouyinPage: () => false,
    createManagedTaskController: (runTask) => ({
      start() {
        void runTask({
          shouldStop: () => false,
          waitIfPaused: async () => {},
        });
      },
      stop() {},
    }),
    batchCollectDouyinProfileVideos: async () => ({ ok: true, success: 1, total: 1 }),
    batchCollectDouyinProfileComments: async () => ({ ok: true, successVideos: 1, totalVideos: 1, totalComments: 1 }),
    BatchNoteController: class {
      async start() {}
    },
    BatchCommentController: class {
      async start() {}
    },
    reportProgress: () => {},
    reportDone: () => {},
    syncTaskUI: () => {},
    startBatchTask: () => {},
    toggleStopButton: () => {},
    hideTaskControlBar: () => {},
    setActiveTaskType: () => {},
    pauseActiveTask: () => {},
    resumeActiveTask: () => {},
    getBatchNoteCtrl: () => null,
    setBatchNoteCtrl: () => {},
    getBatchCommentCtrl: () => null,
    setBatchCommentCtrl: () => {},
    getDouyinAdapter: () => null,
    ...overrides,
  };
}

test('xhs manual batch releases its execution lock after the task finishes', async () => {
  const released = [];
  const lock = { platform: 'xhs', accountId: 'account_1', taskId: 'manual_xhs_1' };
  const handlers = createBatchMessageHandlers(baseDeps({
    releaseExecutionLock: async (executionLock) => {
      released.push(executionLock);
    },
  }));

  const result = await handlers[MSG.START_BATCH_NOTES]({
    count: 1,
    mode: 'profile',
    executionLock: lock,
  });
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(result.success, true);
  assert.deepEqual(released, [lock]);
});

test('douyin manual batch releases its execution lock after the task finishes', async () => {
  const released = [];
  const lock = { platform: 'douyin', accountId: 'account_2', taskId: 'manual_douyin_1' };
  const handlers = createBatchMessageHandlers(baseDeps({
    isDouyinPage: () => true,
    releaseExecutionLock: async (executionLock) => {
      released.push(executionLock);
    },
  }));

  const result = await handlers[MSG.START_BATCH_COMMENTS]({
    count: 1,
    mode: 'profile',
    executionLock: lock,
  });
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(result.success, true);
  assert.deepEqual(released, [lock]);
});
