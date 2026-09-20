import test from 'node:test';
import assert from 'node:assert/strict';

import {
  XHS_DETAIL_COMMENT_CAP,
  XHS_COMMENT_ANALYSIS_USABILITY,
  XHS_COMMENT_COLLECTION_SCOPE,
  XHS_COMMENT_COLLECTION_STATE,
  buildXhsCommentCollectionReceipt,
  buildXhsDetailCaptureReceipt,
  buildXhsSearchSurfaceReceipt,
  normalizeXhsDetailCommentLimit,
} from '../src/platforms/xhs/captureReceipt.js';

test('detail receipt keeps the standard note-detail comment cap at 30', () => {
  assert.equal(XHS_DETAIL_COMMENT_CAP, 30);
  assert.equal(normalizeXhsDetailCommentLimit(undefined), 30);
  assert.equal(normalizeXhsDetailCommentLimit(12), 12);
  assert.equal(normalizeXhsDetailCommentLimit(99), 30);
});

test('detail receipt treats an explicit 0 / 0 comment page as a complete empty window', () => {
  const receipt = buildXhsDetailCaptureReceipt({
    note: { noteId: 'note_1', images: ['image_1'], dataSource: 'xhs.detail_dom' },
    commentResult: { total: 0, comments: [], explicitEmptyState: true, stopReason: 'explicit_empty_state' },
  });

  assert.deepEqual(receipt, {
    kind: 'xhs_note_detail_package',
    noteId: 'note_1',
    detailSource: 'xhs.detail_dom',
    media: { observedSlots: 1, acquisition: 'not_requested' },
    comments: {
      cap: 30,
      version: 1,
      noteId: 'note_1',
      scope: 'detail_window',
      requestedLimit: 30,
      pageCommentCount: 0,
      expectedCount: 0,
      uniqueCollectedCount: 0,
      state: 'complete',
      analysisUsability: 'empty',
      targetIdentity: 'matched',
      stopReason: 'explicit_empty_state',
      ordering: 'unknown',
      repliesPreserved: true,
    },
  });
});

test('detail receipt reports a short nonempty collection without fabricating completion', () => {
  const receipt = buildXhsDetailCaptureReceipt({
    note: { noteId: 'note_2', publicCommentCount: 80, publicCommentCountKnown: true },
    commentResult: {
      total: 12,
      comments: Array.from({ length: 12 }, (_, index) => ({ commentId: `c_${index + 1}` })),
      stopReason: 'collector_stopped_without_target',
    },
  });

  assert.equal(receipt.comments.requestedLimit, 30);
  assert.equal(receipt.comments.uniqueCollectedCount, 12);
  assert.equal(receipt.comments.pageCommentCount, 80);
  assert.equal(receipt.comments.expectedCount, 30);
  assert.equal(receipt.comments.state, 'partial');
  assert.equal(receipt.comments.analysisUsability, 'usable');
  assert.equal(receipt.comments.stopReason, 'collector_stopped_without_target');
});

test('a full deep collection is complete when this Attempt reaches or exceeds the observed page count', () => {
  const complete = buildXhsCommentCollectionReceipt({
    noteId: 'note_300', maxTotal: 0, publicCommentCount: 300, actual: 300,
    stopReason: 'comment_area_end',
  });
  assert.equal(complete.scope, XHS_COMMENT_COLLECTION_SCOPE.ALL_PUBLIC_COMMENTS);
  assert.equal(complete.state, XHS_COMMENT_COLLECTION_STATE.COMPLETE);
  assert.equal(complete.analysisUsability, XHS_COMMENT_ANALYSIS_USABILITY.USABLE);

  const overObservedCount = buildXhsCommentCollectionReceipt({
    noteId: '6a93c543000000002600287a', maxTotal: 0, publicCommentCount: 594, actual: 596,
    stopReason: 'no_progress',
  });
  assert.equal(overObservedCount.state, XHS_COMMENT_COLLECTION_STATE.COMPLETE);
  assert.equal(overObservedCount.expectedCount, 594);
  assert.equal(overObservedCount.uniqueCollectedCount, 596);

  const shortNaturalEnd = buildXhsCommentCollectionReceipt({
    noteId: 'note_300', maxTotal: 0, publicCommentCount: 300, actual: 299,
    stopReason: 'no_progress',
  });
  assert.equal(shortNaturalEnd.state, XHS_COMMENT_COLLECTION_STATE.PARTIAL);

  const partial = buildXhsCommentCollectionReceipt({
    noteId: 'note_300', maxTotal: 0, publicCommentCount: 300, actual: 200,
    stopReason: 'risk_control',
  });
  assert.equal(partial.state, XHS_COMMENT_COLLECTION_STATE.PARTIAL);
  assert.equal(partial.analysisUsability, XHS_COMMENT_ANALYSIS_USABILITY.USABLE);
  assert.equal(partial.expectedCount, 300);
  assert.equal(partial.uniqueCollectedCount, 200);
});

test('a target mismatch is never complete or analytically usable', () => {
  const receipt = buildXhsCommentCollectionReceipt({
    noteId: 'expected', maxTotal: 0, publicCommentCount: 3, actual: 3,
    targetIdentity: 'mismatched', stopReason: 'comment_area_end',
  });
  assert.equal(receipt.state, XHS_COMMENT_COLLECTION_STATE.INVALID_TARGET);
  assert.equal(receipt.analysisUsability, XHS_COMMENT_ANALYSIS_USABILITY.NOT_USABLE);
});

test('an unverified target remains partial and cannot enter analysis as the requested note', () => {
  const receipt = buildXhsCommentCollectionReceipt({
    noteId: 'expected', maxTotal: 0, publicCommentCount: 3, actual: 3,
    targetIdentity: 'unverified', stopReason: 'comment_area_end',
  });
  assert.equal(receipt.state, XHS_COMMENT_COLLECTION_STATE.PARTIAL);
  assert.equal(receipt.analysisUsability, XHS_COMMENT_ANALYSIS_USABILITY.NOT_USABLE);
});

test('search surface receipt records a task limit separately from the loaded page facts', () => {
  const receipt = buildXhsSearchSurfaceReceipt({
    requestedLimit: 50,
    loadedCount: 27,
    stopReason: 'current_surface_read_once',
    activeFilters: { sortBasis: 'latest', noteType: 'all', publishTime: 'one_week' },
    suggestionCount: 4,
  });

  assert.deepEqual(receipt, {
    kind: 'xhs_search_surface',
    requestedLimit: 50,
    loadedCount: 27,
    stopReason: 'current_surface_read_once',
    activeFilters: { sortBasis: 'latest', noteType: 'all', publishTime: 'one_week' },
    suggestionCount: 4,
    resultSetComplete: false,
  });
});
