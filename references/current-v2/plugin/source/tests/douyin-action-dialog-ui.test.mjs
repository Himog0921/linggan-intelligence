import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('douyin injected action dialog keeps a compact layout and strips non-essential copy', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/content/components/ActionDialog.jsx'), 'utf8');

  assert.match(source, /getDialogIconText/);
  assert.match(source, /sanitizeFieldValue/);
  assert.match(source, /inputMode=\{field\.type === 'number' \? 'numeric' : undefined\}/);
  assert.match(source, /width:\s*'42px'/);
  assert.match(source, /fontSize:\s*'24px'/);
  assert.match(source, /fontFamily: HEADLINE_FONT/);
  assert.doesNotMatch(source, /本轮计划/);
  assert.doesNotMatch(source, /当前 \{currentString\}/);
  assert.doesNotMatch(source, /badge:/);
  assert.doesNotMatch(source, /note:/);
});

test('douyin batch comment dialog copy is shortened to core strategy text', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/platforms/douyin/index.js'), 'utf8');

  assert.match(source, /title: isSearchMode \? '批量采集搜索结果评论' : '批量采集博主页评论'/);
  assert.match(source, /从当前搜索结果里选前 N 条视频采评论，可按顺位或高赞优先/);
  assert.match(source, /label: '视频数'/);
  assert.match(source, /label: '高赞优先（Top N）'/);
  assert.match(source, /label: '单条评论上限'/);
  assert.match(source, /placeholder: '例如 50；留空 = 采全'/);
  assert.match(source, /label: '展开更多回复'/);
  assert.match(source, /confirmText: '开始采集'/);
  assert.doesNotMatch(source, /建议先用小样本确认规则，再逐步放大范围/);
  assert.doesNotMatch(source, /本轮视频数/);
});
