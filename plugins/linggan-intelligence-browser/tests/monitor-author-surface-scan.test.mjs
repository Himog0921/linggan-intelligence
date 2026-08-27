import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildMonitorTaskMeta,
  buildDouyinSurfaceNoteRecords,
  buildXhsSurfaceNoteRecords,
} from '../src/workbench/runtime/monitorTask.js';
import { buildDouyinBatchTargetFromAweme } from '../src/platforms/douyin/batchDiscovery.js';
import {
  MONITOR_RECORD_MODE,
  MONITOR_TASK_STRATEGY,
} from '../src/workbench/protocol/schema.js';
import { mapTaskEnvelopeToInternalCommand } from '../src/workbench/runtime/taskEnvelopeMapper.js';

test('author baseline task maps to a surface-only scan using the configured scan limit', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'monitor_task_author_baseline_1',
    taskType: 'xhs.collectAuthor',
    platform: 'xhs',
    taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
    triggerSource: 'collection_task_poller',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/user_1',
    },
    payload: {
      monitorId: 'monitor_author_1',
      taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
      scanLimit: 3,
      detailProbeLimit: 1,
      monitorStrength: 'standard',
      display: { name: '回甘' },
    },
  }, { tabId: 7 });

  assert.equal(command.payload.surfaceOnly, true);
  assert.equal(command.payload.count, 3);
  assert.equal(command.payload.monitorMeta.monitorId, 'monitor_author_1');
  assert.equal(command.payload.monitorMeta.taskStrategy, MONITOR_TASK_STRATEGY.AUTHOR_BASELINE);
  assert.equal(command.payload.monitorMeta.surfaceMode, MONITOR_RECORD_MODE.AUTHOR_SURFACE);
  assert.equal(command.payload.externalTaskMeta.monitorMeta.monitorId, 'monitor_author_1');
  assert.equal(command.taskMeta.monitorMeta.taskStrategy, MONITOR_TASK_STRATEGY.AUTHOR_BASELINE);
});

test('author baseline task defaults scan limit to 50 when workbench payload omits it', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'monitor_task_author_baseline_default_1',
    taskType: 'xhs.collectAuthor',
    platform: 'xhs',
    taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
    triggerSource: 'collection_task_poller',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/user_1',
    },
    payload: {
      monitorId: 'monitor_author_default_1',
      taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
      detailProbeLimit: 10,
    },
  }, { tabId: 7 });

  assert.equal(command.payload.count, 50);
  assert.equal(command.payload.monitorMeta.scanLimit, 50);
  assert.equal(command.payload.monitorMeta.limit, 50);
});

test('author patrol task defaults scan limit to 10 when workbench payload omits it', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'monitor_task_author_patrol_default_1',
    taskType: 'xhs.collectAuthor',
    platform: 'xhs',
    taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_PATROL,
    triggerSource: 'collection_task_poller',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/user_1',
    },
    payload: {
      monitorId: 'monitor_author_patrol_default_1',
      taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_PATROL,
      detailProbeLimit: 3,
    },
  }, { tabId: 7 });

  assert.equal(command.payload.count, 10);
  assert.equal(command.payload.monitorMeta.scanLimit, 10);
  assert.equal(command.payload.monitorMeta.limit, 10);
});

test('xhs author surface records stop at scanLimit and carry monitor-shaped metadata', () => {
  const monitorMeta = buildMonitorTaskMeta({
    platform: 'xhs',
    taskType: 'xhs.collectAuthor',
    taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
    payload: {
      monitorId: 'monitor_author_1',
      scanLimit: 2,
      detailProbeLimit: 1,
      display: { name: '回甘' },
    },
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/user_1',
    },
  });

  const records = buildXhsSurfaceNoteRecords([
    { noteId: 'note_1', url: '/explore/note_1', title: '第一条', likes: '5567', type: 'normal' },
    { noteId: 'note_2', url: 'https://www.xiaohongshu.com/explore/note_2', title: '第二条', likes: '2' },
    { noteId: 'note_3', url: '/explore/note_3', title: '第三条', likes: '3' },
  ], {
    monitorMeta,
    collectionRunId: 'run_author_surface_1',
    mode: MONITOR_RECORD_MODE.AUTHOR_SURFACE,
    limit: 2,
    sourcePageUrl: 'https://www.xiaohongshu.com/user/profile/user_1',
  });

  assert.equal(records.length, 2);
  assert.deepEqual(records.map((record) => record.noteId), ['note_1', 'note_2']);
  assert.equal(records[0].monitorMode, MONITOR_RECORD_MODE.AUTHOR_SURFACE);
  assert.equal(records[0].monitorId, 'monitor_author_1');
  assert.equal(records[0].taskStrategy, MONITOR_TASK_STRATEGY.AUTHOR_BASELINE);
  assert.equal(records[0].collectionRunId, 'run_author_surface_1');
  assert.equal(records[0].dataSource, 'monitor_surface_card');
  assert.equal(records[0].dataQuality, 'seed');
  assert.equal(records[0].qualityReason, 'monitor_surface_seed');
  assert.equal(records[0].sourceTier, 'seed');
  assert.equal(records[0].platform, 'xhs');
  assert.equal(records[0].targetKey, 'xhs:note:note_1');
  assert.equal(records[0].platformContentId, 'note_1');
  assert.equal(records[0].contentId, 'xhs_note_1');
  assert.equal(records[0].url, 'https://www.xiaohongshu.com/explore/note_1');
  assert.equal(records[0].rawUrl, 'https://www.xiaohongshu.com/explore/note_1');
  assert.equal(records[0].sourceUrl, 'https://www.xiaohongshu.com/user/profile/user_1');
  assert.equal(records[0].runnableDetailUrl, 'https://www.xiaohongshu.com/discovery/item/note_1?source=webshare&xhsshare=pc_web&xsec_source=pc_share');
  assert.equal(records[0].likes, 5567);
  assert.equal(records[0].likeCount, 5567);
  assert.equal(records[0].rank, 1);
  assert.equal(records[0].batchRank, 1);
  assert.equal(records[0].publicCommentCount, null);
  assert.equal(records[0].publicCommentCountKnown, false);
  assert.equal(records[0].monitorMeta.monitorId, 'monitor_author_1');
});

test('xhs author surface records mark explicit public comment counts', () => {
  const records = buildXhsSurfaceNoteRecords([
    { noteId: 'note_known_zero', title: '明确 0 评论', comments: '0' },
  ], {
    limit: 1,
  });

  assert.equal(records[0].comments, 0);
  assert.equal(records[0].publicCommentCount, 0);
  assert.equal(records[0].publicCommentCountKnown, true);
});

test('xhs author surface records rewrite profile-style note links to canonical explore detail urls', () => {
  const records = buildXhsSurfaceNoteRecords([
    {
      noteId: '680123456789abcdef012345',
      url: '/user/profile/5f1234567890abcd12345678/680123456789abcdef012345',
      title: '详情补采候选',
    },
  ], {
    limit: 1,
  });

  assert.equal(records[0].url, 'https://www.xiaohongshu.com/explore/680123456789abcdef012345');
  assert.equal(records[0].canonicalUrl, 'https://www.xiaohongshu.com/explore/680123456789abcdef012345');
});

test('xhs author surface records preserve media candidates for workbench previews', () => {
  const records = buildXhsSurfaceNoteRecords([
    {
      noteId: 'note_with_candidate_cover',
      title: '候选封面',
      imageCandidates: [[{ url: 'https://sns-img.example.com/candidate-cover.jpg' }]],
    },
  ], {
    limit: 1,
  });

  assert.equal(records[0].cover, 'https://sns-img.example.com/candidate-cover.jpg');
  assert.equal(records[0].coverImg, 'https://sns-img.example.com/candidate-cover.jpg');
  assert.equal(records[0].coverUrl, 'https://sns-img.example.com/candidate-cover.jpg');
  assert.equal(records[0].thumbnail, 'https://sns-img.example.com/candidate-cover.jpg');
  assert.deepEqual(records[0].images, ['https://sns-img.example.com/candidate-cover.jpg']);
  assert.deepEqual(records[0].imageCandidates, [[{ url: 'https://sns-img.example.com/candidate-cover.jpg' }]]);
});

test('douyin author surface records preserve aweme cover candidates for workbench previews', () => {
  const target = buildDouyinBatchTargetFromAweme({
    aweme_id: '7601151661356158258',
    desc: '抖音封面候选',
    statistics: {
      digg_count: 66,
      comment_count: 23,
    },
    video: {
      cover: {
        url_list: ['https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg'],
      },
      dynamic_cover: {
        url_list: ['https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-b.webp'],
      },
    },
  }, 0);

  const records = buildDouyinSurfaceNoteRecords([target], {
    limit: 1,
  });

  assert.equal(target.coverImg, 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg');
  assert.equal(records[0].cover, 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg');
  assert.equal(records[0].coverImg, 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg');
  assert.equal(records[0].coverUrl, 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg');
  assert.equal(records[0].thumbnail, 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg');
  assert.deepEqual(records[0].images, ['https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg']);
  assert.deepEqual(records[0].imageCandidates, [
    { url: 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg' },
    { url: 'https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-b.webp' },
  ]);
});

test('douyin author surface records carry publish time and monitored author identity', () => {
  const monitorMeta = buildMonitorTaskMeta({
    platform: 'douyin',
    taskType: 'douyin.collectAuthor',
    taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
    payload: {
      monitorId: 'monitor_author_douyin_1',
      platformAuthorId: 'dy_MS4wLjABAAAAuthor1',
      authorEntityId: 'author-row-1',
      authorName: '懿直成长',
      scanLimit: 20,
    },
    target: {
      pageType: 'profile',
      url: 'https://www.douyin.com/user/MS4wLjABAAAAuthor1',
    },
  });

  const target = buildDouyinBatchTargetFromAweme({
    aweme_id: '7630831732593437428',
    desc: '多动症孩子规则建立',
    create_time: 1779617280,
    statistics: {
      digg_count: 38,
      comment_count: 19,
    },
    video: {
      cover: {
        url_list: ['https://p3-sign.douyinpic.com/tos-cn-cover-main/cover-a.jpeg'],
      },
    },
  }, 0);

  const records = buildDouyinSurfaceNoteRecords([target], {
    monitorMeta,
    collectionRunId: 'run_douyin_author_surface_1',
    mode: MONITOR_RECORD_MODE.AUTHOR_SURFACE,
    limit: 1,
  });

  assert.equal(records.length, 1);
  assert.equal(records[0].platformContentId, '7630831732593437428');
  assert.equal(records[0].publishedAt, 1779617280);
  assert.equal(records[0].createTime, 1779617280);
  assert.equal(records[0].authorPlatformId, 'dy_MS4wLjABAAAAuthor1');
  assert.equal(records[0].platformAuthorId, 'dy_MS4wLjABAAAAuthor1');
  assert.equal(records[0].authorEntityId, 'author-row-1');
  assert.equal(records[0].authorName, '懿直成长');
  assert.equal(records[0].profileUrl, 'https://www.douyin.com/user/MS4wLjABAAAAuthor1');
});

test('xhs author surface records keep signed share links when xsec_token is already present', () => {
  const signedUrl = 'https://www.xiaohongshu.com/user/profile/5f1234567890abcd12345678/680123456789abcdef012345?xsec_token=abc123';
  const records = buildXhsSurfaceNoteRecords([
    {
      noteId: '680123456789abcdef012345',
      url: signedUrl,
      title: '带签名链接',
    },
  ], {
    limit: 1,
  });

  assert.equal(records[0].url, signedUrl);
  assert.equal(records[0].canonicalUrl, signedUrl);
});
