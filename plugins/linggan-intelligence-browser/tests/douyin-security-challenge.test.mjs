import test from 'node:test';
import assert from 'node:assert/strict';

import {
  maybeCreateDouyinSecurityChallengeError,
  pauseForDouyinSecurityChallenge,
  matchesDouyinSecurityChallengeText,
  isDouyinSecurityChallengeError,
} from '../src/platforms/douyin/securityChallenge.js';

test('matchesDouyinSecurityChallengeText recognizes common verification copy', () => {
  assert.equal(matchesDouyinSecurityChallengeText('请完成安全验证后继续访问'), true);
  assert.equal(matchesDouyinSecurityChallengeText('检测到滑块验证码，请稍后再试'), true);
  assert.equal(matchesDouyinSecurityChallengeText('普通接口错误'), false);
});

test('maybeCreateDouyinSecurityChallengeError upgrades status_code errors with verification payload copy', () => {
  const error = maybeCreateDouyinSecurityChallengeError({
    statusCode: 8,
    payload: {
      status_msg: '请完成验证后继续访问',
    },
  });

  assert.equal(isDouyinSecurityChallengeError(error), true);
  assert.equal(error.statusCode, 8);
  assert.match(error.userMessage, /安全验证/);
});

test('maybeCreateDouyinSecurityChallengeError upgrades status_code errors when page DOM already shows captcha', () => {
  const error = maybeCreateDouyinSecurityChallengeError({
    statusCode: 8,
    payload: {
      status_msg: '接口失败',
    },
    root: {
      querySelector(selector) {
        return selector.includes('captcha') ? { offsetHeight: 32 } : null;
      },
      body: {
        innerText: '',
      },
    },
  });

  assert.equal(isDouyinSecurityChallengeError(error), true);
  assert.equal(error.reason, 'status_code');
});

test('pauseForDouyinSecurityChallenge forwards readable paused message and waits for resume', async () => {
  const error = maybeCreateDouyinSecurityChallengeError({
    statusCode: 8,
    payload: {
      status_msg: '请完成验证后继续访问',
    },
  });

  const calls = [];
  const result = await pauseForDouyinSecurityChallenge(error, {
    current: 12,
    scannedImages: 4,
    onPause: async (payload) => {
      calls.push(payload);
    },
    waitIfPaused: async () => {
      calls.push({ resumed: true });
    },
    shouldStop: () => false,
  });

  assert.equal(result.handled, true);
  assert.equal(result.stopped, false);
  assert.match(result.message, /当前已采集 12 条评论/);
  assert.match(result.message, /已发现 4 张图片/);
  assert.equal(calls.length, 2);
  assert.match(calls[0].message, /安全验证/);
  assert.deepEqual(calls[1], { resumed: true });
});
