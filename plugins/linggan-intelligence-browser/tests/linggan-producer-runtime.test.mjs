import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { waitForStableTab } from '../src/linggan/tabReadiness.js';

import {
  PRODUCER_CAPABILITY,
  createCapturePackage,
  createManualRuntimeTask,
  packageComments,
  packageDiscovery,
  packageMediaSlots,
  packageReplies,
} from '../src/linggan/producerRuntime.js';
import { buildDiscoveryExecutionSummary } from '../src/platforms/xhs/noteCollector.js';
import { requireControlReceipt } from '../src/linggan/controlReceipt.js';
import { resolveDouyinBatchControlReceipt } from '../src/platforms/douyin/controlReceipt.js';
import { taskFor } from '../src/linggan/contentRuntimeAdapter.js';

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

test('scheduled page delivery preserves the exact leased TaskSpec while manual delivery still creates a manual task', () => {
  const scheduled = {
    contractVersion: 'linggan.producer.task-spec.v1',
    taskId: '11111111-1111-4111-8111-111111111111',
    source: 'scheduled',
    platform: 'xhs',
    pageType: 'profile',
    target: { authorExternalId: 'creator-1' },
    capabilitiesRequested: ['author_profile'],
    maximumQuota: 1,
    commentLimit: 'not_requested',
    acquireMedia: 'not_requested',
    riskPolicy: 'server_authorized_leased',
    stopConditions: ['maximum_quota', 'surface_ended', 'time_budget'],
  };
  assert.equal(taskFor('xhs', 'author_profile', scheduled.target, { taskSpec: scheduled }), scheduled);
  const scheduledDiscovery = {
    ...scheduled,
    taskId: '22222222-2222-4222-8222-222222222222',
    capabilitiesRequested: ['profile_discovery'],
    maximumQuota: 10,
  };
  assert.equal(
    taskFor('xhs', 'profile_discovery', scheduledDiscovery.target, { taskSpec: scheduledDiscovery }),
    scheduledDiscovery,
  );
  assert.equal(taskFor('xhs', 'author_profile', scheduled.target).source, 'manual');
  assert.throws(
    () => taskFor('xhs', 'profile_discovery', scheduled.target, { taskSpec: scheduled }),
    /dispatched_task_spec_mismatch/,
  );
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

test('a Live Photo remains one logical slot with independently addressable still and motion candidates', () => {
  const packageValue = packageMediaSlots({
    platform: 'xhs',
    note: {
      noteId: 'note-live-1',
      livePhotoStreams: [{
        url: 'https://sns-video.example/live.mp4',
        candidates: ['https://sns-video.example/live.mp4', 'https://sns-video.example/live-backup.mp4'],
        coverUrl: 'https://sns-img.example/live-still.webp',
      }],
    },
  });
  assert.equal(packageValue.records.length, 1);
  assert.equal(packageValue.records[0].slot.role, 'live_photo');
  assert.deepEqual(packageValue.records[0].observation.components.still.candidateUris, [
    'https://sns-img.example/live-still.webp',
  ]);
  assert.deepEqual(packageValue.records[0].observation.components.motion.candidateUris, [
    'https://sns-video.example/live.mp4',
    'https://sns-video.example/live-backup.mp4',
  ]);
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

test('search discovery retains its page receipt in the contract-supported checkpoint without claiming the result set is complete', () => {
  const executionSummary = buildDiscoveryExecutionSummary({
    stopReason: 'risk_control',
    rounds: 3,
    maxRounds: 40,
    scrollTrace: [{
      round: 3,
      scrollTarget: 'window',
      visibleCards: 27,
      newlyDiscovered: 0,
      totalDiscovered: 27,
      action: 'none',
      stopReason: 'risk_control',
      noteId: 'must_not_leave_the_page',
      title: 'must_not_leave_the_page',
      before: { scrollTop: 800, viewportHeight: 600, documentHeight: 2000, atBottom: false },
    }],
  });
  const packageValue = packageDiscovery({
    platform: 'xhs',
    query: 'ADHD',
    cards: [{ noteId: 'note-1', title: 'visible' }],
    pageFacts: {
      kind: 'xhs_search_surface',
      requestedLimit: 50,
      loadedCount: 1,
      stopReason: 'current_surface_read_once',
      resultSetComplete: false,
      ...executionSummary,
    },
  });
  assert.deepEqual(Object.keys(packageValue).sort(), [
    'capturedAt', 'checkpoint', 'contractVersion', 'coverage', 'observedAt', 'packageKind', 'packageRef', 'platform', 'records',
  ]);
  assert.equal(packageValue.checkpoint.kind, 'search_surface_receipt');
  assert.equal(packageValue.checkpoint.surfaceReceipt.requestedLimit, 50);
  assert.equal(packageValue.checkpoint.surfaceReceipt.loadedCount, 1);
  assert.equal(packageValue.checkpoint.surfaceReceipt.resultSetComplete, false);
  assert.equal(packageValue.checkpoint.surfaceReceipt.scrollTrace[0].stopReason, 'risk_control');
  assert.equal(JSON.stringify(packageValue.checkpoint.surfaceReceipt.scrollTrace).includes('must_not_leave_the_page'), false);
});

test('observed media slots request slots rather than media bytes', () => {
  const adapter = readFileSync(new URL('../src/linggan/contentRuntimeAdapter.js', import.meta.url), 'utf8');
  const submitMediaSlots = adapter.slice(adapter.indexOf('async submitMediaSlots'), adapter.indexOf('async acquireMediaSlots'));
  assert.match(submitMediaSlots, /acquireMedia: 'slots'/);
  assert.doesNotMatch(submitMediaSlots, /acquireMedia: 'bytes'/);
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
  assert.equal(replies.coverage.target.commentCollection, undefined);
});

test('comment packages carry one Attempt receipt instead of adding old and new attempts as a fake total', () => {
  const source = {
    noteId: 'note-retry',
    total: 200,
    stopReason: 'risk_control',
    collectionScope: 'all_public_comments',
    collectionState: 'partial',
    analysisUsability: 'usable',
    collectionReceipt: {
      version: 1,
      noteId: 'note-retry',
      scope: 'all_public_comments',
      pageCommentCount: 300,
      expectedCount: 300,
      uniqueCollectedCount: 200,
      state: 'partial',
      analysisUsability: 'usable',
      targetIdentity: 'matched',
      stopReason: 'risk_control',
    },
    comments: [{ commentId: 'root-1', text: 'first' }],
  };
  const packageValue = packageComments({ platform: 'xhs', result: source, noteId: 'note-retry' });
  assert.deepEqual(packageValue.coverage.target.commentCollection, source.collectionReceipt);
  assert.equal(packageValue.coverage.layers[0].acquired, 1);
  assert.equal(packageValue.coverage.layers[0].unknown, 0);
});

test('not-usable wrong-target comment trees keep the receipt but emit no material records', () => {
  const source = {
    noteId: 'expected-note',
    comments: [
      { commentId: 'wrong-root', text: 'must not enter material search' },
      { commentId: 'wrong-reply', replyToCommentId: 'wrong-root', text: 'must not enter replies' },
    ],
    collectionReceipt: {
      version: 1,
      noteId: 'expected-note',
      scope: 'all_public_comments',
      pageCommentCount: 2,
      expectedCount: 2,
      uniqueCollectedCount: 2,
      state: 'invalid_target',
      analysisUsability: 'not_usable',
      targetIdentity: 'mismatched',
      stopReason: 'target_identity_mismatch',
    },
  };
  const comments = packageComments({ platform: 'xhs', result: source, noteId: 'expected-note' });
  const replies = packageReplies({ platform: 'xhs', result: source, noteId: 'expected-note' });
  assert.equal(comments.records.length, 0);
  assert.equal(replies.records.length, 0);
  assert.equal(comments.coverage.target.commentCollection.analysisUsability, 'not_usable');
});

test('media delivery uses its own resumable chunk lane instead of blocking text delivery with one raw upload', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  assert.match(background, /MEDIA_CHUNK_BYTES = 1024 \* 1024/);
  assert.match(background, /media-observations\/\$\{encodeURIComponent\(mediaObservationRef\)\}\/uploads/);
  assert.match(background, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/chunks/);
  assert.match(background, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/finalize/);
  assert.doesNotMatch(background, /media-observations\/\$\{encodeURIComponent\(upload\.mediaObservationRef\)\}\/blob/);
});

test('background forwards the claimed scheduled identity into the content page action', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const start = background.indexOf('async function runDispatchedTask()');
  const end = background.indexOf('\n/**\n * 在一个独立的', start);
  const dispatched = background.slice(start, end);
  assert.match(dispatched, /taskSpec: spec/);
  assert.match(dispatched, /triggerSource: 'linggan_dispatched_task'/);
  assert.doesNotMatch(dispatched, /createManualRuntimeTask/);
});

test('background immediately auto-claims and executes bounded baseline plus fixed material deepening', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  assert.match(background, /onInstalled\?\.addListener[\s\S]*checkInStationOnce\(\)\.then\(\(\) => patrolTick\(\)\)/);
  assert.match(background, /onStartup\?\.addListener[\s\S]*checkInStationOnce\(\)\.then\(\(\) => patrolTick\(\)\)/);
  for (const capability of ['author_profile', 'profile_discovery', 'discovery_search', 'content_detail', 'media_slots', 'comments', 'replies']) {
    assert.match(background, new RegExp(`'${capability}'`));
  }
  assert.match(background, /search_result\?keyword=\$\{encodeURIComponent\(targetValue\)\}/);
  assert.match(background, /buildSignedXhsDetailExecutionUrl\(targetValue, executionSourceUrl\)/);
  assert.doesNotMatch(background, /explore\/\$\{encodeURIComponent\(targetValue\)\}/);
  assert.match(background, /mode: capability === 'discovery_search' \? 'search' : 'profile'/);
  assert.match(background, /COLLECT_CURRENT_COMMENTS/);
  assert.match(background, /COLLECT_CURRENT_CONTENT/);
  assert.match(background, /queueCachedDetailPageSessionLane/);
  assert.match(background, /COLLECT_NOTE_FULL/);
  assert.match(background, /pageSessionPlan: claim\.pageSessionPlan/);
});

test('task window readiness delegates final-document stability and content-runtime probing', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const start = background.indexOf('function waitForTabReady(');
  const end = background.indexOf('\n\nchrome.runtime.onMessage', start);
  const readiness = background.slice(start, end);
  assert.match(readiness, /waitForStableTab/);
  assert.match(readiness, /readinessAction: LINGGAN_RUNTIME_ACTION\.GET_PAGE_CONTEXT/);
});

test('stable tab wait ignores the first complete document when XHS redirects', async () => {
  const listeners = new Set();
  let tab = { status: 'complete', url: 'https://www.xiaohongshu.com/discovery/item/note-id?xsec_token=signed' };
  const probedUrls = [];
  const tabs = {
    onUpdated: {
      addListener(listener) { listeners.add(listener); },
      removeListener(listener) { listeners.delete(listener); },
    },
    async get() { return { ...tab }; },
    async sendMessage() {
      probedUrls.push(tab.url);
      return { success: true, context: { url: tab.url } };
    },
  };
  const waiting = waitForStableTab({
    tabs,
    tabId: 42,
    readinessAction: 'getPageContext',
    timeoutMs: 250,
    stableForMs: 30,
    probeRetryMs: 5,
  });
  await new Promise((resolve) => setTimeout(resolve, 5));
  tab = { status: 'loading', url: 'https://www.xiaohongshu.com/explore/note-id?xsec_token=signed' };
  for (const listener of listeners) listener(42, { status: 'loading', url: tab.url }, { ...tab });
  await new Promise((resolve) => setTimeout(resolve, 5));
  tab = { ...tab, status: 'complete' };
  for (const listener of listeners) listener(42, { status: 'complete' }, { ...tab });

  assert.equal(await waiting, true);
  assert.deepEqual(probedUrls, ['https://www.xiaohongshu.com/explore/note-id?xsec_token=signed']);
  assert.equal(listeners.size, 0);
});

test('stable tab wait retries a late content-script injection within the bounded timeout', async () => {
  const listeners = new Set();
  let probes = 0;
  const url = 'https://www.xiaohongshu.com/explore/note-id?xsec_token=signed';
  const tabs = {
    onUpdated: {
      addListener(listener) { listeners.add(listener); },
      removeListener(listener) { listeners.delete(listener); },
    },
    async get() { return { status: 'complete', url }; },
    async sendMessage() {
      probes += 1;
      if (probes === 1) throw new Error('Receiving end does not exist');
      return { success: true, context: { url } };
    },
  };

  assert.equal(await waitForStableTab({
    tabs,
    tabId: 43,
    readinessAction: 'getPageContext',
    timeoutMs: 150,
    stableForMs: 5,
    probeRetryMs: 5,
  }), true);
  assert.equal(probes, 2);
  assert.equal(listeners.size, 0);
});

test('scheduled material lanes preserve the server task and submit one capability package', () => {
  const adapter = readFileSync(new URL('../src/linggan/contentRuntimeAdapter.js', import.meta.url), 'utf8');
  const detail = readFileSync(new URL('../src/platforms/xhs/detailPackageCollector.js', import.meta.url), 'utf8');
  const comments = readFileSync(new URL('../src/platforms/xhs/commentCollector.js', import.meta.url), 'utf8');
  assert.match(adapter, /taskSpec: options\.taskSpec/);
  assert.match(adapter, /capability === 'replies'/);
  assert.match(detail, /scheduledCapability === 'content_detail'/);
  assert.match(detail, /scheduledCapability === 'media_slots'/);
  assert.match(comments, /taskSpec = undefined/);
  assert.match(comments, /commentDepthMode, taskSpec/);
});

test('content message gate admits scheduled profile discovery without dropping dispatch identity', () => {
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const gateStart = content.indexOf('if ([', content.indexOf('chrome.runtime.onMessage.addListener'));
  const gateEnd = content.indexOf('].includes(action))', gateStart);
  const gate = content.slice(gateStart, gateEnd);
  assert.match(gate, /LINGGAN_RUNTIME_ACTION\.DISCOVER_SURFACE/);
  assert.match(content, /taskSpec: message\.taskSpec/);
  assert.match(content, /triggerSource: message\.triggerSource \|\| 'popup_linggan_runtime'/);
});

test('scheduled page execution propagates a collector failure instead of reporting a false start', () => {
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  assert.match(content, /if \(pageResult\?\.success === false\) return pageResult/);
  assert.match(background, /if \(response\?\.success === false\)/);
  assert.match(background, /response\.state \|\| 'page_read_failed'/);
});

test('producer controls use Linggan runtime commands while manual media remains an explicit separate action', () => {
  const popup = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const douyin = readFileSync(new URL('../src/platforms/douyin/index.js', import.meta.url), 'utf8');
  const xhs = readFileSync(new URL('../src/content/xhsPageController.js', import.meta.url), 'utf8');
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.COLLECT_CURRENT_CONTENT/);
  assert.match(popup, /LINGGAN_RUNTIME_ACTION\.START_BATCH_CONTENT/);
  assert.doesNotMatch(popup, /sendToBackground\(MSG\.START_BATCH/);
  assert.doesNotMatch(popup, /action: MSG\.COLLECT_SINGLE/);
  assert.match(content, /dispatchProducerRuntimeAction/);
  assert.match(content, /discoverWithScroll/);
  assert.doesNotMatch(content, /discoverSurfaceNotesFromBestSource/);
  assert.match(content, /LINGGAN_RUNTIME_ACTION\.START_BATCH_COMMENTS/);
  assert.match(douyin, /Linggan's media lane/);
  assert.doesNotMatch(douyin, /downloadDouyinVideo\(/);
  assert.doesNotMatch(douyin, /downloadDouyinCommentImages\(/);
  assert.match(douyin, /batchCheckpoint/);
  assert.match(xhs, /评论图片区暂不可用/);
  assert.match(xhs, /collectNoteWithManualMedia/);
});

test('XHS startup keeps page initialization passive while the target-driven route owns active loading', () => {
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const initStart = content.indexOf('async function initXhs()');
  const initEnd = content.indexOf('\nasync function initDouyin()', initStart);
  const currentSurfaceStart = content.indexOf('discoverSurface: async');
  const currentSurfaceEnd = content.indexOf('\n  submitDiscovery:', currentSurfaceStart);
  const init = content.slice(initStart, initEnd);
  const currentSurface = content.slice(currentSurfaceStart, currentSurfaceEnd);
  assert.doesNotMatch(init, /ensureXhsCommentApiBridge|discoverSurfaceNotesFromBestSource|discoverWithScroll|requestXhs(?:Search|Profile)NotesSnapshot|Batch(?:Note|Comment)Controller/);
  assert.match(currentSurface, /discoverWithScroll/);
  assert.match(currentSurface, /expectedCount/);
  assert.doesNotMatch(currentSurface, /ensureXhsCommentApiBridge|discoverSurfaceNotesFromBestSource|requestXhs(?:Search|Profile)NotesSnapshot|Batch(?:Note|Comment)Controller/);
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
