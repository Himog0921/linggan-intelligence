import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildSelectorCheck,
  buildSelectorHealthAlertMessage,
  consumeSelectorHealthAlertMessage,
  finalizeSelectorPreflight,
} from '../src/shared/selectorHealth.js';

test('selector health alert message surfaces stale checks even when probe passes', () => {
  const result = finalizeSelectorPreflight('xhs', 'bootstrap', {
    ok: true,
    checks: [
      buildSelectorCheck({
        name: 'feed_container',
        ok: true,
        selector: '.feeds-container',
        detail: '笔记流容器',
        verifiedAt: '2026-03-01T00:00:00+08:00',
      }),
    ],
  });

  const message = buildSelectorHealthAlertMessage(result);
  assert.match(message, /超过 30 天/);
  assert.match(message, /笔记流容器/);
});

test('selector health alert message is deduped within the cooldown window', () => {
  const win = {};
  const result = finalizeSelectorPreflight('douyin', 'bootstrap', {
    ok: false,
    code: 'selector_missing',
    message: '当前页面缺少抖音详情页信号，建议刷新页面后重试',
    checks: [
      buildSelectorCheck({
        name: 'detailDom',
        ok: false,
        selector: 'video',
        detail: 'video/detail metadata',
        verifiedAt: '2026-04-20T00:00:00+08:00',
      }),
    ],
  });

  const first = consumeSelectorHealthAlertMessage(result, { win, now: 1_000 });
  const second = consumeSelectorHealthAlertMessage(result, { win, now: 2_000 });
  const third = consumeSelectorHealthAlertMessage(result, { win, now: 5 * 60 * 1000 + 2_000 });

  assert.match(first, /抖音详情页信号|页面结构/);
  assert.equal(second, '');
  assert.match(third, /抖音详情页信号|页面结构/);
});
