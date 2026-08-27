import test from 'node:test';
import assert from 'node:assert/strict';

import {
  sortDouyinVisualSearchEntries,
  mergeDouyinSearchTargetsByVisibleOrder,
} from '../src/platforms/douyin/batchDiscovery.js';
import { collectDouyinBatchVideoTarget } from '../src/platforms/douyin/batchController.js';

test('sortDouyinVisualSearchEntries keeps visual top-to-bottom then left-to-right order for multi-column results', () => {
  const sorted = sortDouyinVisualSearchEntries([
    { awemeId: 'right-top', top: 120, left: 260, domIndex: 0 },
    { awemeId: 'left-top', top: 118, left: 24, domIndex: 1 },
    { awemeId: 'left-second-row', top: 360, left: 20, domIndex: 2 },
    { awemeId: 'right-second-row', top: 362, left: 250, domIndex: 3 },
  ]);

  assert.deepEqual(
    sorted.map((item) => item.awemeId),
    ['left-top', 'right-top', 'left-second-row', 'right-second-row'],
  );
});

test('mergeDouyinSearchTargetsByVisibleOrder preserves DOM order and hydrates aweme payload from api targets', () => {
  const merged = mergeDouyinSearchTargetsByVisibleOrder(
    [
      {
        key: '2002',
        awemeId: '2002',
        href: 'https://www.douyin.com/video/2002',
        titleHint: 'dom-second',
        orderIndex: 0,
        sourceUrl: 'dom.search_result',
      },
      {
        key: '1001',
        awemeId: '1001',
        href: 'https://www.douyin.com/video/1001',
        titleHint: 'dom-first',
        orderIndex: 1,
        sourceUrl: 'dom.search_result',
      },
    ],
    [
      {
        key: '1001',
        awemeId: '1001',
        aweme: { aweme_id: '1001', desc: '#first', video: { play_addr: { url_list: ['https://example.com/1.mp4'] } } },
        likes: 11,
        sourceUrl: 'captured.search_stream',
      },
      {
        key: '2002',
        awemeId: '2002',
        aweme: { aweme_id: '2002', desc: '#second', video: { play_addr: { url_list: ['https://example.com/2.mp4'] } } },
        likes: 22,
        sourceUrl: 'captured.search_stream',
      },
      {
        key: '3003',
        awemeId: '3003',
        aweme: { aweme_id: '3003', desc: '#third', video: { play_addr: { url_list: ['https://example.com/3.mp4'] } } },
        likes: 33,
        sourceUrl: 'captured.search_stream',
      },
    ],
  );

  assert.deepEqual(
    merged.map((item) => item.awemeId),
    ['2002', '1001', '3003'],
  );
  assert.equal(merged[0].aweme?.aweme_id, '2002');
  assert.equal(merged[0].likes, 22);
  assert.equal(merged[0].titleHint, 'dom-second');
});

test('collectDouyinBatchVideoTarget prefers aweme seed when search target already carries aweme payload', async () => {
  const calls = [];
  const target = {
    awemeId: '9001',
    aweme: { aweme_id: '9001', desc: '#seed', video: { play_addr: { url_list: ['https://example.com/seed.mp4'] } } },
    href: 'https://www.douyin.com/video/9001',
    titleHint: 'seed',
    authorHint: 'author',
    timeHint: '',
    sourceUrl: 'captured.search_stream',
    searchKeyword: '咖啡',
    likes: 88,
  };

  const result = await collectDouyinBatchVideoTarget(target, {
    isSearch: true,
    index: 0,
    topByLikes: false,
    selectionMode: 'search_order',
    collectionRunId: 'run_1',
    searchPageUrl: 'https://www.douyin.com/search/%E5%92%96%E5%95%A1',
  }, {
    collectByAweme: async (aweme, options) => {
      calls.push({ method: 'aweme', awemeId: aweme.aweme_id, options });
      return { ok: true, data: { noteId: 'dy_9001', title: 'seed' } };
    },
    collectById: async (videoId) => {
      calls.push({ method: 'id', videoId });
      return { ok: true, data: { noteId: 'dy_9001', title: 'seed' } };
    },
  });

  assert.equal(result.ok, true);
  assert.deepEqual(calls.map((item) => item.method), ['aweme']);
  assert.equal(calls[0].awemeId, '9001');
  assert.equal(calls[0].options.searchKeyword, '咖啡');
});

test('collectDouyinBatchVideoTarget forwards security-challenge propagation to video collectors', async () => {
  const calls = [];
  const target = {
    awemeId: '9002',
    href: 'https://www.douyin.com/video/9002',
    titleHint: 'seed',
    authorHint: 'author',
    likes: 12,
  };

  const result = await collectDouyinBatchVideoTarget(target, {
    isSearch: false,
    index: 1,
    topByLikes: false,
    selectionMode: 'profile_order',
    collectionRunId: 'run_2',
  }, {
    collectByAweme: async () => {
      throw new Error('collectByAweme should not be called without aweme seed');
    },
    collectById: async (videoId, options) => {
      calls.push({ videoId, options });
      return { ok: true, data: { noteId: 'dy_9002', title: 'seed' } };
    },
  });

  assert.equal(result.ok, true);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].videoId, '9002');
  assert.equal(calls[0].options.propagateSecurityChallenge, true);
});
