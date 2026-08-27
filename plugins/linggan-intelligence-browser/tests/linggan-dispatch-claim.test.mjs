import test from 'node:test';
import assert from 'node:assert/strict';

import {
  claimLingganDispatch,
  dispatchClaimRouteFromHealth,
} from '../src/linggan/adapter.js';

const HEALTH = { routes: { dispatch: { claim: '/api/local/dispatch/claim' } } };

test('the claim route comes from /health and is namespace-checked', () => {
  assert.equal(dispatchClaimRouteFromHealth(HEALTH), '/api/local/dispatch/claim');
  assert.equal(dispatchClaimRouteFromHealth({ routes: {} }), null);
  assert.equal(
    dispatchClaimRouteFromHealth({ routes: { dispatch: { claim: '/api/evil' } } }),
    null,
  );
});

test('a task body without permission is not carried out of the claim', async () => {
  // 服务端可能在不允许执行时仍带回任务体。只要下游还能拿到它，就总有人会「反正拿到了
  // 就跑」——那绕过的正是整条授权链。因此不许执行时任务体不带出去。
  const result = await claimLingganDispatch({
    installKey: 'i-1',
    health: HEALTH,
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return {
          decision: 'execution_gate_closed',
          mayExecute: false,
          taskSpec: { capabilitiesRequested: ['author_profile'] },
          reason: '真实执行闸门未开。',
        };
      },
    }),
  });
  assert.equal(result.mayExecute, false);
  assert.equal(result.taskSpec, null, '不许执行时不得带出任务体');
  assert.equal(result.leaseRef, '');
});

test('only an explicit permission is treated as permission', async () => {
  for (const body of [{ decision: 'dispatch' }, { decision: 'dispatch', mayExecute: 'true' }]) {
    const result = await claimLingganDispatch({
      installKey: 'i-1', health: HEALTH,
      fetchImpl: async () => ({ ok: true, async json() { return body; } }),
    });
    // 缺字段、字符串 'true' 都不算许可——只认布尔真。
    assert.equal(result.mayExecute, false);
  }
});

test('a permitted claim carries the task and its lease', async () => {
  const result = await claimLingganDispatch({
    installKey: 'i-1', health: HEALTH,
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return {
          decision: 'dispatch', mayExecute: true, leaseRef: 'lease-1',
          taskSpec: { capabilitiesRequested: ['author_profile'], maximumQuota: 1 },
        };
      },
    }),
  });
  assert.equal(result.mayExecute, true);
  assert.equal(result.leaseRef, 'lease-1');
  assert.deepEqual(result.taskSpec.capabilitiesRequested, ['author_profile']);
});

test('an unreachable server never reports permission', async () => {
  const result = await claimLingganDispatch({
    installKey: 'i-1', health: HEALTH,
    fetchImpl: async () => { throw new Error('offline'); },
  });
  assert.equal(result.mayExecute, false);
  assert.equal(result.taskSpec, undefined);
});
