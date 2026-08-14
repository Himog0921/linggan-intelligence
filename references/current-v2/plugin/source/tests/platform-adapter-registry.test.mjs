import test from 'node:test';
import assert from 'node:assert/strict';

import { createPlatformAdapterRegistry, PLATFORM_ID } from '../src/platforms/registry.js';

test('platform registry exposes one adapter contract for xhs and douyin', () => {
  const registry = createPlatformAdapterRegistry({
    xhs: {
      detectPage: () => ({ type: 'profile', url: 'https://www.xiaohongshu.com/user/profile/u1' }),
      getWindow: () => ({ location: { href: 'https://www.xiaohongshu.com/user/profile/u1' } }),
    },
    douyin: {
      detectDouyinPageType: () => ({ type: 'profile', url: 'https://www.douyin.com/user/u1' }),
      detectDouyinSearchBatchContext: () => ({ stableSearchList: false, keyword: '' }),
      detectDouyinSecurityChallenge: () => false,
      isStrictDouyinDetailPage: () => false,
      getWindow: () => ({ location: { href: 'https://www.douyin.com/user/u1' } }),
      getDocument: () => ({}),
    },
  });

  for (const platform of [PLATFORM_ID.XHS, PLATFORM_ID.DOUYIN]) {
    const adapter = registry.require(platform);
    assert.equal(adapter.platform, platform);
    assert.equal(typeof adapter.detectPage, 'function');
    assert.equal(typeof adapter.checkCapability, 'function');
    assert.equal(typeof adapter.normalizeTarget, 'function');
    assert.equal(typeof adapter.prepare, 'function');
    assert.equal(typeof adapter.collect, 'function');
    assert.equal(typeof adapter.pause, 'function');
    assert.equal(typeof adapter.resume, 'function');
    assert.equal(typeof adapter.stop, 'function');
    assert.equal(typeof adapter.cleanup, 'function');
  }
});

test('xhs adapter builds capability report through the shared adapter contract', async () => {
  const registry = createPlatformAdapterRegistry({
    xhs: {
      detectPage: () => ({ type: 'noteDetail', url: 'https://www.xiaohongshu.com/explore/n1' }),
      getWindow: () => ({ location: { href: 'https://www.xiaohongshu.com/explore/n1' } }),
    },
  });

  const report = await registry.require(PLATFORM_ID.XHS).checkCapability({
    taskType: 'xhs.batchNotes',
    target: {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/explore/n1',
    },
  });

  assert.equal(report.platform, PLATFORM_ID.XHS);
  assert.equal(report.mode, 'detail');
  assert.equal(report.capabilities.canCollectPrimary, true);
  assert.equal(report.capabilities.canBatchNotes, false);
  assert.equal(report.target.taskType, 'xhs.batchNotes');
});

test('xhs adapter allows remote comment collection on note detail pages', async () => {
  const registry = createPlatformAdapterRegistry({
    xhs: {
      detectPage: () => ({ type: 'noteDetail', url: 'https://www.xiaohongshu.com/explore/n1' }),
      getWindow: () => ({ location: { href: 'https://www.xiaohongshu.com/explore/n1' } }),
    },
  });

  const report = await registry.require(PLATFORM_ID.XHS).checkCapability({
    taskType: 'xhs.batchComments',
    target: {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/explore/n1',
    },
  });

  assert.equal(report.platform, PLATFORM_ID.XHS);
  assert.equal(report.mode, 'detail');
  assert.equal(report.capabilities.canCollectComments, true);
  assert.ok(report.capabilities.canRunTaskTypes.includes('xhs.batchComments'));
});

test('douyin adapter reports platform block through the shared adapter contract', async () => {
  const registry = createPlatformAdapterRegistry({
    douyin: {
      detectDouyinSecurityChallenge: () => true,
      getWindow: () => ({ location: { href: 'https://www.douyin.com/video/1' } }),
      getDocument: () => ({}),
    },
  });

  const report = await registry.require(PLATFORM_ID.DOUYIN).checkCapability();

  assert.equal(report.platform, PLATFORM_ID.DOUYIN);
  assert.equal(report.platformBlocked, true);
  assert.equal(report.capabilities.canCollectPrimary, false);
  assert.equal(report.blockReasonCode, 'platform_security_challenge');
});

test('douyin adapter enables batch collection only on stable search lists', async () => {
  const registry = createPlatformAdapterRegistry({
    douyin: {
      detectDouyinPageType: () => ({ type: 'search', url: 'https://www.douyin.com/search/demo' }),
      detectDouyinSearchBatchContext: () => ({ stableSearchList: true, keyword: 'demo' }),
      detectDouyinSecurityChallenge: () => false,
      isStrictDouyinDetailPage: () => false,
      getWindow: () => ({ location: { href: 'https://www.douyin.com/search/demo' } }),
      getDocument: () => ({}),
    },
  });

  const report = await registry.require(PLATFORM_ID.DOUYIN).checkCapability();

  assert.equal(report.mode, 'search');
  assert.equal(report.isStableSearchList, true);
  assert.equal(report.searchKeyword, 'demo');
  assert.equal(report.capabilities.canBatchNotes, true);
  assert.equal(report.capabilities.canBatchComments, true);
});
