import test from 'node:test';
import assert from 'node:assert/strict';

import {
  looksLikeDeadPageTitle,
  explicitXhsDetailPageUrlInvalid,
  DEAD_PAGE_TITLE_PATTERN,
} from '../src/shared/deadPageSignals.js';

test('looksLikeDeadPageTitle 命中已知失效页 title 词表', () => {
  assert.equal(looksLikeDeadPageTitle('页面不见了_小红书'), true);
  assert.equal(looksLikeDeadPageTitle('暂时无法浏览该内容'), true);
  assert.equal(looksLikeDeadPageTitle('笔记已删除'), true);
  assert.equal(looksLikeDeadPageTitle('该内容已私密'), true);
  assert.equal(looksLikeDeadPageTitle('视频不可见'), true);
});

test('looksLikeDeadPageTitle 不误伤正常笔记 title', () => {
  assert.equal(looksLikeDeadPageTitle('今天分享一个数学小技巧_小红书'), false);
  assert.equal(looksLikeDeadPageTitle(''), false);
  assert.equal(looksLikeDeadPageTitle(undefined), false);
});

test('DEAD_PAGE_TITLE_PATTERN 与 capabilityCheck.js 历史词表保持一致', () => {
  // 2026-07-08 从 capabilityCheck.js 抽取为共享常量时的原词表，回归锁定，
  // 防止后续修改一处而漏改另一处。
  const expectedSource =
    '页面不见了|暂时无法浏览|无法浏览|已删除|已私密|页面不存在|访问的页面|作品不存在|视频不可见|已失效';
  assert.equal(DEAD_PAGE_TITLE_PATTERN.source, expectedSource);
});

test('明确的 XHS 404 落地页只否定当前签名 URL，不读取笔记正文', () => {
  assert.equal(explicitXhsDetailPageUrlInvalid({
    currentUrl: 'https://www.xiaohongshu.com/explore?source=404',
    title: '小红书',
    expectedContentExternalId: 'note-1',
  }), true);
  assert.equal(explicitXhsDetailPageUrlInvalid({
    currentUrl: 'https://www.xiaohongshu.com/explore/note-1?xsec_token=fresh',
    // 正常详情页标题；正文/评论里的词语不会传给这个只看 URL + document.title 的判定器。
    title: '孩子今天说了句让人意外的话_小红书',
    expectedContentExternalId: 'note-1',
  }), false);
  assert.equal(explicitXhsDetailPageUrlInvalid({
    currentUrl: 'https://www.xiaohongshu.com/explore/note-1?xsec_token=fresh',
    title: '页面不见了_小红书',
    expectedContentExternalId: 'note-1',
  }), true);
});
