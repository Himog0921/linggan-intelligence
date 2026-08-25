import test from 'node:test';
import assert from 'node:assert/strict';

import { createXhsPageController } from '../src/content/xhsPageController.js';
import { TASK_STATE } from '../src/shared/constants.js';

function createControllerHarness() {
  const taskBarStates = [];
  const pauseResumeStates = [];

  const controller = createXhsPageController({
    MSG: {},
    collectComments: async () => ({ total: 0, comments: [] }),
    collectCommentImages: async () => ({ total: 0, images: [] }),
    collectNote: async () => ({}),
    collectAuthor: async () => ({}),
    BatchNoteController: function BatchNoteController() {},
    BatchCommentController: function BatchCommentController() {},
    injectUI: () => {},
    toggleStopButton: () => {},
    togglePauseResumeButtons: (isPaused) => {
      pauseResumeStates.push(isPaused);
    },
    showToast: () => {},
    showCommentLimitDialog: async () => ({}),
    showMediaDownloadDialog: async () => false,
    showBatchSettingsDialog: async () => ({}),
    ensureTaskControlBar: () => {},
    updateTaskControlBar: (state) => {
      taskBarStates.push(state);
    },
    hideTaskControlBar: () => {},
    isContextValid: () => true,
    reportDone: () => {},
    extractNoteId: () => 'note_1',
    sendToBackground: async () => ({}),
    downloadNoteMediaFromRecord: async () => ({}),
  });

  return { controller, taskBarStates, pauseResumeStates };
}

test('xhs batch pause keeps the latest progress summary instead of resetting to zero', () => {
  const { controller, taskBarStates, pauseResumeStates } = createControllerHarness();
  const batchNoteCtrl = {
    isRunning: true,
    pauseCalled: 0,
    pause() {
      this.pauseCalled += 1;
    },
    resume() {},
  };

  controller.setBatchNoteCtrl(batchNoteCtrl);
  controller.syncTaskUI({
    taskType: 'batchNotes',
    taskState: TASK_STATE.RUNNING,
    current: 3,
    total: 10,
    message: '正在采集第 3/10 条',
  });

  controller.pauseActiveTask();

  assert.equal(batchNoteCtrl.pauseCalled, 1);
  assert.deepEqual(pauseResumeStates, [true]);

  const paused = taskBarStates.at(-1);
  assert.equal(paused.taskState, TASK_STATE.PAUSED);
  assert.equal(paused.current, 3);
  assert.equal(paused.total, 10);
  assert.equal(paused.message, '已暂停');
});

test('xhs batch resume keeps the latest progress summary instead of resetting to zero', () => {
  const { controller, taskBarStates, pauseResumeStates } = createControllerHarness();
  const batchCommentCtrl = {
    isRunning: true,
    resumeCalled: 0,
    pause() {},
    resume() {
      this.resumeCalled += 1;
    },
  };

  controller.setBatchCommentCtrl(batchCommentCtrl);
  controller.syncTaskUI({
    taskType: 'batchComments',
    taskState: TASK_STATE.PAUSED,
    current: 5,
    total: 12,
    message: '已暂停',
  });

  controller.resumeActiveTask();

  assert.equal(batchCommentCtrl.resumeCalled, 1);
  assert.deepEqual(pauseResumeStates, [false]);

  const resumed = taskBarStates.at(-1);
  assert.equal(resumed.taskState, TASK_STATE.RUNNING);
  assert.equal(resumed.current, 5);
  assert.equal(resumed.total, 12);
  assert.equal(resumed.message, '继续采集');
});

test('xhs task UI falls back to TASK_STATE status when taskState carries a collection terminal status', () => {
  const { controller, taskBarStates } = createControllerHarness();

  controller.syncTaskUI({
    taskType: 'batchNotes',
    taskState: 'stopped',
    status: TASK_STATE.IDLE,
    current: 2,
    total: 5,
    message: '批量笔记已停止',
  });

  const stopped = taskBarStates.at(-1);
  assert.equal(stopped.taskState, TASK_STATE.IDLE);
  assert.equal(stopped.current, 2);
  assert.equal(stopped.total, 5);
});

test('xhs page controller does not duplicate page listeners on repeated init and cleans them', () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  const originalMutationObserver = globalThis.MutationObserver;
  const originalSetTimeout = globalThis.setTimeout;
  const originalClearTimeout = globalThis.clearTimeout;

  const added = [];
  const removed = [];
  const observers = [];
  const clearedTimers = [];
  let timerId = 0;

  globalThis.window = {
    location: { href: 'https://www.xiaohongshu.com/explore/note_1' },
    __lgboom_injecting: false,
  };
  globalThis.document = {
    body: {},
    querySelector: () => ({ className: 'lgboom-btn-group' }),
    addEventListener(type, handler) {
      added.push({ type, handler });
    },
    removeEventListener(type, handler) {
      removed.push({ type, handler });
    },
  };
  globalThis.MutationObserver = class TestMutationObserver {
    constructor(callback) {
      this.callback = callback;
      this.disconnected = false;
      observers.push(this);
    }

    observe(target, options) {
      this.target = target;
      this.options = options;
    }

    disconnect() {
      this.disconnected = true;
    }
  };
  globalThis.setTimeout = () => {
    timerId += 1;
    return timerId;
  };
  globalThis.clearTimeout = (id) => {
    if (id) clearedTimers.push(id);
  };

  try {
    const { controller } = createControllerHarness();
    controller.initPage();
    controller.initPage();

    assert.equal(observers.length, 1);
    assert.equal(added.filter((entry) => entry.type === 'click').length, 1);

    controller.cleanupPage();

    assert.equal(observers[0].disconnected, true);
    assert.equal(removed.length, 1);
    assert.equal(removed[0].handler, added[0].handler);
    assert.ok(clearedTimers.length >= 1);
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
    globalThis.MutationObserver = originalMutationObserver;
    globalThis.setTimeout = originalSetTimeout;
    globalThis.clearTimeout = originalClearTimeout;
  }
});
