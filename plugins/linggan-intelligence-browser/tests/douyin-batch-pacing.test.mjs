import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createDouyinBatchPacer,
  waitDouyinBatchStep,
} from '../src/platforms/douyin/batchDiscovery.js';

test('waitDouyinBatchStep accepts a min/max range and returns a randomized delay inside the range', async () => {
  const startedAt = Date.now();
  const elapsed = await waitDouyinBatchStep({ min: 5, max: 9 }, {
    random: () => 0.5,
  });
  const wallTime = Date.now() - startedAt;

  assert.equal(elapsed, 7);
  assert.ok(wallTime >= 5);
});

test('createDouyinBatchPacer increases delay range when recent failure rate rises', () => {
  const pacer = createDouyinBatchPacer({
    baseRange: { min: 120, max: 180 },
  });

  assert.deepEqual(pacer.getDelayRange(), {
    min: 120,
    max: 180,
    errorRate: 0,
    backoffLevel: 0,
  });

  pacer.recordSuccess();
  pacer.recordFailure();
  pacer.recordFailure();

  assert.deepEqual(pacer.getDelayRange(), {
    min: 180,
    max: 270,
    errorRate: 2 / 3,
    backoffLevel: 1,
  });

  pacer.recordFailure();
  pacer.recordFailure();

  assert.deepEqual(pacer.getDelayRange(), {
    min: 264,
    max: 396,
    errorRate: 4 / 5,
    backoffLevel: 2,
  });
});

test('createDouyinBatchPacer slides its error window instead of backoff growing forever', () => {
  const pacer = createDouyinBatchPacer({
    baseRange: { min: 180, max: 260 },
    windowSize: 4,
  });

  pacer.recordFailure();
  pacer.recordFailure();
  pacer.recordSuccess();
  pacer.recordSuccess();
  assert.equal(pacer.getDelayRange().backoffLevel, 1);

  pacer.recordSuccess();
  pacer.recordSuccess();
  assert.deepEqual(pacer.getDelayRange(), {
    min: 180,
    max: 260,
    errorRate: 0,
    backoffLevel: 0,
  });
});
