import test from 'node:test';
import assert from 'node:assert/strict';

import { applyPackagedInstallBootstrap } from '../src/workbench/runtime/pluginInstallBootstrap.js';

test('packaged install bootstrap authorizes the plugin and stores the returned station once', async () => {
  const calls = [];
  const authorizationClient = {
    async getStoredAuthorization() {
      return {};
    },
    async authorizeWithCode(input) {
      calls.push(['authorize', input]);
      return {
        authorizationId: 'auth_1',
        authorizationToken: 'auth_token_1',
        status: 'active',
        station: {
          stationId: 'station-1',
          stationToken: 'station-token-1',
          displayName: '程烈的工位',
          role: 'execution',
        },
      };
    },
  };
  const stationClient = {
    async getStoredStationIdentity() {
      return {};
    },
    async ensureStationKey() {
      calls.push(['station-key']);
      return 'station-key-1';
    },
    async saveStationIdentity(input) {
      calls.push(['station', input]);
      return input;
    },
  };

  const result = await applyPackagedInstallBootstrap({
    readConfig: async () => ({
      schemaVersion: 1,
      serverUrl: 'https://lingganboom.fun',
      authorizationCode: 'LGBOOM-AUTO',
      expiresAt: '2099-01-01T00:00:00.000Z',
    }),
    authorizationClient,
    stationClient,
    saveFlywheelConfig: async (config) => calls.push(['config', config]),
    stationCapabilities: ['xhs.list_scan'],
    pluginVersion: '2.0.1',
    browserLabel: 'Chrome',
    now: () => 1_000,
  });

  assert.equal(result.applied, true);
  assert.deepEqual(calls, [
    ['config', { serverUrl: 'https://lingganboom.fun', enabled: true }],
    ['station-key'],
    ['authorize', {
      authorizationCode: 'LGBOOM-AUTO',
      stationKey: 'station-key-1',
      pluginVersion: '2.0.1',
      browserLabel: 'Chrome',
      capabilities: ['xhs.list_scan'],
    }],
    ['config', {
      enabled: true,
      apiToken: 'auth_token_1',
      dataToken: '',
      dataTokenExpiresAt: '',
      dataWorkspaceId: '',
      dataUserEmail: '',
      dataUserName: '',
    }],
    ['station', {
      stationKey: 'station-key-1',
      stationId: 'station-1',
      stationToken: 'station-token-1',
      displayName: '程烈的工位',
      role: 'execution',
      capabilities: ['xhs.list_scan'],
      pairedAt: 1_000,
    }],
  ]);
});

test('packaged install bootstrap skips when authorization and station already exist', async () => {
  const result = await applyPackagedInstallBootstrap({
    readConfig: async () => ({
      schemaVersion: 1,
      serverUrl: 'https://lingganboom.fun',
      authorizationCode: 'LGBOOM-AUTO',
      expiresAt: '2099-01-01T00:00:00.000Z',
    }),
    authorizationClient: {
      async getStoredAuthorization() {
        return { authorizationToken: 'auth-token', status: 'active' };
      },
    },
    stationClient: {
      async getStoredStationIdentity() {
        return { stationId: 'station-1', stationToken: 'station-token-1' };
      },
    },
  });

  assert.deepEqual(result, { applied: false, reason: 'already_configured' });
});

test('packaged install bootstrap preserves existing authorization when only station identity is missing', async () => {
  const calls = [];
  const result = await applyPackagedInstallBootstrap({
    readConfig: async () => ({
      schemaVersion: 1,
      serverUrl: 'https://lingganboom.fun',
      authorizationCode: 'LGBOOM-NEW',
      expiresAt: '2099-01-01T00:00:00.000Z',
    }),
    authorizationClient: {
      async getStoredAuthorization() {
        return {
          authorizationId: 'old-auth',
          authorizationToken: 'old-token',
          status: 'active',
        };
      },
      async authorizeWithCode(input) {
        calls.push(['authorize', input]);
        return {
          authorizationId: 'old-auth',
          authorizationToken: 'new-token',
          status: 'active',
          station: {
            stationId: 'station-2',
            stationToken: 'station-token-2',
            displayName: 'Chrome 工位',
            role: 'execution',
          },
        };
      },
    },
    stationClient: {
      async getStoredStationIdentity() {
        return {};
      },
      async ensureStationKey() {
        calls.push(['station-key']);
        return 'station-key-existing';
      },
      async saveStationIdentity(input) {
        calls.push(['station', input]);
        return input;
      },
    },
    saveFlywheelConfig: async (config) => calls.push(['config', config]),
    stationCapabilities: ['xhs.note_full'],
    pluginVersion: '2.0.2',
    browserLabel: 'Chrome',
    now: () => 2_000,
  });

  assert.equal(result.applied, true);
  assert.equal(result.authorization.authorizationId, 'old-auth');
  assert.deepEqual(calls, [
    ['config', { serverUrl: 'https://lingganboom.fun', enabled: true }],
    ['station-key'],
    ['authorize', {
      authorizationCode: 'LGBOOM-NEW',
      stationKey: 'station-key-existing',
      pluginVersion: '2.0.2',
      browserLabel: 'Chrome',
      capabilities: ['xhs.note_full'],
    }],
    ['config', {
      enabled: true,
      apiToken: 'new-token',
      dataToken: '',
      dataTokenExpiresAt: '',
      dataWorkspaceId: '',
      dataUserEmail: '',
      dataUserName: '',
    }],
    ['station', {
      stationKey: 'station-key-existing',
      stationId: 'station-2',
      stationToken: 'station-token-2',
      displayName: 'Chrome 工位',
      role: 'execution',
      capabilities: ['xhs.note_full'],
      pairedAt: 2_000,
    }],
  ]);
});
