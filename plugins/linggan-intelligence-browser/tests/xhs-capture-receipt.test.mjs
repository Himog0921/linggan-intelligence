import test from 'node:test';
import assert from 'node:assert/strict';

import {
  XHS_DETAIL_COMMENT_CAP,
  buildXhsDetailCaptureReceipt,
  buildXhsSearchSurfaceReceipt,
  normalizeXhsDetailCommentLimit,
} from '../src/platforms/xhs/captureReceipt.js';

test('detail receipt keeps the workbench note-detail comment cap at 30', () => {
  assert.equal(XHS_DETAIL_COMMENT_CAP, 30);
  assert.equal(normalizeXhsDetailCommentLimit(undefined), 30);
  assert.equal(normalizeXhsDetailCommentLimit(12), 12);
  assert.equal(normalizeXhsDetailCommentLimit(99), 30);
});

test('detail receipt keeps an explicit empty comment page distinct from an unknown public count', () => {
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
      requested: 30,
      actual: 0,
      publicCount: null,
      state: 'explicit_empty_state',
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

  assert.equal(receipt.comments.requested, 30);
  assert.equal(receipt.comments.actual, 12);
  assert.equal(receipt.comments.publicCount, 80);
  assert.equal(receipt.comments.state, 'partial');
  assert.equal(receipt.comments.stopReason, 'collector_stopped_without_target');
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
