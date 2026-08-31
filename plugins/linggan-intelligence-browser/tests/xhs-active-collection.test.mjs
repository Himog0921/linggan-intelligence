import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { commentTaskInstruction } from '../src/linggan/contentRuntimeAdapter.js';
import {
  buildXhsBatchCommentsProgressPatch,
  buildXhsBatchCommentsRunPatch,
} from '../src/linggan/localExecutionSupport.js';
import { buildDiscoveryPlan } from '../src/platforms/xhs/noteCollector.js';
import { buildXhsDetailCaptureReceipt } from '../src/platforms/xhs/captureReceipt.js';
import { createCommentTaskController } from '../src/content/commentTaskController.js';
import { requireBatchTargetCount } from '../src/shared/batchLimits.js';

test('unlimited deep comments use an explicit all-public execution target instead of invalid zero quota', () => {
  assert.deepEqual(commentTaskInstruction('note_1', 0), {
    maximumQuota: null,
    commentLimit: 'not_requested',
    target: {
      contentExternalId: 'note_1',
      commentScope: 'all_public_until_natural_end',
    },
    stopConditions: ['manual_stop', 'collector_complete', 'time_budget', 'risk_budget'],
  });
  assert.deepEqual(commentTaskInstruction('note_1', 80), {
    maximumQuota: 80,
    commentLimit: 80,
    target: {
      contentExternalId: 'note_1',
      commentScope: 'maximum_quota',
      requestedCommentLimit: 80,
    },
    stopConditions: ['manual_stop', 'maximum_quota', 'collector_complete', 'time_budget', 'risk_budget'],
  });
});

test('the standard detail comment window stays capped at 30 without limiting a separately requested deep run', () => {
  const receipt = buildXhsDetailCaptureReceipt({
    note: { noteId: 'note_1', publicCommentCountKnown: true, comments: 120 },
    commentResult: { total: 30, stopReason: 'comment_cap_reached' },
    requestedCommentLimit: 0,
  });
  assert.equal(receipt.comments.cap, 30);
  assert.equal(receipt.comments.scope, 'detail_window');
  assert.equal(receipt.comments.requestedLimit, 30);
  assert.equal(receipt.comments.expectedCount, 30);
  assert.equal(receipt.comments.uniqueCollectedCount, 30);
  assert.equal(receipt.comments.state, 'complete');
  assert.equal(commentTaskInstruction('note_1', 0).target.commentScope, 'all_public_until_natural_end');
});

test('an explicit empty comment page counts as a completed batch target and remains resumable', () => {
  const noteList = [{ noteId: 'note_1' }, { noteId: 'note_2' }];
  const results = [
    { noteId: 'note_1', total: 0, collectionState: 'complete', collectionScope: 'all_public_comments', analysisUsability: 'empty', collectionReceipt: { expectedCount: 0 } },
    { noteId: 'note_2', total: 9, collectionState: 'complete', collectionScope: 'all_public_comments', analysisUsability: 'usable', collectionReceipt: { expectedCount: 9 } },
  ];
  const summary = buildXhsBatchCommentsRunPatch({ noteList, results });
  assert.equal(summary.itemsSucceeded, 2);
  assert.equal(summary.itemsFailed, 0);
  const progress = buildXhsBatchCommentsProgressPatch({ noteList, results, processedCount: 1 });
  assert.equal(progress.resumeCheckpoint.resultStatuses[0].ok, true);
  assert.equal(progress.resumeCheckpoint.resultStatuses[0].totalComments, 0);
  assert.equal(progress.resumeCheckpoint.resultStatuses[0].collectionState, 'complete');
});

test('an invalid-target comment result is failed and never becomes an analysis content id', () => {
  const summary = buildXhsBatchCommentsRunPatch({
    noteList: [{ noteId: 'expected-note' }],
    results: [{
      noteId: 'expected-note',
      total: 12,
      collectionState: 'invalid_target',
      analysisUsability: 'not_usable',
      targetIdentity: 'mismatched',
    }],
  });
  assert.equal(summary.itemsSucceeded, 0);
  assert.equal(summary.itemsPartiallyCollected, 0);
  assert.equal(summary.itemsFailed, 1);
  assert.deepEqual(summary.contentIds, []);
  assert.equal(summary.failedTargets[0].error, 'invalid_target');
});

test('batch comment collection never supplies the requested note id as observed page identity', () => {
  const source = readFileSync(new URL('../src/platforms/xhs/batchCommentController.js', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /_collectOneNoteComments\([^\n]*noteInfo\.noteId/);
  assert.doesNotMatch(source, /observedNoteId:\s*noteInfo\.noteId/);
});

test('single-note comment UI reports 200 / 300 as partial instead of success', async () => {
  const toasts = [];
  const progress = [];
  const controller = createCommentTaskController({
    collectComments: async () => ({
      total: 200,
      collectionState: 'partial',
      stopReason: 'risk_control',
      collectionReceipt: { expectedCount: 300 },
    }),
    showToast: (message, state) => toasts.push({ message, state }),
    syncTaskUI: (value) => progress.push(value),
    startBatchTask() {},
    toggleStopButton() {},
    hideTaskControlBar() {},
    setActiveTaskType() {},
  });

  await controller.start({
    noteId: 'note_partial',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_partial',
  });

  assert.match(progress.at(-1).message, /评论部分采集/);
  assert.match(progress.at(-1).message, /200 \/ 300/);
  assert.equal(toasts.at(-1).state, 'warning');
});

test('single-note all-replies settings reach the comment collector', async () => {
  let received;
  const controller = createCommentTaskController({
    collectComments: async (options) => {
      received = options;
      return {
        total: 0,
        collectionState: 'partial',
        stopReason: 'no_progress',
        collectionReceipt: { expectedCount: 0 },
      };
    },
    showToast() {},
    syncTaskUI() {},
    startBatchTask() {},
    toggleStopButton() {},
    hideTaskControlBar() {},
    setActiveTaskType() {},
  });

  await controller.start({
    noteId: 'note_all_replies',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_all_replies',
    maxTotal: 0,
    maxSubComments: 0,
    commentDepthMode: 'allReplies',
  });

  assert.equal(received.commentDepthMode, 'allReplies');
  assert.equal(received.maxSubComments, 0);
});

test('unlimited comment progress uses the page count and never latches the first collected count as total', async () => {
  const progress = [];
  const controller = createCommentTaskController({
    collectComments: async ({ onProgress }) => {
      onProgress({ current: 53, pageCommentCount: 452, message: '已采集 53 条评论' });
      onProgress({ current: 130, pageCommentCount: 452, message: '已采集 130 条评论' });
      return {
        total: 130,
        collectionState: 'partial',
        stopReason: 'manual_stop',
        collectionReceipt: { expectedCount: 452, pageCommentCount: 452 },
      };
    },
    showToast() {},
    syncTaskUI: (value) => progress.push(value),
    startBatchTask() {},
    toggleStopButton() {},
    hideTaskControlBar() {},
    setActiveTaskType() {},
  });

  await controller.start({
    noteId: 'note_long',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_long',
    maxTotal: 0,
  });

  const at130 = progress.find((value) => value.current === 130 && value.taskState === 'running');
  assert.equal(at130.total, 452);
  assert.equal(progress.some((value) => value.current === 130 && value.total === 53), false);
});

test('a stale page denominator smaller than collected comments falls back to an open-ended progress label', async () => {
  const progress = [];
  const controller = createCommentTaskController({
    collectComments: async ({ onProgress }) => {
      onProgress({ current: 130, pageCommentCount: 53, message: '已采集 130 条评论' });
      return {
        total: 130,
        collectionState: 'partial',
        stopReason: 'manual_stop',
        collectionReceipt: { expectedCount: 53, pageCommentCount: 53 },
      };
    },
    showToast() {}, syncTaskUI: (value) => progress.push(value), startBatchTask() {},
    toggleStopButton() {}, hideTaskControlBar() {}, setActiveTaskType() {},
  });

  await controller.start({
    noteId: 'note_stale_total',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_stale_total',
    maxTotal: 0,
  });
  const at130 = progress.find((value) => value.current === 130 && value.taskState === 'running');
  assert.equal(at130.total, 0);
  assert.doesNotMatch(progress.at(-1).message, /130\s*\/\s*53/);
});

test('pausing a manual deep comment task submits the latest material snapshot as partial data', async () => {
  const checkpoints = [];
  let publishSnapshot;
  let finishCollection;
  const collectionFinished = new Promise((resolve) => { finishCollection = resolve; });
  const controller = createCommentTaskController({
    collectComments: async ({ onSnapshot, waitIfPaused }) => {
      publishSnapshot = onSnapshot;
      await collectionFinished;
      await waitIfPaused();
      return {
        total: 2,
        comments: [{ commentId: 'root_1', level: 1 }, { commentId: 'reply_1', level: 2, rootCommentId: 'root_1' }],
        collectionState: 'partial',
        stopReason: 'manual_stop',
        collectionReceipt: { expectedCount: 10, pageCommentCount: 10 },
      };
    },
    submitCommentCheckpoint: async (result, noteId, settings) => {
      checkpoints.push({ result, noteId, settings });
      return { delivery: 'pending' };
    },
    showToast() {},
    syncTaskUI() {},
    startBatchTask() {},
    toggleStopButton() {},
    hideTaskControlBar() {},
    setActiveTaskType() {},
  });

  const run = controller.start({
    noteId: 'note_checkpoint',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_checkpoint',
    maxTotal: 0,
  });
  await new Promise((resolve) => setImmediate(resolve));
  publishSnapshot({
    total: 2,
    comments: [{ commentId: 'root_1', level: 1 }, { commentId: 'reply_1', level: 2, rootCommentId: 'root_1' }],
    collectionState: 'partial',
    stopReason: 'in_progress',
    collectionReceipt: {
      scope: 'all_public_comments',
      expectedCount: 10,
      pageCommentCount: 10,
      uniqueCollectedCount: 2,
      state: 'partial',
      analysisUsability: 'usable',
      stopReason: 'in_progress',
    },
  });
  controller.pause();
  await new Promise((resolve) => setImmediate(resolve));

  assert.equal(checkpoints.length, 1);
  assert.equal(checkpoints[0].noteId, 'note_checkpoint');
  assert.equal(checkpoints[0].result.total, 2);
  assert.equal(checkpoints[0].result.stopReason, 'manual_pause');
  assert.equal(checkpoints[0].result.collectionState, 'partial');
  assert.equal(checkpoints[0].result.collectionReceipt.stopReason, 'manual_pause');
  assert.equal(checkpoints[0].result.collectionReceipt.uniqueCollectedCount, 2);

  controller.stop();
  finishCollection();
  await run;
});

test('stop keeps the current comment collector exclusive until its final action drains', async () => {
  let releaseFirst;
  let calls = 0;
  const firstFinished = new Promise((resolve) => { releaseFirst = resolve; });
  const controller = createCommentTaskController({
    collectComments: async ({ shouldStop }) => {
      calls += 1;
      if (calls === 1) {
        await firstFinished;
        assert.equal(shouldStop(), true);
      }
      return { total: 0, collectionState: 'partial', stopReason: 'manual_stop', collectionReceipt: { expectedCount: 0 } };
    },
    showToast() {}, syncTaskUI() {}, startBatchTask() {}, toggleStopButton() {},
    hideTaskControlBar() {}, setActiveTaskType() {},
  });
  const first = controller.start({ noteId: 'note-1', noteUrl: 'https://www.xiaohongshu.com/explore/note-1' });
  await new Promise((resolve) => setImmediate(resolve));
  controller.stop();
  assert.deepEqual(
    await controller.start({ noteId: 'note-2', noteUrl: 'https://www.xiaohongshu.com/explore/note-2' }),
    { success: false, error: 'task_already_running' },
  );
  releaseFirst();
  await first;
  await controller.start({ noteId: 'note-2', noteUrl: 'https://www.xiaohongshu.com/explore/note-2' });
  assert.equal(calls, 2);
});

test('batch target counts are explicit and never silently clamped', () => {
  assert.equal(requireBatchTargetCount(1), 1);
  assert.equal(requireBatchTargetCount(37), 37);
  assert.equal(requireBatchTargetCount(50), 50);
  assert.throws(() => requireBatchTargetCount(0), /batch_target_count_must_be_1_to_50/);
  assert.throws(() => requireBatchTargetCount(51), /batch_target_count_must_be_1_to_50/);
  assert.throws(() => requireBatchTargetCount(4.5), /batch_target_count_must_be_1_to_50/);
});

test('search target size expands the active DOM-loading plan beyond its old ten-round snapshot', () => {
  const plan = buildDiscoveryPlan('.feeds-container', { expectedCount: 50 });
  assert.equal(plan.maxRounds, 40);
  assert.equal(plan.expectedCount, 50);
});

test('page discovery exposes target count and current search filters before active loading', () => {
  const controller = readFileSync(new URL('../src/content/xhsPageController.js', import.meta.url), 'utf8');
  assert.match(controller, /title: mode === COLLECT_MODE\.PROFILE \? '博主页发现设置' : '搜索发现设置'/);
  assert.match(controller, /enableSearchFilters: mode === COLLECT_MODE\.SEARCH/);
  assert.match(controller, /applyXhsSearchFilters\(searchFilters/);
});

test('page bridge replies and media candidates are bound to the active request and approved HTTPS platform origins', () => {
  const utils = readFileSync(new URL('../src/shared/utils.js', import.meta.url), 'utf8');
  const noteMap = readFileSync(new URL('../src/injected/noteMap.js', import.meta.url), 'utf8');
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  assert.match(utils, /const requestId = crypto\.randomUUID\(\)/);
  assert.match(utils, /event\.data\?\.requestId !== requestId/);
  assert.match(noteMap, /requestId: requestId/);
  assert.match(background, /uri\.protocol !== 'https:'/);
  assert.match(background, /allowedMediaCandidateUri\(response\.url \|\| candidate\)/);
  assert.match(background, /filter\(allowedMediaCandidateUri\)/);
});
