/**
 * sync-envelope-v11.test.mjs
 *
 * 单元测试：V1.1 /sync 协议字段构造 + 响应解析。
 *
 * 覆盖：
 *   - buildSyncRequestV11 输出纯 V1.1 字段
 *   - stationSessionId 持久化 + 清理
 *   - mailboxCursors 构造（station + lanes）
 *   - capacity 按 lane 推导（capabilities + activeLane）
 *   - activeLeases[] 从 localLease 转换（含 lane/progress/stage/lastProgressAt）
 *   - accountReports[] 从 platformAccounts 转换（含字段映射）
 *   - extractMailboxVersionsFromResponse（V1.1 mailboxVersions）
 *   - extractNextSyncFromResponse（V1.1 nextSync）
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  buildSyncRequestV11,
  buildMailboxCursors,
  buildLaneCapacity,
  buildActiveLeases,
  buildAccountReports,
  resolveStationSessionId,
  clearStationSessionId,
  extractMailboxVersionsFromResponse,
  extractNextSyncFromResponse,
  SYNC_PROTOCOL_VERSION_V11,
  SYNC_MIN_PLUGIN_VERSION_V11,
} from '../src/workbench/protocol/syncEnvelopeV11.js';

// --- 内存版 chrome.storage.local ---
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

test('SYNC_PROTOCOL_VERSION_V11 是 "3"，与服务端对齐', () => {
  assert.equal(SYNC_PROTOCOL_VERSION_V11, '3');
  assert.equal(SYNC_MIN_PLUGIN_VERSION_V11, '2.0.58');
});

test('resolveStationSessionId 持久化（同 storageArea 多次调用返回相同 id）', async () => {
  const storage = createMemoryStorage();
  const uuids = ['sess-1', 'sess-2'];
  let i = 0;
  const randomUUID = () => uuids[i++];

  const a = await resolveStationSessionId({ storageArea: storage, randomUUID });
  const b = await resolveStationSessionId({ storageArea: storage, randomUUID });
  assert.equal(a, 'sess-1');
  assert.equal(b, 'sess-1', '第二次调用应返回缓存的 sess-1');
  assert.equal(i, 1, 'randomUUID 只应调用一次（缓存命中）');
});

test('clearStationSessionId 后 resolve 生成新 id', async () => {
  const storage = createMemoryStorage();
  const uuids = ['sess-A', 'sess-B'];
  let i = 0;
  const randomUUID = () => uuids[i++];

  const a = await resolveStationSessionId({ storageArea: storage, randomUUID });
  await clearStationSessionId({ storageArea: storage });
  const b = await resolveStationSessionId({ storageArea: storage, randomUUID });
  assert.equal(a, 'sess-A');
  assert.equal(b, 'sess-B');
});

test('buildMailboxCursors 构造 station + lanes 对象', () => {
  const cursors = buildMailboxCursors({
    stationVersion: 42,
    laneVersions: { 'xhs.monitor_patrol': 5, 'douyin.governance': 7 },
  });
  assert.deepEqual(cursors, { station: 42, 'xhs.monitor_patrol': 5, 'douyin.governance': 7 });
});

test('buildMailboxCursors 忽略非法 lane/version', () => {
  const cursors = buildMailboxCursors({
    stationVersion: 42,
    laneVersions: { '': 1, xhs: 'invalid', douyin: 3 },
  });
  assert.deepEqual(cursors, { station: 42, douyin: 3 });
});

test('buildMailboxCursors 缺失 station 时返回空对象（不输出 station 键）', () => {
  const cursors = buildMailboxCursors({ stationVersion: undefined, laneVersions: {} });
  assert.deepEqual(cursors, {});
});

test('buildLaneCapacity 从 capabilities + activeLane 推导 lane', () => {
  const capacity = buildLaneCapacity({
    capabilities: ['xhs.list_scan', 'xhs.author_links', 'douyin.list_scan', 'other'],
    activeLane: 'xhs',
  });
  assert.deepEqual(capacity, {
    'xhs.monitor_patrol': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'xhs.monitor_checkpoint': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'xhs.manual_hot': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'xhs.archive': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'douyin.governance': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
  });
});

test('buildLaneCapacity 无已知 lane 时返回空对象', () => {
  const capacity = buildLaneCapacity({ capabilities: ['other'], activeLane: '' });
  assert.deepEqual(capacity, {});
});

test('buildActiveLeases 把 localLease 单对象转换为单元素数组', () => {
  const leases = buildActiveLeases({
    localLease: { taskId: 'task-1', leaseToken: 'tok-1', leaseEpoch: 3 },
    activeTask: { platform: 'xhs', stage: 'collecting', progress: 50, lastProgressAtMs: 1700000000000 },
  });
  assert.equal(leases.length, 1);
  assert.equal(leases[0].jobId, 'task-1');
  assert.equal(leases[0].leaseToken, 'tok-1');
  assert.equal(leases[0].leaseEpoch, 3);
  assert.equal(leases[0].lane, 'xhs');
  assert.equal(leases[0].stage, 'collecting');
  assert.equal(leases[0].progress, 50);
  assert.equal(leases[0].lastProgressAt, '2023-11-14T22:13:20.000Z');
});

test('buildActiveLeases 无 localLease 返回空数组', () => {
  assert.deepEqual(buildActiveLeases({ localLease: null }), []);
  assert.deepEqual(buildActiveLeases({}), []);
});

test('buildActiveLeases 缺 leaseToken 返回空数组', () => {
  const leases = buildActiveLeases({ localLease: { taskId: 'task-1' } });
  assert.deepEqual(leases, []);
});

test('buildAccountReports 从 platformAccounts 转换字段', () => {
  const reports = buildAccountReports([
    { platform: 'xhs', id: 'acc-1', health: 'healthy' },
    { platform: 'douyin', platformAccountId: 'acc-2', healthStatus: 'cooldown', cooldownUntil: '2026-12-01T00:00:00.000Z' },
    { platform: '', healthStatus: 'ignored' }, // 空 platform 被过滤
    'not-an-object',
  ]);
  assert.deepEqual(reports, [
    { platform: 'xhs', healthStatus: 'healthy', platformAccountId: 'acc-1' },
    { platform: 'douyin', healthStatus: 'cooldown', platformAccountId: 'acc-2', cooldownUntil: '2026-12-01T00:00:00.000Z' },
  ]);
});

test('buildAccountReports 缺 healthStatus 时填 unknown', () => {
  const reports = buildAccountReports([{ platform: 'xhs' }]);
  assert.equal(reports[0].healthStatus, 'unknown');
});

test('buildSyncRequestV11 输出纯 V1.1 字段且不混入非协议字段', () => {
  const body = buildSyncRequestV11({
    stationId: 'station-1',
    stationToken: 'token-1',
    pluginVersion: '2.0.55',
    stationSessionId: 'sess-1',
    capabilities: ['xhs.list_scan'],
    platformAccounts: [{ platform: 'xhs', id: 'acc-1', health: 'healthy' }],
    localLease: { taskId: 'task-1', leaseToken: 'tok-1', leaseEpoch: 3 },
    activeTask: { platform: 'xhs' },
    mailboxStationVersion: 12,
    mailboxLaneVersions: { 'xhs.monitor_patrol': 5 },
  });

  // V1.1 字段
  assert.equal(body.stationId, 'station-1');
  assert.equal(body.stationToken, 'token-1');
  assert.equal(body.pluginVersion, '2.0.55');
  assert.equal(body.protocolVersion, '3');
  assert.equal(body.stationSessionId, 'sess-1');
  assert.deepEqual(body.capabilities, ['xhs.list_scan']);
  assert.deepEqual(body.mailboxCursors, { station: 12, 'xhs.monitor_patrol': 5 });
  assert.deepEqual(body.capacity, {
    'xhs.monitor_patrol': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'xhs.monitor_checkpoint': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
    'xhs.manual_hot': { remainingWorkSeconds: 0, targetWorkSeconds: 600, maxReservedTasks: 1 },
  });
  assert.equal(body.activeLeases.length, 1);
  assert.equal(body.activeLeases[0].jobId, 'task-1');
  assert.deepEqual(body.operations, []);
  assert.equal(body.accountReports.length, 1);
  assert.equal(body.accountReports[0].platform, 'xhs');

  assert.deepEqual(Object.keys(body).sort(), [
    'accountReports',
    'activeLeases',
    'capabilities',
    'capacity',
    'mailboxCursors',
    'operations',
    'pluginVersion',
    'protocolVersion',
    'stationId',
    'stationSessionId',
    'stationToken',
  ]);
});

test('buildSyncRequestV11 在 mailboxStationVersion 未定义时不发 mailboxCursors/mailboxVersion', () => {
  const body = buildSyncRequestV11({
    stationId: 's1', stationToken: 't1',
    pluginVersion: '2.0.55', stationSessionId: 'sess',
    capabilities: [], platformAccounts: [],
  });
  assert.equal(Object.prototype.hasOwnProperty.call(body, 'mailboxCursors'), false);
  assert.deepEqual(body.operations, []);
});

test('extractMailboxVersionsFromResponse 解析 V1.1 响应', () => {
  const result = extractMailboxVersionsFromResponse({
    mailboxVersions: { station: 42, 'xhs.monitor_patrol': 5, 'douyin.governance': 7 },
  });
  assert.equal(result.station, 42);
  assert.deepEqual(result.lanes, { 'xhs.monitor_patrol': 5, 'douyin.governance': 7 });
});

test('extractNextSyncFromResponse 解析 V1.1 对象', () => {
  const result = extractNextSyncFromResponse({ nextSync: { afterMs: 30000, reason: 'running' } });
  assert.equal(result.afterMs, 30000);
  assert.equal(result.reason, 'running');
});

test('extractNextSyncFromResponse 无字段时返回默认 60s', () => {
  const result = extractNextSyncFromResponse({});
  assert.equal(result.afterMs, 60000);
  assert.equal(result.reason, 'default_interval');
});
