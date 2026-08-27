import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createXhsPageController,
  formatXhsDetailDeliveryMessage,
} from '../src/content/xhsPageController.js';
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

  const receipt = controller.pauseActiveTask();

  assert.deepEqual(receipt, { success: true, state: 'paused' });
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

  const receipt = controller.resumeActiveTask();

  assert.deepEqual(receipt, { success: true, state: 'running' });
  assert.equal(batchCommentCtrl.resumeCalled, 1);
  assert.deepEqual(pauseResumeStates, [false]);

  const resumed = taskBarStates.at(-1);
  assert.equal(resumed.taskState, TASK_STATE.RUNNING);
  assert.equal(resumed.current, 5);
  assert.equal(resumed.total, 12);
  assert.equal(resumed.message, '继续采集');
});

test('xhs stop reports success only while an existing batch task is active', () => {
  const { controller } = createControllerHarness();
  const batchNoteCtrl = {
    isRunning: true,
    stopCalled: 0,
    pause() {},
    resume() {},
    stop() {
      this.stopCalled += 1;
    },
  };

  controller.setBatchNoteCtrl(batchNoteCtrl);
  assert.deepEqual(controller.stopActiveTask(), { success: true, state: 'stopped' });
  assert.equal(batchNoteCtrl.stopCalled, 1);
});

test('xhs control actions refuse absent tasks without changing visible progress', () => {
  const { controller, taskBarStates, pauseResumeStates } = createControllerHarness();

  assert.deepEqual(controller.pauseActiveTask(), { success: false, state: 'no_active_task' });
  assert.deepEqual(controller.resumeActiveTask(), { success: false, state: 'no_active_task' });
  assert.deepEqual(controller.stopActiveTask(), { success: false, state: 'no_active_task' });
  assert.deepEqual(taskBarStates, []);
  assert.deepEqual(pauseResumeStates, []);
});

test('Linggan detail collection reports every receipt lane and never opens the manual media window', async () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  const toasts = [];
  let mediaDialogCalls = 0;
  let mediaDownloadCalls = 0;
  globalThis.window = { location: { href: 'https://www.xiaohongshu.com/explore/note_1' } };
  globalThis.document = {};

  try {
    const controller = createXhsPageController({
      MSG: {},
      assertPluginAuthorized: async () => ({ authorized: true }),
      collectComments: async () => ({ total: 0, comments: [] }),
      collectCommentImages: async () => ({ total: 0, images: [] }),
      collectNote: async () => ({
        title: '不应出现在回执里的原始标题',
        images: ['https://example.com/one.jpg'],
        lingganDetailPackageDelivery: {
          state: 'pending',
          lanes: { content: 'acknowledged', mediaSlots: 'pending', comments: 'acknowledged' },
        },
      }),
      collectAuthor: async () => ({}),
      BatchNoteController: function BatchNoteController() {},
      BatchCommentController: function BatchCommentController() {},
      injectUI: () => {},
      toggleStopButton: () => {},
      togglePauseResumeButtons: () => {},
      showToast: (message, tone) => toasts.push({ message, tone }),
      showCommentLimitDialog: async () => ({}),
      showMediaDownloadDialog: async () => {
        mediaDialogCalls += 1;
        return { mediaTypes: ['images'], count: 1 };
      },
      showBatchSettingsDialog: async () => ({}),
      ensureTaskControlBar: () => {},
      updateTaskControlBar: () => {},
      hideTaskControlBar: () => {},
      isContextValid: () => true,
      reportDone: () => {},
      extractNoteId: () => 'note_1',
      sendToBackground: async () => ({}),
      downloadNoteMediaFromRecord: async () => {
        mediaDownloadCalls += 1;
        return { total: 1, success: 1, failed: 0 };
      },
    });
    const button = { dataset: { action: 'collectNote', params: '{}' } };
    await controller.handleButtonClick({
      target: { closest: (selector) => (selector === '.lgboom-btn' ? button : null) },
    });

    assert.equal(mediaDialogCalls, 0);
    assert.equal(mediaDownloadCalls, 0);
    assert.equal(toasts.at(-1).message, '笔记详情已读取。Linggan 接纳回执：笔记详情：已接纳；媒体观察：待本机交付；评论与回复：已接纳');
    assert.doesNotMatch(toasts.at(-1).message, /原始标题/);
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  }
});

test('explicit manual detail action retains the media selection and local download workflow', async () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  let mediaDialogCalls = 0;
  let mediaDownloadCalls = 0;
  globalThis.window = { location: { href: 'https://www.xiaohongshu.com/explore/note_1' } };
  globalThis.document = {};

  try {
    const controller = createXhsPageController({
      MSG: {},
      assertPluginAuthorized: async () => ({ authorized: true }),
      collectComments: async () => ({ total: 0, comments: [] }),
      collectCommentImages: async () => ({ total: 0, images: [] }),
      collectNote: async () => ({
        images: ['https://example.com/one.jpg'],
        lingganDetailPackageDelivery: {
          state: 'acknowledged',
          lanes: { content: 'acknowledged', mediaSlots: 'acknowledged', comments: 'acknowledged' },
        },
      }),
      collectAuthor: async () => ({}),
      BatchNoteController: function BatchNoteController() {},
      BatchCommentController: function BatchCommentController() {},
      injectUI: () => {},
      toggleStopButton: () => {},
      togglePauseResumeButtons: () => {},
      showToast: () => {},
      showCommentLimitDialog: async () => ({}),
      showMediaDownloadDialog: async () => {
        mediaDialogCalls += 1;
        return { mediaTypes: ['images'], count: 1 };
      },
      showBatchSettingsDialog: async () => ({}),
      ensureTaskControlBar: () => {},
      updateTaskControlBar: () => {},
      hideTaskControlBar: () => {},
      isContextValid: () => true,
      reportDone: () => {},
      extractNoteId: () => 'note_1',
      sendToBackground: async () => ({}),
      downloadNoteMediaFromRecord: async (_note, options) => {
        mediaDownloadCalls += 1;
        assert.deepEqual(options, { mediaTypes: ['images'] });
        return { total: 1, success: 1, failed: 0 };
      },
    });
    const button = { dataset: { action: 'collectNoteWithManualMedia', params: '{}' } };
    await controller.handleButtonClick({
      target: { closest: (selector) => (selector === '.lgboom-btn' ? button : null) },
    });

    assert.equal(mediaDialogCalls, 1);
    assert.equal(mediaDownloadCalls, 1);
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  }
});

test('detail receipt formatting keeps a terminal lane visible instead of collapsing it into a package success', () => {
  assert.deepEqual(formatXhsDetailDeliveryMessage({
    state: 'partial_delivery_failure',
    lanes: { content: 'acknowledged', mediaSlots: 'terminal', comments: 'pending' },
  }), {
    allAccepted: false,
    message: '笔记详情已读取。Linggan 接纳回执：笔记详情：已接纳；媒体观察：未接纳；评论与回复：待本机交付',
  });
});

test('current-surface discovery submits only after its bounded page reader has returned cards', async () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  const order = [];
  let releaseCards;
  const cardRead = new Promise((resolve) => {
    releaseCards = resolve;
  });
  globalThis.window = {
    location: { href: 'https://www.xiaohongshu.com/search_result?keyword=ADHD' },
  };
  globalThis.document = {};
  try {
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
      togglePauseResumeButtons: () => {},
      showToast: () => {},
      showCommentLimitDialog: async () => ({}),
      showMediaDownloadDialog: async () => false,
      showBatchSettingsDialog: async () => ({}),
      ensureTaskControlBar: () => {},
      updateTaskControlBar: () => {},
      hideTaskControlBar: () => {},
      isContextValid: () => true,
      reportDone: () => {},
      extractNoteId: () => 'note_1',
      sendToBackground: async () => ({}),
      downloadNoteMediaFromRecord: async () => ({}),
      discoverSurface: async ({ maximumQuota }) => {
        order.push(`read:${maximumQuota}`);
        return cardRead;
      },
      submitDiscovery: async (cards, context) => {
        order.push(`submit:${cards.length}`);
        assert.equal(context.query, 'ADHD');
        return { delivery: 'pending' };
      },
    });
    const button = { dataset: { action: 'discoverSurface', params: JSON.stringify({ mode: 'search', maximumQuota: 20 }) } };
    const pending = controller.handleButtonClick({
      target: { closest: (selector) => (selector === '.lgboom-btn' ? button : null) },
    });
    await Promise.resolve();
    assert.deepEqual(order, ['read:20']);
    releaseCards([{ noteId: 'note_1' }]);
    await pending;
    assert.deepEqual(order, ['read:20', 'submit:1']);
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  }
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
