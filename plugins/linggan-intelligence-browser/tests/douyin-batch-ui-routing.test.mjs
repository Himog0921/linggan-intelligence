import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createBatchMessageHandlers } from '../src/content/douyinBatchMessageHandlers.js';

test('douyin batch notes uses the late-bound douyin task UI adapter instead of xhs fallback controls', async () => {
  const fallbackCalls = [];
  const adapterCalls = [];
  let currentDouyinAdapter = null;
  let batchNoteController = null;

  const handlers = createBatchMessageHandlers({
    isDouyinPage: () => true,
    createManagedTaskController(runTask) {
      return {
        start() {
          void runTask({
            shouldStop: () => false,
            waitIfPaused: async () => {},
          });
        },
        stop() {},
      };
    },
    batchCollectDouyinProfileVideos: async () => ({
      ok: true,
      success: 1,
      total: 1,
    }),
    batchCollectDouyinProfileComments: async () => ({
      ok: true,
      successVideos: 1,
      totalVideos: 1,
      totalComments: 3,
    }),
    BatchNoteController: class {},
    BatchCommentController: class {},
    reportProgress: () => {},
    reportDone: () => {},
    syncTaskUI: () => {
      fallbackCalls.push('syncTaskUI');
    },
    startBatchTask(taskType) {
      fallbackCalls.push(`start:${taskType}`);
    },
    toggleStopButton: () => {},
    hideTaskControlBar: () => {
      fallbackCalls.push('hideTaskControlBar');
    },
    setActiveTaskType: () => {},
    pauseActiveTask: () => {},
    resumeActiveTask: () => {},
    getBatchNoteCtrl: () => batchNoteController,
    setBatchNoteCtrl: (value) => {
      batchNoteController = value;
    },
    getBatchCommentCtrl: () => null,
    setBatchCommentCtrl: () => {},
    getDouyinAdapter: () => currentDouyinAdapter,
  });

  currentDouyinAdapter = {
    syncTaskUI(progress) {
      adapterCalls.push(['syncTaskUI', progress.taskType]);
    },
    startBatchTask(taskType) {
      adapterCalls.push(['startBatchTask', taskType]);
    },
    hideTaskControlBar() {
      adapterCalls.push(['hideTaskControlBar']);
    },
    setActiveTaskType(value) {
      adapterCalls.push(['setActiveTaskType', value]);
    },
    pauseBatch() {},
    resumeBatch() {},
    stopBatch() {},
  };

  const result = await handlers[MSG.START_BATCH_NOTES]({
    count: 1,
    mode: 'profile',
  });

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(result.success, true);
  assert.deepEqual(adapterCalls[0], ['startBatchTask', 'batchNotes']);
  assert.equal(
    fallbackCalls.includes('start:batchNotes'),
    false,
    'xhs batch task controls should not be used for douyin batch tasks',
  );
});

test('douyin batch pause resume and stop controls use the late-bound adapter', async () => {
  const adapterCalls = [];
  let currentDouyinAdapter = null;

  const handlers = createBatchMessageHandlers({
    isDouyinPage: () => true,
    createManagedTaskController: () => ({
      start() {},
      stop() {},
    }),
    batchCollectDouyinProfileVideos: async () => ({ ok: true, success: 1, total: 1 }),
    batchCollectDouyinProfileComments: async () => ({ ok: true, successVideos: 1, totalVideos: 1, totalComments: 1 }),
    BatchNoteController: class {},
    BatchCommentController: class {},
    reportProgress: () => {},
    reportDone: () => {},
    syncTaskUI: () => {},
    startBatchTask: () => {},
    toggleStopButton: () => {},
    hideTaskControlBar: () => {},
    setActiveTaskType: () => {},
    pauseActiveTask: () => {
      throw new Error('xhs pause fallback should not run');
    },
    resumeActiveTask: () => {
      throw new Error('xhs resume fallback should not run');
    },
    getBatchNoteCtrl: () => ({ stop() { adapterCalls.push(['controllerStop']); } }),
    setBatchNoteCtrl: () => {},
    getBatchCommentCtrl: () => ({ stop() { adapterCalls.push(['controllerStop']); } }),
    setBatchCommentCtrl: () => {},
    getDouyinAdapter: () => currentDouyinAdapter,
  });

  currentDouyinAdapter = {
    syncTaskUI() {},
    startBatchTask() {},
    hideTaskControlBar() {
      adapterCalls.push(['hideTaskControlBar']);
    },
    setActiveTaskType(value) {
      adapterCalls.push(['setActiveTaskType', value]);
    },
    pauseBatch() {
      adapterCalls.push(['pauseBatch']);
    },
    resumeBatch() {
      adapterCalls.push(['resumeBatch']);
    },
    stopBatch() {
      adapterCalls.push(['stopBatch']);
    },
  };

  assert.deepEqual(handlers[MSG.PAUSE_BATCH_NOTES](), { success: true });
  assert.deepEqual(handlers[MSG.RESUME_BATCH_NOTES](), { success: true });
  assert.deepEqual(handlers[MSG.STOP_BATCH_NOTES](), { success: true });

  assert.deepEqual(adapterCalls, [
    ['pauseBatch'],
    ['resumeBatch'],
    ['controllerStop'],
    ['stopBatch'],
    ['hideTaskControlBar'],
    ['setActiveTaskType', null],
  ]);
});

test('douyin batch comments auto security pause also pauses the managed controller and can resume', async () => {
  const adapterCalls = [];
  let currentDouyinAdapter = null;
  let managedController = null;
  let waitObserved = false;
  let runFinished = null;

  const handlers = createBatchMessageHandlers({
    isDouyinPage: () => true,
    createManagedTaskController(runTask) {
      const state = {
        isRunning: false,
        isPaused: false,
        pauseResolve: null,
      };
      const controller = {
        get isRunning() {
          return state.isRunning;
        },
        pause() {
          state.isPaused = true;
        },
        resume() {
          state.isPaused = false;
          if (state.pauseResolve) {
            state.pauseResolve();
            state.pauseResolve = null;
          }
        },
        stop() {
          controller.resume();
        },
        async waitIfPaused() {
          if (!state.isPaused) return;
          waitObserved = true;
          await new Promise((resolve) => {
            state.pauseResolve = resolve;
          });
        },
        start() {
          state.isRunning = true;
          runFinished = Promise.resolve(runTask({
            shouldStop: () => false,
            waitIfPaused: controller.waitIfPaused,
          })).finally(() => {
            state.isRunning = false;
          });
        },
      };
      managedController = controller;
      return controller;
    },
    batchCollectDouyinProfileVideos: async () => ({ ok: true, success: 1, total: 1 }),
    batchCollectDouyinProfileComments: async ({ onProgress, waitIfPaused }) => {
      onProgress({
        taskState: 'paused',
        current: 1,
        total: 3,
        message: '检测到抖音安全验证，请先完成验证后点击“继续”。',
      });
      await waitIfPaused();
      return { ok: true, successVideos: 1, totalVideos: 1, totalComments: 6 };
    },
    BatchNoteController: class {},
    BatchCommentController: class {},
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
    getBatchCommentCtrl: () => managedController,
    setBatchCommentCtrl: (value) => {
      managedController = value;
    },
    getDouyinAdapter: () => currentDouyinAdapter,
  });

  currentDouyinAdapter = {
    syncTaskUI(progress) {
      adapterCalls.push(['syncTaskUI', progress.taskState || 'running', progress.message]);
    },
    startBatchTask(taskType) {
      adapterCalls.push(['startBatchTask', taskType]);
    },
    hideTaskControlBar() {
      adapterCalls.push(['hideTaskControlBar']);
    },
    setActiveTaskType(value) {
      adapterCalls.push(['setActiveTaskType', value]);
    },
    attachExternalBatchController(controller) {
      adapterCalls.push(['attachExternalBatchController', Boolean(controller)]);
    },
    pauseBatch(payload) {
      adapterCalls.push(['pauseBatch', payload?.message, payload?.current, payload?.total]);
    },
    resumeBatch() {
      adapterCalls.push(['resumeBatch']);
    },
    stopBatch() {
      adapterCalls.push(['stopBatch']);
    },
  };

  const startResult = await handlers[MSG.START_BATCH_COMMENTS]({
    count: 3,
    mode: 'search',
  });

  assert.equal(startResult.success, true);
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(waitObserved, true);
  assert.deepEqual(adapterCalls.slice(0, 3), [
    ['attachExternalBatchController', true],
    ['startBatchTask', 'batchComments'],
    ['pauseBatch', '检测到抖音安全验证，请先完成验证后点击“继续”。', 1, 3],
  ]);

  assert.deepEqual(handlers[MSG.RESUME_BATCH_COMMENTS](), { success: true });
  await runFinished;

  assert.equal(
    adapterCalls.some((entry) => entry[0] === 'resumeBatch'),
    true,
    'resume should still be forwarded to the douyin adapter',
  );
  assert.equal(
    adapterCalls.some((entry) => entry[0] === 'attachExternalBatchController' && entry[1] === false),
    true,
    'adapter should release the external controller when the task finishes',
  );
});
