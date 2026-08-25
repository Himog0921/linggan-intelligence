import test from 'node:test';
import assert from 'node:assert/strict';

import { BatchNoteController, mergeSurfaceCoverFallback } from '../src/platforms/xhs/batchController.js';
import { MSG } from '../src/shared/constants.js';

test('mergeSurfaceCoverFallback keeps a card cover when detail data has no image list', () => {
  const merged = mergeSurfaceCoverFallback(
    {
      noteId: 'note_video_1',
      title: '视频作品',
      cover: '',
      images: [],
    },
    {
      cover: 'https://sns-img.example.com/video-card.jpg',
      images: ['https://sns-img.example.com/video-card.jpg'],
    },
  );

  assert.equal(merged.cover, 'https://sns-img.example.com/video-card.jpg');
  assert.equal(merged.coverImg, 'https://sns-img.example.com/video-card.jpg');
  assert.equal(merged.coverUrl, 'https://sns-img.example.com/video-card.jpg');
  assert.deepEqual(merged.images, ['https://sns-img.example.com/video-card.jpg']);
});

test('mergeSurfaceCoverFallback uses nested image candidates when card cover is empty', () => {
  const merged = mergeSurfaceCoverFallback(
    {
      noteId: 'note_candidate_1',
      title: '候选图作品',
      cover: '',
      images: [],
    },
    {
      imageCandidates: [[{ url: 'https://sns-img.example.com/candidate-card.jpg' }]],
    },
  );

  assert.equal(merged.cover, 'https://sns-img.example.com/candidate-card.jpg');
  assert.equal(merged.coverImg, 'https://sns-img.example.com/candidate-card.jpg');
  assert.equal(merged.coverUrl, 'https://sns-img.example.com/candidate-card.jpg');
  assert.deepEqual(merged.images, ['https://sns-img.example.com/candidate-card.jpg']);
});

test('BatchNoteController reports a workbench record delta after collecting one note', () => {
  const messages = [];
  globalThis.chrome = {
    runtime: {
      id: 'extension-id',
      sendMessage: (message) => {
        messages.push(message);
      },
    },
  };

  try {
    const controller = new BatchNoteController();
    controller.collectionRunId = 'run_xhs_1';
    controller.externalTaskId = 'task_xhs_1';

    controller._reportCollectedNote({
      noteId: 'note_xhs_1',
      platformContentId: 'note_xhs_1',
      title: 'ADHD 采集样例',
      content: '一条实时写入内容工作台的笔记',
      url: 'https://www.xiaohongshu.com/explore/note_xhs_1',
      canonicalUrl: 'https://www.xiaohongshu.com/discovery/item/note_xhs_1?xsec_token=abc123',
      rawUrl: 'https://www.xiaohongshu.com/discovery/item/note_xhs_1?xsec_token=abc123',
      rawShareText: '复制这条小红书，打开【ADHD 采集样例】',
      cover: 'https://images.example.com/note_xhs_1.jpg',
      images: ['https://images.example.com/note_xhs_1.jpg'],
      imageCandidates: [['https://images.example.com/note_xhs_1.jpg']],
      video: 'https://videos.example.com/note_xhs_1.mp4',
      likes: '12',
      collects: 3,
      comments: 4,
      shares: 5,
      authorId: 'author_xhs_1',
      authorPlatformId: 'author_xhs_1',
      authorEntityId: 'xhs_author_xhs_1',
      authorName: '作者',
      authorAvatar: 'https://images.example.com/author.jpg',
      publishedAt: 1776766122,
      publishedAtText: '4月21日 18:08',
      type: 'video',
      dataSource: '__INITIAL_STATE__',
    });
  } finally {
    delete globalThis.chrome;
  }

  assert.equal(messages.length, 1);
  assert.equal(messages[0].action, MSG.WORKBENCH_RECORD_DELTA);
  assert.equal(messages[0].recordType, 'note');
  assert.equal(messages[0].externalRecordId, 'note_xhs_1');
  assert.equal(messages[0].collectionRunId, 'run_xhs_1');
  assert.equal(messages[0].externalTaskId, 'task_xhs_1');
  assert.match(messages[0].collectedAt, /^\d{4}-\d{2}-\d{2}T/);
  assert.deepEqual(messages[0].record, {
    platform: 'xhs',
    noteId: 'note_xhs_1',
    platformContentId: 'note_xhs_1',
    title: 'ADHD 采集样例',
    content: '一条实时写入内容工作台的笔记',
    url: 'https://www.xiaohongshu.com/explore/note_xhs_1',
    canonicalUrl: 'https://www.xiaohongshu.com/discovery/item/note_xhs_1?xsec_token=abc123',
    rawUrl: 'https://www.xiaohongshu.com/discovery/item/note_xhs_1?xsec_token=abc123',
    rawShareText: '复制这条小红书，打开【ADHD 采集样例】',
    cover: 'https://images.example.com/note_xhs_1.jpg',
    coverImg: 'https://images.example.com/note_xhs_1.jpg',
    coverUrl: 'https://images.example.com/note_xhs_1.jpg',
    images: ['https://images.example.com/note_xhs_1.jpg'],
    imageCandidates: [['https://images.example.com/note_xhs_1.jpg']],
    videoUrl: 'https://videos.example.com/note_xhs_1.mp4',
    likes: 12,
    collects: 3,
    comments: 4,
    shares: 5,
    authorId: 'author_xhs_1',
    authorPlatformId: 'author_xhs_1',
    authorEntityId: 'xhs_author_xhs_1',
    authorName: '作者',
    authorAvatar: 'https://images.example.com/author.jpg',
    publishedAt: 1776766122,
    publishedAtText: '4月21日 18:08',
    type: 'video',
    contentType: 'video',
    dataSource: '__INITIAL_STATE__',
    dataQuality: '',
    qualityReason: '',
    sourceTier: '',
  });
});

test('BatchNoteController waits for the final package when detail collection includes comments', () => {
  const messages = [];
  globalThis.chrome = {
    runtime: {
      id: 'extension-id',
      sendMessage: (message) => {
        messages.push(message);
      },
    },
  };

  try {
    const controller = new BatchNoteController();
    controller.collectionRunId = 'run_xhs_note_full';
    controller.externalTaskId = 'task_xhs_note_full';
    controller._includeComments = true;
    controller._commentLimit = 20;

    controller._reportCollectedNote({
      noteId: 'note_full_1',
      platformContentId: 'note_full_1',
      title: '完整详情采集',
      content: '正文',
    });
    controller._reportCollectedComment({
      commentId: 'comment_1',
      noteId: 'note_full_1',
      text: '评论',
    }, { noteId: 'note_full_1' });
  } finally {
    delete globalThis.chrome;
  }

  assert.equal(messages.length, 0);
});
