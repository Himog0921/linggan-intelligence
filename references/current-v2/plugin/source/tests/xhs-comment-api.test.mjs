import test from 'node:test';
import assert from 'node:assert/strict';

import {
  parseXhsCommentPagePayload,
  buildXhsCommentsFromSnapshot,
  hydrateXhsCommentSnapshot,
} from '../src/platforms/xhs/commentApi.js';

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
  assert.equal(main.rawSource, 'xhs.comments.api');
  assert.equal(main.dataQuality, 'full');
  assert.equal(main.qualityReason, '');
  assert.equal(main.sourceTier, 'api');

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
