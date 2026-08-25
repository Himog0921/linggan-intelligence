import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MONITOR_RECORD_MODE,
  MONITOR_TASK_STRATEGY,
} from '../src/workbench/protocol/schema.js';
import { mapTaskEnvelopeToInternalCommand } from '../src/workbench/runtime/taskEnvelopeMapper.js';
import {
  buildDouyinSurfaceNoteRecords,
  buildMonitorTaskMeta,
  withMonitorRecordMeta,
} from '../src/workbench/runtime/monitorTask.js';
import { filterTargetedXhsNoteList } from '../src/platforms/xhs/batchController.js';

test('detail probe task keeps detail collection enabled and tags records as detail_probe', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'monitor_task_probe_1',
    taskType: 'xhs.batchNotes',
    platform: 'xhs',
    taskStrategy: MONITOR_TASK_STRATEGY.DETAIL_PROBE,
    triggerSource: 'collection_task_poller',
    target: {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/user/profile/demo_author/note_1',
    },
    payload: {
      monitorId: 'monitor_author_1',
      taskStrategy: MONITOR_TASK_STRATEGY.DETAIL_PROBE,
      platformContentId: 'note_1',
      detailProbeLimit: 5,
      scanLimit: 50,
    },
  }, { tabId: 9 });

  assert.equal(command.payload.surfaceOnly, false);
  assert.equal(command.payload.count, 1);
  assert.equal(command.payload.mode, 'detail');
  assert.equal(command.payload.targetNoteId, 'note_1');
  assert.equal(command.payload.monitorMeta.monitorMode, MONITOR_RECORD_MODE.DETAIL_PROBE);
  assert.equal(command.payload.externalTaskMeta.monitorMeta.monitorId, 'monitor_author_1');
  assert.equal(command.payload.externalTaskMeta.monitorMeta.targetNoteId, 'note_1');

  const tagged = withMonitorRecordMeta({
    noteId: 'note_1',
    title: '补全详情后的笔记',
    comments: 312,
  }, command.payload.monitorMeta);

  assert.equal(tagged.monitorMode, MONITOR_RECORD_MODE.DETAIL_PROBE);
  assert.equal(tagged.monitorId, 'monitor_author_1');
  assert.equal(tagged.taskStrategy, MONITOR_TASK_STRATEGY.DETAIL_PROBE);
  assert.equal(tagged.comments, 312);
});

test('detail probe only keeps the targeted xhs note when author-page relay is used', () => {
  const filtered = filterTargetedXhsNoteList([
    { noteId: 'note_1', title: '目标作品' },
    { noteId: 'note_2', title: '无关作品' },
  ], 'note_1');

  assert.deepEqual(filtered, [
    { noteId: 'note_1', title: '目标作品' },
  ]);
});

test('keyword patrol task maps to a surface scan and creates keyword surface records', () => {
  const monitorMeta = buildMonitorTaskMeta({
    platform: 'douyin',
    taskType: 'douyin.batchNotes',
    taskStrategy: MONITOR_TASK_STRATEGY.KEYWORD_PATROL,
    payload: {
      monitorId: 'monitor_keyword_1',
      taskStrategy: MONITOR_TASK_STRATEGY.KEYWORD_PATROL,
      keyword: '数学思维',
      scanLimit: 2,
    },
    target: {
      pageType: 'search',
      url: 'https://www.douyin.com/search/%E6%95%B0%E5%AD%A6%E6%80%9D%E7%BB%B4',
    },
  });

  assert.equal(monitorMeta.surfaceOnly, true);
  assert.equal(monitorMeta.surfaceMode, MONITOR_RECORD_MODE.KEYWORD_SURFACE);

  const records = buildDouyinSurfaceNoteRecords([
    {
      awemeId: '7260000000000000001',
      href: 'https://www.douyin.com/video/7260000000000000001',
      titleHint: '数学思维爆款',
      likes: '1.2万',
      comments: '312',
      authorHint: '李老师',
    },
    {
      awemeId: '7260000000000000002',
      href: 'https://www.douyin.com/video/7260000000000000002',
      titleHint: '数学启蒙',
      likes: 88,
      comments: 4,
    },
  ], {
    monitorMeta,
    collectionRunId: 'run_keyword_surface_1',
    mode: MONITOR_RECORD_MODE.KEYWORD_SURFACE,
    limit: 1,
    searchKeyword: '数学思维',
    searchPageUrl: 'https://www.douyin.com/search/%E6%95%B0%E5%AD%A6%E6%80%9D%E7%BB%B4',
  });

  assert.equal(records.length, 1);
  assert.equal(records[0].platform, 'douyin');
  assert.equal(records[0].monitorMode, MONITOR_RECORD_MODE.KEYWORD_SURFACE);
  assert.equal(records[0].monitorId, 'monitor_keyword_1');
  assert.equal(records[0].taskStrategy, MONITOR_TASK_STRATEGY.KEYWORD_PATROL);
  assert.equal(records[0].noteId, '7260000000000000001');
  assert.equal(records[0].platformContentId, '7260000000000000001');
  assert.equal(records[0].contentId, 'dy_7260000000000000001');
  assert.equal(records[0].title, '数学思维爆款');
  assert.equal(records[0].likes, 12000);
  assert.equal(records[0].comments, 312);
  assert.equal(records[0].searchKeyword, '数学思维');
  assert.equal(records[0].dataQuality, 'seed');
  assert.equal(records[0].qualityReason, 'monitor_surface_seed');
  assert.equal(records[0].sourceTier, 'seed');
});
