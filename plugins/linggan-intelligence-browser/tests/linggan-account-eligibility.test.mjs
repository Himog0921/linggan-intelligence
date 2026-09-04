import assert from 'node:assert/strict';
import test from 'node:test';

import {
  accountObservationFromUserInfo,
  reportPassiveAccountEligibility,
} from '../src/linggan/accountEligibilityProbe.js';
import {
  accountEligibilityRouteFromHealth,
  reportLingganAccountEligibility,
} from '../src/linggan/adapter.js';

const HEALTH = {
  routes: {
    station: {
      eligibilityReport: '/api/local/stations/account-eligibility-observations',
    },
  },
};

test('a passive page probe distinguishes authenticated identity from unknown', () => {
  assert.deepEqual(accountObservationFromUserInfo(null), {
    signal: 'signal_incomplete', rawPlatformAccountId: '',
  });
  assert.deepEqual(accountObservationFromUserInfo({ userId: 'stable-user-1' }), {
    signal: 'authenticated_observed', rawPlatformAccountId: 'stable-user-1',
  });
  assert.deepEqual(accountObservationFromUserInfo({ nickname: 'not-an-identity' }), {
    signal: 'signal_incomplete', rawPlatformAccountId: '',
  });
});

test('the passive probe emits a closed signal without caching or logging identity', async () => {
  let message;
  await reportPassiveAccountEligibility({
    readPageUser: async () => ({ userInfo: { user_id: 'stable-user-2' } }),
    sendMessage: async (value) => { message = value; return { reported: true }; },
  });
  assert.deepEqual(message, {
    action: 'lingganReportAccountEligibility',
    signal: 'authenticated_observed',
    rawPlatformAccountId: 'stable-user-2',
  });
});

test('eligibility reporting uses only the health-advertised station route', async () => {
  assert.equal(
    accountEligibilityRouteFromHealth(HEALTH),
    '/api/local/stations/account-eligibility-observations',
  );
  assert.equal(accountEligibilityRouteFromHealth({ routes: {} }), null);
  let request;
  const result = await reportLingganAccountEligibility({
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    rawPlatformAccountId: 'stable-user-3',
    signal: 'authenticated_observed',
    health: HEALTH,
    fetchImpl: async (url, options) => {
      request = { url, body: JSON.parse(options.body) };
      return {
        ok: true,
        async json() {
          return {
            outcome: 'observed',
            accountRef: '22222222-2222-4222-8222-222222222222',
            eligibilityState: 'usable',
            bindingRequired: true,
          };
        },
      };
    },
  });
  assert.equal(result.reported, true);
  assert.deepEqual(request.body, {
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    rawPlatformAccountId: 'stable-user-3',
    signal: 'authenticated_observed',
  });
  assert.match(request.url, /^http:\/\/localhost:3000\/api\/local\/stations\//);
  assert.equal(JSON.stringify(result).includes('stable-user-3'), false);
  assert.equal(JSON.stringify(result).includes('high-entropy-fixture-credential'), false);
});

test('an unavailable page reports unknown without inventing a usable identity', async () => {
  let message;
  await reportPassiveAccountEligibility({
    readPageUser: async () => { throw new Error('page state unavailable'); },
    sendMessage: async (value) => { message = value; return { reported: false }; },
  });
  assert.equal(message.signal, 'signal_incomplete');
  assert.equal(message.rawPlatformAccountId, '');
});
