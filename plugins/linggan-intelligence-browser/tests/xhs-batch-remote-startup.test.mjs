import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createBatchMessageHandlers } from '../src/content/douyinBatchMessageHandlers.js';

function createHandlers({
  BatchNoteController,
  reportProgress = () => {},
  syncTaskUI = () => {},
} = {}) {
  let batchNoteController = null;

  const handlers = createBatchMessageHandlers({
    isDouyinPage: () => false,
    createManagedTaskController: () => ({ start() {}, stop() {} }),
    batchCollectDouyinProfileVideos: async () => ({ ok: true, success: 0, total: 0 }),
    batchCollectDouyinProfileComments: async () => ({
      ok: true,
      successVideos: 0,
      totalVideos: 0,
      totalComments: 0,
    }),
    BatchNoteController,
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
    getBatchCommentCtrl: () => null,
    setBatchCommentCtrl: () => {},
    getDouyinAdapter: () => null,
  });

  return {
    handlers,
    getBatchNoteController: () => batchNoteController,
  };
}

test('xhs remote batch start waits for collectionRunId before acknowledging success', async () => {
  let releaseStartup = null;

  class RemoteBatchNoteController {
    constructor() {
      this.collectionRunId = '';
    }

    async start() {
      await new Promise((resolve) => {
        releaseStartup = resolve;
      });
      this.collectionRunId = 'run_xhs_remote_1';
      await new Promise(() => {});
    }
  }

  const { handlers } = createHandlers({
    BatchNoteController: RemoteBatchNoteController,
  });

  let settled = false;
  const resultPromise = handlers[MSG.START_BATCH_NOTES]({
    count: 5,
    mode: 'profile',
    externalTaskMeta: {
      externalTaskId: 'wb_xhs_batch_1',
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
  assert.equal(result.collectionRunId, 'run_xhs_remote_1');
});

test('xhs batch start forwards search filters to the page controller', async () => {
  let received = null;

  class SearchFilterBatchNoteController {
    constructor() {
      this.collectionRunId = 'run_xhs_filter_1';
    }

    async start(mode, onProgress, settings) {
      received = { mode, settings };
    }
  }

  const { handlers } = createHandlers({
    BatchNoteController: SearchFilterBatchNoteController,
  });

  await handlers[MSG.START_BATCH_NOTES]({
    count: 5,
    mode: 'search',
    searchFilters: {
      sortBasis: 'most_commented',
      noteType: 'image',
      publishTime: 'one_week',
    },
  });

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(received.mode, 'search');
  assert.deepEqual(received.settings.searchFilters, {
    sortBasis: 'most_commented',
    noteType: 'image',
    publishTime: 'one_week',
  });
});

test('xhs remote detail batch forwards attached comment settings to the page controller', async () => {
  let received = null;

  class DetailBatchNoteController {
    constructor() {
      this.collectionRunId = 'run_xhs_detail_comments_1';
    }

    async start(mode, onProgress, settings) {
      received = { mode, settings };
    }
  }

  const { handlers } = createHandlers({
    BatchNoteController: DetailBatchNoteController,
  });

  const result = await handlers[MSG.START_BATCH_NOTES]({
    mode: 'detail',
    targetNoteId: 'xhs_69baad5e00000000230055ef',
    includeComments: true,
    commentLimit: 20,
    commentDepthMode: 'twoLevel',
    triggerSource: 'workbench_dispatch',
    externalTaskMeta: {
      externalTaskId: 'wb_xhs_detail_comments_1',
      externalTaskType: 'xhs.batchNotes',
      monitorMeta: {
        monitorMode: 'detail_probe',
      },
    },
  });

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(result.collectionRunId, 'run_xhs_detail_comments_1');
  assert.equal(received.mode, 'detail');
  assert.equal(received.settings.targetNoteId, '69baad5e00000000230055ef');
  assert.equal(received.settings.includeComments, true);
  assert.equal(received.settings.commentLimit, 20);
  assert.equal(received.settings.commentDepthMode, 'twoLevel');
  assert.equal(received.settings.triggerSource, 'workbench_dispatch');
  assert.deepEqual(received.settings.monitorMeta, {
    monitorMode: 'detail_probe',
  });
  assert.deepEqual(received.settings.externalTaskMeta, {
    externalTaskId: 'wb_xhs_detail_comments_1',
    externalTaskType: 'xhs.batchNotes',
    monitorMeta: {
      monitorMode: 'detail_probe',
    },
  });
});

test('xhs remote search batch allows filter settling before startup timeout', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout', 'setInterval'], now: 0 });

  class SlowFilteredStartupController {
    constructor() {
      this.collectionRunId = '';
    }

    async start() {
      await new Promise((resolve) => setTimeout(resolve, 8500));
      this.collectionRunId = 'run_xhs_filtered_remote_1';
      await new Promise(() => {});
    }
  }

  const { handlers } = createHandlers({
    BatchNoteController: SlowFilteredStartupController,
  });

  const resultPromise = handlers[MSG.START_BATCH_NOTES]({
    count: 5,
    mode: 'search',
    searchFilters: {
      sortBasis: 'most_commented',
      noteType: 'image',
      publishTime: 'one_week',
    },
    externalTaskMeta: {
      externalTaskId: 'wb_xhs_filter_slow_1',
    },
  });

  await Promise.resolve();
  t.mock.timers.tick(8500);
  await Promise.resolve();
  t.mock.timers.tick(50);

  const result = await resultPromise;

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_xhs_filtered_remote_1');
});

test('xhs remote batch start throws startup errors instead of faking dispatch success', async () => {
  const progressCalls = [];
  const syncCalls = [];

  class FailingBatchNoteController {
    constructor() {
      this.collectionRunId = '';
    }

    async start() {
      throw new Error('IndexedDB API missing. Please visit https://tinyurl.com/y2uuvskb');
    }
  }

  const { handlers, getBatchNoteController } = createHandlers({
    BatchNoteController: FailingBatchNoteController,
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
        count: 3,
        mode: 'profile',
        externalTaskMeta: {
          externalTaskId: 'wb_xhs_batch_fail_1',
        },
      }),
    /IndexedDB API missing/,
  );

  assert.equal(progressCalls.length, 1);
  assert.equal(progressCalls[0][2], 'IndexedDB API missing. Please visit https://tinyurl.com/y2uuvskb');
  assert.equal(syncCalls.length, 1);
  assert.equal(syncCalls[0].taskState, 'error');
  assert.equal(syncCalls[0].message, '批量采集失败：IndexedDB API missing. Please visit https://tinyurl.com/y2uuvskb');
  assert.equal(getBatchNoteController(), null);
});
