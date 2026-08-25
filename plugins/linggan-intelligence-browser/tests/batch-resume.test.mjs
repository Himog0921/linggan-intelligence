import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildBatchResumeCheckpoint,
  resolveBatchResumeState,
} from '../src/workbench/runtime/batchResume.js';

test('buildBatchResumeCheckpoint stores the next target index and processed result statuses', () => {
  const patch = buildBatchResumeCheckpoint({
    targetIds: ['n1', 'n2', 'n3'],
    processedCount: 2,
    resultStatuses: [
      { targetId: 'n1', ok: true, contentId: 'xhs_n1' },
      { targetId: 'n2', error: 'timeout' },
      { targetId: 'n3', ok: true, contentId: 'xhs_n3' },
    ],
    updatedAt: 123,
  });

  assert.equal(patch.processedCount, 2);
  assert.equal(patch.nextIndex, 2);
  assert.deepEqual(patch.resumeCheckpoint, {
    version: 1,
    processedCount: 2,
    nextIndex: 2,
    targetIds: ['n1', 'n2', 'n3'],
    resultStatuses: [
      { targetId: 'n1', ok: true, contentId: 'xhs_n1', error: '', totalComments: undefined },
      { targetId: 'n2', ok: false, contentId: '', error: 'timeout', totalComments: undefined },
    ],
    updatedAt: 123,
  });
});

test('resolveBatchResumeState resumes from the stored next index after page refresh', () => {
  const state = resolveBatchResumeState({
    runRecord: {
      resumeCheckpoint: {
        targetIds: ['n1', 'n2', 'n3'],
        nextIndex: 2,
        resultStatuses: [
          { targetId: 'n1', ok: true, contentId: 'xhs_n1' },
          { targetId: 'n2', ok: false, error: 'blocked' },
        ],
      },
    },
    targets: [{ noteId: 'n3' }, { noteId: 'n1' }, { noteId: 'n2' }],
    getTargetId: (item) => item.noteId,
  });

  assert.equal(state.resumed, true);
  assert.equal(state.nextIndex, 2);
  assert.deepEqual(state.targetIds, ['n1', 'n2', 'n3']);
  assert.deepEqual(state.completedTargetIds, ['n1', 'n2']);
  assert.deepEqual(state.targets.map((item) => item.noteId), ['n1', 'n2', 'n3']);
});

test('resolveBatchResumeState can infer progress from old run summary fields', () => {
  const state = resolveBatchResumeState({
    runRecord: {
      targetIds: ['7001', '7002', '7003'],
      contentIds: ['dy_7001'],
      failedTargets: [{ awemeId: '7002', error: 'risk control' }],
    },
    targets: [{ awemeId: '7001' }, { awemeId: '7002' }, { awemeId: '7003' }],
    getTargetId: (item) => item.awemeId,
  });

  assert.equal(state.nextIndex, 2);
  assert.deepEqual(state.completedTargetIds, ['7001', '7002']);
});
