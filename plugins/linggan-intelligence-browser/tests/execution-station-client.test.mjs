import test from 'node:test';
import assert from 'node:assert/strict';

import { createExecutionStationClient } from '../src/workbench/runtime/executionStationClient.js';

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

test('execution station client stores registration identity and keeps it after heartbeat failure', async () => {
  const storageArea = createMemoryStorage();
  const requests = [];
  const client = createExecutionStationClient({
    storageArea,
    randomUUID: () => 'station-key-1',
    resolveServerUrl: async () => 'http://localhost:3000',
    resolveAuthorization: async () => ({
      authorizationId: 'auth_1',
      authorizationToken: 'auth_token_1',
    }),
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      if (url.endsWith('/api/execution-stations/register')) {
        return {
          ok: true,
          status: 200,
          async json() {
            return {
              stationId: 'station-1',
              stationToken: 'token-1',
              displayName: '小红书监控 01',
            };
          },
        };
      }
      return {
        ok: false,
        status: 503,
        async text() {
          return 'temporary outage';
        },
      };
    },
  });

  const registration = await client.registerWithPairingCode({
    pairingCode: '123456',
    capabilities: ['xhs.list_scan'],
    pluginVersion: '1.0.0',
  });
  const heartbeat = await client.sendHeartbeat({
    status: 'online',
    capabilities: ['xhs.list_scan'],
    platformAccounts: [{ platform: 'xhs', healthStatus: 'healthy', purpose: 'author_monitor' }],
  });
  const stored = await client.getStoredStationIdentity();

  assert.equal(registration.stationId, 'station-1');
  assert.equal(heartbeat.success, false);
  assert.equal(heartbeat.retryable, true);
  assert.equal(stored.stationId, 'station-1');
  assert.equal(stored.stationToken, 'token-1');
  assert.equal(stored.stationKey, 'station-key-1');
  assert.equal(requests.length, 2);
  assert.equal(requests[0][1].headers.Authorization, 'Bearer auth_token_1');
  assert.equal(JSON.parse(requests[0][1].body).authorizationId, 'auth_1');
  assert.equal(requests[1][0], 'http://localhost:3000/api/execution-stations/sync');
  assert.equal(requests[1][1].headers.Authorization, 'Bearer auth_token_1');
  const heartbeatBody = JSON.parse(requests[1][1].body);
  assert.equal(heartbeatBody.protocolVersion, '3');
  assert.deepEqual(heartbeatBody.capabilities, ['xhs.list_scan']);
  assert.deepEqual(Object.keys(heartbeatBody).sort(), [
    'accountReports',
    'capabilities',
    'operations',
    'pluginVersion',
    'protocolVersion',
    'stationId',
    'stationSessionId',
    'stationToken',
  ]);
  assert.deepEqual(heartbeatBody.accountReports, [
    { platform: 'xhs', healthStatus: 'healthy' },
  ]);
});

test('execution station client reuses stable station key before registration', async () => {
  const storageArea = createMemoryStorage();
  const client = createExecutionStationClient({
    storageArea,
    randomUUID: () => 'station-key-approval',
  });

  const first = await client.ensureStationKey();
  const second = await client.ensureStationKey();
  const stored = await client.getStoredStationIdentity();

  assert.equal(first, 'station-key-approval');
  assert.equal(second, 'station-key-approval');
  assert.equal(stored.stationKey, 'station-key-approval');
});

test('execution station client keeps existing station key when clearing identity', async () => {
  const storageArea = createMemoryStorage({
    workbenchExecutionStation: {
      stationKey: 'station-key-existing',
      stationId: 'station-1',
      stationToken: 'token-1',
    },
  });
  const client = createExecutionStationClient({ storageArea });

  await client.clearStationIdentity();
  const stationKey = await client.ensureStationKey();
  const stored = await client.getStoredStationIdentity();

  assert.equal(stationKey, 'station-key-existing');
  assert.equal(stored.stationKey, 'station-key-existing');
  assert.equal(stored.stationId, undefined);
});

test('execution station heartbeat respects server retry-after backpressure', async () => {
  const storageArea = createMemoryStorage({
    workbenchExecutionStation: {
      stationKey: 'station-key-1',
      stationId: 'station-1',
      stationToken: 'token-1',
      capabilities: ['xhs.list_scan'],
    },
  });
  const client = createExecutionStationClient({
    storageArea,
    now: () => 1_000,
    resolveServerUrl: async () => 'http://localhost:3000',
    resolveAuthorization: async () => ({ authorizationToken: 'auth_token_1' }),
    fetchFn: async () => ({
      ok: false,
      status: 503,
      headers: {
        get(name) {
          return String(name || '').toLowerCase() === 'retry-after' ? '120' : null;
        },
      },
      async text() {
        return JSON.stringify({
          error: '执行设备通道正在保护数据库，请稍后重试。',
          code: 'plugin_protocol_backpressure',
          retryAfterSeconds: 120,
        });
      },
    }),
  });

  const heartbeat = await client.sendHeartbeat({
    status: 'online',
    capabilities: ['xhs.list_scan'],
  });

  assert.equal(heartbeat.success, false);
  assert.equal(heartbeat.retryable, true);
  assert.equal(heartbeat.status, 503);
  assert.equal(heartbeat.reasonCode, 'plugin_protocol_backpressure');
  assert.equal(heartbeat.nextRetryAfterMs, 120_000);
  assert.equal(heartbeat.nextRetryAt, 121_000);
});

test('execution station heartbeat sync stores mailbox version and wakes task polling on changes', async () => {
  const storageArea = createMemoryStorage({
    workbenchExecutionStation: {
      stationKey: 'station-key-1',
      stationId: 'station-1',
      stationToken: 'token-1',
      capabilities: ['xhs.list_scan'],
      mailboxVersion: 6,
    },
  });
  const requests = [];
  const client = createExecutionStationClient({
    storageArea,
    now: () => 5_000,
    resolveServerUrl: async () => 'http://localhost:3000',
    resolveAuthorization: async () => ({
      authorizationId: 'auth_1',
      authorizationToken: 'auth_token_1',
    }),
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            heartbeat: {
              success: true,
              station: {
                id: 'station-1',
                role: 'execution',
              },
            },
            mailboxVersions: { station: 7, 'xhs.monitor_patrol': 3 },
            operationResults: {},
            reservations: [],
            controlCommands: [],
            nextSync: { afterMs: 30000, reason: 'mailbox_changed' },
          };
        },
      };
    },
  });

  const heartbeat = await client.sendHeartbeat({
    status: 'online',
    capabilities: ['xhs.list_scan'],
    pluginVersion: '2.0.42',
  });
  const stored = await client.getStoredStationIdentity();
  const body = JSON.parse(requests[0][1].body);

  assert.equal(requests[0][0], 'http://localhost:3000/api/execution-stations/sync');
  assert.deepEqual(body.mailboxCursors, { station: 6 });
  assert.equal(body.protocolVersion, '3');
  assert.deepEqual(body.capabilities, ['xhs.list_scan']);
  assert.deepEqual(Object.keys(body).sort(), [
    'capabilities',
    'mailboxCursors',
    'operations',
    'pluginVersion',
    'protocolVersion',
    'stationId',
    'stationSessionId',
    'stationToken',
  ]);
  assert.equal(heartbeat.success, true);
  assert.equal(heartbeat.shouldPollNow, true);
  assert.equal(heartbeat.mailbox.version, 7);
  assert.equal(stored.mailboxVersion, 7);
  assert.deepEqual(stored.mailboxLaneVersions, { 'xhs.monitor_patrol': 3 });
  assert.equal(stored.lastHeartbeatAt, 5_000);
});

test('execution station client fetches VAPID public key and registers push subscription', async () => {
  const storageArea = createMemoryStorage({
    workbenchExecutionStation: {
      stationKey: 'station-key-1',
      stationId: 'station-1',
      stationToken: 'token-1',
    },
  });
  const requests = [];
  const client = createExecutionStationClient({
    storageArea,
    now: () => 2_000,
    resolveServerUrl: async () => 'http://localhost:3000',
    resolveAuthorization: async () => ({
      authorizationId: 'auth_1',
      authorizationToken: 'auth_token_1',
    }),
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      if (url.endsWith('/api/push/vapid-public-key')) {
        return {
          ok: true,
          status: 200,
          async json() {
            return { enabled: true, publicKey: 'AQIDBA' };
          },
        };
      }
      return {
        ok: true,
        status: 200,
        async json() {
          return { ok: true };
        },
      };
    },
  });

  const key = await client.fetchVapidPublicKey();
  const registration = await client.registerPushSubscription({
    subscription: {
      endpoint: 'https://fcm.googleapis.com/fcm/send/sub-1',
      keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
    },
    pluginVersion: '2.0.15',
    browserLabel: 'Chrome',
  });
  const stored = await client.getStoredStationIdentity();

  assert.deepEqual(key, { enabled: true, publicKey: 'AQIDBA' });
  assert.equal(registration.ok, true);
  assert.equal(requests[0][1].method, 'GET');
  assert.equal(requests[1][1].headers.Authorization, 'Bearer auth_token_1');
  assert.deepEqual(JSON.parse(requests[1][1].body), {
    stationId: 'station-1',
    stationToken: 'token-1',
    authorizationId: 'auth_1',
    pluginVersion: '2.0.15',
    browserLabel: 'Chrome',
    subscription: {
      endpoint: 'https://fcm.googleapis.com/fcm/send/sub-1',
      keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
    },
  });
  assert.equal(stored.pushSubscriptionEndpoint, 'https://fcm.googleapis.com/fcm/send/sub-1');
  assert.equal(stored.pushSubscriptionRegisteredAt, 2_000);
});
