import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  PRODUCER_CAPABILITY,
  createCapturePackage,
  createManualRuntimeTask,
  packageComments,
  packageDiscovery,
  packageMediaSlots,
  packageReplies,
} from '../src/linggan/producerRuntime.js';
import { requireControlReceipt } from '../src/linggan/controlReceipt.js';
import { resolveDouyinBatchControlReceipt } from '../src/platforms/douyin/controlReceipt.js';

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
    capabilitiesRequested: ['content_detail'], maximumQuota: 1,
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

test('current-surface discovery packages exclude temporary DOM ordering references', () => {
  const element = {};
  element.self = element;
  const packageValue = packageDiscovery({
    platform: 'xhs',
    query: 'ADHD',
    cards: [{ noteId: 'note-1', title: 'visible', element, _top: 32, _left: 16 }],
  });
  assert.equal(packageValue.records[0].sourceObject.externalId, 'note-1');
  assert.deepEqual(packageValue.records[0].payload, { noteId: 'note-1', title: 'visible' });
  assert.doesNotThrow(() => JSON.stringify(packageValue));
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

test('visible producer controls use Linggan runtime commands and never revive old browser downloads', () => {
  const popup = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const douyin = readFileSync(new URL('../src/platforms/douyin/index.js', import.meta.url), 'utf8');
  const xhs = readFileSync(new URL('../src/content/xhsPageController.js', import.meta.url), 'utf8');
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.COLLECT_CURRENT_CONTENT/);
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.START_BATCH_CONTENT/);
  assert.doesNotMatch(popup, /sendToBackground\(MSG\.START_BATCH/);
  assert.doesNotMatch(popup, /action: MSG\.COLLECT_SINGLE/);
  assert.match(content, /dispatchProducerRuntimeAction/);
  assert.match(content, /readCurrentVisibleSurfaceNotes/);
  assert.doesNotMatch(content, /discoverSurfaceNotesFromBestSource/);
  assert.match(content, /LINGGAN_RUNTIME_ACTION\.START_BATCH_COMMENTS/);
  assert.match(douyin, /Linggan's media lane/);
  assert.doesNotMatch(douyin, /downloadDouyinVideo\(/);
  assert.doesNotMatch(douyin, /downloadDouyinCommentImages\(/);
  assert.match(douyin, /batchCheckpoint/);
  assert.match(xhs, /评论图片区暂不可用/);
});

test('page-read completion is not rendered as Linggan acceptance', () => {
  const popup = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
  const dashboard = readFileSync(new URL('../src/dashboard/App.jsx', import.meta.url), 'utf8');
  assert.match(popup, /页面读取已结束；请等待 Linggan 本机交付或接纳状态/);
  assert.doesNotMatch(dashboard, /提交到 Linggan（待接通）/);
  assert.match(dashboard, /本机交付状态/);
});

test('every retained Popup control is a Linggan action or an explicit no-side-effect unavailable action', () => {
  const popup = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.PAUSE_ACTIVE_BATCH/);
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.RESUME_ACTIVE_BATCH/);
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.STOP_ACTIVE_BATCH/);
  assert.doesNotMatch(popup, /MSG\.(PAUSE_BATCH|RESUME_BATCH|STOP_BATCH)/);
  assert.match(popup, /requireControlReceipt\(result, 'paused'\)/);
  assert.match(popup, /requireControlReceipt\(result, 'running'\)/);
  assert.match(popup, /requireControlReceipt\(result, 'stopped'\)/);
  assert.match(popup, /快速导出暂不可用：尚未具备 Linggan Runtime 导出合同，未导出任何数据/);
  assert.match(popup, /数据维护暂不可用：尚未具备 Linggan Runtime 数据维护合同，未修改任何本机数据/);
  assert.match(content, /PAUSE_ACTIVE_BATCH.*'dy_pauseBatch'/s);
  assert.match(content, /PAUSE_ACTIVE_BATCH.*'pauseBatch'/s);
  assert.match(content, /return pageResult && typeof pageResult === 'object'/);
});

test('control receipts preserve a rejected page response instead of producing a Popup success message', () => {
  assert.throws(
    () => requireControlReceipt({ success: false, state: 'no_active_task' }, 'paused'),
    /页面没有确认paused当前任务/,
  );
  assert.deepEqual(
    requireControlReceipt({ success: true, state: 'paused' }, 'paused'),
    { success: true, state: 'paused' },
  );
});

test('douyin control receipt is negative without an active controller and positive only with one', () => {
  assert.deepEqual(resolveDouyinBatchControlReceipt(null, 'paused'), {
    success: false,
    state: 'no_active_task',
  });
  assert.deepEqual(resolveDouyinBatchControlReceipt({ isRunning: false }, 'running'), {
    success: false,
    state: 'no_active_task',
  });
  assert.deepEqual(resolveDouyinBatchControlReceipt({ isRunning: true }, 'stopped'), {
    success: true,
    state: 'stopped',
  });
});
