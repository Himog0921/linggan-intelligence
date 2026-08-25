import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeAuthorRecord,
  normalizeCommentRecord,
  normalizeNoteRecord,
} from '../src/db/recordNormalization.js';
import { inferDouyinVideoQualityMeta } from '../src/platforms/douyin/videoCollector.js';

test('record normalization preserves explicit quality fields', () => {
  const note = normalizeNoteRecord({
    platform: 'douyin',
    noteId: 'dy_1',
    contentId: 'dy_1',
    dataQuality: 'seed',
    qualityReason: 'search_summary_seed',
    sourceTier: 'seed',
    rawSource: 'search_dom_result',
  });
  const comment = normalizeCommentRecord({
    platform: 'xhs',
    noteId: 'note_1',
    commentId: 'comment_1',
    dataQuality: 'degraded',
    qualityReason: 'synthetic_comment_id',
    sourceTier: 'dom',
    rawSource: 'xhs.comments.dom',
  });
  const author = normalizeAuthorRecord({
    platform: 'douyin',
    userId: 'author_1',
    dataQuality: 'degraded',
    qualityReason: 'handle_route_fallback',
    sourceTier: 'mixed',
    rawSource: 'douyin.user-detail+profile-api',
  });

  assert.equal(note.dataQuality, 'seed');
  assert.equal(note.qualityReason, 'search_summary_seed');
  assert.equal(note.sourceTier, 'seed');

  assert.equal(comment.dataQuality, 'degraded');
  assert.equal(comment.qualityReason, 'synthetic_comment_id');
  assert.equal(comment.sourceTier, 'dom');

  assert.equal(author.dataQuality, 'degraded');
  assert.equal(author.qualityReason, 'handle_route_fallback');
  assert.equal(author.sourceTier, 'mixed');
});

test('record normalization infers source tier from raw source when explicit tier is missing', () => {
  assert.equal(normalizeNoteRecord({ rawSource: 'detail_api' }).sourceTier, 'api');
  assert.equal(normalizeNoteRecord({ rawSource: 'render_data' }).sourceTier, 'mixed');
  assert.equal(normalizeNoteRecord({ rawSource: 'xhs.comments.dom' }).sourceTier, 'dom');
  assert.equal(normalizeNoteRecord({ rawSource: 'search_dom_result' }).sourceTier, 'seed');
});

test('record normalization derives a stable note id from platform content ids', () => {
  assert.equal(
    normalizeNoteRecord({
      platform: 'xhs',
      platformContentId: 'xhs_note_1',
    }).noteId,
    'xhs_note_1',
  );

  assert.equal(
    normalizeNoteRecord({
      platform: 'douyin',
      platformContentId: '9001',
    }).noteId,
    'dy_9001',
  );
});

test('inferDouyinVideoQualityMeta classifies seed and dom fallback sources', () => {
  assert.deepEqual(
    inferDouyinVideoQualityMeta('search_dom_result'),
    {
      dataQuality: 'seed',
      qualityReason: 'search_summary_seed',
      sourceTier: 'seed',
    },
  );

  assert.deepEqual(
    inferDouyinVideoQualityMeta('detail_api_fallback'),
    {
      dataQuality: 'seed',
      qualityReason: 'aweme_seed_without_detail',
      sourceTier: 'seed',
    },
  );

  assert.deepEqual(
    inferDouyinVideoQualityMeta('dom'),
    {
      dataQuality: 'degraded',
      qualityReason: 'detail_context_dom_fallback',
      sourceTier: 'dom',
    },
  );
});
