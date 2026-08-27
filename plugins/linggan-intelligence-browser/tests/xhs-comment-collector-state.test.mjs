import test from 'node:test';
import assert from 'node:assert/strict';

import {
  collectComments,
  initializeCollectedComments,
  parseCommentTextTail,
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
      if (message?.type !== '__lgboom_xhs_comment_api_request__') return;
      setTimeout(() => {
        listeners.forEach((listener) => listener({
          source: globalThis.window,
          data: {
            source: 'lgboom-xhs-api-capture',
            type: '__lgboom_xhs_comment_api_response__',
            payload: {
              requestId: message.payload.requestId,
              ok: true,
              pages: [{
                noteId: 'note_api_only',
                comments: [{
                  id: 'comment_1',
                  content: '这是一条接口评论',
                  user_info: {
                    nickname: '评论用户',
                    user_id: 'user_1',
                  },
                }],
                hasMore: false,
                capturedAt: Date.now(),
              }],
              subPages: [],
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
