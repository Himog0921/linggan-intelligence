/**
 * verify-sync-v11-e2e.mjs
 *
 * V1.1 /sync 协议端到端联调模拟。
 *
 * 不依赖真实工作台。起一个内存版 V1.1 服务端 mock，让插件的
 * executionStationClient.sendHeartbeat 和 taskLeaseClient.claimCollectionTaskLease
 * 真实调用它，验证：
 *   1. 插件发出去的 body 含全部 V1.1 字段（protocolVersion/stationSessionId/
 *      capacity/activeLeases/operations/accountReports/mailboxCursors）
 *   2. 插件能正确解析 V1.1 响应（mailboxVersions 对象 / operationResults /
 *      reservations[] / controlCommands[] / nextSync{afterMs,reason}）
 *   3. minPluginVersion 校验（pluginVersion < 2.0.53 时返回 426）
 *
 * 运行：node scripts/verify-sync-v11-e2e.mjs
 */
import assert from 'node:assert/strict';
import http from 'node:http';
import { once } from 'node:events';

import { createExecutionStationClient } from '../src/workbench/runtime/executionStationClient.js';
import { claimCollectionTaskLease } from '../src/workbench/runtime/taskLeaseClient.js';

// ---------------------------------------------------------------------------
// 内存版 V1.1 服务端（严格按 execution-sync-service.ts 协议响应）
// ---------------------------------------------------------------------------

function createV11MockServer({ minPluginVersion = '2.0.53' } = {}) {
  const receivedRequests = [];
  let nextMailboxVersion = 100;

  const server = http.createServer((req, res) => {
    if (req.method !== 'POST' || !req.url.endsWith('/api/execution-stations/sync')) {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'not_found' }));
      return;
    }
    let bodyText = '';
    req.on('data', (chunk) => { bodyText += chunk; });
    req.on('end', () => {
      const body = JSON.parse(bodyText || '{}');
      receivedRequests.push({ url: req.url, body });

      // 模拟 minPluginVersion 强硬校验（V11 决策 1）
      function isAtLeast(version, min) {
        const parse = (v) => v.split('.').map((n) => parseInt(n, 10) || 0);
        const a = parse(version || '0');
        const b = parse(min);
        for (let i = 0; i < 3; i++) {
          if ((a[i] || 0) > (b[i] || 0)) return true;
          if ((a[i] || 0) < (b[i] || 0)) return false;
        }
        return true;
      }
      if (!isAtLeast(body.pluginVersion, minPluginVersion)) {
        res.writeHead(426, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          error: 'VERSION_REJECTED',
          message: `Plugin version too old. Please upgrade to ${minPluginVersion}+.`,
          minPluginVersion,
        }));
        return;
      }

      // 模拟 protocolVersion 校验
      if (body.protocolVersion && body.protocolVersion !== '3' && !isAtLeast(body.protocolVersion, '3')) {
        res.writeHead(401, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          error: 'PROTOCOL_VERSION_REJECTED',
          message: `protocolVersion ${body.protocolVersion} not supported; require 3`,
        }));
        return;
      }

      const operationResults = {};
      for (const op of Array.isArray(body.operations) ? body.operations : []) {
        if (op.type === 'start_job') {
          operationResults[op.operationId] = {
            status: 'accepted',
            attemptId: `attempt-${op.jobId}`,
            leaseToken: `lease-${op.jobId}`,
            leaseEpoch: Number(op.reservationEpoch || 0) + 1,
            leaseExpiresAt: new Date(Date.now() + 120000).toISOString(),
          };
        }
      }

      const reservations = [];
      if (body.capacity?.['douyin.governance'] && (!Array.isArray(body.operations) || body.operations.length === 0)) {
        reservations.push({
          jobId: 'job-douyin-governance-1',
          reserveToken: 'reserve-douyin-1',
          reservationEpoch: 4,
          lane: 'douyin.governance',
          startBefore: new Date(Date.now() + 300000).toISOString(),
          taskSpec: {
            id: 'job-douyin-governance-1',
            platform: 'douyin',
            lane: 'governance',
            taskType: 'douyin.collectAuthor',
            source: 'data_governance',
            taskStrategy: 'author_patrol',
            target: 'https://www.douyin.com/user/demo',
            targetKey: 'douyin:author:demo',
            collectionProfile: 'note_detail',
            jobType: 'collect_detail',
            requiredFields: {},
            payload: { taskStrategy: 'author_patrol' },
          },
        });
      }

      // 构造 V1.1 成功响应
      const stationMailboxVersion = ++nextMailboxVersion;
      const response = {
        serverTime: new Date().toISOString(),
        mailboxVersions: {
          station: stationMailboxVersion,
          ...(body.capacity?.['xhs.monitor_patrol'] ? { 'xhs.monitor_patrol': 5 } : {}),
          ...(body.capacity?.['douyin.governance'] ? { 'douyin.governance': 7 } : {}),
        },
        operationResults,
        reservations,
        controlCommands: [],
        nextSync: {
          afterMs: reservations.length > 0 ? 1000 : body.activeLeases && body.activeLeases.length > 0 ? 30000 : 240000,
          reason: reservations.length > 0 ? 'reservations_granted' : body.activeLeases && body.activeLeases.length > 0 ? 'running' : 'idle',
        },
      };
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(response));
    });
  });

  return {
    server,
    receivedRequests,
    async start() {
      return new Promise((resolve) => {
        server.listen(0, '127.0.0.1', () => {
          const { port } = server.address();
          resolve({ port });
        });
      });
    },
    async stop() {
      return new Promise((resolve) => server.close(() => resolve()));
    },
  };
}

// ---------------------------------------------------------------------------
// 内存版 chrome.storage.local
// ---------------------------------------------------------------------------
function createMemoryStorage() {
  const store = new Map();
  return {
    async get(keys) {
      if (Array.isArray(keys)) {
        const result = {};
        for (const k of keys) if (store.has(k)) result[k] = store.get(k);
        return result;
      }
      if (typeof keys === 'string') {
        return store.has(keys) ? { [keys]: store.get(keys) } : {};
      }
      return Object.fromEntries(store);
    },
    async set(items) {
      for (const [k, v] of Object.entries(items || {})) store.set(k, v);
    },
    async remove(keys) {
      const arr = Array.isArray(keys) ? keys : [keys];
      for (const k of arr) store.delete(k);
    },
  };
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------
async function main() {
  const mock = createV11MockServer();
  const { port } = await mock.start();
  const baseUrl = `http://127.0.0.1:${port}`;
  console.log(`[mock] V1.1 server listening at ${baseUrl}\n`);

  const storage = createMemoryStorage();
  // 预置工位身份（注意：createMemoryStorage.set 接受 items 对象，与 chrome.storage.local 一致）
  await storage.set({
    workbenchExecutionStation: {
      stationId: 'station-e2e-1',
      stationToken: 'token-e2e-1',
      capabilities: ['xhs'],
      mailboxVersion: 99,
    },
  });

  let assertionsFailed = 0;
  function check(label, cond, detail) {
    if (cond) {
      console.log(`  ✔ ${label}`);
    } else {
      console.log(`  ✖ ${label} — ${detail || ''}`);
      assertionsFailed++;
    }
  }

  // =========================================================================
  // 1. sendHeartbeat 完整流程（含活动任务）
  // =========================================================================
  console.log('=== Test 1: sendHeartbeat 发 V1.1 完整字段 ===');

  const client = createExecutionStationClient({
    storageArea: storage,
    fetchFn: globalThis.fetch.bind(globalThis),
    resolveServerUrl: async () => baseUrl,
    resolveAuthorization: async () => ({ authorizationId: 'auth-e2e-1', authorizationToken: 'auth-token-e2e-1' }),
  });

  const heartbeatResult = await client.sendHeartbeat({
    status: 'online',
    capabilities: ['xhs'],
    pluginVersion: '2.0.55',
    platformAccounts: [
      { platform: 'xhs', id: 'acc-1', health: 'healthy' },
      { platform: 'douyin', platformAccountId: 'acc-2', healthStatus: 'cooldown', cooldownUntil: '2026-12-01T00:00:00.000Z' },
    ],
    activeLane: 'xhs',
    localLease: { taskId: 'task-active-1', leaseToken: 'tok-active-1', leaseEpoch: 5 },
    activeTask: { platform: 'xhs', stage: 'collecting', progress: 30, lastProgressAtMs: Date.now() },
  });

  check('sendHeartbeat success', heartbeatResult.success === true, `success=${heartbeatResult.success} err=${heartbeatResult.error}`);
  check('收到 1 个请求', mock.receivedRequests.length === 1, `len=${mock.receivedRequests.length}`);

  const body1 = mock.receivedRequests[0].body;
  console.log('\n  --- 发送 body 字段 ---');
  check('protocolVersion="3"', body1.protocolVersion === '3', `actual=${body1.protocolVersion}`);
  check('stationSessionId 是非空字符串', typeof body1.stationSessionId === 'string' && body1.stationSessionId.length > 0, `actual=${body1.stationSessionId}`);
  check('stationId 正确', body1.stationId === 'station-e2e-1');
  check('pluginVersion="2.0.55"', body1.pluginVersion === '2.0.55');
  check('mailboxCursors 含 station + xhs', body1.mailboxCursors && body1.mailboxCursors.station === 99, `actual=${JSON.stringify(body1.mailboxCursors)}`);
  check('capacity 含 xhs.monitor_patrol lane', body1.capacity && body1.capacity['xhs.monitor_patrol'] && body1.capacity['xhs.monitor_patrol'].maxReservedTasks === 1, `actual=${JSON.stringify(body1.capacity)}`);
  check('activeLeases 是数组且含 1 个', Array.isArray(body1.activeLeases) && body1.activeLeases.length === 1, `actual=${JSON.stringify(body1.activeLeases)}`);
  check('activeLeases[0].jobId=task-active-1', body1.activeLeases[0]?.jobId === 'task-active-1');
  check('activeLeases[0].lane=xhs', body1.activeLeases[0]?.lane === 'xhs');
  check('activeLeases[0].progress=30', body1.activeLeases[0]?.progress === 30);
  check('operations 是空数组', Array.isArray(body1.operations) && body1.operations.length === 0);
  check('accountReports 是数组且含 2 个', Array.isArray(body1.accountReports) && body1.accountReports.length === 2, `actual=${JSON.stringify(body1.accountReports)}`);
  check('accountReports[0].platform=xhs', body1.accountReports[0]?.platform === 'xhs');
  check('accountReports[1].healthStatus=cooldown', body1.accountReports[1]?.healthStatus === 'cooldown');

  check('不再发送旧 authorizationId', !Object.prototype.hasOwnProperty.call(body1, 'authorizationId'));
  check('不再发送旧 platformAccounts', !Object.prototype.hasOwnProperty.call(body1, 'platformAccounts'));
  check('不再发送旧 claimMode', !Object.prototype.hasOwnProperty.call(body1, 'claimMode'));
  check('不再发送旧 mailboxVersion', !Object.prototype.hasOwnProperty.call(body1, 'mailboxVersion'));
  check('不再发送旧 localLease', !Object.prototype.hasOwnProperty.call(body1, 'localLease'));

  console.log('\n  --- 解析响应 ---');
  check('mailboxVersions.station > 99', heartbeatResult.mailboxVersions?.station > 99, `actual=${JSON.stringify(heartbeatResult.mailboxVersions)}`);
  check('mailboxVersions.lanes 含 xhs.monitor_patrol', heartbeatResult.mailboxVersions?.lanes?.['xhs.monitor_patrol'] === 5);
  check('nextSync.afterMs=30000（running）', heartbeatResult.nextSync?.afterMs === 30000, `actual=${JSON.stringify(heartbeatResult.nextSync)}`);
  check('nextSync.reason=running', heartbeatResult.nextSync?.reason === 'running');
  check('shouldPollNow=true（有 activeLease 触发）', heartbeatResult.shouldPollNow === true);

  // 持久化检查
  const identity = await client.getStoredStationIdentity();
  check('identity.mailboxVersion 已更新为新版本', identity.mailboxVersion > 99, `actual=${identity.mailboxVersion}`);
  check('identity.mailboxLaneVersions 已持久化', identity.mailboxLaneVersions && identity.mailboxLaneVersions['xhs.monitor_patrol'] === 5, `actual=${JSON.stringify(identity.mailboxLaneVersions)}`);

  // =========================================================================
  // 2. session id 持久化（第二次调用应返回相同 session）
  // =========================================================================
  console.log('\n=== Test 2: stationSessionId 持久化 ===');
  const sessionBeforeClear = await client.resolveStationSessionId();
  const sessionAfterRestart = await client.resolveStationSessionId();
  check('SW 重启后 session id 不变', sessionBeforeClear === sessionAfterRestart, `${sessionBeforeClear} vs ${sessionAfterRestart}`);

  // =========================================================================
  // 3. claimCollectionTaskLease 通过 /sync claim（无活动 lease）
  // =========================================================================
  console.log('\n=== Test 3: claimCollectionTaskLease 也发 V1.1 字段 ===');
  const claimResponse = await claimCollectionTaskLease({
    serverUrl: baseUrl,
    stationId: 'station-e2e-2',
    stationToken: 'token-e2e-2',
    authorizationId: 'auth-e2e-2',
    authorizationToken: 'auth-token-e2e-2',
    capabilities: ['douyin'],
    platformAccounts: [],
    pluginVersion: '2.0.55',
    fetchFn: globalThis.fetch.bind(globalThis),
    storageArea: storage,
    store: { read: async () => null, write: async () => null, clear: async () => null },
  });

  const claimBody = mock.receivedRequests[mock.receivedRequests.length - 2].body;
  const startBody = mock.receivedRequests[mock.receivedRequests.length - 1].body;
  check('claim 请求 protocolVersion="3"', claimBody.protocolVersion === '3', `actual=${claimBody.protocolVersion}`);
  check('claim 请求 stationSessionId 非空', typeof claimBody.stationSessionId === 'string' && claimBody.stationSessionId.length > 0);
  check('claim 请求 capacity 含 douyin.governance lane', claimBody.capacity && claimBody.capacity['douyin.governance'], `actual=${JSON.stringify(claimBody.capacity)}`);
  check('claim 请求 operations 是空数组', Array.isArray(claimBody.operations) && claimBody.operations.length === 0);
  check('claim 请求不再发送 localLease', !Object.prototype.hasOwnProperty.call(claimBody, 'localLease'), `actual=${claimBody.localLease}`);
  check('start_job 请求不带 capacity', !startBody.capacity, `actual=${JSON.stringify(startBody.capacity)}`);
  check('start_job 操作已发送', startBody.operations?.[0]?.type === 'start_job', `actual=${JSON.stringify(startBody.operations)}`);
  check('claimResponse 返回任务', claimResponse.task?.id === 'job-douyin-governance-1', `actual=${JSON.stringify(claimResponse.task)}`);
  check('claimResponse 返回租约', claimResponse.lease?.leaseToken === 'lease-job-douyin-governance-1', `actual=${JSON.stringify(claimResponse.lease)}`);

  // =========================================================================
  // 4. minPluginVersion 426 拒绝
  // =========================================================================
  console.log('\n=== Test 4: minPluginVersion < 2.0.53 触发 426 ===');
  const oldResult = await client.sendHeartbeat({
    status: 'online',
    capabilities: ['xhs'],
    pluginVersion: '2.0.52',
    platformAccounts: [],
  });
  check('pluginVersion < 2.0.53 时 sendHeartbeat 返回 fail', oldResult.success === false, `success=${oldResult.success}`);
  check('status=426', oldResult.status === 426, `status=${oldResult.status}`);
  check('reasonCode=VERSION_REJECTED', oldResult.reasonCode === 'VERSION_REJECTED', `reasonCode=${oldResult.reasonCode}`);

  // =========================================================================
  // 总结
  // =========================================================================
  console.log('\n=== 总结 ===');
  if (assertionsFailed === 0) {
    console.log('全部断言通过 ✅');
  } else {
    console.log(`失败 ${assertionsFailed} 个断言 ❌`);
  }

  await mock.stop();
  process.exit(assertionsFailed === 0 ? 0 : 1);
}

main().catch((error) => {
  console.error('FATAL:', error);
  process.exit(2);
});
