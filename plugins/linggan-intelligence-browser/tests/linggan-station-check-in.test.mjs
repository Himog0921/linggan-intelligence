import test from 'node:test';
import assert from 'node:assert/strict';

import {
  activateLingganInstallationCredential,
  checkInLingganStation,
  credentialActivationRouteFromHealth,
  stationCheckInRouteFromHealth,
} from '../src/linggan/adapter.js';

const HEALTH = { routes: { station: {
  checkIn: '/api/local/stations/installations',
  credentialActivation: '/api/local/stations/installation-credentials/activate',
} } };

test('the check-in route is taken from /health, never hardcoded', () => {
  assert.equal(
    stationCheckInRouteFromHealth(HEALTH),
    '/api/local/stations/installations',
  );
  // A server that has not applied the station schema advertises nothing.
  assert.equal(stationCheckInRouteFromHealth({ routes: {} }), null);
  // Anything outside the station namespace, or carrying a query, is not a route we call.
  assert.equal(
    stationCheckInRouteFromHealth({ routes: { station: { checkIn: '/api/evil' } } }),
    null,
  );
  assert.equal(
    stationCheckInRouteFromHealth({ routes: { station: { checkIn: '/api/local/stations/x?a=1' } } }),
    null,
  );
});

test('no request is sent when the server does not advertise station check-in', async () => {
  let called = false;
  const result = await checkInLingganStation({
    installKey: 'install-1',
    pluginVersion: '0.4.8',
    health: { routes: {} },
    fetchImpl: async () => { called = true; },
  });
  assert.equal(called, false, '未通告路由时不得发出请求');
  assert.equal(result.checkedIn, false);
});

test('an install that reports in without a station says so plainly', async () => {
  const sent = [];
  const result = await checkInLingganStation({
    installKey: 'install-1',
    pluginVersion: '0.4.8',
    browserLabel: 'Chrome',
    capabilities: ['discovery_search'],
    health: HEALTH,
    fetchImpl: async (url, options) => {
      sent.push([url, JSON.parse(options.body)]);
      return { ok: true, async json() { return { state: 'awaiting_claim', installationRef: 'i-1' }; } };
    },
  });
  assert.equal(sent[0][0], 'http://localhost:3000/api/local/stations/installations');
  // 报的是「这次安装」，不含任何工位选择——工位由人在 Linggan 里决定。
  assert.deepEqual(sent[0][1], {
    installKey: 'install-1',
    installationCredential: null,
    pluginVersion: '0.4.8',
    browserLabel: 'Chrome',
    capabilities: ['discovery_search'],
  });
  assert.equal(result.state, 'awaiting_claim');
  assert.match(result.message, /还没有归位/);
});

test('an existing installation includes its active credential in heartbeat check-in', async () => {
  const sent = [];
  await checkInLingganStation({
    installKey: 'install-1',
    installationCredential: 'active-secret',
    pluginVersion: '0.8.34',
    health: HEALTH,
    fetchImpl: async (_url, options) => {
      sent.push(JSON.parse(options.body));
      return { ok: true, async json() { return { state: 'heartbeat' }; } };
    },
  });
  assert.equal(sent[0].installationCredential, 'active-secret');
});

test('a claimed install and a heartbeat are not reported the same way', async () => {
  const claimed = await checkInLingganStation({
    installKey: 'install-1', pluginVersion: '0.4.8', health: HEALTH,
    fetchImpl: async () => ({ ok: true, async json() { return {
      state: 'claimed', stationRef: 's-1', installationCredentialRef: 'c-1', installationCredential: 'one-time-secret',
    }; } }),
  });
  assert.match(claimed.message, /归位到一台工位/);
  assert.equal(claimed.installationCredentialRef, 'c-1');
  assert.equal(claimed.installationCredential, 'one-time-secret');
  const heartbeat = await checkInLingganStation({
    installKey: 'install-1', pluginVersion: '0.4.8', health: HEALTH,
    fetchImpl: async () => ({ ok: true, async json() { return { state: 'heartbeat' }; } }),
  });
  assert.match(heartbeat.message, /此前已归位/);
  assert.equal(heartbeat.installationCredential, '');
});

test('credential activation is health-advertised and sends the stored secret only in the ack body', async () => {
  assert.equal(
    credentialActivationRouteFromHealth(HEALTH),
    '/api/local/stations/installation-credentials/activate',
  );
  const sent = [];
  const result = await activateLingganInstallationCredential({
    installationRef: 'installation-1',
    credentialRef: 'credential-1',
    installationCredential: 'one-time-secret',
    health: HEALTH,
    fetchImpl: async (url, options) => {
      sent.push([url, JSON.parse(options.body)]);
      return { ok: true, async json() { return { outcome: 'activated' }; } };
    },
  });
  assert.deepEqual(result, { activated: true });
  assert.deepEqual(sent[0], [
    'http://localhost:3000/api/local/stations/installation-credentials/activate',
    {
      installationRef: 'installation-1',
      credentialRef: 'credential-1',
      installationCredential: 'one-time-secret',
    },
  ]);
  assert.equal(JSON.stringify(result).includes('one-time-secret'), false);
});

test('an unreachable server never reports itself as checked in', async () => {
  const result = await checkInLingganStation({
    installKey: 'install-1', pluginVersion: '0.4.8', health: HEALTH,
    fetchImpl: async () => { throw new Error('offline'); },
  });
  assert.equal(result.checkedIn, false);
});
