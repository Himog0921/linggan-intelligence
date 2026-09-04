import assert from 'node:assert/strict';
import test from 'node:test';

import {
  accountObservationFromCurrentAccountHref,
  currentAccountHrefFromDocument,
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

test('a passive account probe accepts only the explicitly marked current-account profile', () => {
  assert.deepEqual(accountObservationFromCurrentAccountHref(''), {
    signal: 'signal_incomplete', rawPlatformAccountId: '',
  });
  assert.deepEqual(accountObservationFromCurrentAccountHref('/user/profile/111111111111111111111111'), {
    signal: 'authenticated_observed', rawPlatformAccountId: '111111111111111111111111',
  });
  assert.deepEqual(accountObservationFromCurrentAccountHref('/user/profile/not-a-valid-xhs-id'), {
    signal: 'signal_incomplete', rawPlatformAccountId: '',
  });
});

test('the passive account probe never mistakes the viewed creator for the logged-in account', () => {
  const links = [
    {
      textContent: 'ADHD好爸正念成长记',
      getAttribute(name) {
        return name === 'href' ? '/user/profile/aaaaaaaaaaaaaaaaaaaaaaaa' : '';
      },
    },
    {
      textContent: '我',
      getAttribute(name) {
        return name === 'href' ? '/user/profile/bbbbbbbbbbbbbbbbbbbbbbbb' : '';
      },
    },
  ];
  const doc = { querySelectorAll: () => links };
  assert.equal(
    currentAccountHrefFromDocument(doc),
    '/user/profile/bbbbbbbbbbbbbbbbbbbbbbbb',
  );
  assert.deepEqual(
    accountObservationFromCurrentAccountHref(currentAccountHrefFromDocument(doc)),
    {
      signal: 'authenticated_observed',
      rawPlatformAccountId: 'bbbbbbbbbbbbbbbbbbbbbbbb',
    },
  );
});

test('the passive probe emits a closed signal without caching or logging identity', async () => {
  let message;
  await reportPassiveAccountEligibility({
    readCurrentAccountHref: async () => '/user/profile/cccccccccccccccccccccccc',
    sendMessage: async (value) => { message = value; return { reported: true }; },
  });
  assert.deepEqual(message, {
    action: 'lingganReportAccountEligibility',
    signal: 'authenticated_observed',
    rawPlatformAccountId: 'cccccccccccccccccccccccc',
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
    readCurrentAccountHref: async () => { throw new Error('page state unavailable'); },
    sendMessage: async (value) => { message = value; return { reported: false }; },
  });
  assert.equal(message.signal, 'signal_incomplete');
  assert.equal(message.rawPlatformAccountId, '');
});
