import test from 'node:test';
import assert from 'node:assert/strict';

import {
  commentPlatformTimesFromSnapshot,
  parseXhsCommentPagePayload,
  buildXhsCommentsFromSnapshot,
  hydrateXhsCommentSnapshot,
  readXhsSsrCommentPlatformTimes,
  resolveXhsCommentImageUrls,
  startFreshXhsCommentSnapshot,
} from '../src/platforms/xhs/commentApi.js';
import { packageComments } from '../src/linggan/producerRuntime.js';

test('comment image extraction keeps public media candidates and excludes unrelated avatar fields', () => {
  assert.deepEqual(resolveXhsCommentImageUrls({
    pictures: [
      { url_default: '//sns-img.example/comment-a.webp' },
      { url_list: ['https://sns-img.example/comment-a.webp', 'https://sns-img.example/comment-b.webp'] },
    ],
    image_list: [{ origin_url: 'https://sns-img.example/comment-c.webp' }],
    user_info: { image: 'https://sns-avatar.example/author.webp' },
  }), [
    'https://sns-img.example/comment-a.webp',
    'https://sns-img.example/comment-b.webp',
    'https://sns-img.example/comment-c.webp',
  ]);
});

test('a new comment Attempt resets prior pages and fetches a fresh first page', async () => {
  const events = [];
  let generation = 0;
  const options = {
    resetSnapshot: async (noteId) => {
      events.push(`reset:${noteId}`);
      return true;
    },
    beforeExternalAction: async () => { events.push('before'); },
    afterExternalAction: async () => { events.push('after'); },
    fetchJson: async () => {
      generation += 1;
      return { data: { comments: [{ id: `fresh_${generation}` }], has_more: true, cursor: `cursor_${generation}` } };
    },
  };
  const first = await startFreshXhsCommentSnapshot('note_1', options);
  const second = await startFreshXhsCommentSnapshot('note_1', options);
  assert.deepEqual(first.pages[0].comments.map((comment) => comment.id), ['fresh_1']);
  assert.deepEqual(second.pages[0].comments.map((comment) => comment.id), ['fresh_2']);
  assert.deepEqual(second.subPages, []);
  assert.deepEqual(events, ['reset:note_1', 'before', 'after', 'reset:note_1', 'before', 'after']);
});

test('a new comment Attempt never fetches when snapshot reset is not confirmed', async () => {
  let fetchCalled = false;
  await assert.rejects(
    startFreshXhsCommentSnapshot('note_1', {
      resetSnapshot: async () => false,
      fetchJson: async () => {
        fetchCalled = true;
        return {};
      },
    }),
    /comment_snapshot_reset_not_confirmed/,
  );
  assert.equal(fetchCalled, false);
});

test('parseXhsCommentPagePayload reads note id, cursor, hasMore and endpoint from main comment payload', () => {
  const payload = parseXhsCommentPagePayload({
    data: {
      comments: [{ id: 'c_1', content: '主评论' }],
      cursor: '10',
      has_more: true,
    },
  }, {
    sourceUrl: 'https://www.xiaohongshu.com/api/sns/web/v2/comment/page?note_id=note_1&cursor=',
  });

  assert.equal(payload.endpoint, 'page');
  assert.equal(payload.noteId, 'note_1');
  assert.equal(payload.rootCommentId, '');
  assert.equal(payload.cursor, '10');
  assert.equal(payload.hasMore, true);
  assert.equal(payload.comments.length, 1);
});

test('buildXhsCommentsFromSnapshot maps main comments and replies into normalized records', () => {
  const snapshot = {
    pages: [
      {
        noteId: 'note_1',
        cursor: '',
        capturedAt: 10,
        comments: [
          {
            id: 'root_1',
            content: '主评论',
            create_time: 1710000000,
            like_count: 12,
            ip_location: '上海',
            pictures: [{ url_default: '//sns-img.example/comment-root.webp' }],
            user_info: {
              nickname: '作者甲',
              user_id: 'user_1',
              image: '//sns-avatar-1.jpg',
            },
            sub_comments: [
              {
                id: 'reply_1',
                content: '内联回复',
                create_time: 1710000010,
                like_count: 1,
                user_info: {
                  nickname: '作者乙',
                  user_id: 'user_2',
                  image: 'https://sns-avatar-2.jpg',
                },
                target_comment: {
                  id: 'root_1',
                  user_info: {
                    nickname: '作者甲',
                  },
                },
              },
            ],
          },
        ],
      },
    ],
    subPages: [
      {
        noteId: 'note_1',
        rootCommentId: 'root_1',
        cursor: '20',
        capturedAt: 20,
        comments: [
          {
            id: 'reply_2',
            content: '展开回复',
            create_time: 1710000020,
            like_count: 2,
            user_info: {
              nickname: '作者丙',
              user_id: 'user_3',
            },
            target_comment: {
              id: 'reply_1',
              user_info: {
                nickname: '作者乙',
              },
            },
          },
        ],
      },
    ],
  };

  const records = buildXhsCommentsFromSnapshot(snapshot, {
    noteId: 'note_1',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_1',
    url: 'https://www.xiaohongshu.com/explore/note_1',
    contentId: 'xhs_note_1',
  }, {
    maxSubComments: 0,
    collectionRunId: 'run_1',
  });

  assert.equal(records.length, 3);

  const main = records.find((item) => item.commentId === 'root_1');
  const inlineReply = records.find((item) => item.commentId === 'reply_1');
  const pagedReply = records.find((item) => item.commentId === 'reply_2');

  assert.equal(main.level, 1);
  assert.equal(main.rootCommentId, 'root_1');
  assert.equal(main.authorEntityId, 'xhs_user_1');
  assert.equal(main.avatarUrl, 'https://sns-avatar-1.jpg');
  assert.equal(main.publishedAt, 1710000000000);
  assert.equal(main.rawSource, 'xhs.comments.api');
  assert.equal(main.dataQuality, 'full');
  assert.equal(main.qualityReason, '');
  assert.equal(main.sourceTier, 'api');
  assert.deepEqual(main.commentImageUrls, ['https://sns-img.example/comment-root.webp']);

  assert.equal(inlineReply.level, 2);
  assert.equal(inlineReply.parentCommentId, 'root_1');
  assert.equal(inlineReply.rootCommentId, 'root_1');
  assert.equal(inlineReply.replyToCommentId, 'root_1');
  assert.equal(inlineReply.replyToUserName, '作者甲');

  assert.equal(pagedReply.level, 2);
  assert.equal(pagedReply.parentCommentId, 'root_1');
  assert.equal(pagedReply.rootCommentId, 'root_1');
  assert.equal(pagedReply.replyToCommentId, 'reply_1');
  assert.equal(pagedReply.replyToUserName, '作者乙');
  assert.equal(pagedReply.collectionRunId, 'run_1');
  assert.equal(pagedReply.dataQuality, 'full');
  assert.equal(pagedReply.sourceTier, 'api');
});

test('comment platform time is a millisecond epoch or null, never collection time or display text', () => {
  const records = buildXhsCommentsFromSnapshot({
    pages: [{
      noteId: 'note_1',
      cursor: '',
      comments: [
        { id: 'seconds', create_time: 1710000000, user_info: { nickname: '甲', user_id: 'u1' } },
        { id: 'milliseconds', create_time: 1710000000123, user_info: { nickname: '乙', user_id: 'u2' } },
        { id: 'missing', user_info: { nickname: '丙', user_id: 'u3' } },
        { id: 'display_text', time: '3 天前', user_info: { nickname: '丁', user_id: 'u4' } },
      ],
    }],
    subPages: [],
  }, { noteId: 'note_1', contentId: 'xhs_note_1' }, {});

  const byId = Object.fromEntries(records.map((record) => [record.commentId, record]));
  assert.equal(byId.seconds.publishedAt, 1710000000000);
  assert.equal(byId.milliseconds.publishedAt, 1710000000123);
  assert.equal(byId.missing.publishedAt, null);
  assert.notEqual(byId.missing.publishedAt, byId.missing.collectedAt);
  assert.equal(byId.display_text.publishedAt, null);
  assert.equal(byId.display_text.publishedAtText, '3 天前');
});

test('buildXhsCommentsFromSnapshot respects per-root reply cap when maxSubComments is set', () => {
  const snapshot = {
    pages: [
      {
        noteId: 'note_1',
        cursor: '',
        capturedAt: 10,
        comments: [
          {
            id: 'root_1',
            content: '主评论',
            user_info: {
              nickname: '作者甲',
              user_id: 'user_1',
            },
            sub_comments: [
              {
                id: 'reply_1',
                content: '内联回复 1',
                user_info: {
                  nickname: '作者乙',
                  user_id: 'user_2',
                },
              },
              {
                id: 'reply_2',
                content: '内联回复 2',
                user_info: {
                  nickname: '作者丙',
                  user_id: 'user_3',
                },
              },
            ],
          },
        ],
      },
    ],
    subPages: [],
  };

  const records = buildXhsCommentsFromSnapshot(snapshot, {
    noteId: 'note_1',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_1',
    url: 'https://www.xiaohongshu.com/explore/note_1',
    contentId: 'xhs_note_1',
  }, {
    maxSubComments: 1,
  });

  assert.equal(records.length, 2);
  assert.deepEqual(records.map((item) => item.commentId), ['root_1', 'reply_1']);
});

test('hydrateXhsCommentSnapshot fetches additional main comment pages when snapshot indicates hasMore', async () => {
  const fetchCalls = [];
  const snapshot = {
    noteId: 'note_1',
    pages: [
      {
        endpoint: 'page',
        noteId: 'note_1',
        cursor: 'cursor_1',
        hasMore: true,
        capturedAt: 1,
        comments: [
          { id: 'root_1', content: '主评论 1' },
        ],
      },
    ],
    subPages: [],
  };

  const hydrated = await hydrateXhsCommentSnapshot(snapshot, {
    noteId: 'note_1',
    fetchJson: async (urls) => {
      fetchCalls.push(urls);
      return {
        data: {
          comments: [
            { id: 'root_2', content: '主评论 2' },
          ],
          cursor: 'cursor_2',
          has_more: false,
        },
      };
    },
  });

  assert.equal(fetchCalls.length, 1);
  assert.match(fetchCalls[0][0], /\/api\/sns\/web\/v2\/comment\/page\?/);
  assert.match(fetchCalls[0][0], /note_id=note_1/);
  assert.match(fetchCalls[0][0], /cursor=cursor_1/);
  assert.equal(hydrated.pages.length, 2);
  assert.equal(hydrated.pages[1].comments[0].id, 'root_2');
});

test('hydrateXhsCommentSnapshot fetches sub comment pages when root comment still has more replies', async () => {
  const fetchCalls = [];
  const snapshot = {
    noteId: 'note_1',
    pages: [
      {
        endpoint: 'page',
        noteId: 'note_1',
        cursor: '',
        hasMore: false,
        capturedAt: 1,
        comments: [
          {
            id: 'root_1',
            content: '主评论 1',
            sub_comment_count: 3,
            sub_comments: [
              { id: 'reply_inline_1', content: '内联回复 1' },
            ],
          },
        ],
      },
    ],
    subPages: [],
  };

  const hydrated = await hydrateXhsCommentSnapshot(snapshot, {
    noteId: 'note_1',
    fetchJson: async (urls) => {
      fetchCalls.push(urls);
      return {
        data: {
          comments: [
            { id: 'reply_page_2', content: '分页回复 2' },
            { id: 'reply_page_3', content: '分页回复 3' },
          ],
          cursor: 'sub_cursor_1',
          has_more: false,
          root_comment_id: 'root_1',
        },
      };
    },
  });

  assert.equal(fetchCalls.length, 1);
  assert.match(fetchCalls[0][0], /\/api\/sns\/web\/v2\/comment\/sub\/page\?/);
  assert.match(fetchCalls[0][0], /note_id=note_1/);
  assert.match(fetchCalls[0][0], /root_comment_id=root_1/);
  assert.match(fetchCalls[0][0], /num=10/);
  assert.equal(hydrated.subPages.length, 1);
  assert.equal(hydrated.subPages[0].rootCommentId, 'root_1');
  assert.equal(hydrated.subPages[0].comments.length, 2);
});

test('hydrateXhsCommentSnapshot keeps xsec token when fetching additional main comment pages', async () => {
  const fetchCalls = [];
  const snapshot = {
    noteId: 'note_1',
    pages: [
      {
        endpoint: 'page',
        noteId: 'note_1',
        cursor: 'cursor_1',
        hasMore: true,
        capturedAt: 1,
        sourceUrl: 'https://edith.xiaohongshu.com/api/sns/web/v2/comment/page?note_id=note_1&cursor=cursor_1&xsec_token=token_123',
        comments: [
          { id: 'root_1', content: '主评论 1' },
        ],
      },
    ],
    subPages: [],
  };

  await hydrateXhsCommentSnapshot(snapshot, {
    noteId: 'note_1',
    fetchJson: async (urls) => {
      fetchCalls.push(urls);
      return {
        data: {
          comments: [{ id: 'root_2', content: '主评论 2' }],
          cursor: 'cursor_2',
          has_more: false,
        },
      };
    },
  });

  assert.equal(fetchCalls.length, 1);
  assert.match(fetchCalls[0][0], /xsec_token=token_123/);
  assert.match(fetchCalls[0][0], /image_formats=jpg%2Cwebp%2Cavif/);
  assert.match(fetchCalls[0][0], /top_comment_id=/);
});

test('hydrateXhsCommentSnapshot keeps xsec token when fetching sub comment pages', async () => {
  const fetchCalls = [];
  const snapshot = {
    noteId: 'note_1',
    pages: [
      {
        endpoint: 'page',
        noteId: 'note_1',
        cursor: '',
        hasMore: false,
        capturedAt: 1,
        sourceUrl: 'https://edith.xiaohongshu.com/api/sns/web/v2/comment/page?note_id=note_1&xsec_token=token_abc',
        comments: [
          {
            id: 'root_1',
            content: '主评论 1',
            sub_comment_count: 2,
            sub_comments: [],
          },
        ],
      },
    ],
    subPages: [],
  };

  await hydrateXhsCommentSnapshot(snapshot, {
    noteId: 'note_1',
    fetchJson: async (urls) => {
      fetchCalls.push(urls);
      return {
        data: {
          comments: [{ id: 'reply_page_1', content: '分页回复 1' }],
          cursor: '',
          has_more: false,
          root_comment_id: 'root_1',
        },
      };
    },
  });

  assert.equal(fetchCalls.length, 1);
  assert.match(fetchCalls[0][0], /xsec_token=token_abc/);
  assert.match(fetchCalls[0][0], /image_formats=jpg%2Cwebp%2Cavif/);
  assert.match(fetchCalls[0][0], /top_comment_id=/);
});

test('stepwise hydration performs at most one external request per acquisition loop', async () => {
  const actions = [];
  let page = 1;
  const snapshot = {
    noteId: 'note_1',
    pages: [{
      endpoint: 'page',
      noteId: 'note_1',
      cursor: 'cursor_1',
      hasMore: true,
      capturedAt: 1,
      comments: [{ id: 'root_1', content: '主评论 1' }],
    }],
    subPages: [],
  };
  const fetchJson = async () => {
    page += 1;
    return {
      data: {
        comments: [{ id: `root_${page}`, content: `主评论 ${page}` }],
        cursor: `cursor_${page}`,
        has_more: page < 3,
      },
    };
  };

  const first = await hydrateXhsCommentSnapshot(snapshot, {
    noteId: 'note_1',
    fetchJson,
    maxExternalActions: 1,
    beforeExternalAction: async (action) => actions.push(`before:${action.kind}`),
    afterExternalAction: async (action) => actions.push(`after:${action.kind}`),
  });
  assert.equal(first.pages.length, 2);
  assert.equal(first.hydrationMeta.actionsPerformed, 1);
  assert.equal(first.hydrationMeta.pending, true);
  assert.deepEqual(actions, ['before:api_main_page', 'after:api_main_page']);

  const second = await hydrateXhsCommentSnapshot(first, {
    noteId: 'note_1',
    fetchJson,
    maxExternalActions: 1,
  });
  assert.equal(second.pages.length, 3);
  assert.equal(second.hydrationMeta.actionsPerformed, 1);
  assert.equal(second.hydrationMeta.pending, false);
});

test('a final reply page overrides a stale root has-more flag on later acquisition loops', async () => {
  let fetchCount = 0;
  const hydrated = await hydrateXhsCommentSnapshot({
    noteId: 'note_1',
    pages: [{
      noteId: 'note_1',
      cursor: '',
      hasMore: false,
      comments: [{
        id: 'root_1',
        sub_comment_count: 2,
        sub_comment_has_more: true,
        sub_comments: [],
      }],
    }],
    subPages: [{
      noteId: 'note_1',
      rootCommentId: 'root_1',
      cursor: '',
      hasMore: false,
      comments: [{ id: 'reply_1' }, { id: 'reply_2' }],
    }],
  }, {
    noteId: 'note_1',
    maxExternalActions: 1,
    fetchJson: async () => { fetchCount += 1; return {}; },
  });

  assert.equal(fetchCount, 0);
  assert.equal(hydrated.hydrationMeta.pending, false);
});

test('snapshot platform times join ids from main pages, inline replies and sub pages', () => {
  const times = commentPlatformTimesFromSnapshot({
    pages: [{
      comments: [
        { id: 'root_1', create_time: 1710000000 },
        { id: 'root_text', time: '3 天前' },
        { id: 'root_inline', create_time: 1710000100, sub_comments: [{ id: 'reply_inline', create_time: 1710000200 }] },
      ],
    }],
    subPages: [{ rootCommentId: 'root_1', comments: [{ id: 'reply_page', create_time: 1710000300123 }] }],
  });

  assert.equal(times.get('root_1'), 1710000000000);
  assert.equal(times.get('reply_inline'), 1710000200000);
  assert.equal(times.get('reply_page'), 1710000300123);
  assert.equal(times.has('root_text'), false);
});

test('SSR note detail map contributes platform times only from serialized script text', async (t) => {
  const savedDocument = globalThis.document;
  globalThis.document = {
    scripts: [{
      textContent: 'window.__INITIAL_STATE__={"note":{"noteDetailMap":{"note_1":{"comments":{"list":['
        + '{"id":"root_1","create_time":1710000000},'
        + '{"id":"root_text","time":"昨天"},'
        + '{"id":"root_missing"}]}}}}};',
    }],
  };
  t.after(() => { globalThis.document = savedDocument; });

  const ssr = readXhsSsrCommentPlatformTimes('note_1');
  assert.equal(ssr.present, true);
  assert.equal(ssr.commentCount, 3);
  assert.equal(ssr.epochTimes, 1);
  assert.equal(ssr.times.get('root_1'), 1710000000000);
  assert.equal(ssr.times.has('root_text'), false);

  const absent = readXhsSsrCommentPlatformTimes('note_2');
  assert.equal(absent.present, false);
  assert.equal(absent.commentCount, 0);
});

test('comment packages carry the bounded time probe as an opaque checkpoint', () => {
  const probe = {
    bridgeRequestedAt: 1710000000000,
    comments: { total: 2, withEpoch: 1, fromPassiveApi: 1, fromInitialState: 0, displayOnly: 1 },
  };
  const result = {
    noteId: 'note_1',
    comments: [{ commentId: 'root_1', level: 1, publishedAt: 1710000000000 }],
    commentTimeProbe: probe,
  };
  const packaged = packageComments({ platform: 'xhs', result, noteId: 'note_1' });
  assert.deepEqual(packaged.checkpoint, { commentTimeProbe: probe });

  const withoutProbe = packageComments({ platform: 'xhs', result: { noteId: 'note_1', comments: [] }, noteId: 'note_1' });
  assert.equal('checkpoint' in withoutProbe, false);
});

test('SSR comment reader unwraps Vue refs and reports only structural keys', async (t) => {
  const savedDocument = globalThis.document;
  globalThis.document = {
    scripts: [{
      textContent: 'window.__INITIAL_STATE__={"note":{"noteDetailMap":{"note_1":{"note":{"_rawValue":{"title":"合成标题"}},'
        + '"comments":{"_rawValue":{"list":{"__v_isRef":true,"_rawValue":[{"id":"root_1","create_time":1710000000}]},"cursor":""}}}}}}};',
    }],
  };
  t.after(() => { globalThis.document = savedDocument; });

  const ssr = readXhsSsrCommentPlatformTimes('note_1');
  assert.equal(ssr.present, true);
  assert.equal(ssr.commentCount, 1);
  assert.equal(ssr.times.get('root_1'), 1710000000000);
  assert.deepEqual(ssr.shape.entryKeys.sort(), ['comments', 'note']);
  assert.deepEqual([...ssr.shape.commentsKeys].sort(), ['cursor', 'list']);
  assert.equal(ssr.shape.commentsListKind, 'array');
  assert.equal(ssr.shape.commentsListLength, 1);
  assert.deepEqual([...ssr.shape.commentKeys].sort(), ['create_time', 'id']);
  assert.equal(JSON.stringify(ssr.shape).includes('合成标题'), false);
});
