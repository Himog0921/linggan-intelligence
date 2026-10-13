// OPS-MIGRATION-001 D2 · 运营页面脚本的显示规则（LIDS 数据边界四态中的数值部分）。
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('./ops.js', import.meta.url), 'utf8');

function loadFormat() {
  const sandbox = { module: { exports: {} }, URL };
  vm.runInNewContext(source, sandbox);
  return sandbox.module.exports.OpsFormat;
}

// Objects built inside the sandbox have that realm's prototypes; compare them as plain data.
const plain = (value) => JSON.parse(JSON.stringify(value));

test('unknown is a dash, zero stays zero, and large counts abbreviate to 万 with the exact value kept', () => {
  const format = loadFormat();
  const count = (value) => plain(format.count(value));
  assert.deepEqual(count(null), { text: '—', title: '没有值或未知', missing: true });
  assert.deepEqual(count(undefined), { text: '—', title: '没有值或未知', missing: true });
  assert.deepEqual(count(0), { text: '0', title: '' });
  assert.deepEqual(count(9439), { text: '9,439', title: '' });
  assert.deepEqual(count(10000), { text: '1万', title: '10,000' });
  assert.deepEqual(count(103583), { text: '10.4万', title: '103,583' });
});

test('rates show as percentages and an undefined rate stays unknown', () => {
  const { percent } = loadFormat();
  assert.equal(percent(null, 1).text, '—');
  assert.equal(percent(40, 1).text, '40%');
  assert.equal(percent(33.333, 1).text, '33.3%');
  assert.equal(percent(0, 1).text, '0%');
});

test('only http and https links become clickable', () => {
  const { safeHref, platform } = loadFormat();
  assert.equal(safeHref('https://www.xiaohongshu.com/explore/abc'), 'https://www.xiaohongshu.com/explore/abc');
  assert.equal(safeHref('javascript:alert(1)'), null);
  assert.equal(safeHref('not a url'), null);
  assert.equal(platform('douyin'), '抖音');
  assert.equal(platform('xhs'), '小红书');
});
