import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const testDir = path.dirname(fileURLToPath(import.meta.url));
const sourceRoot = path.resolve(testDir, '..', 'src', 'platforms', 'xhs');

function readSource(name) {
  return fs.readFileSync(path.join(sourceRoot, name), 'utf8');
}

test('active XHS collection paths use bounded page-state waits and stop rather than captcha resume', () => {
  const commentCollector = readSource('commentCollector.js');
  const batchNotes = readSource('batchController.js');
  const batchComments = readSource('batchCommentController.js');

  for (const source of [commentCollector, batchNotes, batchComments]) {
    assert.doesNotMatch(source, /randomDelay\s*\(/);
    assert.doesNotMatch(source, /watchCaptcha\s*\(/);
    assert.doesNotMatch(source, /showCaptchaPauseOverlay\s*\(/);
  }

  assert.match(commentCollector, /loadMoreCommentSurface/);
  assert.match(commentCollector, /hasXhsCollectionRiskSignal/);
  assert.match(batchNotes, /this\.stop\(\)/);
  assert.match(batchComments, /this\.stop\(\)/);
});
