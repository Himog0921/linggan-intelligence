import test from 'node:test';
import assert from 'node:assert/strict';

const originalChrome = globalThis.chrome;
globalThis.chrome = {
  runtime: {
    onMessage: { addListener: () => {}, removeListener: () => {} },
    onStartup: { addListener: () => {} },
    onInstalled: { addListener: () => {} },
    lastError: null,
    getManifest: () => ({ version: '0.0.0-test' }),
  },
  tabs: {
    query: async () => [],
    sendMessage: () => {},
    update: async () => {},
  },
  downloads: {
    download: async () => 1,
    remove: async () => {},
    onChanged: { addListener: () => {}, removeListener: () => {} },
  },
  declarativeNetRequest: {
    updateDynamicRules: () => Promise.resolve(),
  },
  action: {
    setBadgeText: async () => {},
    setBadgeBackgroundColor: async () => {},
  },
  alarms: {
    create: () => {},
    onAlarm: { addListener: () => {} },
  },
  debugger: {
    attach: async () => {},
    sendCommand: async () => {},
    detach: async () => {},
  },
  cookies: {
    getAll: async () => [],
  },
  storage: {
    local: {
      get: async () => ({}),
      set: async () => {},
    },
  },
};

const {
  resolveRiskControlAccountId,
  buildRiskControlActiveTaskPatch,
} = await import('../src/background/index.js');

globalThis.chrome = originalChrome;

test('300017 cooldown targets the selected account instead of executor instance', () => {
  assert.equal(
    resolveRiskControlAccountId({
      accountId: 'xhs_account_1',
      executorInstanceId: 'plugin_executor_1',
    }),
    'xhs_account_1',
  );
});

test('300017 local task patch pauses the task and defers replacement account usage', () => {
  assert.deepEqual(
    buildRiskControlActiveTaskPatch({
      accountId: 'xhs_account_2',
      accountName: '账号 B',
    }),
    {
      workbenchStatus: 'paused',
      accountId: 'xhs_account_2',
      pendingAccountUsageId: 'xhs_account_2',
      errorMessage: '风控(300017)，已切换到账号"账号 B"，请恢复任务继续',
    },
  );
});
