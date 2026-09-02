import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { waitForStableTab } from '../src/linggan/tabReadiness.js';

import {
  PRODUCER_CAPABILITY,
  createCapturePackage,
  createManualRuntimeTask,
  packageAuthorAvatarMediaSlots,
  packageComments,
  packageDiscovery,
  packageMediaSlots,
  packageReplies,
} from '../src/linggan/producerRuntime.js';
import { buildDiscoveryExecutionSummary } from '../src/platforms/xhs/noteCollector.js';
import { requireControlReceipt } from '../src/linggan/controlReceipt.js';
import { resolveDouyinBatchControlReceipt } from '../src/platforms/douyin/controlReceipt.js';
import { commentTaskInstruction, taskFor } from '../src/linggan/contentRuntimeAdapter.js';
import { createLingganContentRuntime } from '../src/linggan/contentRuntimeAdapter.js';

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
  assert.equal(packageValue.records[0].slotKey, 'xhs:note-1:image:1');
  assert.equal(packageValue.records[1].slotKey, 'xhs:note-1:cover:1');
  assert.deepEqual(packageValue.records.map((record) => record.slot.ordinal), [1, 1]);
});

test('content detail media package carries the observed author avatar as an author-owned slot', () => {
  const packageValue = packageMediaSlots({
    platform: 'xhs',
    note: {
      noteId: 'note-with-author-avatar',
      authorId: 'author-42',
      authorAvatar: 'https://sns-avatar.example/author-42.jpg',
      images: [{ url: 'https://sns-img.example/body.jpg' }],
    },
  });
  assert.equal(packageValue.coverage.layers[0].observed, 2);
  assert.deepEqual(packageValue.records.map((record) => record.slotKey), [
    'xhs:note-with-author-avatar:image:1',
    'xhs:author:author-42:avatar:1',
  ]);
  assert.deepEqual(packageValue.records[1].sourceObject, {
    platform: 'xhs',
    type: 'author',
    externalId: 'author-42',
  });
  assert.equal(packageValue.records[1].contextContentExternalId, 'note-with-author-avatar');
  assert.equal(packageValue.records[1].observation.externalUri, 'https://sns-avatar.example/author-42.jpg');
  assert.equal(Object.hasOwn(packageValue.records[1], 'localAssetUrl'), false);
});

test('profile-only author avatar uses the stable author target without inventing a content context', () => {
  const packageValue = packageAuthorAvatarMediaSlots({
    platform: 'xhs',
    author: { userId: 'author-profile-1', avatar: 'https://cdn.example/author-profile-1.jpg' },
  });
  assert.equal(packageValue.packageKind, 'media_slots');
  assert.deepEqual(packageValue.coverage.target, {
    basis: 'known_set', authorExternalId: 'author-profile-1',
  });
  assert.equal(packageValue.records[0].slotKey, 'xhs:author:author-profile-1:avatar:1');
  assert.equal(packageValue.records[0].sourceObject.type, 'author');
  assert.equal(Object.hasOwn(packageValue.records[0], 'contextContentExternalId'), false);
  assert.equal(
    taskFor('xhs', 'media_slots', packageValue.coverage.target, { acquireMedia: 'bytes' }).pageType,
    'profile',
  );
});

test('standard detail media package carries comment images as comment-owned slots', () => {
  const packageValue = packageMediaSlots({
    platform: 'xhs',
    note: {
      noteId: 'note-with-comment-images',
      coverUrl: 'https://sns-img.example/cover.jpg',
    },
    commentRecords: [
      {
        commentId: 'comment-1',
        commentImageUrls: [
          'https://sns-img.example/comment-1-a.jpg',
          'https://sns-img.example/comment-1-b.jpg',
        ],
      },
      {
        commentId: 'comment-2',
        commentImageUrls: ['https://sns-img.example/comment-2-a.jpg'],
      },
    ],
  });

  assert.deepEqual(packageValue.records.map((record) => record.slotKey), [
    'xhs:note-with-comment-images:cover:1',
    'xhs:comment:comment-1:comment_image:1',
    'xhs:comment:comment-1:comment_image:2',
    'xhs:comment:comment-2:comment_image:1',
  ]);
  const commentImage = packageValue.records[1];
  assert.deepEqual(commentImage.sourceObject, {
    platform: 'xhs',
    type: 'comment',
    externalId: 'comment-1',
  });
  assert.equal(commentImage.contextContentExternalId, 'note-with-comment-images');
  assert.equal(commentImage.slot.role, 'comment_image');
});

test('a reply queue failure does not rewrite the already queued comments lane', async () => {
  const originalChrome = globalThis.chrome;
  const calls = [];
  globalThis.chrome = {
    runtime: {
      id: 'synthetic-extension',
      lastError: null,
      sendMessage(message, callback) {
        calls.push(message.capturePackage.packageKind);
        if (message.capturePackage.packageKind === 'comments') {
          callback({ success: true, delivery: 'acknowledged' });
        } else {
          callback({ success: false, message: 'reply_outbox_unavailable' });
        }
      },
    },
  };
  try {
    const runtime = createLingganContentRuntime({ platform: 'xhs' });
    const result = await runtime.submitComments({
      comments: [
        { commentId: 'comment-1', text: 'top level' },
        { commentId: 'reply-1', rootCommentId: 'comment-1', parentCommentId: 'comment-1', text: 'reply' },
      ],
      stopReason: 'comment_cap_reached',
    }, 'note-reply-failure', { maxTotal: 30 });

    assert.deepEqual(calls, ['comments', 'replies']);
    assert.equal(result.delivery, 'acknowledged');
    assert.deepEqual(result.replies, {
      delivery: 'rejected',
      code: 'replies_queue_failed',
      message: 'reply_outbox_unavailable',
    });
  } finally {
    globalThis.chrome = originalChrome;
  }
});

test('media slot ordinals are scoped to each relationship purpose', () => {
  const packageValue = packageMediaSlots({
    platform: 'xhs',
    note: {
      noteId: 'note-many-images',
      images: Array.from({ length: 7 }, (_, index) => ({ url: `https://cdn.example/${index + 1}.jpg` })),
      coverUrl: 'https://cdn.example/cover.jpg',
      video: { url: 'https://cdn.example/video.mp4' },
    },
  });
  assert.deepEqual(packageValue.records.map((record) => record.slotKey), [
    'xhs:note-many-images:image:1',
    'xhs:note-many-images:image:2',
    'xhs:note-many-images:image:3',
    'xhs:note-many-images:image:4',
    'xhs:note-many-images:image:5',
    'xhs:note-many-images:image:6',
    'xhs:note-many-images:image:7',
    'xhs:note-many-images:cover:1',
    'xhs:note-many-images:video:1',
  ]);
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

test('standard detail media submits slots and immediately queues the approved byte lane', () => {
  const adapter = readFileSync(new URL('../src/linggan/contentRuntimeAdapter.js', import.meta.url), 'utf8');
  const submitMediaSlots = adapter.slice(adapter.indexOf('async submitMediaSlots'), adapter.indexOf('async acquireMediaSlots'));
  assert.match(submitMediaSlots, /acquireMedia: 'bytes'/);
  assert.match(submitMediaSlots, /LINGGAN_RUNTIME_ACTION\.SUBMIT_MEDIA_SLOTS/);
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

test('real xhs fallback comment identities stay in the right lane and bind to their manual task', () => {
  const source = {
    noteId: 'note-real-shape',
    comments: [
      {
        noteId: 'note-real-shape',
        commentId: 'comment-root',
        rootCommentId: 'comment-root',
        parentCommentId: '',
        replyToCommentId: '',
        level: 1,
        text: 'top level from the live DOM fallback',
      },
      {
        noteId: 'note-real-shape',
        commentId: 'comment-reply',
        rootCommentId: 'comment-root',
        parentCommentId: 'comment-root',
        replyToCommentId: 'comment-root',
        level: 2,
        text: 'reply from the live DOM fallback',
      },
    ],
    collectionReceipt: {
      version: 1,
      noteId: 'note-real-shape',
      scope: 'detail_window',
      requestedLimit: 30,
      pageCommentCount: 2,
      expectedCount: 2,
      uniqueCollectedCount: 2,
      state: 'complete',
      analysisUsability: 'usable',
      targetIdentity: 'matched',
      stopReason: 'comment_area_end',
    },
  };
  const instruction = commentTaskInstruction('note-real-shape', 30);
  const comments = packageComments({
    platform: 'xhs', result: source, noteId: source.noteId, taskTarget: instruction.target,
  });
  const replies = packageReplies({
    platform: 'xhs', result: source, noteId: source.noteId, taskTarget: instruction.target,
  });
  const commentTask = taskFor('xhs', 'comments', instruction.target, instruction);
  const replyTask = taskFor('xhs', 'replies', instruction.target, instruction);

  assert.equal(comments.records.length, 1);
  assert.equal(comments.records[0].payload.commentId, 'comment-root');
  assert.equal(comments.records[0].payload.noteId, 'note-real-shape');
  assert.equal(Object.hasOwn(comments.records[0].payload, 'rootCommentId'), false);
  assert.equal(Object.hasOwn(comments.records[0].payload, 'parentCommentId'), false);
  assert.equal(Object.hasOwn(comments.records[0].payload, 'replyToCommentId'), false);
  assert.equal(replies.records.length, 1);
  assert.equal(replies.records[0].payload.commentId, 'comment-reply');
  assert.equal(replies.records[0].payload.noteId, 'note-real-shape');
  assert.equal(replies.records[0].payload.rootCommentId, 'comment-root');
  assert.equal(replies.records[0].payload.parentCommentId, 'comment-root');
  assert.equal(Object.hasOwn(replies.records[0].payload, 'replyToCommentId'), false);
  assert.equal(commentTask.maximumQuota, 30);
  assert.equal(replyTask.maximumQuota, 30);
  assert.equal(comments.coverage.target.commentScope, instruction.target.commentScope);
  assert.equal(comments.coverage.target.requestedCommentLimit, 30);
  assert.equal(replies.coverage.target.commentScope, instruction.target.commentScope);
  assert.equal(replies.coverage.target.requestedCommentLimit, 30);
});

test('unlimited deep comments keep a real natural-end target without inventing a result quota', () => {
  const instruction = commentTaskInstruction('note-deep', 0);
  const task = taskFor('xhs', 'comments', instruction.target, instruction);
  assert.equal(instruction.commentLimit, 'not_requested');
  assert.equal(instruction.maximumQuota, null);
  assert.equal(task.maximumQuota, null);
  assert.equal(instruction.target.commentScope, 'all_public_until_natural_end');
  assert.equal(Object.hasOwn(instruction.target, 'requestedCommentLimit'), false);
});

test('nested replies retain one exact parent identity for material admission', () => {
  const result = {
    noteId: 'note-nested',
    comments: [{
      noteId: 'note-nested',
      commentId: 'reply-child',
      rootCommentId: 'comment-root',
      parentCommentId: 'comment-root',
      replyToCommentId: 'reply-parent',
      level: 3,
    }],
  };
  const replies = packageReplies({ platform: 'xhs', result, noteId: result.noteId });

  assert.equal(replies.records[0].payload.rootCommentId, 'comment-root');
  assert.equal(replies.records[0].payload.replyToCommentId, 'reply-parent');
  assert.equal(Object.hasOwn(replies.records[0].payload, 'parentCommentId'), false);
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
  const mediaRuntime = readFileSync(new URL('../src/linggan/mediaTransferRuntime.js', import.meta.url), 'utf8');
  assert.match(mediaRuntime, /MEDIA_CHUNK_BYTES = 1024 \* 1024/);
  assert.match(mediaRuntime, /media-observations\/\$\{encodeURIComponent\(mediaObservationRef\)\}\/uploads/);
  assert.match(mediaRuntime, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/chunks/);
  assert.match(mediaRuntime, /media-uploads\/\$\{encodeURIComponent\(sessionRef\)\}\/finalize/);
  assert.doesNotMatch(mediaRuntime, /media-observations\/\$\{encodeURIComponent\(upload\.mediaObservationRef\)\}\/blob/);
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

test('a claimed task which cannot start a page is audibly requeued instead of remaining in progress', () => {
  const background = readFileSync(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const start = background.indexOf('async function runDispatchedTask()');
  const end = background.indexOf('\n/**\n * 在一个独立的', start);
  const dispatched = background.slice(start, end);
  assert.match(background, /reportLingganDispatchFailure/);
  assert.match(dispatched, /requeueClaimedTaskFailure/);
  assert.match(dispatched, /state: 'page_timeout'/);
  assert.match(dispatched, /state: 'page_unavailable'/);
  assert.match(dispatched, /state: 'tab_unavailable'/);
  assert.match(background, /DISPATCH_FAILURE_CODES/);
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

test('stable tab wait accepts a quiet profile document before Chrome finishes long-tail loading', async () => {
  const listeners = new Set();
  const url = 'https://www.xiaohongshu.com/user/profile/creator-id';
  let probes = 0;
  const tabs = {
    onUpdated: {
      addListener(listener) { listeners.add(listener); },
      removeListener(listener) { listeners.delete(listener); },
    },
    // XHS profile pages can remain in Chrome's loading state for images and long-polling after
    // document_end has already installed the content runtime.
    async get() { return { status: 'loading', url }; },
    async sendMessage() {
      probes += 1;
      return { success: true, context: { url } };
    },
  };

  assert.equal(await waitForStableTab({
    tabs,
    tabId: 44,
    readinessAction: 'getPageContext',
    timeoutMs: 150,
    stableForMs: 5,
    probeRetryMs: 5,
  }), true);
  assert.equal(probes, 1);
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
  const adapter = readFileSync(new URL('../src/linggan/adapter.js', import.meta.url), 'utf8');
  assert.match(content, /if \(pageResult\?\.success !== true\)/);
  assert.match(background, /decodePageExecutionReceipt\(response/);
  assert.match(adapter, /page_receipt_identity_mismatch/);
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
  assert.match(dashboard, /同步到观察目标/);
  assert.match(dashboard, /服务端接纳后才会出现在观察目标/);
  assert.doesNotMatch(dashboard, /不会重复提交或发起平台访问/);
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
