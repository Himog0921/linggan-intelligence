import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildHeartbeatPatchForRun,
  isTerminalCollectionRunStatus,
} from '../src/db/collectionRunStatus.js';

test('collection run terminal statuses are recognized consistently', () => {
  assert.equal(isTerminalCollectionRunStatus('done'), true);
  assert.equal(isTerminalCollectionRunStatus('stopped'), true);
  assert.equal(isTerminalCollectionRunStatus('failed'), true);
  assert.equal(isTerminalCollectionRunStatus('running'), false);
  assert.equal(isTerminalCollectionRunStatus('stopping'), false);
});

test('heartbeat patches are ignored once a collection run is already terminal', () => {
  const patch = buildHeartbeatPatchForRun(
    { status: 'stopped', lastHeartbeatAt: 123 },
    {
      status: 'running',
      stage: 'collecting',
      current: 15,
      total: 50,
      message: '迟到心跳',
    },
    456,
  );

  assert.equal(patch, null);
});

test('heartbeat patches still refresh non-terminal collection runs', () => {
  const patch = buildHeartbeatPatchForRun(
    { status: 'running', lastHeartbeatAt: 123 },
    {
      status: 'running',
      stage: 'collecting',
      current: 15,
      total: 50,
      message: '继续采集',
    },
    456,
  );

  assert.deepEqual(patch, {
    status: 'running',
    stage: 'collecting',
    message: '继续采集',
    lastHeartbeatAt: 456,
  });
});
