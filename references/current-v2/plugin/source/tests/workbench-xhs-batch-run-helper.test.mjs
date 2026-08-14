import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildRemoteRunCreatePayload,
  buildXhsAttachedCommentResult,
  buildXhsBatchNotesProgressPatch,
  buildXhsBatchNotesRunPatch,
  buildXhsBatchCommentsProgressPatch,
  buildXhsBatchCommentsRunPatch,
  publicCommentCountFromXhsNote,
} from '../src/workbench/runtime/xhsBatchRunHelper.js';
import { BatchNoteController } from '../src/platforms/xhs/batchController.js';
import { COLLECT_MODE } from '../src/shared/constants.js';

test('buildRemoteRunCreatePayload returns null without external task id', () => {
  const payload = buildRemoteRunCreatePayload({
    platform: 'xhs',
    taskType: 'batchNotes',
    pageType: 'search',
    triggerSource: 'popup_manual',
    externalTaskMeta: {},
  });

  assert.equal(payload, null);
});

test('buildRemoteRunCreatePayload builds createRun payload for remote xhs batch task', () => {
  const payload = buildRemoteRunCreatePayload({
    platform: 'xhs',
    taskType: 'batchComments',
    pageType: 'profile',
    triggerSource: 'workbench_dispatch',
    pageUrl: 'https://www.xiaohongshu.com/user/profile/abc',
    config: { count: 12, topByLikes: true },
    externalTaskMeta: {
      externalTaskId: 'wb_task_xhs_1',
      externalTaskType: 'xhs.batchComments',
      protocolVersion: 'v1',
      executorInstanceId: 'executor_1',
    },
  });

  assert.deepEqual(payload, {
    externalTaskId: 'wb_task_xhs_1',
    externalTaskType: 'xhs.batchComments',
    executorInstanceId: 'executor_1',
    protocolVersion: 'v1',
    platform: 'xhs',
    taskType: 'batchComments',
    pageType: 'profile',
    triggerSource: 'workbench_dispatch',
    resultUploadStatus: 'pending_upload',
    lastHeartbeatAt: payload.lastHeartbeatAt,
    config: { count: 12, topByLikes: true },
    meta: { pageUrl: 'https://www.xiaohongshu.com/user/profile/abc' },
  });
  assert.equal(typeof payload.lastHeartbeatAt, 'number');
  assert.ok(payload.lastHeartbeatAt > 0);
});

test('buildXhsBatchNotesRunPatch summarizes batch note results', () => {
  const patch = buildXhsBatchNotesRunPatch({
    noteList: [{ noteId: 'n1' }, { noteId: 'n2' }, { noteId: 'n3' }],
    collected: [{ noteId: 'n1' }, { noteId: 'n3' }],
    failed: [{ noteId: 'n2', error: 'timeout' }],
  });

  assert.deepEqual(patch, {
    itemsPlanned: 3,
    itemsSucceeded: 2,
    itemsFailed: 1,
    targetIds: ['n1', 'n2', 'n3'],
    contentIds: ['xhs_n1', 'xhs_n3'],
    failedTargets: [{ noteId: 'n2', error: 'timeout' }],
  });
});

test('buildXhsBatchNotesRunPatch keeps valid content id when note id is absent', () => {
  const patch = buildXhsBatchNotesRunPatch({
    noteList: [{ noteId: 'n1' }],
    collected: [{ contentId: 'xhs_n1' }],
  });

  assert.deepEqual(patch.contentIds, ['xhs_n1']);
});

test('buildXhsBatchNotesRunPatch keeps attached comments separate from note success', () => {
  const patch = buildXhsBatchNotesRunPatch({
    noteList: [{ noteId: 'n1' }, { noteId: 'n2' }],
    collected: [{ noteId: 'n1' }, { noteId: 'n2' }],
    commentResults: [
      { noteId: 'n1', total: 20 },
      { noteId: 'n2', total: 0, error: 'comments_not_ready' },
    ],
  });

  assert.equal(patch.itemsSucceeded, 2);
  assert.equal(patch.itemsFailed, 0);
  assert.equal(patch.totalComments, 20);
  assert.deepEqual(patch.attachedCommentResults, [
    { noteId: 'n1', total: 20, error: '' },
    { noteId: 'n2', total: 0, error: 'comments_not_ready' },
  ]);
});

test('buildXhsBatchNotesRunPatch marks requested attached comments as failed when none are returned', () => {
  const patch = buildXhsBatchNotesRunPatch({
    noteList: [{ noteId: 'n1' }],
    collected: [{ noteId: 'n1' }],
    commentResults: [
      { noteId: 'n1', total: 0 },
    ],
  });

  assert.equal(patch.itemsSucceeded, 1);
  assert.equal(patch.itemsFailed, 0);
  assert.equal(patch.totalComments, 0);
  assert.deepEqual(patch.attachedCommentResults, [
    { noteId: 'n1', total: 0, error: 'comments_empty_after_request' },
  ]);
});

test('buildXhsBatchNotesRunPatch treats known public zero comments as complete', () => {
  const patch = buildXhsBatchNotesRunPatch({
    noteList: [{ noteId: 'n1' }],
    collected: [{ noteId: 'n1', comments: 0 }],
    commentResults: [
      { noteId: 'n1', total: 0, publicCommentCount: 0 },
    ],
  });

  assert.equal(patch.itemsSucceeded, 1);
  assert.equal(patch.itemsFailed, 0);
  assert.equal(patch.totalComments, 0);
  assert.deepEqual(patch.attachedCommentResults, [
    { noteId: 'n1', total: 0, publicCommentCount: 0, error: '' },
  ]);
});

test('BatchNoteController records known public zero comments without collecting comments again', async () => {
  const controller = new BatchNoteController();
  controller.isRunning = true;
  controller._includeComments = true;
  controller._commentLimit = 20;

  const result = await controller._collectAttachedComments(
    { noteId: 'n1' },
    'https://www.xiaohongshu.com/explore/n1',
    { noteId: 'n1', comments: 0, publicCommentCount: 0, publicCommentCountKnown: true },
  );

  assert.deepEqual(result, { total: 0, comments: [], publicCommentCount: 0 });
  assert.deepEqual(controller.commentResults, [
    { noteId: 'n1', total: 0, publicCommentCount: 0, error: '' },
  ]);
});

test('BatchNoteController finalizes surface runs before page cleanup', async () => {
  const calls = [];

  class TestBatchNoteController extends BatchNoteController {
    async _finalizeCollectionRun(status, patch) {
      calls.push({ type: 'finalize', status, patch });
      return { status, ...patch };
    }

    async _cleanupAfterLoop() {
      calls.push({ type: 'cleanup' });
    }
  }

  const controller = new TestBatchNoteController();
  controller.collectionRunId = 'run_surface_done';
  controller.noteList = [];

  await controller._completeSurfaceScan({ mode: COLLECT_MODE.PROFILE });

  assert.deepEqual(calls.map((call) => call.type), ['finalize', 'cleanup']);
  assert.equal(calls[0].status, 'done');
});

test('BatchNoteController finalizes completed batch runs before page cleanup', async () => {
  const calls = [];

  class TestBatchNoteController extends BatchNoteController {
    async _finalizeCollectionRun(status, patch) {
      calls.push({ type: 'finalize', status, patch });
      return { status, ...patch };
    }

    async _cleanupAfterLoop() {
      calls.push({ type: 'cleanup' });
    }
  }

  const controller = new TestBatchNoteController();
  controller.collectionRunId = 'run_batch_done';
  controller.noteList = [];
  controller.isRunning = true;

  await controller._captureLoop();

  assert.deepEqual(calls.map((call) => call.type), ['finalize', 'cleanup']);
  assert.equal(calls[0].status, 'done');
});

test('BatchNoteController finalizes detail runs before page cleanup', async () => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const calls = [];

  globalThis.window = {
    __INITIAL_STATE__: { note: { noteDetailMap: { n1: {} } } },
  };
  globalThis.document = {
    body: { innerText: '' },
    querySelector: () => null,
    querySelectorAll: () => [],
  };

  class TestBatchNoteController extends BatchNoteController {
    _resolveCurrentDetailNoteInfo() {
      return {
        noteId: 'n1',
        url: 'https://www.xiaohongshu.com/explore/n1',
        canonicalUrl: 'https://www.xiaohongshu.com/explore/n1',
        rawUrl: 'https://www.xiaohongshu.com/explore/n1',
      };
    }

    async _pauseForRiskControl() {
      return false;
    }

    async _waitForNoteLoad() {
      return true;
    }

    async _settleAfterDetailReady() {}

    async _waitForNoteDataStable() {
      return true;
    }

    async _collectCurrentDetailNote(noteInfo) {
      const note = { noteId: noteInfo.noteId, contentId: 'xhs_n1', url: noteInfo.url };
      this.collected.push(note);
      return note;
    }

    async _collectAttachedComments(noteInfo) {
      this.commentResults.push({ noteId: noteInfo.noteId, total: 10 });
      this._totalCommentsCollected = 10;
      return { total: 10, comments: [] };
    }

    async _syncRunProgress() {
      calls.push({ type: 'progress' });
    }

    async _finalizeCollectionRun(status, patch) {
      calls.push({ type: 'finalize', status, patch });
      return { status, ...patch };
    }

    async _cleanupAfterLoop() {
      calls.push({ type: 'cleanup' });
    }

    _emitProgress() {}
  }

  try {
    const controller = new TestBatchNoteController();
    controller.collectionRunId = 'run_detail_done';
    controller.isRunning = true;

    await controller._captureCurrentDetailTask();

    assert.deepEqual(calls.map((call) => call.type), ['progress', 'finalize', 'cleanup']);
    assert.equal(calls[1].status, 'done');
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});

test('BatchNoteController routes detail mode to current detail collection', async () => {
  assert.equal(COLLECT_MODE.DETAIL, 'detail');

  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  let usedDetailCollector = false;
  const events = [];

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/explore/n1',
      pathname: '/explore/n1',
      origin: 'https://www.xiaohongshu.com',
    },
  };
  globalThis.document = {
    body: { innerText: '' },
    querySelector: () => null,
    querySelectorAll: () => [],
  };

  class DetailBatchNoteController extends BatchNoteController {
    async _captureCurrentDetailTask() {
      usedDetailCollector = true;
      this.noteList = [{ noteId: 'n1', url: globalThis.window.location.href }];
      this.collected = [{ noteId: 'n1', contentId: 'xhs_n1' }];
      this.captchaWatcher?.disconnect?.();
      this.isRunning = false;
    }
  }

  try {
    const controller = new DetailBatchNoteController();
    await controller.start('detail', (progress) => {
      events.push(progress);
    }, {
      targetNoteId: 'n1',
      includeComments: true,
      commentLimit: 20,
    });

    assert.equal(usedDetailCollector, true);
    assert.deepEqual(controller.noteList, [
      { noteId: 'n1', url: 'https://www.xiaohongshu.com/explore/n1' },
    ]);
    assert.equal(events.some((event) => event.status === 'discovering'), false);
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});

test('BatchNoteController treats xhs profile relay urls with note id as loaded detail pages', async () => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/author_1/n1?xsec_token=abc',
      pathname: '/user/profile/author_1/n1',
      origin: 'https://www.xiaohongshu.com',
    },
  };
  globalThis.document = {
    body: { innerText: '' },
    querySelector: () => null,
  };

  try {
    const controller = new BatchNoteController();
    const loaded = await controller._waitForNoteLoad('n1', 5);
    assert.equal(loaded, true);
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});

test('publicCommentCountFromXhsNote normalizes detail comment counts', () => {
  assert.equal(publicCommentCountFromXhsNote({ comments: '1,234', publicCommentCountKnown: true }), 1234);
  assert.equal(publicCommentCountFromXhsNote({ commentCount: '1,234', publicCommentCountKnown: true }), 1234);
  assert.equal(publicCommentCountFromXhsNote({ publicCommentCount: 0 }), 0);
  assert.equal(publicCommentCountFromXhsNote({ commentCount: 5 }), null);
  assert.equal(publicCommentCountFromXhsNote({ comments: 0 }), null);
  assert.equal(publicCommentCountFromXhsNote({}), null);
});

test('buildXhsAttachedCommentResult only marks zero returned comments as failed when public count is not known zero', () => {
  assert.deepEqual(buildXhsAttachedCommentResult({
    noteId: 'n1',
    total: 0,
    publicCommentCount: 0,
  }), {
    noteId: 'n1',
    total: 0,
    publicCommentCount: 0,
    error: '',
  });
  assert.deepEqual(buildXhsAttachedCommentResult({
    noteId: 'n2',
    total: 0,
  }), {
    noteId: 'n2',
    total: 0,
    error: 'comments_empty_after_request',
  });
});

test('buildXhsAttachedCommentResult marks returned comments as failed when fewer than the expected public count are collected', () => {
  assert.deepEqual(buildXhsAttachedCommentResult({
    noteId: 'n3',
    total: 3,
    publicCommentCount: 8,
    requestedCommentLimit: 20,
  }), {
    noteId: 'n3',
    total: 3,
    publicCommentCount: 8,
    expectedCommentCount: 8,
    error: 'comments_under_expected',
  });
});

test('buildXhsAttachedCommentResult uses thirty comments as the default attached comment buffer', () => {
  assert.deepEqual(buildXhsAttachedCommentResult({
    noteId: 'n4',
    total: 19,
    publicCommentCount: 55,
  }), {
    noteId: 'n4',
    total: 19,
    publicCommentCount: 55,
    expectedCommentCount: 30,
    error: 'comments_under_expected',
  });
});

test('buildXhsBatchNotesProgressPatch only counts processed targets during a running task', () => {
  const patch = buildXhsBatchNotesProgressPatch({
    noteList: [{ noteId: 'n1' }, { noteId: 'n2' }, { noteId: 'n3' }],
    processedCount: 2,
    collected: [{ noteId: 'n1', contentId: 'xhs_n1' }],
    failed: [{ noteId: 'n2', error: 'timeout' }],
  });

  assert.deepEqual({
    itemsPlanned: patch.itemsPlanned,
    itemsSucceeded: patch.itemsSucceeded,
    itemsFailed: patch.itemsFailed,
    targetIds: patch.targetIds,
    contentIds: patch.contentIds,
    failedTargets: patch.failedTargets,
  }, {
    itemsPlanned: 3,
    itemsSucceeded: 1,
    itemsFailed: 1,
    targetIds: ['n1', 'n2', 'n3'],
    contentIds: ['xhs_n1'],
    failedTargets: [{ noteId: 'n2', error: 'timeout' }],
  });
  assert.equal(patch.nextIndex, 2);
  assert.deepEqual(patch.resumeCheckpoint.targetIds, ['n1', 'n2', 'n3']);
});

test('buildXhsBatchCommentsRunPatch summarizes batch comment results', () => {
  const patch = buildXhsBatchCommentsRunPatch({
    noteList: [{ noteId: 'n1' }, { noteId: 'n2' }],
    results: [
      { noteId: 'n1', total: 5 },
      { noteId: 'n2', total: 0 },
    ],
  });

  assert.deepEqual(patch, {
    itemsPlanned: 2,
    itemsSucceeded: 1,
    itemsFailed: 1,
    totalComments: 5,
    targetIds: ['n1', 'n2'],
    contentIds: ['xhs_n1'],
    failedTargets: [{ noteId: 'n2', total: 0 }],
  });
});

test('buildXhsBatchCommentsProgressPatch only counts processed targets during a running task', () => {
  const patch = buildXhsBatchCommentsProgressPatch({
    noteList: [{ noteId: 'n1' }, { noteId: 'n2' }, { noteId: 'n3' }],
    processedCount: 2,
    results: [
      { noteId: 'n1', total: 5 },
      { noteId: 'n2', total: 0 },
    ],
  });

  assert.deepEqual({
    itemsPlanned: patch.itemsPlanned,
    itemsSucceeded: patch.itemsSucceeded,
    itemsFailed: patch.itemsFailed,
    totalComments: patch.totalComments,
    targetIds: patch.targetIds,
    contentIds: patch.contentIds,
    failedTargets: patch.failedTargets,
  }, {
    itemsPlanned: 3,
    itemsSucceeded: 1,
    itemsFailed: 1,
    totalComments: 5,
    targetIds: ['n1', 'n2', 'n3'],
    contentIds: ['xhs_n1'],
    failedTargets: [{ noteId: 'n2', total: 0 }],
  });
  assert.equal(patch.nextIndex, 2);
  assert.deepEqual(patch.resumeCheckpoint.targetIds, ['n1', 'n2', 'n3']);
});
