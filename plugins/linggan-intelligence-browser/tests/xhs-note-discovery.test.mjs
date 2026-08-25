import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildDiscoveryPlan,
  discoverNotesFromDOM,
  discoverProfileSurfaceNotesFromApi,
  discoverSearchSurfaceNotesFromApi,
  discoverSurfaceNotesFromBestSource,
  discoverWithScroll,
  normalizeProfilePostedNote,
  normalizeSearchSurfaceNote,
  shouldStopDiscovery,
} from '../src/platforms/xhs/noteCollector.js';

test('profile api note normalization maps Xiaohongshu user_posted cards into surface notes', () => {
  const note = normalizeProfilePostedNote({
    note_id: '684bc095000000002002a91c',
    display_title: '汪汪队发声书，宝宝自主阅读的启蒙神器',
    xsec_token: 'token-1',
    type: 'video',
    user: {
      user_id: '5ebe6d210000000001000afe',
      nickname: '孩悦',
      avatar: 'https://example.com/avatar.jpg',
    },
    interact_info: {
      liked_count: '36',
      sticky: true,
    },
    cover: {
      info_list: [
        { image_scene: 'WB_PRV', url: 'https://example.com/cover-small.jpg' },
        { image_scene: 'WB_DFT', url: 'https://example.com/cover.jpg' },
      ],
    },
  }, {
    index: 0,
    sourceUrl: 'https://edith.xiaohongshu.com/api/sns/web/v1/user_posted',
    userId: '5ebe6d210000000001000afe',
  });

  assert.equal(note.noteId, '684bc095000000002002a91c');
  assert.equal(note.title, '汪汪队发声书，宝宝自主阅读的启蒙神器');
  assert.equal(note.likes, '36');
  assert.equal(note.type, 'video');
  assert.equal(note.authorId, '5ebe6d210000000001000afe');
  assert.equal(note.authorName, '孩悦');
  assert.equal(note.isPinned, true);
  assert.equal(note.sticky, true);
  assert.equal(note.dataSource, 'xhs.user_posted');
  assert.match(note.url, /xsec_token=token-1/);
  assert.ok(note.cover.includes('cover-small.jpg'));
});

test('profile surface discovery prefers captured user_posted pages without scrolling', async () => {
  const notes = await discoverProfileSurfaceNotesFromApi({
    expectedCount: 2,
    currentUrl: 'https://www.xiaohongshu.com/user/profile/5ebe6d210000000001000afe?xsec_token=profile-token&xsec_source=pc_search',
    ensureBridge: () => {},
    requestSnapshot: async () => ({
      userId: '5ebe6d210000000001000afe',
      pages: [
        {
          userId: '5ebe6d210000000001000afe',
          sourceUrl: 'https://edith.xiaohongshu.com/api/sns/web/v1/user_posted?cursor=',
          notes: [
            {
              note_id: '684bc095000000002002a91c',
              display_title: '第一篇',
              xsec_token: 'token-a',
              interact_info: { liked_count: 36 },
              user: { user_id: '5ebe6d210000000001000afe', nickname: '孩悦' },
            },
            {
              note_id: '684b876d0000000021018cb1',
              display_title: '第二篇',
              xsec_token: 'token-b',
              interact_info: { liked_count: '41' },
              user: { user_id: '5ebe6d210000000001000afe', nickname: '孩悦' },
            },
            {
              note_id: '684b876d0000000021018cb1',
              display_title: '重复篇',
              xsec_token: 'token-b',
              interact_info: { liked_count: '41' },
            },
          ],
        },
      ],
    }),
    fetchJson: async () => {
      throw new Error('fetch should not be needed when snapshot is available');
    },
  });

  assert.equal(notes.length, 2);
  assert.deepEqual(notes.map((item) => item.noteId), [
    '684bc095000000002002a91c',
    '684b876d0000000021018cb1',
  ]);
  assert.equal(notes[0].likes, '36');
  assert.equal(notes[1].likes, '41');
});

test('best source discovery uses captured profile notes before scrolling', async () => {
  const scrollCalls = [];
  const notes = await discoverSurfaceNotesFromBestSource('#userPostedFeeds', 30, {
    expectedCount: 1,
    currentUrl: 'https://www.xiaohongshu.com/user/profile/author_1?xsec_token=token',
    profileDiscover: async ({ expectedCount, currentUrl }) => [
      {
        noteId: 'note_1',
        title: `cached ${expectedCount}`,
        url: currentUrl,
      },
    ],
    scrollDiscover: async (...args) => {
      scrollCalls.push(args);
      return [{ noteId: 'scroll_note' }];
    },
  });

  assert.deepEqual(notes.map((item) => item.noteId), ['note_1']);
  assert.equal(notes[0].title, 'cached 1');
  assert.equal(scrollCalls.length, 0);
});

test('best source discovery keeps scrolling when cached profile notes are below target', async () => {
  const scrollCalls = [];
  const notes = await discoverSurfaceNotesFromBestSource('#userPostedFeeds', 30, {
    expectedCount: 3,
    currentUrl: 'https://www.xiaohongshu.com/user/profile/author_1?xsec_token=token',
    profileDiscover: async () => [
      { noteId: 'note_1', title: 'cached first' },
    ],
    scrollDiscover: async (...args) => {
      scrollCalls.push(args);
      return [
        { noteId: 'note_1', title: 'duplicate visible card' },
        { noteId: 'note_2', title: 'scrolled second' },
        { noteId: 'note_3', title: 'scrolled third' },
      ];
    },
  });

  assert.deepEqual(notes.map((item) => item.noteId), ['note_1', 'note_2', 'note_3']);
  assert.equal(notes[0].title, 'cached first');
  assert.equal(scrollCalls.length, 1);
  assert.equal(scrollCalls[0][0], '#userPostedFeeds');
  assert.equal(scrollCalls[0][1], 30);
  assert.equal(scrollCalls[0][2].expectedCount, 3);
});

test('best source discovery treats rejected profile api as normal and keeps scrolling', async () => {
  const scrollCalls = [];
  const notes = await discoverSurfaceNotesFromBestSource('#userPostedFeeds', 30, {
    expectedCount: 200,
    currentUrl: 'https://www.xiaohongshu.com/user/profile/author_1?xsec_token=token',
    profileDiscover: async () => {
      throw new Error('406 Not Acceptable');
    },
    scrollDiscover: async (...args) => {
      scrollCalls.push(args);
      return [
        { noteId: 'scroll_note_1' },
        { noteId: 'scroll_note_2' },
      ];
    },
  });

  assert.deepEqual(notes.map((item) => item.noteId), ['scroll_note_1', 'scroll_note_2']);
  assert.equal(scrollCalls.length, 1);
  assert.equal(scrollCalls[0][0], '#userPostedFeeds');
  assert.equal(scrollCalls[0][1], 30);
  assert.equal(scrollCalls[0][2].expectedCount, 200);
});

test('search api note normalization maps Xiaohongshu search cards into surface notes', () => {
  const note = normalizeSearchSurfaceNote({
    id: '684bc095000000002002a91c',
    xsec_token: 'search-token-1',
    model_type: 'note',
    note_card: {
      type: 'normal',
      display_title: '多动孩子的奖励系统怎么用',
      user: {
        user_id: '5ebe6d210000000001000afe',
        nickname: '作者A',
        avatar: 'https://example.com/avatar.jpg',
      },
      interact_info: {
        liked_count: '105',
        collected_count: '23',
        comment_count: '7',
        shared_count: '3',
      },
      image_list: [{
        info_list: [
          { image_scene: 'WB_PRV', url: 'https://example.com/cover-small.jpg' },
          { image_scene: 'WB_DFT', url: 'https://example.com/cover.jpg' },
        ],
      }],
      corner_tag_info: [{ type: 'publish_time', text: '1天前' }],
    },
  }, {
    index: 0,
    sourceUrl: 'https://so.xiaohongshu.com/api/sns/web/v2/search/notes',
    keyword: '多动',
  });

  assert.equal(note.noteId, '684bc095000000002002a91c');
  assert.equal(note.title, '多动孩子的奖励系统怎么用');
  assert.equal(note.likes, '105');
  assert.equal(note.collects, '23');
  assert.equal(note.comments, '7');
  assert.equal(note.shares, '3');
  assert.equal(note.type, 'normal');
  assert.equal(note.authorId, '5ebe6d210000000001000afe');
  assert.equal(note.authorName, '作者A');
  assert.equal(note.publishedAtText, '1天前');
  assert.equal(note.searchKeyword, '多动');
  assert.equal(note.dataSource, 'xhs.search_notes');
  assert.match(note.url, /xsec_token=search-token-1/);
  assert.ok(note.cover.includes('cover-small.jpg'));
  assert.deepEqual(note.images, [
    'https://example.com/cover-small.jpg',
    'https://example.com/cover.jpg',
  ]);
});

test('search surface discovery prefers captured search pages without scrolling', async () => {
  const notes = await discoverSearchSurfaceNotesFromApi({
    expectedCount: 2,
    currentUrl: 'https://www.xiaohongshu.com/search_result?keyword=%E5%A4%9A%E5%8A%A8',
    ensureBridge: () => {},
    requestSnapshot: async (keyword) => ({
      keyword,
      pages: [
        {
          keyword: '多动',
          sourceUrl: 'https://so.xiaohongshu.com/api/sns/web/v2/search/notes?page=1',
          notes: [
            {
              id: '684bc095000000002002a91c',
              xsec_token: 'token-a',
              note_card: {
                display_title: '第一篇',
                interact_info: { liked_count: 36, collected_count: '12', comment_count: '4' },
                user: { user_id: 'author-a', nickname: '作者A' },
              },
            },
            {
              id: '684b876d0000000021018cb1',
              xsec_token: 'token-b',
              note_card: {
                display_title: '第二篇',
                interact_info: { liked_count: '41', collected_count: '19', comment_count: '8' },
                user: { user_id: 'author-b', nickname: '作者B' },
              },
            },
            {
              id: '684b876d0000000021018cb1',
              xsec_token: 'token-b',
              note_card: {
                display_title: '重复篇',
                interact_info: { liked_count: '41' },
              },
            },
          ],
        },
        {
          keyword: '别的词',
          sourceUrl: 'https://so.xiaohongshu.com/api/sns/web/v2/search/notes?page=2',
          notes: [{
            id: '684000000000000000000000',
            note_card: { display_title: '不应混入' },
          }],
        },
      ],
    }),
  });

  assert.equal(notes.length, 2);
  assert.deepEqual(notes.map((item) => item.noteId), [
    '684bc095000000002002a91c',
    '684b876d0000000021018cb1',
  ]);
  assert.equal(notes[0].likes, '36');
  assert.equal(notes[0].collects, '12');
  assert.equal(notes[0].comments, '4');
  assert.equal(notes[0].searchKeyword, '多动');
  assert.equal(notes[1].likes, '41');
  assert.equal(notes[1].authorName, '作者B');
});

test('profile discovery keeps scanning until enough content is found or the page is actually at the bottom', () => {
  const plan = buildDiscoveryPlan('#userPostedFeeds', {
    maxScrolls: 10,
    expectedCount: 50,
  });

  assert.equal(plan.isProfileMode, true);
  assert.equal(plan.stableNoNewLimit, 10);
  assert.equal(plan.requireBottomOrExpected, true);
  assert.equal(plan.bottomConfirmationRounds, 9);
  assert.ok(plan.maxRounds >= 90);
  assert.equal(plan.humanScroll, true);

  assert.equal(shouldStopDiscovery({
    noNewCount: 10,
    stableNoNewLimit: plan.stableNoNewLimit,
    discoveredCount: 28,
    expectedCount: plan.expectedCount,
    atBottom: false,
    bottomNoNewCount: 0,
    bottomConfirmationRounds: plan.bottomConfirmationRounds,
    requireBottomOrExpected: plan.requireBottomOrExpected,
  }), false);

  assert.equal(shouldStopDiscovery({
    noNewCount: 10,
    stableNoNewLimit: plan.stableNoNewLimit,
    discoveredCount: 28,
    expectedCount: plan.expectedCount,
    atBottom: true,
    bottomNoNewCount: 1,
    bottomConfirmationRounds: plan.bottomConfirmationRounds,
    requireBottomOrExpected: plan.requireBottomOrExpected,
  }), false);

  assert.equal(shouldStopDiscovery({
    noNewCount: 10,
    stableNoNewLimit: plan.stableNoNewLimit,
    discoveredCount: 36,
    expectedCount: plan.expectedCount,
    atBottom: true,
    bottomNoNewCount: 3,
    bottomConfirmationRounds: plan.bottomConfirmationRounds,
    requireBottomOrExpected: plan.requireBottomOrExpected,
  }), false);

  assert.equal(shouldStopDiscovery({
    noNewCount: 10,
    stableNoNewLimit: plan.stableNoNewLimit,
    discoveredCount: 36,
    expectedCount: plan.expectedCount,
    atBottom: true,
    bottomNoNewCount: 9,
    bottomConfirmationRounds: plan.bottomConfirmationRounds,
    requireBottomOrExpected: plan.requireBottomOrExpected,
  }), true);
});

test('deep profile discovery allows enough human-paced rounds for 200-link archive jobs', () => {
  const plan = buildDiscoveryPlan('#userPostedFeeds', {
    maxScrolls: 30,
    expectedCount: 200,
  });

  assert.equal(plan.maxRounds, 180);
  assert.equal(plan.stableNoNewLimit, 15);
  assert.equal(plan.bottomConfirmationRounds, 12);
  assert.equal(plan.stepRatio, 0.74);
  assert.equal(plan.humanScroll, true);
});

test('search discovery keeps the old eager stop behavior', () => {
  const plan = buildDiscoveryPlan('.feeds-container', {
    maxScrolls: 10,
    expectedCount: 50,
  });

  assert.equal(plan.isProfileMode, false);
  assert.equal(plan.stableNoNewLimit, 2);
  assert.equal(plan.requireBottomOrExpected, false);

  assert.equal(shouldStopDiscovery({
    noNewCount: 2,
    stableNoNewLimit: plan.stableNoNewLimit,
    discoveredCount: 12,
    expectedCount: plan.expectedCount,
    atBottom: false,
    requireBottomOrExpected: plan.requireBottomOrExpected,
  }), true);
});

test('profile discovery accumulates virtualized cards beyond the visible 28 items', async () => {
  const originalDocument = globalThis.document;
  const originalWindow = globalThis.window;
  const originalNow = Date.now;
  const ids = Array.from({ length: 50 }, (_, index) => `68${String(index).padStart(22, '0')}`);
  let scrollTopValue = 0;
  let maxScrollTop = 0;
  let now = 0;

  const makeSection = (id, index) => ({
    querySelector(selector) {
      if (selector === 'a.cover') {
        return {
          getAttribute(name) {
            return name === 'href'
              ? `/user/profile/5f1234567890abcd12345678/${id}?xsec_token=token-${id}`
              : null;
          },
        };
      }
      if (selector === '.footer span' || selector === '.title') {
        return { textContent: `标题 ${index}` };
      }
      if (selector === '.like-wrapper .count') {
        return { textContent: String(index) };
      }
      return null;
    },
    getBoundingClientRect() {
      return {
        top: 120 + index * 10,
        left: index % 2 === 0 ? 20 : 320,
      };
    },
  });

  const scrollBox = {
    get scrollTop() {
      return scrollTopValue;
    },
    set scrollTop(value) {
      scrollTopValue = Math.max(0, Number(value || 0));
      maxScrollTop = Math.max(maxScrollTop, scrollTopValue);
    },
    clientHeight: 1000,
    scrollHeight: 2400,
    parentElement: null,
  };
  const feed = {
    clientHeight: 1000,
    scrollHeight: 1000,
    parentElement: scrollBox,
  };

  globalThis.document = {
    documentElement: { scrollTop: 0, clientHeight: 1000, scrollHeight: 1000 },
    body: { scrollTop: 0, clientHeight: 1000, scrollHeight: 1000 },
    querySelector(selector) {
      return selector === '#userPostedFeeds' ? feed : null;
    },
    querySelectorAll(selector) {
      if (selector !== '#userPostedFeeds section') return [];
      const visibleIds = scrollTopValue < 400 ? ids.slice(0, 28) : ids.slice(22, 50);
      return visibleIds.map(makeSection);
    },
  };
  globalThis.window = {
    scrollY: 0,
    innerHeight: 1000,
    scrollTo({ top = 0 } = {}) {
      this.scrollY = top;
    },
    scrollBy({ top = 0 } = {}) {
      this.scrollY += top;
    },
  };
  Date.now = () => {
    now += 5000;
    return now;
  };

  try {
    const records = await discoverWithScroll('#userPostedFeeds', 10, {
      expectedCount: 50,
    });

    assert.equal(records.length, 50);
    assert.equal(records[0].noteId, ids[0]);
    assert.equal(records[49].noteId, ids[49]);
    assert.ok(maxScrollTop > 0);
    assert.equal(records.discoveryMeta.stopReason, 'target_reached');
    assert.equal(records.discoveryMeta.totalNotes, 50);
    assert.equal(records.discoveryMeta.fieldQuality.withTitle, 50);
  } finally {
    globalThis.document = originalDocument;
    globalThis.window = originalWindow;
    Date.now = originalNow;
  }
});

test('profile discovery uses the actual feed viewport height instead of the browser window height', async () => {
  const originalDocument = globalThis.document;
  const originalWindow = globalThis.window;
  const originalNow = Date.now;
  const ids = Array.from({ length: 50 }, (_, index) => `68${String(index).padStart(22, '0')}`);
  let scrollTopValue = 0;
  let maxScrollTop = 0;
  let now = 0;

  const makeSection = (id, index) => ({
    querySelector(selector) {
      if (selector === 'a.cover') {
        return {
          getAttribute(name) {
            return name === 'href'
              ? `/user/profile/5f1234567890abcd12345678/${id}?xsec_token=token-${id}`
              : null;
          },
        };
      }
      if (selector === '.footer span' || selector === '.title') {
        return { textContent: `标题 ${index}` };
      }
      if (selector === '.like-wrapper .count') {
        return { textContent: String(index) };
      }
      return null;
    },
    getBoundingClientRect() {
      return {
        top: 120 + index * 10,
        left: index % 2 === 0 ? 20 : 320,
      };
    },
  });

  const scrollBox = {
    get scrollTop() {
      return scrollTopValue;
    },
    set scrollTop(value) {
      scrollTopValue = Math.max(0, Number(value || 0));
      maxScrollTop = Math.max(maxScrollTop, scrollTopValue);
    },
    clientHeight: 320,
    scrollHeight: 1400,
    parentElement: null,
  };
  const feed = {
    clientHeight: 320,
    scrollHeight: 320,
    parentElement: scrollBox,
  };

  globalThis.document = {
    documentElement: { scrollTop: 0, clientHeight: 1000, scrollHeight: 1000 },
    body: { scrollTop: 0, clientHeight: 1000, scrollHeight: 1000 },
    querySelector(selector) {
      return selector === '#userPostedFeeds' ? feed : null;
    },
    querySelectorAll(selector) {
      if (selector !== '#userPostedFeeds section') return [];
      const visibleIds = scrollTopValue < 700 ? ids.slice(0, 28) : ids.slice(22, 50);
      return visibleIds.map(makeSection);
    },
  };
  globalThis.window = {
    scrollY: 0,
    innerHeight: 1000,
    scrollTo({ top = 0 } = {}) {
      this.scrollY = top;
    },
    scrollBy({ top = 0 } = {}) {
      this.scrollY += top;
    },
  };
  Date.now = () => {
    now += 5000;
    return now;
  };

  try {
    const records = await discoverWithScroll('#userPostedFeeds', 10, {
      expectedCount: 50,
    });

    assert.equal(records.length, 50);
    assert.ok(maxScrollTop >= 700);
  } finally {
    globalThis.document = originalDocument;
    globalThis.window = originalWindow;
    Date.now = originalNow;
  }
});

test('profile discovery nudges the bottom of an infinite profile feed before giving up at 28 cards', async () => {
  const originalDocument = globalThis.document;
  const originalWindow = globalThis.window;
  const originalWheelEvent = globalThis.WheelEvent;
  const originalNow = Date.now;
  const ids = Array.from({ length: 50 }, (_, index) => `68${String(index).padStart(22, '0')}`);
  let loadTriggered = false;
  let scrollY = 0;
  let now = 0;

  class FakeWheelEvent {
    constructor(type, init = {}) {
      this.type = type;
      this.deltaY = init.deltaY || 0;
    }
  }

  const makeSection = (id, index) => ({
    querySelector(selector) {
      if (selector === 'a.cover') {
        return {
          getAttribute(name) {
            return name === 'href'
              ? `/user/profile/5f1234567890abcd12345678/${id}?xsec_token=token-${id}`
              : null;
          },
          querySelector() {
            return { currentSrc: `https://sns-img.example.com/${id}.jpg` };
          },
        };
      }
      if (selector === '.footer span' || selector === '.title') {
        return { textContent: `标题 ${index}` };
      }
      if (selector === '.like-wrapper .count') {
        return { textContent: String(index) };
      }
      return null;
    },
    getBoundingClientRect() {
      return {
        top: 120 + index * 10,
        left: index % 2 === 0 ? 20 : 320,
      };
    },
  });

  const feed = {
    clientHeight: 1000,
    get scrollHeight() {
      return loadTriggered ? 2400 : 1000;
    },
    parentElement: null,
  };

  const documentElement = {
    scrollTop: 0,
    clientHeight: 1000,
    get scrollHeight() {
      return loadTriggered ? 2400 : 1000;
    },
    dispatchEvent(event) {
      if (event?.type === 'wheel' && event.deltaY > 0) loadTriggered = true;
      return true;
    },
  };
  const body = {
    scrollTop: 0,
    clientHeight: 1000,
    get scrollHeight() {
      return loadTriggered ? 2400 : 1000;
    },
    dispatchEvent: documentElement.dispatchEvent,
  };

  globalThis.WheelEvent = FakeWheelEvent;
  globalThis.document = {
    documentElement,
    body,
    querySelector(selector) {
      return selector === '#userPostedFeeds' ? feed : null;
    },
    querySelectorAll(selector) {
      if (selector !== '#userPostedFeeds section') return [];
      const visibleIds = loadTriggered || scrollY > 0 ? ids : ids.slice(0, 28);
      return visibleIds.map(makeSection);
    },
    dispatchEvent(event) {
      if (event?.type === 'wheel' && event.deltaY > 0) loadTriggered = true;
      return true;
    },
  };
  globalThis.window = {
    get scrollY() {
      return scrollY;
    },
    set scrollY(value) {
      scrollY = Math.max(0, Number(value || 0));
    },
    innerHeight: 1000,
    scrollTo({ top = 0 } = {}) {
      scrollY = Math.max(0, Number(top || 0));
      documentElement.scrollTop = scrollY;
      body.scrollTop = scrollY;
    },
    scrollBy({ top = 0 } = {}) {
      scrollY = Math.max(0, scrollY + Number(top || 0));
      documentElement.scrollTop = scrollY;
      body.scrollTop = scrollY;
      if (top > 0 && scrollY === 0) loadTriggered = true;
    },
    dispatchEvent(event) {
      if (event?.type === 'wheel' && event.deltaY > 0) loadTriggered = true;
      return true;
    },
  };
  Date.now = () => {
    now += 5000;
    return now;
  };

  try {
    const records = await discoverWithScroll('#userPostedFeeds', 10, {
      expectedCount: 50,
    });

    assert.equal(records.length, 50);
    assert.equal(loadTriggered, true);
  } finally {
    globalThis.document = originalDocument;
    globalThis.window = originalWindow;
    globalThis.WheelEvent = originalWheelEvent;
    Date.now = originalNow;
  }
});

test('note discovery carries the card cover image into surface records', () => {
  const originalDocument = globalThis.document;
  const originalWindow = globalThis.window;
  const noteId = '680123456789abcdef012345';

  globalThis.document = {
    querySelectorAll(selector) {
      if (selector !== '#userPostedFeeds section') return [];
      return [{
        querySelector(selector) {
          if (selector === 'a.cover') {
            return {
              getAttribute(name) {
                return name === 'href'
                  ? `/user/profile/5f1234567890abcd12345678/${noteId}?xsec_token=abc123`
                  : null;
              },
              querySelector(imageSelector) {
                if (imageSelector !== 'img, picture img, source') return null;
                return {
                  currentSrc: '',
                  src: '',
                  getAttribute(name) {
                    if (name === 'data-src') return 'https://sns-img.example.com/card-cover.jpg';
                    return null;
                  },
                };
              },
            };
          }
          if (selector === '.footer span' || selector === '.title') {
            return { textContent: '带封面的作品' };
          }
          if (selector === '.like-wrapper .count') {
            return { textContent: '123' };
          }
          return null;
        },
        getBoundingClientRect() {
          return { top: 100, left: 20 };
        },
      }];
    },
  };
  globalThis.window = { scrollY: 0 };

  try {
    const records = discoverNotesFromDOM('#userPostedFeeds');

    assert.equal(records.length, 1);
    assert.equal(records[0].cover, 'https://sns-img.example.com/card-cover.jpg');
    assert.equal(records[0].coverImg, 'https://sns-img.example.com/card-cover.jpg');
    assert.deepEqual(records[0].images, ['https://sns-img.example.com/card-cover.jpg']);
  } finally {
    globalThis.document = originalDocument;
    globalThis.window = originalWindow;
  }
});
