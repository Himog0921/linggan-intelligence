import test from 'node:test';
import assert from 'node:assert/strict';

import {
  claimLingganDispatch,
  decodePageExecutionReceipt,
  detailPageRiskSignalRouteFromHealth,
  detailPageSessionGrantRouteFromHealth,
  grantLingganDetailPageSession,
  dispatchClaimRouteFromHealth,
  reportLingganDetailPageRiskSignal,
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

test('detail-page grant route is separately advertised and a lost response can replay the same grant', async () => {
  const health = { routes: { dispatch: { detailPageSessionGrant: '/api/local/dispatch/detail-page-sessions/grant' } } };
  assert.equal(detailPageSessionGrantRouteFromHealth(health), '/api/local/dispatch/detail-page-sessions/grant');
  assert.equal(detailPageSessionGrantRouteFromHealth({ routes: { dispatch: { detailPageSessionGrant: '/api/other' } } }), null);
  const bodies = [
    { outcome: 'authorized', sessionRef: 'session-1', pageSessionPlan: { contractVersion: 'linggan.detail-page-session.v1' } },
    { outcome: 'replay', sessionRef: 'session-1', pageSessionPlan: { contractVersion: 'linggan.detail-page-session.v1' } },
  ];
  const requests = [];
  for (const body of bodies) {
    const result = await grantLingganDetailPageSession({
      installKey: 'install-1', installationCredential: 'credential-1', taskId: 'task-1', grantRequestId: 'request-1',
      executionSourceUrl: 'https://www.xiaohongshu.com/explore/note-1?xsec_token=fixture', health,
      fetchImpl: async (url, options) => {
        requests.push({ url, body: JSON.parse(options.body) });
        return { ok: true, async json() { return body; } };
      },
    });
    assert.equal(result.granted, true);
    assert.equal(result.sessionRef, 'session-1');
  }
  assert.deepEqual(requests.map(({ body }) => body.executionSourceUrl), [
    'https://www.xiaohongshu.com/explore/note-1?xsec_token=fixture',
    'https://www.xiaohongshu.com/explore/note-1?xsec_token=fixture',
  ]);
  assert.ok(requests.every(({ url }) => /\/api\/local\/dispatch\//.test(url)));
});

test('a detail risk signal only uses the health-advertised local route and returns no credential', async () => {
  const health = { routes: { dispatch: { detailPageRiskSignal: '/api/local/dispatch/detail-page-sessions/risk-signals' } } };
  assert.equal(detailPageRiskSignalRouteFromHealth(health), '/api/local/dispatch/detail-page-sessions/risk-signals');
  assert.equal(detailPageRiskSignalRouteFromHealth({ routes: { dispatch: { detailPageRiskSignal: '/api/other' } } }), null);
  let request;
  const result = await reportLingganDetailPageRiskSignal({
    installKey: 'install-1', installationCredential: 'credential-1', taskId: 'task-1',
    riskSignalId: 'signal-1', detectorVersion: 'xhs-risk-interstitial.v1', health,
    fetchImpl: async (url, options) => {
      request = { url, body: JSON.parse(options.body) };
      return { ok: true, async json() { return { outcome: 'recorded', cooldownActive: true, cooldownUntil: '2026-09-21T00:00:00Z' }; } };
    },
  });
  assert.deepEqual(request.body, {
    installKey: 'install-1', installationCredential: 'credential-1', taskId: 'task-1',
    riskSignalId: 'signal-1', detectorVersion: 'xhs-risk-interstitial.v1',
  });
  assert.match(request.url, /^http:\/\/localhost:3000\/api\/local\/dispatch\//);
  assert.deepEqual(result, { reported: true, cooldownActive: true, cooldownUntil: '2026-09-21T00:00:00Z' });
  assert.equal(JSON.stringify(result).includes('credential-1'), false);
});

test('a task body without permission is not carried out of the claim', async () => {
  // 服务端可能在不允许执行时仍带回任务体。只要下游还能拿到它，就总有人会「反正拿到了
  // 就跑」——那绕过的正是整条授权链。因此不许执行时任务体不带出去。
  const result = await claimLingganDispatch({
    installKey: 'i-1',
    installationCredential: 'credential-1',
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
      installKey: 'i-1', installationCredential: 'credential-1', health: HEALTH,
      fetchImpl: async () => ({ ok: true, async json() { return body; } }),
    });
    // 缺字段、字符串 'true' 都不算许可——只认布尔真。
    assert.equal(result.mayExecute, false);
  }
});

test('a permitted claim carries the task and its lease', async () => {
  const taskSpec = {
    contractVersion: 'linggan.producer.task-spec.v1',
    taskId: '11111111-1111-4111-8111-111111111111',
    source: 'scheduled',
    platform: 'xhs',
    pageType: 'profile',
    target: { authorExternalId: 'author-1' },
    capabilitiesRequested: ['author_profile'],
    maximumQuota: 1,
    commentLimit: 'not_requested',
    acquireMedia: 'not_requested',
    riskPolicy: 'server_authorized_leased',
    stopConditions: ['maximum_quota'],
  };
  const result = await claimLingganDispatch({
    installKey: 'i-1', installationCredential: 'credential-1', health: HEALTH,
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return {
          decision: 'dispatch', mayExecute: true, leaseRef: 'lease-1',
          taskSpec, nextPollAfterSeconds: 0,
        };
      },
    }),
  });
  assert.equal(result.mayExecute, true);
  assert.equal(result.leaseRef, 'lease-1');
  assert.deepEqual(result.taskSpec.capabilitiesRequested, ['author_profile']);
  assert.equal(result.nextPollAfterSeconds, 0, 'server-authorized immediate follow-up remains valid');
});

test('a malformed permitted claim is rejected before it controls a page', async () => {
  const result = await claimLingganDispatch({
    installKey: 'i-1', installationCredential: 'credential-1', health: HEALTH,
    fetchImpl: async () => ({
      ok: true,
      async json() {
        return {
          decision: 'dispatch', mayExecute: true, leaseRef: 'lease-1',
          taskSpec: { capabilitiesRequested: ['comments'] },
        };
      },
    }),
  });
  assert.equal(result.mayExecute, false);
  assert.equal(result.decision, 'invalid_dispatch');
  assert.equal(result.taskSpec, null);
});

test('page execution requires an explicit receipt with the dispatched identity', () => {
  const expected = { action: 'collect_comments', capability: 'comments', taskId: 'task-1' };
  assert.equal(decodePageExecutionReceipt(undefined, expected).state, 'page_receipt_missing');
  assert.equal(decodePageExecutionReceipt({ success: true }, expected).state, 'page_receipt_identity_mismatch');
  assert.deepEqual(
    decodePageExecutionReceipt({ success: true, ...expected, state: 'page_read_started' }, expected),
    { ok: true, state: 'page_read_started', message: '' },
  );
});

test('an unreachable server never reports permission', async () => {
  const result = await claimLingganDispatch({
    installKey: 'i-1', installationCredential: 'credential-1', health: HEALTH,
    fetchImpl: async () => { throw new Error('offline'); },
  });
  assert.equal(result.mayExecute, false);
  assert.equal(result.taskSpec, undefined);
});
