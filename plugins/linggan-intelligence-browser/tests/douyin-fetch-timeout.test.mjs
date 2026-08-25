import test from 'node:test';
import assert from 'node:assert/strict';

import { fetchDouyinWithTimeout } from '../src/platforms/douyin/fetchWithTimeout.js';

test('douyin fetch helper aborts a stalled platform request', async () => {
  let receivedSignal = null;

  await assert.rejects(
    fetchDouyinWithTimeout('/aweme/v1/web/user/profile/other/', { credentials: 'include' }, {
      timeoutMs: 1,
      fetchImpl: (_url, options = {}) => {
        receivedSignal = options.signal;
        return new Promise((_resolve, reject) => {
          options.signal.addEventListener('abort', () => reject(new Error('aborted')));
        });
      },
    }),
    (error) => {
      assert.equal(error.code, 'douyin_fetch_timeout');
      return /抖音接口请求超时/.test(error.message);
    },
  );

  assert.equal(receivedSignal?.aborted, true);
});

test('douyin fetch helper keeps credentials and returns successful responses', async () => {
  const response = { ok: true };
  const result = await fetchDouyinWithTimeout('/aweme/v1/web/aweme/post/', { credentials: 'include' }, {
    timeoutMs: 50,
    fetchImpl: async (_url, options = {}) => {
      assert.equal(options.credentials, 'include');
      assert.ok(options.signal);
      return response;
    },
  });

  assert.equal(result, response);
});
