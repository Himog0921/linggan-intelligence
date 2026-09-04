import assert from 'node:assert/strict';
import test from 'node:test';

import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
  claimLingganMediaAcquisition,
  checkInLingganStation,
  dispatchFailureRouteFromHealth,
  formatLingganRuntimeNotice,
  isTerminalLocalDeliveryResult,
  readLingganLocalReadiness,
  reportLingganDispatchFailure,
  taskCreationIsAccepted,
  unavailableLingganStats,
} from '../src/linggan/adapter.js';

test('media acquisition claim only permits an exact server-owned work generation', async () => {
  let request = null;
  const health = {
    routes: {
      localProducer: {
        taskCreation: '/api/local/producer/tasks',
        attemptStart: '/api/local/producer/runtime-attempts',
        submission: '/api/local/producer/runtime-submissions',
        mediaAcquisitionClaim: '/api/local/producer/media-acquisitions/claim',
      },
    },
  };
  const result = await claimLingganMediaAcquisition({
    installKey: 'installation-1',
    installationCredential: 'credential-1',
    health,
    fetchImpl: async (url, options) => {
      request = { url, options };
      return {
        ok: true,
        json: async () => ({
          decision: 'acquired',
          workRef: '11111111-1111-4111-8111-111111111111',
          observationRef: '22222222-2222-4222-8222-222222222222',
          claimGeneration: 2,
          candidateUris: ['https://sns-img-hw.xhscdn.com/cover.jpg'],
          nextPollAfterSeconds: 0,
        }),
      };
    },
  });
  assert.equal(result.mayExecute, true);
  assert.equal(result.claimGeneration, 2);
  assert.deepEqual(result.candidateUris, ['https://sns-img-hw.xhscdn.com/cover.jpg']);
  assert.equal(request.url, `${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-acquisitions/claim`);
  assert.deepEqual(JSON.parse(request.options.body), {
    installKey: 'installation-1',
    installationCredential: 'credential-1',
  });
});

test('a page-start failure is returned only through the server-advertised local dispatch route', async () => {
  let request = null;
  const health = {
    routes: {
      dispatch: {
        claim: '/api/local/dispatch/claim',
        failure: '/api/local/dispatch/failures',
      },
    },
  };
  assert.equal(dispatchFailureRouteFromHealth(health), '/api/local/dispatch/failures');
  assert.equal(dispatchFailureRouteFromHealth({ routes: { dispatch: { failure: '/unsafe?retry=1' } } }), null);
  const result = await reportLingganDispatchFailure({
    installKey: 'installation-1',
    installationCredential: 'credential-1',
    taskId: '11111111-1111-4111-8111-111111111111',
    failureId: '22222222-2222-4222-8222-222222222222',
    failureCode: 'page_timeout',
    health,
    fetchImpl: async (url, options) => {
      request = { url, options };
      return {
        ok: true,
        json: async () => ({
          outcome: 'requeued',
          taskState: 'pending',
          nextPollAfterSeconds: 60,
        }),
      };
    },
  });
  assert.deepEqual(result, { reported: true, outcome: 'requeued', nextPollAfterSeconds: 60 });
  assert.equal(request.url, `${LINGGAN_LOCAL_ORIGIN}/api/local/dispatch/failures`);
  assert.deepEqual(JSON.parse(request.options.body), {
    installKey: 'installation-1',
    installationCredential: 'credential-1',
    taskId: '11111111-1111-4111-8111-111111111111',
    failureId: '22222222-2222-4222-8222-222222222222',
    failureCode: 'page_timeout',
  });
});

test('runtime notice describes automatic claim without overstating admission', () => {
  assert.match(formatLingganRuntimeNotice(), /本机可靠队列/);
  assert.match(formatLingganRuntimeNotice(), /自动领取/);
  assert.match(formatLingganRuntimeNotice(), /再由 Linggan 接纳/);
});

test('station check-in only echoes the server-confirmed canonical station name', async () => {
  let request = null;
  const result = await checkInLingganStation({
    installKey: 'installation-1',
    installationCredential: 'credential-1',
    pluginVersion: '0.8.37',
    browserLabel: 'Chrome',
    capabilities: ['author_profile'],
    health: { routes: { station: { checkIn: '/api/local/stations/installations' } } },
    fetchImpl: async (url, options) => {
      request = { url, options };
      return {
        ok: true,
        json: async () => ({
          state: 'heartbeat',
          installationRef: 'server-installation-ref',
          stationRef: 'server-station-ref',
          stationDisplayName: '本机 Chrome',
          stationAccepting: true,
        }),
      };
    },
  });
  assert.equal(request.url, `${LINGGAN_LOCAL_ORIGIN}/api/local/stations/installations`);
  assert.deepEqual(JSON.parse(request.options.body), {
    installKey: 'installation-1',
    installationCredential: 'credential-1',
    pluginVersion: '0.8.37',
    browserLabel: 'Chrome',
    capabilities: ['author_profile'],
  });
  assert.equal(result.stationRef, 'server-station-ref');
  assert.equal(result.stationDisplayName, '本机 Chrome');
  assert.equal(result.stationAccepting, true);
});

test('unread Linggan stats stay explicitly unavailable instead of becoming zero', () => {
  assert.deepEqual(unavailableLingganStats(), {
    success: true,
    statsState: 'not_connected',
    notes: null,
    comments: null,
    authors: null,
    source: 'linggan_data_not_read',
  });
});

test('only created or replayed tasks and started or replayed attempts may continue to submission', () => {
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'created' } }), true);
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'replay' } }), true);
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'conflict' } }), false);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'started' } }), true);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'replay' } }), true);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'conflict' } }), false);
  assert.equal(isTerminalLocalDeliveryResult({ ok: false, status: 409, payload: { code: 'task_spec_conflict' } }), true);
});

test('legacy local trusted health remains reachable but is not delivery-ready', async () => {
  let received = null;
  const result = await readLingganLocalReadiness(async (url, options) => {
    received = { url, options };
    return {
      ok: true,
      json: async () => ({
        service: 'linggan-local-web',
        listener: 'loopback-only',
        dataState: 'LOCAL_TRUSTED_PRODUCER',
        database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
        routes: { localProducer: null },
      }),
    };
  });
  assert.equal(result.connected, false);
  assert.equal(result.reachable, true);
  assert.equal(result.deliveryReady, false);
  assert.equal(result.producerRoutes, undefined);
  assert.equal(received.url, `${LINGGAN_LOCAL_ORIGIN}/health`);
  assert.equal(received.options.credentials, 'omit');
  assert.match(result.message, /尚未升级/);
});

test('local readiness accepts the exact full Browser Producer runtime contract without credentials', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web',
      listener: 'loopback-only',
      dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
      database: { state: 'READY', schema: 'PLUGIN_RUNTIME_002_SCHEMA_READY' },
      routes: {
        localProducer: {
          taskCreation: '/api/local/producer/tasks',
          attemptStart: '/api/local/producer/runtime-attempts',
          submission: '/api/local/producer/runtime-submissions',
          mediaAcquisitionClaim: '/api/local/producer/media-acquisitions/claim',
        },
      },
    }),
  }));
  assert.equal(result.connected, true);
  assert.equal(result.reachable, true);
  assert.equal(result.deliveryReady, true);
  assert.match(result.message, /LINGGAN_BROWSER_PRODUCER_RUNTIME/);
});

test('local readiness keeps a mixed producer readiness pair reachable but not delivery-ready', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web',
      listener: 'loopback-only',
      dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
      database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
      routes: {
        localProducer: {
          taskCreation: '/api/local/producer/tasks',
          attemptStart: '/api/local/producer/runtime-attempts',
          submission: '/api/local/producer/runtime-submissions',
        },
      },
    }),
  }));
  assert.equal(result.connected, false);
  assert.equal(result.reachable, true);
  assert.equal(result.deliveryReady, false);
});

test('an older local read projection stays reachable but is not Producer delivery-ready', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web',
      listener: 'loopback-only',
      dataState: 'LOCAL_DISCOVERY_READ_PROJECTION',
      database: { state: 'READY', schema: 'LOCAL_001_SCHEMA_READY' },
      routes: { localProducer: { taskCreation: '/api/local/discovery-packages' } },
    }),
  }));
  assert.equal(result.connected, false);
  assert.equal(result.reachable, true);
  assert.equal(result.deliveryReady, false);
  assert.match(result.message, /可访问/);
});

test('a partial producer route bundle remains reachable but never invents a delivery path', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web', listener: 'loopback-only', dataState: 'LOCAL_TRUSTED_PRODUCER',
      database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
      routes: { localProducer: { taskCreation: '/api/local/producer/tasks' } },
    }),
  }));
  assert.equal(result.connected, false);
  assert.equal(result.reachable, true);
  assert.equal(result.deliveryReady, false);
  assert.equal(result.producerRoutes, undefined);
});
