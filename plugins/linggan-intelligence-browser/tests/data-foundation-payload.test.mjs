import test from 'node:test';
import assert from 'node:assert/strict';

import { enrichNoteWithDataFoundationPayload } from '../src/workbench/runtime/dataFoundationPayload.js';

test('enrichNoteWithDataFoundationPayload attaches standard identity and evidence fields', () => {
  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'xhs',
    noteId: 'Note 001',
    type: 'note',
    authorId: 'Author 001',
    authorFans: '900',
    authorFansCollectedAt: 1776766122000,
    keywords: ['ADHD', '#作业'],
    hashtags: ['#学习方法', 'ADHD'],
    visualSummary: '画面总结',
    coverOcrText: '封面大字',
  }, {
    taskId: 'task-1',
    pluginRunId: 'run-1',
    externalRecordId: 'Note 001',
  });

  assert.equal(enriched.standardContentCode, 'cw-content:global:xhs:note:note001');
  assert.equal(enriched.standardAuthorCode, 'cw-author:global:xhs:author001');
  assert.equal(enriched.platformContentId, 'Note 001');
  assert.deepEqual(enriched.keywords, ['ADHD', '作业', '学习方法']);
  assert.equal(enriched.authorFans, 900);
  assert.equal(enriched.authorFansCollectedAt, '2026-04-21T10:08:42.000Z');
  assert.deepEqual(enriched.mediaUnderstanding, {
    visualSummary: '画面总结',
    ocrText: '封面大字',
  });
  assert.deepEqual(enriched.sourceRun, {
    source: 'plugin_task_delta',
    taskId: 'task-1',
    recordId: 'Note 001',
  });
  assert.deepEqual(enriched.dataFoundation.evidenceFields, {
    hasAuthorFans: true,
    hasKeywords: true,
    hasMediaUnderstanding: true,
  });
});

test('enrichNoteWithDataFoundationPayload infers Douyin video identity', () => {
  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'douyin',
    noteId: 'dy_7362',
    videoUrl: 'https://video.example.com/7362.mp4',
    authorPlatformId: 'douyin-author',
    searchKeyword: '注意力',
  }, {
    taskId: 'task-2',
    externalRecordId: 'dy_7362',
  });

  assert.equal(enriched.contentType, 'video');
  assert.equal(enriched.platformContentId, '7362');
  assert.equal(enriched.standardContentCode, 'cw-content:global:douyin:video:7362');
  assert.equal(enriched.standardAuthorCode, 'cw-author:global:douyin:douyin-author');
  assert.deepEqual(enriched.keywords, ['注意力']);
});

test('enrichNoteWithDataFoundationPayload emits canonical media source fields for a captured XHS video', () => {
  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'xhs',
    noteId: '6a68a020000000000f02a43b',
    type: 'video',
    cover: 'https://sns-img.example.com/original-cover.webp',
    images: [
      'https://sns-img.example.com/original-cover.webp',
      'https://sns-img.example.com/detail-1.webp',
    ],
    video: 'https://sns-video.example.com/stream.mp4',
    videoStreams: [{ url: 'https://sns-video.example.com/stream.mp4', quality: '720p' }],
  });

  assert.equal(enriched.coverUrl, 'https://sns-img.example.com/original-cover.webp');
  assert.deepEqual(enriched.imageUrls, [
    'https://sns-img.example.com/original-cover.webp',
    'https://sns-img.example.com/detail-1.webp',
  ]);
  assert.equal(enriched.videoUrl, 'https://sns-video.example.com/stream.mp4');
  assert.equal(enriched.cover, undefined);
  assert.equal(enriched.images, undefined);
  assert.equal(enriched.video, undefined);
  assert.equal(enriched.videoStreams, undefined);
});

test('enrichNoteWithDataFoundationPayload keeps one image slot for a candidate group with fallback URLs', () => {
  const cover = 'https://sns-img.example.com/original-cover.webp';
  const fallback = 'https://sns-img.example.com/original-cover-fallback.webp';

  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'xhs',
    noteId: '68fb9898000000000402084f',
    type: 'video',
    cover,
    images: [cover],
    // 一个图片槽位的两个地址是主地址和备用地址，不能被传输协议误写成两张图片。
    imageCandidates: [[cover, fallback]],
    video: 'https://sns-video.example.com/stream.mp4',
  });

  assert.deepEqual(enriched.imageUrls, [cover]);
  assert.deepEqual(enriched.rawData.imageCandidates, [[cover, fallback]]);
});

test('enrichNoteWithDataFoundationPayload treats a flat candidate list as fallback URLs for one image slot', () => {
  const primary = 'https://sns-img.example.com/candidate-primary.webp';
  const fallback = 'https://sns-img.example.com/candidate-fallback.webp';

  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'douyin',
    noteId: '7350000000000000001',
    imageCandidates: [primary, fallback],
  });

  assert.deepEqual(enriched.imageUrls, [primary]);
});

test('enrichNoteWithDataFoundationPayload preserves the position of multiple candidate groups', () => {
  const firstPrimary = 'https://sns-img.example.com/first-primary.webp';
  const secondPrimary = 'https://sns-img.example.com/second-primary.webp';

  const enriched = enrichNoteWithDataFoundationPayload({
    platform: 'xhs',
    noteId: '68fb98980000000004020850',
    imageCandidates: [
      [firstPrimary, 'https://sns-img.example.com/first-fallback.webp'],
      [secondPrimary, 'https://sns-img.example.com/second-fallback.webp'],
    ],
  });

  assert.deepEqual(enriched.imageUrls, [firstPrimary, secondPrimary]);
});

test('enrichNoteWithDataFoundationPayload parses compact platform fan counts', () => {
  assert.equal(enrichNoteWithDataFoundationPayload({ noteId: 'a', authorFans: '1.2万' }).authorFans, 12000);
  assert.equal(enrichNoteWithDataFoundationPayload({ noteId: 'b', authorFans: '3w' }).authorFans, 30000);
  assert.equal(enrichNoteWithDataFoundationPayload({ noteId: 'c', authorFans: '8千' }).authorFans, 8000);
  assert.equal(enrichNoteWithDataFoundationPayload({ noteId: 'd', authorFans: '2.5k followers' }).authorFans, 2500);
  assert.equal(enrichNoteWithDataFoundationPayload({ noteId: 'e', authorFans: '1,234 粉丝' }).authorFans, 1234);
});
