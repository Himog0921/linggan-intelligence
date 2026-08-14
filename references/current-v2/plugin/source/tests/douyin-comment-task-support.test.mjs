import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildDouyinSingleCommentRunPatch,
  resolveDouyinSingleCommentUiTotal,
} from '../src/platforms/douyin/commentTaskSupport.js';
import { describeTaskProgress } from '../src/shared/taskUi.js';

test('buildDouyinSingleCommentRunPatch keeps partial success semantics when stopped after collecting comments', () => {
  const patch = buildDouyinSingleCommentRunPatch({
    stopped: true,
    totalComments: 18,
    note: {
      contentId: 'dy_content_1',
      platformContentId: 'video_1',
    },
  });

  assert.deepEqual(patch, {
    itemsPlanned: 1,
    itemsSucceeded: 1,
    itemsFailed: 0,
    totalComments: 18,
    contentId: 'dy_content_1',
    targetIds: ['video_1'],
  });
});

test('buildDouyinSingleCommentRunPatch keeps zero success when stopped before collecting any comment', () => {
  const patch = buildDouyinSingleCommentRunPatch({
    stopped: true,
    totalComments: 0,
    note: {
      contentId: 'dy_content_2',
      platformContentId: 'video_2',
    },
  });

  assert.equal(patch.itemsSucceeded, 0);
  assert.equal(patch.totalComments, 0);
  assert.deepEqual(patch.targetIds, ['video_2']);
});

test('resolveDouyinSingleCommentUiTotal does not fake a full progress total for open-ended collection', () => {
  assert.equal(resolveDouyinSingleCommentUiTotal({ maxTotal: 20, current: 5 }), 20);
  assert.equal(resolveDouyinSingleCommentUiTotal({ maxTotal: 0, current: 5 }), 0);
});

test('describeTaskProgress shows collected count when total is unknown', () => {
  assert.equal(describeTaskProgress({ current: 7, total: 0 }), '已处理 7');
  assert.equal(describeTaskProgress({ current: 7, total: 10 }), '7/10');
  assert.equal(describeTaskProgress({ current: 0, total: 0 }), '进行中');
});
