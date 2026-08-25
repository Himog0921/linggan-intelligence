import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildCommentImageAssets,
  countCommentImageAssets,
  extractImageCandidatesFromComment,
  fetchImageBlob,
} from '../src/platforms/douyin/commentMedia.js';

test('extractImageCandidatesFromComment prefers origin/download urls and keeps normalized candidates', () => {
  globalThis.window = {
    location: {
      protocol: 'https:',
      origin: 'https://www.douyin.com',
    },
  };

  const candidates = extractImageCandidatesFromComment({
    image_list: [
      {
        origin_url: { url_list: ['//p3-sign.douyinpic.com/high.webp?x=1'] },
        download_url: { url_list: ['https://p26-sign.douyinpic.com/download.webp'] },
        medium_url: { url_list: ['https://p6-sign.douyinpic.com/medium.webp'] },
        thumb_url: { url_list: ['https://p6-sign.douyinpic.com/thumb.webp'] },
      },
    ],
  });

  assert.deepEqual(candidates, [[
    'https://p3-sign.douyinpic.com/high.webp?x=1',
    'https://p3-sign.douyinpic.com/high.webp',
    'https://p26-sign.douyinpic.com/download.webp',
    'https://p6-sign.douyinpic.com/medium.webp',
    'https://p6-sign.douyinpic.com/thumb.webp',
  ]]);
});

test('buildCommentImageAssets maps comment images to media assets with stable ids', () => {
  globalThis.window = {
    location: {
      protocol: 'https:',
      origin: 'https://www.douyin.com',
    },
  };

  const assets = buildCommentImageAssets(
    [
      {
        contentId: 'video_1',
        commentId: 'comment_1',
        commentEntityId: 'dy_comment_1',
      },
    ],
    [
      {
        image_list: [
          {
            origin_url: { url_list: ['https://p3-sign.douyinpic.com/high-1.webp'] },
          },
          {
            download_url: { url_list: ['https://p3-sign.douyinpic.com/high-2.webp'] },
          },
        ],
      },
    ],
    { collectionRunId: 'run_comment_images_1' },
  );

  assert.equal(assets.length, 2);
  assert.deepEqual(assets[0], {
    assetId: 'dy_comment_image_video_1_comment_1_1',
    contentId: 'video_1',
    commentEntityId: 'dy_comment_1',
    commentId: 'comment_1',
    assetType: 'comment_image',
    role: 'primary',
    url: 'https://p3-sign.douyinpic.com/high-1.webp',
    candidateUrls: ['https://p3-sign.douyinpic.com/high-1.webp'],
    quality: 'unknown',
    collectionRunId: 'run_comment_images_1',
    downloadStatus: '待下载',
    lastResolvedAt: assets[0].lastResolvedAt,
    createdAt: assets[0].createdAt,
  });
  assert.equal(assets[1].assetId, 'dy_comment_image_video_1_comment_1_2');
  assert.equal(assets[1].role, 'fallback');
  assert.equal(assets[1].url, 'https://p3-sign.douyinpic.com/high-2.webp');
});

test('comment image counting dedupes duplicated logical comments with synthetic ids', () => {
  globalThis.window = {
    location: {
      protocol: 'https:',
      origin: 'https://www.douyin.com',
    },
  };

  const duplicatedRawComment = {
    user: { uid: 'author_1' },
    text: '同一条无 cid 的图片评论',
    create_time: 1711111111,
    image_list: [
      {
        origin_url: { url_list: ['https://p3-sign.douyinpic.com/high-1.webp'] },
      },
      {
        origin_url: { url_list: ['https://p3-sign.douyinpic.com/high-2.webp'] },
      },
    ],
  };

  const commentRecords = [
    {
      contentId: 'video_1',
      commentId: 'dy_fb_aweme_1_1',
      commentEntityId: 'dy_comment_1',
      authorId: 'author_1',
      text: '同一条无 cid 的图片评论',
      publishedAt: 1711111111000,
      qualityReason: 'synthetic_comment_id',
    },
    {
      contentId: 'video_1',
      commentId: 'dy_fb_aweme_1_9',
      commentEntityId: 'dy_comment_9',
      authorId: 'author_1',
      text: '同一条无 cid 的图片评论',
      publishedAt: 1711111111000,
      qualityReason: 'synthetic_comment_id',
    },
  ];

  const rawComments = [duplicatedRawComment, duplicatedRawComment];

  assert.equal(countCommentImageAssets(commentRecords, rawComments), 2);

  const assets = buildCommentImageAssets(commentRecords, rawComments, {
    collectionRunId: 'run_comment_images_dedup',
  });

  assert.equal(assets.length, 2);
  assert.equal(assets[0].assetId, 'dy_comment_image_video_1_dy_fb_aweme_1_1_1');
  assert.equal(assets[1].assetId, 'dy_comment_image_video_1_dy_fb_aweme_1_1_2');
});

test('fetchImageBlob skips non-image blobs and falls back to later image candidates', async () => {
  const fetchCalls = [];
  globalThis.fetch = async (url) => {
    fetchCalls.push(url);
    if (String(url).includes('html-error')) {
      return {
        ok: true,
        async blob() {
          return new Blob(['<html>blocked</html>'], { type: 'text/html' });
        },
      };
    }
    return {
      ok: true,
      async blob() {
        return new Blob(['image-binary'], { type: 'image/webp' });
      },
    };
  };

  const result = await fetchImageBlob([
    'https://example.com/html-error',
    'https://example.com/high.webp',
  ]);

  assert.equal(result.success, true);
  assert.equal(result.candidate, 'https://example.com/high.webp');
  assert.equal(result.candidateIndex, 1);
  assert.deepEqual(fetchCalls, [
    'https://example.com/html-error',
    'https://example.com/high.webp',
  ]);
  assert.equal(result.blob.type, 'image/webp');
});

test('fetchImageBlob falls back to background data url when runtime fetches fail', async () => {
  const runtimeMessages = [];
  globalThis.chrome = {
    runtime: {
      id: 'test-extension',
      lastError: null,
      sendMessage(payload, callback) {
        runtimeMessages.push(payload);
        callback({
          success: true,
          dataUrl: 'data:image/png;base64,ZmFrZQ==',
          candidate: 'https://example.com/fallback.png',
          candidateIndex: 0,
        });
      },
    },
  };

  globalThis.fetch = async (url) => {
    if (String(url).startsWith('data:image/png')) {
      return {
        ok: true,
        async blob() {
          return new Blob(['fallback-image'], { type: 'image/png' });
        },
      };
    }
    throw new Error('network_failed');
  };

  const result = await fetchImageBlob(['https://example.com/expired.png']);

  assert.equal(result.success, true);
  assert.equal(result.candidate, 'https://example.com/fallback.png');
  assert.equal(result.candidateIndex, 0);
  assert.equal(result.blob.type, 'image/png');
  assert.deepEqual(runtimeMessages, [
    {
      action: 'fetchBinaryAsDataUrl',
      candidates: ['https://example.com/expired.png'],
    },
  ]);
});
