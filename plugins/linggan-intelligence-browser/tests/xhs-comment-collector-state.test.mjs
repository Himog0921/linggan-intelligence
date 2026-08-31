import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectComments,
  initializeCollectedComments,
  parseCommentTextTail,
  resolveCommentContinuationHint,
  resolveXhsCommentTargetIdentity,
  rewindCommentSurface,
  shouldContinueDomAfterApi,
} from '../src/platforms/xhs/commentCollector.js';
import { COMMENT_DEPTH_MODE } from '../src/shared/constants.js';

test('initializeCollectedComments seeds API comments for later DOM continuation without duplicates', () => {
  const seeded = initializeCollectedComments([
    { commentId: 'root_1', level: 1 },
    { commentId: 'reply_1', level: 2 },
    { commentId: 'reply_1', level: 2 },
    { commentId: '', level: 2 },
  ]);

  assert.deepEqual(seeded.allComments.map((item) => item.commentId), ['root_1', 'reply_1']);
  assert.deepEqual([...seeded.seenIds], ['root_1', 'reply_1']);
});

test('comment target identity never treats an unobserved or different page as matched', () => {
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore/expected',
  }), 'matched');
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore/different',
  }), 'mismatched');
  assert.equal(resolveXhsCommentTargetIdentity({
    expectedNoteId: 'expected',
    currentUrl: 'https://www.xiaohongshu.com/explore?source=webshare',
  }), 'unverified');
});

test('shouldContinueDomAfterApi keeps reply continuation and visible top-up paths alive', () => {
  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    hydrationDegraded: true,
    hasExpandableReplies: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    hydrationDegraded: false,
    hasExpandableReplies: true,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    hydrationDegraded: true,
    hasExpandableReplies: true,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 16,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 16,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: false,
  }), false);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    currentTotal: 0,
    maxTotal: 20,
    commentHint: 27,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    currentTotal: 17,
    maxTotal: 0,
    commentHint: 492,
    hasDomComments: true,
  }), true);

  assert.equal(shouldContinueDomAfterApi({
    depthMode: COMMENT_DEPTH_MODE.ALL_REPLIES,
    currentTotal: 492,
    maxTotal: 0,
    commentHint: 492,
    hasDomComments: true,
  }), false);
});

test('known public count survives when the current DOM heading cannot be parsed', () => {
  assert.equal(resolveCommentContinuationHint(0, 492), 492);
  assert.equal(resolveCommentContinuationHint(493, 492), 493);
  assert.equal(resolveCommentContinuationHint(0, null), 0);
});

test('DOM fallback rewinds the comment surface before reading a new Attempt', async (t) => {
  const previousWindow = globalThis.window;
  const events = [];
  const scrollParent = {
    parentElement: null,
    scrollTop: 640,
    scrollTo({ top }) {
      events.push(`scroll:${top}`);
      this.scrollTop = top;
    },
  };
  globalThis.window = {
    getComputedStyle() {
      return { overflow: 'auto', overflowY: 'auto' };
    },
  };
  t.after(() => { globalThis.window = previousWindow; });

  const changed = await rewindCommentSurface({ parentElement: scrollParent }, {
    actionGate: {
      async before({ kind }) {
        events.push(`before:${kind}`);
        return true;
      },
      async after({ kind, changed: didChange }) {
        events.push(`after:${kind}:${didChange}`);
      },
    },
  });

  assert.equal(changed, true);
  assert.equal(scrollParent.scrollTop, 0);
  assert.deepEqual(events, ['before:dom_rewind', 'scroll:0', 'after:dom_rewind:true']);
});

test('parseCommentTextTail splits visible XHS comment text without mixing metrics into content', () => {
  assert.deepEqual(parseCommentTextTail(
    '热死人了😭😭 在美国也算正常吧毕竟美国人的思想开放多了 4小时前 广东 309 回复',
    '热死人了😭😭',
  ), {
    contentText: '在美国也算正常吧毕竟美国人的思想开放多了',
    replyToNickname: '',
    timeText: '4小时前',
    ipLocation: '广东',
    likeText: '309',
    replyCount: 0,
    replyCountText: '',
  });

  assert.deepEqual(parseCommentTextTail(
    '西瓜葉 回复 热死人了😭😭 : 跟国家没关系，美国也分人 59分钟前 安徽 7 回复',
    '西瓜葉',
  ), {
    contentText: '跟国家没关系，美国也分人',
    replyToNickname: '热死人了😭😭',
    timeText: '59分钟前',
    ipLocation: '安徽',
    likeText: '7',
    replyCount: 0,
    replyCountText: '',
  });
});

test('collectComments can use API snapshot before comments container renders', async (t) => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const previousChrome = globalThis.chrome;
  const listeners = new Set();

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/explore/note_api_only',
      pathname: '/explore/note_api_only',
    },
    innerHeight: 800,
    addEventListener(type, listener) {
      if (type === 'message') listeners.add(listener);
    },
    removeEventListener(type, listener) {
      if (type === 'message') listeners.delete(listener);
    },
    setTimeout,
    clearTimeout,
    getComputedStyle() {
      return {
        display: 'block',
        visibility: 'visible',
        opacity: '1',
        overflow: 'visible',
        overflowY: 'visible',
      };
    },
    postMessage(message) {
      const responseByRequestType = {
        __lgboom_xhs_comment_api_reset_request__: {
          type: '__lgboom_xhs_comment_api_reset_response__',
          payload: {
            ok: true,
            noteId: 'note_api_only',
            reset: true,
          },
        },
        __lgboom_xhs_page_fetch_request__: {
          type: '__lgboom_xhs_page_fetch_response__',
          payload: {
            ok: true,
            json: {
              data: {
                comments: [{
                  id: 'comment_1',
                  content: '这是一条接口评论',
                  user_info: {
                    nickname: '评论用户',
                    user_id: 'user_1',
                  },
                }],
                has_more: false,
                cursor: '',
              },
            },
          },
        },
      };
      const response = responseByRequestType[message?.type];
      if (!response) return;
      setTimeout(() => {
        listeners.forEach((listener) => listener({
          source: globalThis.window,
          data: {
            source: 'lgboom-xhs-api-capture',
            type: response.type,
            payload: {
              requestId: message.payload.requestId,
              ...response.payload,
            },
          },
        }));
      }, 0);
    },
  };
  globalThis.document = {
    readyState: 'complete',
    body: {
      innerText: '',
      querySelectorAll() {
        return [];
      },
    },
    documentElement: {
      scrollBy() {},
      getBoundingClientRect() {
        return { top: 0, bottom: 800 };
      },
    },
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
  };
  globalThis.chrome = {
    runtime: {
      getURL() {
        return '';
      },
    },
  };

  t.after(() => {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
    globalThis.chrome = previousChrome;
  });

  const result = await collectComments({
    noteId: 'note_api_only',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_api_only',
    maxTotal: 1,
    persist: false,
  });

  assert.equal(result.total, 1);
  assert.equal(result.comments[0].text, '这是一条接口评论');
});

test('collectComments stops immediately when the page reports an access risk', async (t) => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/explore/note_risk',
      pathname: '/explore/note_risk',
    },
  };
  globalThis.document = {
    body: { innerText: '操作频繁，请稍后再试' },
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
  };

  t.after(() => {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  });

  const result = await collectComments({
    noteId: 'note_risk',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_risk',
    persist: false,
    emitReceipt: false,
  });

  assert.equal(result.total, 0);
  assert.equal(result.stopReason, 'risk_control');
});
