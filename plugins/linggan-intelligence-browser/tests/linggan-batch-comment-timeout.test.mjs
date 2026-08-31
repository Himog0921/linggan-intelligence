import assert from 'node:assert/strict';
import test from 'node:test';

import { BatchCommentController } from '../src/platforms/xhs/batchCommentController.js';

test('a timed-out note drains its collector before another note may navigate', async () => {
  const previousWindow = globalThis.window;
  globalThis.window = { setTimeout, clearTimeout };
  try {
    let releaseCapture;
    let settled = false;
    const captureFinished = new Promise((resolve) => { releaseCapture = resolve; });
    const controller = new BatchCommentController();
    controller._noteCollectionTimeoutMs = 5;
    controller._captureNote = async () => {
      await captureFinished;
      settled = true;
      return true;
    };
    const run = controller._captureNoteWithTimeout({ noteId: 'note-1' });
    await new Promise((resolve) => setTimeout(resolve, 15));
    assert.equal(controller._noteTimedOut, true);
    assert.equal(settled, false);
    let rejected = false;
    void run.catch(() => { rejected = true; });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(rejected, false, 'timeout must not let the loop advance while the collector still owns the page');
    releaseCapture();
    await assert.rejects(run, /单篇评论采集长时间没有结束/);
    assert.equal(settled, true);
  } finally {
    globalThis.window = previousWindow;
  }
});
