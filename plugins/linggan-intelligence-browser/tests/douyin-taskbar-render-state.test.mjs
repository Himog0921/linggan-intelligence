import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { TASK_STATE } from '../src/shared/constants.js';
import { resolveDouyinTaskbarRenderState } from '../src/platforms/douyin/taskbarRenderState.js';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('douyin terminal taskbar states render final status before hiding', () => {
  const done = resolveDouyinTaskbarRenderState({ taskState: TASK_STATE.DONE });

  assert.equal(done.taskState, TASK_STATE.DONE);
  assert.equal(done.visible, true);
  assert.equal(done.shouldHideAfterRender, true);
  assert.equal(done.hideImmediate, false);
});

test('douyin idle taskbar state can hide immediately after rendering stopped status', () => {
  const idle = resolveDouyinTaskbarRenderState({ taskState: TASK_STATE.IDLE });

  assert.equal(idle.taskState, TASK_STATE.IDLE);
  assert.equal(idle.visible, true);
  assert.equal(idle.shouldHideAfterRender, true);
  assert.equal(idle.hideImmediate, true);
});

test('douyin running taskbar state remains visible without hide scheduling', () => {
  const running = resolveDouyinTaskbarRenderState({
    taskState: TASK_STATE.RUNNING,
    message: '已采集 1 条评论',
  });

  assert.equal(running.taskState, TASK_STATE.RUNNING);
  assert.equal(running.visible, true);
  assert.equal(running.shouldHideAfterRender, false);
  assert.equal(running.hideImmediate, false);
  assert.equal(running.message, '已采集 1 条评论');
});

test('task control stop button is disabled after a task reaches terminal state', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/content/components/TaskControlBar.jsx'), 'utf8');

  assert.match(source, /const isStopDisabled = isStopping \|\| isDone/);
  assert.match(source, /disabled=\{isStopDisabled\}/);
  assert.match(source, /isStopDisabled \? 'not-allowed' : 'pointer'/);
});
