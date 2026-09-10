import assert from 'node:assert/strict';
import test from 'node:test';

import {
  accountObservationFromCurrentAccountHref,
  currentAccountHrefFromDocument,
  observeXhsAccountFromDocument,
  reportPassiveAccountEligibility,
} from '../src/linggan/accountEligibilityProbe.js';
import {
  accountEligibilityRouteFromHealth,
  accountObservationAvailabilityFromHealth,
  claimedTaskAccountDecision,
  confirmedAccountObservationBlocksClaim,
  reportLingganAccountEligibility,
} from '../src/linggan/adapter.js';

const HEALTH = {
  routes: {
    station: {
      eligibilityReport: '/api/local/stations/account-eligibility-observations',
      accountObservation: 'ready',
    },
  },
};

function documentWithCurrentAccount(accountId) {
  let currentAccountLink;
  const navigation = {
    querySelectorAll() {
      return [
        { getAttribute: (name) => (name === 'href' ? '/explore' : '') },
        { getAttribute: (name) => (name === 'href' ? '/notification' : '') },
        { getAttribute: (name) => (name === 'href' ? '/chat' : '') },
        currentAccountLink,
      ];
    },
  };
  currentAccountLink = {
    textContent: '我',
    getAttribute(name) {
      return name === 'href' ? `/user/profile/${accountId}` : '';
    },
    closest() {
      return navigation;
    },
  };
  return { querySelectorAll: () => [currentAccountLink], title: '', body: { innerText: '' } };
}

test('a passive account probe accepts only the explicitly marked current-account profile', () => {
  assert.equal(accountObservationFromCurrentAccountHref(''), null);
  assert.deepEqual(accountObservationFromCurrentAccountHref('/user/profile/111111111111111111111111'), {
    signal: 'authenticated_observed', rawPlatformAccountId: '111111111111111111111111',
  });
  assert.equal(accountObservationFromCurrentAccountHref('/user/profile/not-a-valid-xhs-id'), null);
  assert.deepEqual(
    accountObservationFromCurrentAccountHref(
      'http://www.xiaohongshu.com/user/profile/111111111111111111111111',
    ),
    null,
  );
});

test('the passive account probe never mistakes the viewed creator for the logged-in account', () => {
  const navigationLinks = [];
  const globalNavigation = {
    querySelectorAll() {
      return navigationLinks;
    },
  };
  const selfLookingProfileInContent = {
    textContent: '我',
    getAttribute(name) {
      return name === 'href' ? '/user/profile/cccccccccccccccccccccccc' : '';
    },
    closest() {
      return { querySelectorAll: () => [selfLookingProfileInContent] };
    },
  };
  const links = [
    selfLookingProfileInContent,
    {
      textContent: 'ADHD好爸正念成长记',
      getAttribute(name) {
        return name === 'href' ? '/user/profile/aaaaaaaaaaaaaaaaaaaaaaaa' : '';
      },
      closest() {
        return null;
      },
    },
    {
      textContent: '我',
      getAttribute(name) {
        return name === 'href' ? '/user/profile/bbbbbbbbbbbbbbbbbbbbbbbb' : '';
      },
      closest() {
        return globalNavigation;
      },
    },
  ];
  navigationLinks.push(
    { getAttribute: (name) => (name === 'href' ? '/explore' : '') },
    { getAttribute: (name) => (name === 'href' ? '/notification' : '') },
    { getAttribute: (name) => (name === 'href' ? '/chat' : '') },
    links[2],
  );
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

test('the passive probe reads one already-rendered document and sends a nested positive observation', async () => {
  let message;
  await reportPassiveAccountEligibility({
    document: documentWithCurrentAccount('dddddddddddddddddddddddd'),
    sendMessage: async (value) => { message = value; return { reported: true }; },
  });
  assert.deepEqual(message, {
    action: 'lingganReportAccountEligibility',
    observation: {
      signal: 'authenticated_observed',
      rawPlatformAccountId: 'dddddddddddddddddddddddd',
    },
  });
});

test('the passive probe emits a closed signal without caching or logging identity', async () => {
  let message;
  await reportPassiveAccountEligibility({
    document: documentWithCurrentAccount('cccccccccccccccccccccccc'),
    sendMessage: async (value) => { message = value; return { reported: true }; },
  });
  assert.deepEqual(message, {
    action: 'lingganReportAccountEligibility',
    observation: {
      signal: 'authenticated_observed',
      rawPlatformAccountId: 'cccccccccccccccccccccccc',
    },
  });
});

test('only a platform status surface becomes a negative account observation', () => {
  const loginPrompt = { innerText: '请使用已登录小红书 APP 扫码验证身份' };
  assert.deepEqual(
    observeXhsAccountFromDocument({
      querySelectorAll: () => [loginPrompt],
      title: '小红书',
      body: { innerText: '请使用已登录小红书 APP 扫码验证身份' },
    }),
    { signal: 'login_required' },
  );
  assert.equal(
    observeXhsAccountFromDocument({ querySelectorAll: () => [], title: '小红书', body: { innerText: '' } }),
    null,
  );
  assert.equal(
    observeXhsAccountFromDocument({
      querySelectorAll: () => [],
      title: '普通笔记',
      body: { innerText: '账号被限制怎么办？操作过于频繁请稍后再试。' },
    }),
    null,
    'user-authored note or comment text is not a platform account decision',
  );
});

test('only a server-confirmed negative or binding change stops an already claimed task', () => {
  const unavailable = claimedTaskAccountDecision({ reported: false });
  assert.deepEqual(unavailable, {
    mayExecute: true,
    state: 'account_observation_inconclusive',
    message: '',
  }, 'a local report failure is inconclusive and does not revoke an already claimed task');
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: false }), false);
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, eligibilityState: 'unknown' }), false);
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, eligibilityState: 'usable' }), false);
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, bindingRequired: true }), false);
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, bindingMismatch: true }), true);
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, frozenAccountMismatch: true }), true);
  assert.deepEqual(claimedTaskAccountDecision({ reported: false, claimedTaskNotHeld: true }), {
    mayExecute: false,
    state: 'account_observation_blocked',
    message: '任务页已观察到账号状态变化或限制，已停止本次采集并等待服务端恢复条件。',
  });
  assert.equal(confirmedAccountObservationBlocksClaim({ reported: true, eligibilityState: 'needs_login' }), true);
});

test('eligibility reporting uses only the health-advertised station route', async () => {
  assert.equal(
    accountEligibilityRouteFromHealth(HEALTH),
    '/api/local/stations/account-eligibility-observations',
  );
  assert.equal(accountEligibilityRouteFromHealth({ routes: {} }), null);
  assert.equal(accountObservationAvailabilityFromHealth(HEALTH), 'ready');
  assert.equal(
    accountObservationAvailabilityFromHealth({ routes: { station: { accountObservation: 'identity_key_missing' } } }),
    'identity_key_missing',
  );
  let request;
  const result = await reportLingganAccountEligibility({
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    taskId: '33333333-3333-4333-8333-333333333333',
    observation: { signal: 'authenticated_observed', rawPlatformAccountId: 'stable-user-3' },
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
            bindingMismatch: false,
            frozenAccountMismatch: false,
          };
        },
      };
    },
  });
  assert.equal(result.reported, true);
  assert.deepEqual(request.body, {
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    taskId: '33333333-3333-4333-8333-333333333333',
    observation: {
      signal: 'authenticated_observed',
      rawPlatformAccountId: 'stable-user-3',
    },
  });
  assert.match(request.url, /^http:\/\/localhost:3000\/api\/local\/stations\//);
  assert.equal(JSON.stringify(result).includes('stable-user-3'), false);
  assert.equal(JSON.stringify(result).includes('high-entropy-fixture-credential'), false);
});

test('a missing identity key still sends a closed negative account observation', async () => {
  let request;
  const result = await reportLingganAccountEligibility({
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    observation: { signal: 'login_required' },
    health: {
      routes: {
        station: {
          eligibilityReport: '/api/local/stations/account-eligibility-observations',
          accountObservation: 'identity_key_missing',
        },
      },
    },
    fetchImpl: async (_url, options) => {
      request = JSON.parse(options.body);
      return {
        ok: true,
        async json() {
          return { outcome: 'observed', eligibilityState: 'needs_login' };
        },
      };
    },
  });
  assert.equal(result.reported, true);
  assert.equal(result.eligibilityState, 'needs_login');
  assert.deepEqual(request.observation, { signal: 'login_required' });
  assert.equal(JSON.stringify(request).includes('high-entropy-fixture-credential'), true,
    'the credential belongs only to the loopback request body');
  assert.equal(JSON.stringify(result).includes('high-entropy-fixture-credential'), false);
});

test('a server-confirmed missing claimed task stops the page without exposing task context', async () => {
  const result = await reportLingganAccountEligibility({
    installationRef: '11111111-1111-4111-8111-111111111111',
    installationCredential: 'high-entropy-fixture-credential',
    taskId: '33333333-3333-4333-8333-333333333333',
    observation: { signal: 'authenticated_observed', rawPlatformAccountId: 'stable-user-3' },
    health: HEALTH,
    fetchImpl: async () => ({
      ok: false,
      async json() { return { code: 'account_observation_claim_not_held' }; },
    }),
  });
  assert.deepEqual(result, {
    reported: false,
    reasonCode: 'account_observation_claim_not_held',
    claimedTaskNotHeld: true,
  });
  assert.equal(JSON.stringify(result).includes('stable-user-3'), false);
});

test('an unavailable page does not turn missing positive evidence into a negative account report', async () => {
  let message;
  const result = await reportPassiveAccountEligibility({
    document: { querySelectorAll: () => [], title: '', body: { innerText: '' } },
    sendMessage: async (value) => { message = value; return { reported: false }; },
  });
  assert.deepEqual(result, { reported: false, reason: 'account_observation_inconclusive' });
  assert.equal(message, undefined);
});
