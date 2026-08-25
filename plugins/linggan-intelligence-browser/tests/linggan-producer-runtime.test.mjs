import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  PRODUCER_CAPABILITY,
  createCapturePackage,
  createManualRuntimeTask,
  packageComments,
  packageMediaSlots,
  packageReplies,
} from '../src/linggan/producerRuntime.js';

test('one adapter uses the same bounded package shape for every retained collector capability', () => {
  for (const capability of Object.values(PRODUCER_CAPABILITY)) {
    const value = createCapturePackage({
      packageRef: crypto.randomUUID(), packageKind: capability, platform: 'xhs',
      target: { basis: 'known_set' }, coverage: { observed: 1, attempted: 1, acquired: 1, verified: 0, stoppedReason: 'fixture' }, records: [],
    });
    assert.equal(value.packageKind, capability);
    assert.equal(value.coverage.layers[0].capability, capability);
  }
  const task = createManualRuntimeTask({
    platform: 'douyin', pageType: 'detail', target: { contentExternalId: 'fixture' },
    capabilitiesRequested: ['content_detail', 'comments'], maximumQuota: 1,
  });
  assert.equal(task.source, 'manual');
  assert.equal(task.platform, 'douyin');
});

test('media slots retain URL observations but no remote URL becomes a local presentation URL', () => {
  const packageValue = packageMediaSlots({
    platform: 'xhs',
    note: { noteId: 'note-1', coverUrl: 'https://cdn.example/cover.jpg', images: [{ url: 'https://cdn.example/one.jpg' }] },
  });
  assert.equal(packageValue.packageKind, 'media_slots');
  assert.equal(packageValue.coverage.layers[0].observed, 2);
  assert.equal(packageValue.records[0].observation.externalUri, 'https://cdn.example/one.jpg');
  assert.equal(Object.hasOwn(packageValue.records[0], 'localAssetUrl'), false);
});

test('partial media coverage retains acquired bytes and explicitly keeps unknown/not-attempted distinct', () => {
  const packageValue = createCapturePackage({
    packageKind: 'media_slots', platform: 'xhs', target: { basis: 'known_set' },
    coverage: { observed: 9, attempted: 7, acquired: 6, verified: 6, failed: 1, notAttempted: 2, unknown: 0, stoppedReason: 'risk_control' }, records: [],
  });
  assert.deepEqual(packageValue.coverage.layers[0], {
    capability: 'media_slots', observed: 9, attempted: 7, acquired: 6, verified: 6,
    failed: 1, notAttempted: 2, unknown: 0, stoppedReason: 'risk_control',
  });
});

test('a retained collector can return one comment tree without flattening replies into comments', () => {
  const source = {
    noteId: 'note-1',
    comments: [
      { commentId: 'top-1', content: 'top level' },
      { commentId: 'reply-1', replyToCommentId: 'top-1', content: 'reply level' },
    ],
  };
  const comments = packageComments({ platform: 'xhs', result: source, noteId: 'note-1' });
  const replies = packageReplies({ platform: 'xhs', result: source, noteId: 'note-1' });
  assert.equal(comments.packageKind, 'comments');
  assert.equal(comments.records.length, 1);
  assert.equal(replies.packageKind, 'replies');
  assert.equal(replies.records.length, 1);
  assert.equal(replies.records[0].payload.commentId, 'reply-1');
});

test('media delivery uses its own resumable chunk lane instead of blocking text delivery with one raw upload', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  assert.match(background, /MEDIA_CHUNK_BYTES = 1024 \* 1024/);
  assert.match(background, /media-observations\/\$\{encodeURIComponent\(mediaObservationRef\)\}\/uploads/);
  assert.match(background, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/chunks/);
  assert.match(background, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/finalize/);
  assert.doesNotMatch(background, /media-observations\/\$\{encodeURIComponent\(upload\.mediaObservationRef\)\}\/blob/);
});
