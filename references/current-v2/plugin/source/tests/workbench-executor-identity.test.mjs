import test from 'node:test';
import assert from 'node:assert/strict';

import { createExecutorIdentity } from '../src/workbench/runtime/executorIdentity.js';

function createMemoryStorage(initial = {}) {
  const values = { ...initial };
  return {
    values,
    async get(key) {
      return { [key]: values[key] };
    },
    async set(next) {
      Object.assign(values, next);
    },
  };
}

test('executor identity persists across service worker identity factories', async () => {
  const storageArea = createMemoryStorage();
  const firstWorker = createExecutorIdentity({
    storageArea,
    randomUUID: () => 'uuid_first',
  });

  const firstId = await firstWorker.getExecutorInstanceId();
  const secondWorker = createExecutorIdentity({
    storageArea,
    randomUUID: () => 'uuid_second',
  });
  const secondId = await secondWorker.getExecutorInstanceId();

  assert.equal(firstId, 'plugin_uuid_first');
  assert.equal(secondId, firstId);
  assert.equal(storageArea.values.workbenchExecutorInstanceId, firstId);
});
