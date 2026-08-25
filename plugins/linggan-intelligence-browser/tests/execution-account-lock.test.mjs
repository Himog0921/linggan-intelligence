import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createExecutionAccountLockManager,
  createExecutionAccountLockMemoryStore,
  createExecutionAccountLockStorageStore,
} from '../src/workbench/runtime/executionAccountLock.js';

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
    async remove(key) {
      delete values[key];
    },
  };
}

test('execution account lock rejects another task on the same platform account', async () => {
  let now = 1000;
  const manager = createExecutionAccountLockManager({
    store: createExecutionAccountLockMemoryStore(),
    now: () => now,
    ttlMs: 10000,
  });

  const first = await manager.acquire({
    platform: 'xhs',
    accountId: 'account_1',
    taskId: 'task_1',
  });
  const second = await manager.acquire({
    platform: 'xhs',
    accountId: 'account_1',
    taskId: 'task_2',
  });

  assert.equal(first.acquired, true);
  assert.equal(second.acquired, false);
  assert.equal(second.reasonCode, 'account_busy');
  assert.equal(second.existingTaskId, 'task_1');
  assert.equal(second.retryAfterMs, 10000);
});

test('execution account lock serializes concurrent acquisition for the same platform account', async () => {
  let snapshot = { locks: {} };
  const clone = (value) => JSON.parse(JSON.stringify(value));
  const store = {
    async read() {
      await new Promise((resolve) => setTimeout(resolve, 10));
      return clone(snapshot);
    },
    async write(next) {
      await new Promise((resolve) => setTimeout(resolve, 10));
      snapshot = clone(next);
    },
    async clear() {
      snapshot = { locks: {} };
    },
  };
  const manager = createExecutionAccountLockManager({
    store,
    now: () => 1000,
    ttlMs: 10000,
  });

  const results = await Promise.all([
    manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' }),
    manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_2' }),
  ]);

  assert.deepEqual(
    results.map((result) => result.acquired).sort(),
    [false, true],
  );
  const rejected = results.find((result) => !result.acquired);
  assert.equal(rejected.reasonCode, 'account_busy');
});

test('execution account lock lets the same task refresh its own lock', async () => {
  let now = 1000;
  const manager = createExecutionAccountLockManager({
    store: createExecutionAccountLockMemoryStore(),
    now: () => now,
    ttlMs: 10000,
  });

  await manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' });
  now = 6000;
  const refreshed = await manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' });
  const snapshot = await manager.snapshot();

  assert.equal(refreshed.acquired, true);
  assert.equal(snapshot.locks['xhs:account_1'].expiresAtMs, 16000);
});

test('execution account lock replaces expired locks', async () => {
  let now = 1000;
  const manager = createExecutionAccountLockManager({
    store: createExecutionAccountLockMemoryStore(),
    now: () => now,
    ttlMs: 10000,
  });

  await manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' });
  now = 12001;
  const acquired = await manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_2' });
  const snapshot = await manager.snapshot();

  assert.equal(acquired.acquired, true);
  assert.equal(snapshot.locks['xhs:account_1'].taskId, 'task_2');
});

test('execution account lock release only removes the owner task', async () => {
  const manager = createExecutionAccountLockManager({
    store: createExecutionAccountLockMemoryStore(),
    now: () => 1000,
    ttlMs: 10000,
  });

  await manager.acquire({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' });
  const ignored = await manager.release({ platform: 'xhs', accountId: 'account_1', taskId: 'task_2' });
  assert.equal(ignored.released, false);
  assert.equal((await manager.snapshot()).locks['xhs:account_1'].taskId, 'task_1');

  const released = await manager.release({ platform: 'xhs', accountId: 'account_1', taskId: 'task_1' });
  assert.equal(released.released, true);
  assert.deepEqual((await manager.snapshot()).locks, {});
});

test('execution account lock storage store persists and clears locks', async () => {
  const storageArea = createMemoryStorage();
  const store = createExecutionAccountLockStorageStore({ storageArea, storageKey: 'locks' });

  await store.write({ locks: { 'xhs:account_1': { taskId: 'task_1' } } });

  assert.equal((await store.read()).locks['xhs:account_1'].taskId, 'task_1');
  await store.clear();
  assert.deepEqual(await store.read(), { locks: {} });
});

test('execution account lock restores session-backed locks after service worker restart', async () => {
  let now = 1000;
  const storageArea = createMemoryStorage();
  const storageKey = 'session-locks';
  const firstManager = createExecutionAccountLockManager({
    store: createExecutionAccountLockStorageStore({ storageArea, storageKey }),
    now: () => now,
    ttlMs: 10000,
  });

  await firstManager.acquire({
    platform: 'xhs',
    accountId: 'account_1',
    taskId: 'task_1',
  });

  const restartedManager = createExecutionAccountLockManager({
    store: createExecutionAccountLockStorageStore({ storageArea, storageKey }),
    now: () => now,
    ttlMs: 10000,
  });
  const blocked = await restartedManager.acquire({
    platform: 'xhs',
    accountId: 'account_1',
    taskId: 'task_2',
  });

  assert.equal(blocked.acquired, false);
  assert.equal(blocked.reasonCode, 'account_busy');
  assert.equal(blocked.existingTaskId, 'task_1');

  now = 12001;
  const replaced = await restartedManager.acquire({
    platform: 'xhs',
    accountId: 'account_1',
    taskId: 'task_2',
  });
  assert.equal(replaced.acquired, true);
  assert.equal((await restartedManager.snapshot()).locks['xhs:account_1'].taskId, 'task_2');
});
