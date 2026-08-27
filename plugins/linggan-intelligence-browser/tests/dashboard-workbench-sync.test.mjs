import test from 'node:test';
import assert from 'node:assert/strict';

import {
  DASHBOARD_SYNC_TO_WORKBENCH_TIMEOUT_MS,
  buildWorkbenchSyncPayload,
  summarizeWorkbenchSyncResult,
} from '../src/dashboard/utils.js';

test('buildWorkbenchSyncPayload preserves quality fields for manual workbench sync', () => {
  const extractedAt = 1776671494763;

  const notesPayload = buildWorkbenchSyncPayload('notes', [{
    noteId: 'note_1',
    title: '标题',
    content: '正文',
    platform: 'douyin',
    url: 'https://www.douyin.com/video/1',
    dataQuality: 'seed',
    qualityReason: 'search_summary_seed',
    sourceTier: 'seed',
    collectionRunId: 'run_note_1',
  }], { extractedAt });

  const commentsPayload = buildWorkbenchSyncPayload('comments', [{
    commentId: 'comment_1',
    contentId: 'xhs_note_1',
    text: '评论',
    author: 'alice',
    platform: 'xhs',
    noteUrl: 'https://www.xiaohongshu.com/explore/note_1?xsec_token=token',
    dataQuality: 'full',
    qualityReason: '',
    sourceTier: 'api',
    collectionRunId: 'run_comment_1',
  }], { extractedAt });

  const authorsPayload = buildWorkbenchSyncPayload('authors', [{
    userId: 'author_1',
    platform: 'xhs',
    nickname: '作者',
    profileUrl: 'https://www.xiaohongshu.com/user/profile/author_1',
    dataQuality: 'degraded',
    qualityReason: 'inject_failed_dom_only',
    sourceTier: 'dom',
    collectionRunId: 'run_author_1',
  }], { extractedAt });

  assert.equal(notesPayload.notes[0].dataQuality, 'seed');
  assert.equal(notesPayload.notes[0].qualityReason, 'search_summary_seed');
  assert.equal(notesPayload.notes[0].sourceTier, 'seed');
  assert.equal(notesPayload.notes[0].collectionRunId, 'run_note_1');
  assert.equal(notesPayload.notes[0].extractedAt, extractedAt);

  assert.equal(commentsPayload.comments[0].dataQuality, 'full');
  assert.equal(commentsPayload.comments[0].qualityReason, '');
  assert.equal(commentsPayload.comments[0].sourceTier, 'api');
  assert.equal(commentsPayload.comments[0].collectionRunId, 'run_comment_1');
  assert.equal(commentsPayload.comments[0].extractedAt, extractedAt);

  assert.equal(authorsPayload.authors[0].dataQuality, 'degraded');
  assert.equal(authorsPayload.authors[0].qualityReason, 'inject_failed_dom_only');
  assert.equal(authorsPayload.authors[0].sourceTier, 'dom');
  assert.equal(authorsPayload.authors[0].collectionRunId, 'run_author_1');
  assert.equal(authorsPayload.authors[0].extractedAt, extractedAt);
});

test('buildWorkbenchSyncPayload keeps rich note media fields for manual workbench sync', () => {
  const payload = buildWorkbenchSyncPayload('notes', [{
    platform: 'douyin',
    platformContentId: '9001',
    contentId: 'dy_9001',
    title: '视频标题',
    content: '视频正文',
    cover: 'https://img.example.com/cover.webp',
    coverImg: 'https://img.example.com/cover.webp',
    images: ['https://img.example.com/cover.webp'],
    imageCandidates: [{ url: 'https://img.example.com/origin.webp', quality: 'origin' }],
    videoDownloadUrl: 'https://video.example.com/download.mp4',
    videoPlayUrl: 'https://video.example.com/play.mp4',
    videoStreams: [{ url: 'https://video.example.com/stream.mp4', quality: '720p' }],
    mediaDownloadStatus: '已完成',
    rawUrl: 'https://www.douyin.com/video/9001',
  }], { extractedAt: 1776671494763 });

  assert.equal(payload.notes[0].noteId, 'dy_9001');
  assert.equal(payload.notes[0].platformContentId, '9001');
  assert.equal(payload.notes[0].contentId, 'dy_9001');
  assert.equal(payload.notes[0].coverImg, 'https://img.example.com/cover.webp');
  assert.deepEqual(payload.notes[0].imageCandidates, [{ url: 'https://img.example.com/origin.webp', quality: 'origin' }]);
  assert.equal(payload.notes[0].videoDownloadUrl, 'https://video.example.com/download.mp4');
  assert.deepEqual(payload.notes[0].videoStreams, [{ url: 'https://video.example.com/stream.mp4', quality: '720p' }]);
  assert.equal(payload.notes[0].mediaDownloadStatus, '已完成');
});

test('dashboard workbench sync waits long enough for larger comment batches', () => {
  assert.equal(DASHBOARD_SYNC_TO_WORKBENCH_TIMEOUT_MS, 45000);
});

test('dashboard workbench sync summarizes comment imports from comment metadata', () => {
  const summary = summarizeWorkbenchSyncResult('comments', 50, {
    success: true,
    imported: 0,
    skipped: 0,
    meta: {
      commentsReceived: 50,
      commentsRegistered: 48,
      commentsProcessed: 47,
      commentsQueued: 0,
      commentsFailed: 1,
      commentsSkipped: 2,
      commentsInvalid: 0,
    },
  });

  assert.equal(summary.imported, 47);
  assert.equal(summary.skipped, 2);
  assert.equal(summary.invalid, 0);
  assert.equal(summary.total, 50);
  assert.equal(summary.detailText, '（评论 50）');
  assert.equal(summary.outcomeText, '已接收 50 条，已入库 47 条，已跳过 2 条，待重试 1 条');
});

test('dashboard workbench sync identifies synced authors as monitor sources', () => {
  const summary = summarizeWorkbenchSyncResult('authors', 2, {
    success: true,
    imported: 2,
    skipped: 0,
    meta: {
      authorsReceived: 2,
      authorsIngested: 1,
      authorsSkipped: 1,
      monitorSourcesCreated: 1,
      monitorSourcesExisting: 0,
      monitorSourcesSkipped: 1,
    },
  });

  assert.equal(summary.imported, 1);
  assert.equal(summary.skipped, 1);
  assert.equal(summary.monitorOutcomeConfirmed, true);
  assert.equal(summary.outcomeText, '已新增监控来源 1 条，未创建 1 条');
});

test('dashboard workbench sync distinguishes media registration from completed media delivery', () => {
  const summary = summarizeWorkbenchSyncResult('notes', 1, {
    success: true,
    imported: 1,
    skipped: 0,
    meta: {
      mediaRegistrationConfirmed: true,
      notesReceived: 1,
      mediaObserved: 2,
      mediaEnqueued: 2,
      mediaRequiredMissing: 1,
      mediaUnlinked: 0,
      mediaInvalid: 0,
      mediaConflicted: 0,
      mediaRejected: 0,
    },
  });

  assert.equal(summary.mediaRegistrationConfirmed, true);
  assert.equal(summary.mediaIncomplete, true);
  assert.equal(summary.outcomeText, '媒体已登记 2 项，已进入处理队列 2 项，媒体登记异常 1 项');
});

test('dashboard workbench sync does not infer media registration confirmation from counts', () => {
  const summary = summarizeWorkbenchSyncResult('notes', 1, {
    success: true,
    imported: 1,
    skipped: 0,
    meta: {
      mediaObserved: 2,
      mediaEnqueued: 2,
    },
  });

  assert.equal(summary.mediaRegistrationConfirmed, false);
  assert.match(summary.outcomeText, /工作台未确认媒体账本登记/);
});
