import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createExecutionAccountLockManager,
  createExecutionAccountLockStorageStore,
} from '../src/workbench/runtime/executionAccountLock.js';
import { createManualExecutionLockCoordinator } from '../src/workbench/runtime/manualExecutionLock.js';

function createAccountStore(accounts = []) {
  return {
    async getAll() {
      return accounts;
    },
  };
}

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

test('manual execution lock skips remote workbench dispatch messages', async () => {
  const coordinator = createManualExecutionLockCoordinator({
    accountStore: createAccountStore([]),
    lockManager: {
      acquire: async () => {
        throw new Error('remote task should not use manual lock');
      },
    },
  });

  const prepared = await coordinator.prepare({
    action: 'startBatchNotes',
    msg: { externalTaskMeta: { externalTaskId: 'task_remote_1' } },
    tabId: 1,
    tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
  });

  assert.equal(prepared.locked, false);
});

test('manual execution lock attaches lock identity for an unlocked account', async () => {
  const acquiredLocks = [];
  const injected = [];
  const coordinator = createManualExecutionLockCoordinator({
    accountStore: createAccountStore([
      {
        accountId: 'account_1',
        platform: 'xhs',
        status: 'available',
        cookieJson: '[]',
        dailyQuotaUsed: 0,
        dailyQuotaLimit: 100,
      },
    ]),
    lockManager: {
      acquire: async (lock) => {
        acquiredLocks.push(lock);
        return { acquired: true };
      },
    },
    injectCookiesForAccount: async (cookieJson, platform) => {
      injected.push([cookieJson, platform]);
      return { success: true };
    },
    now: () => 1000,
    random: () => 0.1,
  });

  const prepared = await coordinator.prepare({
    action: 'startBatchNotes',
    msg: { count: 10 },
    tabId: 3,
    tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
  });

  assert.equal(prepared.locked, true);
  assert.equal(prepared.message.accountId, 'account_1');
  assert.equal(prepared.message.executionLock.accountId, 'account_1');
  assert.equal(prepared.message.executionLock.platform, 'xhs');
  assert.match(prepared.message.executionLock.taskId, /^manual:startBatchNotes:3:/);
  assert.deepEqual(injected, [['[]', 'xhs']]);
  assert.deepEqual(acquiredLocks[0], prepared.message.executionLock);
});

test('manual execution lock reports a clear account-busy message', async () => {
  const coordinator = createManualExecutionLockCoordinator({
    accountStore: createAccountStore([
      {
        accountId: 'account_busy',
        platform: 'xhs',
        status: 'available',
        cookieJson: '[]',
        dailyQuotaUsed: 0,
        dailyQuotaLimit: 100,
      },
    ]),
    lockManager: {
      acquire: async () => ({
        acquired: false,
        reasonCode: 'account_busy',
        reasonMessage: '同一账号正在执行另一个采集任务',
      }),
    },
  });

  await assert.rejects(
    () => coordinator.prepare({
      action: 'startBatchNotes',
      msg: { count: 10 },
      tabId: 3,
      tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
    }),
    /同一账号正在执行另一个采集任务/,
  );
});

test('manual execution lock clears stale workbench account lock before manual collection', async () => {
  const acquiredLocks = [];
  const releasedLocks = [];
  const coordinator = createManualExecutionLockCoordinator({
    accountStore: createAccountStore([
      {
        accountId: 'account_1',
        platform: 'douyin',
        status: 'available',
        cookieJson: '[]',
        dailyQuotaUsed: 0,
        dailyQuotaLimit: 100,
      },
    ]),
    lockManager: {
      acquire: async (lock) => {
        acquiredLocks.push(lock);
        if (acquiredLocks.length === 1) {
          return {
            acquired: false,
            reasonCode: 'account_busy',
            reasonMessage: '同一账号正在执行另一个采集任务',
            existingTaskId: 'old_workbench_task',
          };
        }
        return { acquired: true };
      },
      release: async (lock) => {
        releasedLocks.push(lock);
      },
    },
    shouldReleaseStaleWorkbenchLock: async ({ existingTaskId }) => existingTaskId === 'old_workbench_task',
    injectCookiesForAccount: async () => ({ success: true }),
    now: () => 1000,
    random: () => 0.1,
  });

  const prepared = await coordinator.prepare({
    action: 'startBatchNotes',
    msg: { count: 10 },
    tabId: 3,
    tabUrl: 'https://www.douyin.com/user/demo',
  });

  assert.equal(prepared.locked, true);
  assert.equal(acquiredLocks.length, 2);
  assert.deepEqual(releasedLocks, [{
    platform: 'douyin',
    accountId: 'account_1',
    taskId: 'old_workbench_task',
  }]);
});

test('manual execution lock releases the lock when cookie injection fails', async () => {
  const released = [];
  const coordinator = createManualExecutionLockCoordinator({
    accountStore: createAccountStore([
      {
        accountId: 'account_1',
        platform: 'xhs',
        status: 'available',
        cookieJson: 'bad',
        dailyQuotaUsed: 0,
        dailyQuotaLimit: 100,
      },
    ]),
    lockManager: {
      acquire: async () => ({ acquired: true }),
      release: async (lock) => {
        released.push(lock);
      },
    },
    injectCookiesForAccount: async () => ({ success: false, error: 'bad_cookie' }),
    now: () => 1000,
    random: () => 0.1,
  });

  await assert.rejects(
    () => coordinator.prepare({
      action: 'startBatchNotes',
      msg: { count: 10 },
      tabId: 3,
      tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
    }),
    /账号登录状态不可用/,
  );
  assert.equal(released.length, 1);
});

test('manual execution lock sees session-backed locks after service worker restart', async () => {
  let now = 1000;
  const storageArea = createMemoryStorage();
  const storageKey = 'manual-session-locks';
  const accounts = createAccountStore([
    {
      accountId: 'account_1',
      platform: 'xhs',
      status: 'available',
      cookieJson: '[]',
      dailyQuotaUsed: 0,
      dailyQuotaLimit: 100,
    },
  ]);
  const firstCoordinator = createManualExecutionLockCoordinator({
    accountStore: accounts,
    lockManager: createExecutionAccountLockManager({
      store: createExecutionAccountLockStorageStore({ storageArea, storageKey }),
      now: () => now,
      ttlMs: 10000,
    }),
    injectCookiesForAccount: async () => ({ success: true }),
    now: () => now,
    random: () => 0.1,
  });

  await firstCoordinator.prepare({
    action: 'startBatchNotes',
    msg: { count: 10 },
    tabId: 3,
    tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
  });

  const restartedCoordinator = createManualExecutionLockCoordinator({
    accountStore: accounts,
    lockManager: createExecutionAccountLockManager({
      store: createExecutionAccountLockStorageStore({ storageArea, storageKey }),
      now: () => now,
      ttlMs: 10000,
    }),
    injectCookiesForAccount: async () => ({ success: true }),
    now: () => now,
    random: () => 0.2,
  });

  await assert.rejects(
    () => restartedCoordinator.prepare({
      action: 'startBatchNotes',
      msg: { count: 10 },
      tabId: 4,
      tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
    }),
    /同一账号正在执行另一个采集任务/,
  );

  now = 12001;
  const preparedAfterExpiry = await restartedCoordinator.prepare({
    action: 'startBatchNotes',
    msg: { count: 10 },
    tabId: 4,
    tabUrl: 'https://www.xiaohongshu.com/user/profile/demo',
  });
  assert.equal(preparedAfterExpiry.locked, true);
});
