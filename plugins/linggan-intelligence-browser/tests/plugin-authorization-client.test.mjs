import test from 'node:test';
import assert from 'node:assert/strict';

import {
  assertActivePluginAuthorization,
  createPluginAuthorizationClient,
  getPluginAuthorizationBlockedMessage,
  hasActivePluginAuthorization,
} from '../src/workbench/runtime/pluginAuthorization.js';

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

test('plugin authorization client exchanges authorization code and stores active authorization', async () => {
  const storageArea = createMemoryStorage();
  const requests = [];
  const client = createPluginAuthorizationClient({
    storageArea,
    randomUUID: () => 'device-1',
    resolveServerUrl: async () => 'http://localhost:3000',
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            authorizationId: 'auth_1',
            authorizationToken: 'auth_token_1',
            status: 'active',
            teamName: '内容团队',
            memberName: '测试同学',
            expiresAt: '2026-05-01T00:00:00.000Z',
          };
        },
      };
    },
  });

  const authorization = await client.authorizeWithCode({
    authorizationCode: 'AUTH-001',
    pluginVersion: '2.0.0',
    browserLabel: 'Chrome',
  });

  assert.equal(authorization.authorizationId, 'auth_1');
  assert.equal(authorization.authorizationToken, 'auth_token_1');
  assert.equal(authorization.deviceId, 'device-1');
  assert.equal(authorization.teamName, '内容团队');
  assert.equal(hasActivePluginAuthorization(authorization), true);
  assert.equal(requests.length, 1);
  assert.equal(JSON.parse(requests[0][1].body).authorizationCode, 'AUTH-001');
  assert.equal(JSON.parse(requests[0][1].body).deviceId, 'device-1');
});

test('plugin authorization client sends station identity during authorization code activation', async () => {
  const storageArea = createMemoryStorage();
  const requests = [];
  const client = createPluginAuthorizationClient({
    storageArea,
    randomUUID: () => 'device-1',
    resolveServerUrl: async () => 'http://localhost:3000',
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            authorizationId: 'auth_1',
            authorizationToken: 'auth_token_1',
            status: 'active',
            memberName: 'Mog',
            seatName: 'Mog Chrome 工位',
            station: {
              stationId: 'station-1',
              stationToken: 'station-token-1',
              displayName: 'Mog Chrome 工位',
              role: 'execution',
            },
          };
        },
      };
    },
  });

  const authorization = await client.authorizeWithCode({
    authorizationCode: 'AUTH-001',
    stationKey: 'station-key-1',
    pluginVersion: '2.0.52',
    browserLabel: 'Chrome',
    capabilities: ['xhs.list_scan'],
  });

  assert.equal(authorization.authorizationToken, 'auth_token_1');
  assert.equal(authorization.station.stationId, 'station-1');
  assert.deepEqual(JSON.parse(requests[0][1].body), {
    authorizationCode: 'AUTH-001',
    deviceId: 'device-1',
    stationKey: 'station-key-1',
    pluginVersion: '2.0.52',
    browserLabel: 'Chrome',
    capabilities: ['xhs.list_scan'],
  });
});

test('plugin authorization guard throws a user-facing error when authorization is missing', async () => {
  const storageArea = createMemoryStorage();

  await assert.rejects(
    () => assertActivePluginAuthorization({ storageArea }),
    (error) => {
      assert.equal(error.code, 'plugin_authorization_required');
      assert.equal(
        error.message,
        '当前浏览器还没有插件授权。可以从内容工作台重新下载安装，或在插件里发起授权申请。',
      );
      return true;
    },
  );

  assert.equal(
    getPluginAuthorizationBlockedMessage({ status: 'revoked' }),
    '插件授权已被撤销，请联系管理员重新授权。',
  );
  assert.equal(
    getPluginAuthorizationBlockedMessage({ status: 'pending' }),
    '授权申请已发送，等待内容工作台审批。',
  );
  assert.equal(
    getPluginAuthorizationBlockedMessage({ status: 'approved' }),
    '授权已通过，请在插件里点击检查审批结果完成激活。',
  );
});

test('plugin authorization client creates a pending workbench approval request', async () => {
  const storageArea = createMemoryStorage();
  const requests = [];
  const client = createPluginAuthorizationClient({
    storageArea,
    randomUUID: () => 'device-external-1',
    resolveServerUrl: async () => 'https://lingganboom.fun',
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      return {
        ok: true,
        status: 202,
        async json() {
          return {
            requestId: 'request-1',
            status: 'pending',
            message: '授权申请已发送，请等待内容工作台审批。',
          };
        },
      };
    },
  });

  const authorization = await client.requestWorkbenchApproval({
    pluginVersion: '2.0.30',
    browserLabel: 'Chrome 外部安装',
  });

  assert.equal(authorization.authorizationId, 'request-1');
  assert.equal(authorization.status, 'pending');
  assert.equal(authorization.deviceId, 'device-external-1');
  assert.equal(hasActivePluginAuthorization(authorization), false);
  assert.equal(requests[0][0], 'https://lingganboom.fun/api/plugin-authorizations/requests');
  assert.deepEqual(JSON.parse(requests[0][1].body), {
    deviceId: 'device-external-1',
    pluginVersion: '2.0.30',
    browserLabel: 'Chrome 外部安装',
  });
});

test('plugin authorization client claims an approved request and stores active authorization', async () => {
  const storageArea = createMemoryStorage({
    workbenchPluginAuthorization: {
      deviceId: 'device-external-1',
      authorizationId: 'request-1',
      status: 'pending',
    },
  });
  const requests = [];
  const client = createPluginAuthorizationClient({
    storageArea,
    resolveServerUrl: async () => 'https://lingganboom.fun',
    fetchFn: async (url, options = {}) => {
      requests.push([url, options]);
      return {
        ok: true,
        status: 200,
        async json() {
          return {
            status: 'active',
            authorizationId: 'request-1',
            authorizationToken: 'auth-token-1',
            memberName: '程烈',
            seatName: '程烈 Chrome 工位',
            expiresAt: '2026-08-03T08:00:00.000Z',
            station: {
              stationId: 'station-1',
              stationToken: 'station-token-1',
              displayName: '程烈 Chrome 工位',
              role: 'execution',
            },
          };
        },
      };
    },
  });

  const result = await client.claimApprovedRequest({
    stationKey: 'station-key-1',
    pluginVersion: '2.0.30',
    browserLabel: 'Chrome 外部安装',
    capabilities: ['xhs.list_scan'],
  });

  assert.equal(result.authorization.authorizationToken, 'auth-token-1');
  assert.equal(result.authorization.status, 'active');
  assert.equal(result.station.stationId, 'station-1');
  assert.equal(requests[0][0], 'https://lingganboom.fun/api/plugin-authorizations/requests/request-1/claim');
  assert.deepEqual(JSON.parse(requests[0][1].body), {
    deviceId: 'device-external-1',
    stationKey: 'station-key-1',
    pluginVersion: '2.0.30',
    browserLabel: 'Chrome 外部安装',
    capabilities: ['xhs.list_scan'],
  });
});

test('plugin authorization client preserves existing token when claim response has no token', async () => {
  const storageArea = createMemoryStorage({
    workbenchPluginAuthorization: {
      deviceId: 'device-external-1',
      authorizationId: 'auth-active-1',
      authorizationToken: 'existing-token-1',
      status: 'active',
    },
  });
  const client = createPluginAuthorizationClient({
    storageArea,
    resolveServerUrl: async () => 'https://lingganboom.fun',
    fetchFn: async () => ({
      ok: true,
      status: 200,
      async json() {
        return {
          status: 'active',
          requestId: 'auth-active-1',
          message: '这条授权申请已经处理过，请在插件里查看当前授权状态。',
        };
      },
    }),
  });

  const result = await client.claimApprovedRequest({
    stationKey: 'station-key-1',
    pluginVersion: '2.0.52',
    browserLabel: 'Chrome 外部安装',
  });

  assert.equal(result.claimed, false);
  assert.equal(result.authorization.authorizationToken, 'existing-token-1');
  assert.equal(result.authorization.status, 'active');
  assert.equal(result.authorization.authorizationMessage, '这条授权申请已经处理过，请在插件里查看当前授权状态。');
});
