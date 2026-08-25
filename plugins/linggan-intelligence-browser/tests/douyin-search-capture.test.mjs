import test from 'node:test';
import assert from 'node:assert/strict';

import {
  mergeCapturedDouyinSearchPages,
  normalizeDouyinSearchChannel,
  normalizeDouyinSearchKeyword,
  parseDouyinSearchPagePayload,
  upsertDouyinSearchPage,
} from '../src/platforms/douyin/searchCapture.js';

test('parseDouyinSearchPagePayload keeps keyword, channel and aweme order', () => {
  const page = parseDouyinSearchPagePayload(
    {
      data: [
        { aweme_info: { aweme_id: '1001', desc: 'first', video: {} } },
        { awemeInfo: { aweme_id: '1002', desc: 'second', video: {} } },
      ],
      offset: 10,
      next_offset: 20,
      has_more: 1,
    },
    'https://www.douyin.com/aweme/v1/web/general/search/stream/?keyword=%E5%92%96%E5%95%A1&search_channel=aweme_general&offset=10',
    12345,
  );

  assert.equal(page.keyword, '咖啡');
  assert.equal(page.searchChannel, 'aweme_general');
  assert.equal(page.offset, 10);
  assert.equal(page.nextOffset, 20);
  assert.equal(page.hasMore, true);
  assert.deepEqual(
    page.items.map((item) => item.awemeId),
    ['1001', '1002'],
  );
});

test('upsertDouyinSearchPage replaces same keyword/channel/offset with newer page', () => {
  const first = {
    keyword: '咖啡',
    searchChannel: 'aweme_general',
    offset: 0,
    nextOffset: 10,
    hasMore: true,
    capturedAt: 1,
    items: [{ awemeId: '1001', aweme: { aweme_id: '1001', video: {} } }],
  };
  const second = {
    ...first,
    capturedAt: 2,
    items: [{ awemeId: '1002', aweme: { aweme_id: '1002', video: {} } }],
  };

  const pages = upsertDouyinSearchPage([first], second);
  assert.equal(pages.length, 1);
  assert.equal(pages[0].capturedAt, 2);
  assert.equal(pages[0].items[0].awemeId, '1002');
});

test('mergeCapturedDouyinSearchPages keeps offset order, dedupes ids and filters by keyword/channel', () => {
  const merged = mergeCapturedDouyinSearchPages([
    {
      keyword: '咖啡',
      searchChannel: 'aweme_general',
      offset: 10,
      nextOffset: 20,
      hasMore: false,
      capturedAt: 2,
      sourceUrl: 'page-10',
      items: [
        { awemeId: '1002', aweme: { aweme_id: '1002', desc: 'second', video: {} } },
        { awemeId: '1003', aweme: { aweme_id: '1003', desc: 'third', video: {} } },
      ],
    },
    {
      keyword: '咖啡',
      searchChannel: 'aweme_general',
      offset: 0,
      nextOffset: 10,
      hasMore: true,
      capturedAt: 1,
      sourceUrl: 'page-0',
      items: [
        { awemeId: '1001', aweme: { aweme_id: '1001', desc: 'first', video: {} } },
        { awemeId: '1002', aweme: { aweme_id: '1002', desc: 'duplicate', video: {} } },
      ],
    },
    {
      keyword: '奶茶',
      searchChannel: 'aweme_general',
      offset: 0,
      nextOffset: 10,
      hasMore: true,
      capturedAt: 9,
      sourceUrl: 'wrong-keyword',
      items: [{ awemeId: '9999', aweme: { aweme_id: '9999', video: {} } }],
    },
    {
      keyword: '咖啡',
      searchChannel: 'aweme_video',
      offset: 0,
      nextOffset: 10,
      hasMore: true,
      capturedAt: 9,
      sourceUrl: 'wrong-channel',
      items: [{ awemeId: '8888', aweme: { aweme_id: '8888', video: {} } }],
    },
  ], {
    keyword: '咖啡',
    searchChannel: 'aweme_general',
  });

  assert.deepEqual(
    merged.items.map((item) => item.awemeId),
    ['1001', '1002', '1003'],
  );
  assert.equal(merged.pageCount, 2);
  assert.equal(merged.nextOffset, 20);
  assert.equal(merged.hasMore, false);
});

test('search keyword and channel normalization is stable', () => {
  assert.equal(normalizeDouyinSearchKeyword('%E5%92%96%E5%95%A1'), '咖啡');
  assert.equal(normalizeDouyinSearchKeyword(' 咖啡 '), '咖啡');
  assert.equal(normalizeDouyinSearchChannel('video'), 'aweme_video');
  assert.equal(normalizeDouyinSearchChannel('aweme_general'), 'aweme_general');
});
