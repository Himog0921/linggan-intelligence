import test from 'node:test';
import assert from 'node:assert/strict';

import { CONTENT_PLATFORM, resolveContentPlatform } from '../src/content/contentPlatformRegistry.js';
import { createContentRouter } from '../src/content/contentRouter.js';

test('resolveContentPlatform maps douyin hostnames and defaults other hosts to xhs', () => {
  assert.equal(resolveContentPlatform('www.douyin.com'), CONTENT_PLATFORM.DOUYIN);
  assert.equal(resolveContentPlatform('m.douyin.com'), CONTENT_PLATFORM.DOUYIN);
  assert.equal(resolveContentPlatform('www.xiaohongshu.com'), CONTENT_PLATFORM.XHS);
  assert.equal(resolveContentPlatform(''), CONTENT_PLATFORM.XHS);
});

test('content router dispatches init to the resolved platform handler', async () => {
  const calls = [];
  const router = createContentRouter({
    getHostname: () => 'www.douyin.com',
    initByPlatform: {
      [CONTENT_PLATFORM.DOUYIN]: async ({ platform, hostname }) => {
        calls.push(['douyin', platform, hostname]);
      },
      [CONTENT_PLATFORM.XHS]: async ({ platform, hostname }) => {
        calls.push(['xhs', platform, hostname]);
      },
    },
  });

  const platform = await router.init();

  assert.equal(platform, CONTENT_PLATFORM.DOUYIN);
  assert.deepEqual(calls, [['douyin', CONTENT_PLATFORM.DOUYIN, 'www.douyin.com']]);
});

test('content router falls back to the default platform init for non-douyin hosts', async () => {
  const calls = [];
  const router = createContentRouter({
    getHostname: () => 'www.xiaohongshu.com',
    initByPlatform: {
      [CONTENT_PLATFORM.XHS]: async ({ platform, hostname }) => {
        calls.push(['xhs', platform, hostname]);
      },
    },
  });

  const platform = await router.init();

  assert.equal(platform, CONTENT_PLATFORM.XHS);
  assert.deepEqual(calls, [['xhs', CONTENT_PLATFORM.XHS, 'www.xiaohongshu.com']]);
});
