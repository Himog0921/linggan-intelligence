import test from 'node:test';
import assert from 'node:assert/strict';

import { fetchDetailApiData } from '../src/platforms/douyin/videoApiData.js';

test('fetchDetailApiData can rethrow security challenge when caller opts out of swallowing errors', async (t) => {
  const originalFetch = globalThis.fetch;
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;

  globalThis.window = {
    location: {
      href: 'https://www.douyin.com/video/9003',
      origin: 'https://www.douyin.com',
      protocol: 'https:',
    },
  };
  globalThis.document = {
    querySelector: () => null,
    body: { innerText: '' },
    documentElement: { innerText: '' },
  };
  globalThis.fetch = async () => ({
    ok: true,
    json: async () => ({
      status_code: 8,
      status_msg: '请完成验证后继续访问',
    }),
  });

  t.after(() => {
    globalThis.fetch = originalFetch;
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  });

  await assert.rejects(
    () => fetchDetailApiData('9003', { suppressErrors: false }),
    (error) => {
      assert.equal(error?.name, 'DouyinSecurityChallengeError');
      assert.equal(error?.code, 'douyin_security_verification');
      return true;
    },
  );
});
