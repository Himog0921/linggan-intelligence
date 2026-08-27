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

test('unlimited deep comments use an explicit all-public execution target instead of invalid zero quota', () => {
  assert.deepEqual(commentTaskInstruction('note_1', 0), {
    commentLimit: 'not_requested',
    target: {
      contentExternalId: 'note_1',
      commentScope: 'all_public_until_natural_end',
    },
    stopConditions: ['manual_stop', 'collector_complete', 'time_budget', 'risk_budget'],
  });
  assert.deepEqual(commentTaskInstruction('note_1', 80), {
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
  assert.equal(receipt.comments.requested, 30);
  assert.equal(receipt.comments.state, 'target_reached');
  assert.equal(commentTaskInstruction('note_1', 0).target.commentScope, 'all_public_until_natural_end');
});

test('an explicit empty comment page counts as a completed batch target and remains resumable', () => {
  const noteList = [{ noteId: 'note_1' }, { noteId: 'note_2' }];
  const results = [
    { noteId: 'note_1', total: 0, explicitEmptyState: true },
    { noteId: 'note_2', total: 9 },
  ];
  const summary = buildXhsBatchCommentsRunPatch({ noteList, results });
  assert.equal(summary.itemsSucceeded, 2);
  assert.equal(summary.itemsFailed, 0);
  const progress = buildXhsBatchCommentsProgressPatch({ noteList, results, processedCount: 1 });
  assert.equal(progress.resumeCheckpoint.resultStatuses[0].ok, true);
  assert.equal(progress.resumeCheckpoint.resultStatuses[0].totalComments, 0);
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
