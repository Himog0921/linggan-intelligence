import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildPlatformAccountReports,
  collectStationRuntimeStates,
  MONITOR_STATION_CAPABILITIES,
  stationCapabilitiesForRuntimeStates,
} from '../src/workbench/runtime/executionStationRuntime.js';

test('execution station runtime reports generic execution accounts and surface capabilities', () => {
  const reports = buildPlatformAccountReports([
    {
      accountId: 'account-1',
      name: '监控小红书 01',
      platform: 'xhs',
      status: 'available',
      dailyQuotaUsed: 3,
      dailyQuotaLimit: 100,
      cooldownUntil: 0,
    },
  ], {
    now: new Date('2026-04-17T12:00:00.000Z').getTime(),
  });

  assert.deepEqual(reports.map((account) => ({
    platform: account.platform,
    purpose: account.purpose,
    healthStatus: account.healthStatus,
    dailyTaskCount: account.dailyTaskCount,
    dailyOpenedCount: account.dailyOpenedCount,
  })), [
    {
      platform: 'xhs',
      purpose: 'execution',
      healthStatus: 'healthy',
      dailyTaskCount: 3,
      dailyOpenedCount: 3,
    },
  ]);
  assert.equal(MONITOR_STATION_CAPABILITIES.includes('xhs.list_scan'), true);
  assert.equal(MONITOR_STATION_CAPABILITIES.includes('xhs.author_links'), true);
  assert.equal(MONITOR_STATION_CAPABILITIES.includes('xhs.note_full'), true);
  assert.equal(MONITOR_STATION_CAPABILITIES.includes('xhs.author_profile'), true);
  assert.equal(MONITOR_STATION_CAPABILITIES.includes('xhs.batchNotes'), false);
});

test('monitor station runtime reports cooling accounts with explicit cooldown metadata', () => {
  const reports = buildPlatformAccountReports([
    {
      accountId: 'account-1',
      name: '监控小红书 01',
      platform: 'xhs',
      status: 'cooldown',
      dailyQuotaUsed: 8,
      dailyQuotaLimit: 100,
      cooldownUntil: new Date('2026-04-17T12:05:00.000Z').getTime(),
    },
  ], {
    now: new Date('2026-04-17T12:00:00.000Z').getTime(),
  });

  assert.deepEqual(reports.map((account) => ({
    platform: account.platform,
    healthStatus: account.healthStatus,
    cooldownUntil: account.cooldownUntil,
    dailyTaskCount: account.dailyTaskCount,
  })), [
    {
      platform: 'xhs',
      healthStatus: 'cooling',
      cooldownUntil: new Date('2026-04-17T12:05:00.000Z').getTime(),
      dailyTaskCount: 8,
    },
  ]);
});

test('execution station runtime merges page permission and login state into account reports', () => {
  const reports = buildPlatformAccountReports([
    {
      accountId: 'account-1',
      name: '监控小红书 01',
      platform: 'xhs',
      status: 'available',
    },
  ], {
    now: new Date('2026-04-17T12:00:00.000Z').getTime(),
    runtimeStates: [
      {
        platform: 'xhs',
        pagePermission: 'denied',
        loginState: 'logged_in',
        cookiesReadable: true,
        platformBlocked: false,
        checkedAt: 1776427200000,
      },
      {
        platform: 'douyin',
        label: '抖音',
        pagePermission: 'granted',
        loginState: 'logged_out',
        cookiesReadable: true,
        platformBlocked: false,
        checkedAt: 1776427200000,
      },
    ],
  });

  const byPlatform = new Map(reports.map((account) => [account.platform, account]));
  assert.equal(byPlatform.get('xhs').healthStatus, 'restricted');
  assert.equal(byPlatform.get('xhs').rawProfile.pagePermission, 'denied');
  assert.equal(byPlatform.get('douyin').healthStatus, 'needs_login');
  assert.equal(byPlatform.get('douyin').rawProfile.loginState, 'logged_out');
});

test('stored execution accounts remain dispatchable before cookies are injected into the browser', () => {
  const reports = buildPlatformAccountReports([
    {
      accountId: 'account-1',
      name: '监控小红书 01',
      platform: 'xhs',
      status: 'available',
      cookieJson: '[{"name":"web_session","value":"stored"}]',
      dailyQuotaUsed: 3,
      dailyQuotaLimit: 100,
      cooldownUntil: 0,
    },
  ], {
    now: new Date('2026-04-17T12:00:00.000Z').getTime(),
    runtimeStates: [
      {
        platform: 'xhs',
        pagePermission: 'granted',
        loginState: 'logged_out',
        cookiesReadable: true,
        platformBlocked: false,
        checkedAt: 1776427200000,
      },
    ],
  });

  assert.equal(reports.length, 1);
  assert.equal(reports[0].healthStatus, 'healthy');
  assert.equal(reports[0].rawProfile.loginState, 'logged_out');
  assert.equal(reports[0].rawProfile.storedAccountAvailable, true);
  assert.equal(reports[0].rawProfile.executionLoginMode, 'stored_cookie_injection');
});

test('execution station runtime detects browser platform state from chrome APIs', async () => {
  const chromeApi = {
    permissions: {
      async contains({ origins }) {
        return origins.some((origin) => origin.includes('xiaohongshu'));
      },
    },
    cookies: {
      async getAll({ domain }) {
        if (domain.includes('xiaohongshu')) return [{ name: 'web_session' }];
        return [];
      },
    },
  };

  const states = await collectStationRuntimeStates({
    chromeApi,
    now: 1776427200000,
  });

  const byPlatform = new Map(states.map((state) => [state.platform, state]));
  assert.equal(byPlatform.get('xhs').pagePermission, 'granted');
  assert.equal(byPlatform.get('xhs').loginState, 'logged_in');
  assert.equal(byPlatform.get('douyin').pagePermission, 'denied');
  assert.equal(byPlatform.get('douyin').loginState, 'logged_out');

  const capabilities = stationCapabilitiesForRuntimeStates(states);
  assert.equal(capabilities.includes('xhs.pageAccess'), true);
  assert.equal(capabilities.includes('xhs.loggedIn'), true);
  assert.equal(capabilities.includes('douyin.loggedIn'), false);
});

test('execution station runtime waits for callback-style permission checks and keeps unknown separate from granted', async () => {
  const chromeApi = {
    permissions: {
      contains({ origins }, callback) {
        setTimeout(() => {
          callback(origins.some((origin) => origin.includes('douyin')));
        }, 1);
      },
    },
    cookies: {
      async getAll({ domain }) {
        if (domain.includes('douyin')) return [{ name: 'sessionid' }];
        return [];
      },
    },
  };

  const states = await collectStationRuntimeStates({
    chromeApi,
    now: 1776427200000,
  });

  const byPlatform = new Map(states.map((state) => [state.platform, state]));
  assert.equal(byPlatform.get('xhs').pagePermission, 'denied');
  assert.equal(byPlatform.get('douyin').pagePermission, 'granted');

  const unknownStates = await collectStationRuntimeStates({
    chromeApi: {
      permissions: {
        contains() {
          return undefined;
        },
      },
      cookies: {
        async getAll() {
          return [];
        },
      },
    },
    now: 1776427200000,
  });
  assert.equal(unknownStates.every((state) => state.pagePermission === 'unknown'), true);
  assert.equal(stationCapabilitiesForRuntimeStates(unknownStates).includes('xhs.pageAccess'), false);
});
