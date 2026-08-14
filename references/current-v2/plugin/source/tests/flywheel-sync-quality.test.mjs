import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';

import {
  mapNoteToFlywheel,
  mapCommentToFlywheel,
  mapAuthorToFlywheel,
} from '../src/popup/utils.js';
import {
  saveFlywheelConfig,
  syncManualRecordsToWorkbench,
  syncToFlywheel,
} from '../src/sync/flywheelSync.js';

const popupAppSourcePath = new URL('../src/popup/App.jsx', import.meta.url);
const popupUtilsSourcePath = new URL('../src/popup/utils.js', import.meta.url);
const backgroundSourcePath = new URL('../src/background/index.js', import.meta.url);
const flywheelSyncSourcePath = new URL('../src/sync/flywheelSync.js', import.meta.url);
const OBSERVED_AT = '2026-08-12T03:04:05.000Z';

function installChrome(overrides = {}) {
  const originalChrome = globalThis.chrome;
  globalThis.chrome = {
    ...overrides,
    runtime: {
      getManifest: () => ({ version: '2.0.92' }),
      ...(overrides.runtime || {}),
    },
  };
  return () => { globalThis.chrome = originalChrome; };
}

function strictXhsNote(index = 1, overrides = {}) {
  const id = `note_${index}`;
  return {
    noteId: id,
    platformContentId: id,
    platform: 'xhs',
    type: 'normal',
    collectedAt: Date.parse(OBSERVED_AT),
    ...overrides,
  };
}

function packagePayload(body) {
  return JSON.parse(Buffer.from(body.capturePackage.packagePayload, 'base64').toString('utf8'));
}

test('popup flywheel mappers preserve quality fields and collectionRunId', () => {
  const note = mapNoteToFlywheel({
    noteId: 'note_1',
    platformContentId: 'note_1',
    platform: 'douyin',
    title: '标题',
    content: '正文',
    url: 'https://www.douyin.com/video/1',
    dataQuality: 'seed',
    qualityReason: 'search_summary_seed',
    sourceTier: 'seed',
    collectionRunId: 'run_note_1',
  });
  const comment = mapCommentToFlywheel({
    commentId: 'comment_1',
    platform: 'xhs',
    text: '评论',
    noteId: 'note_1',
    dataQuality: 'full',
    qualityReason: '',
    sourceTier: 'api',
    collectionRunId: 'run_comment_1',
  });
  const author = mapAuthorToFlywheel({
    userId: 'author_1',
    authorId: 'author_1',
    platformAuthorId: 'author_1',
    platform: 'xhs',
    name: '作者',
    dataQuality: 'degraded',
    qualityReason: 'inject_failed_dom_only',
    sourceTier: 'dom',
    collectionRunId: 'run_author_1',
  });

  assert.equal(note.dataQuality, 'seed');
  assert.equal(note.qualityReason, 'search_summary_seed');
  assert.equal(note.sourceTier, 'seed');
  assert.equal(note.collectionRunId, 'run_note_1');
  assert.equal(note.platformContentId, 'note_1');

  assert.equal(comment.dataQuality, 'full');
  assert.equal(comment.qualityReason, '');
  assert.equal(comment.sourceTier, 'api');
  assert.equal(comment.collectionRunId, 'run_comment_1');

  assert.equal(author.dataQuality, 'degraded');
  assert.equal(author.qualityReason, 'inject_failed_dom_only');
  assert.equal(author.sourceTier, 'dom');
  assert.equal(author.collectionRunId, 'run_author_1');
  assert.equal(author.authorId, 'author_1');
  assert.equal(author.platformAuthorId, 'author_1');
});

test('syncToFlywheel posts quality fields through strict XHS Evidence records', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome();
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      status: 'committed',
      receiptId: 'receipt-quality',
    }), { status: 200 });
  };

  try {
    const result = await syncToFlywheel([strictXhsNote(1, {
      url: 'https://www.xiaohongshu.com/explore/note_1',
      title: '标题',
      content: '正文',
      dataQuality: 'seed',
      qualityReason: 'search_summary_seed',
      sourceTier: 'seed',
      collectionRunId: 'run_note_1',
    })]);

    assert.equal(result.success, true);
    assert.equal(calls.length, 1);

    const body = JSON.parse(calls[0][1].body);
    const note = packagePayload(body).records[0].payload;
    assert.equal(note.dataQuality, 'seed');
    assert.equal(note.qualityReason, 'search_summary_seed');
    assert.equal(note.sourceTier, 'seed');
    assert.equal(note.collectionRunId, 'run_note_1');
    assert.equal(note.rawData.collectionRunId, 'run_note_1');
    assert.equal(body.header.ingressKind, 'manual_import');
    assert.match(calls[0][0], /\/api\/execution-tasks\/manual-import$/);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('syncToFlywheel has no partial-success batch path when the Evidence request fails', async () => {
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome();
  let callCount = 0;
  globalThis.fetch = async () => {
    callCount += 1;
    return new Response(JSON.stringify({
      error: 'batch failed',
    }), { status: 500 });
  };

  try {
    const payload = Array.from({ length: 51 }, (_, index) => strictXhsNote(index + 1, {
      url: `https://www.xiaohongshu.com/explore/${index + 1}`,
      title: `标题 ${index + 1}`,
    }));
    const result = await syncToFlywheel(payload);

    assert.equal(result.success, false);
    assert.equal(result.imported, 0);
    assert.equal(result.skipped, 0);
    assert.equal(callCount, 1);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('manual workbench sync emits one strict Evidence package per CollectionContract kind', async () => {
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome();
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      status: 'committed',
      receiptId: `receipt-${calls.length}`,
    }), { status: 200 });
  };

  try {
    const result = await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        comments: [{ commentId: 'comment_1', noteId: 'note_1', text: '求方法', platform: 'xhs', collectedAt: Date.parse(OBSERVED_AT) }],
        authors: [{ authorId: 'author_1', platformAuthorId: 'author_1', name: '作者', platform: 'xhs', collectedAt: Date.parse(OBSERVED_AT) }],
      },
    );
    assert.equal(result.success, true);
    assert.equal(result.imported, 2);
    assert.equal(calls.length, 2);
    const bodies = calls.map(([, options]) => JSON.parse(options.body));
    assert.deepEqual(bodies.map((body) => body.header.contractId), [
      'xhs.comment-probe',
      'xhs.author-profile',
    ]);
    assert.deepEqual(bodies.map((body) => packagePayload(body).records[0].recordKind), [
      'comment',
      'author',
    ]);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('manual workbench sync preserves media addresses only as raw note Evidence', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome();
  const cover = 'https://sns-img.example.com/original-cover.webp';
  const fallback = 'https://sns-img.example.com/original-cover-fallback.webp';
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      status: 'committed',
      receiptId: 'receipt-media-source',
    }), { status: 200 });
  };

  try {
    const result = await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        notes: [strictXhsNote(1, {
          noteId: '68fb9898000000000402084f',
          platformContentId: '68fb9898000000000402084f',
          type: 'video',
          url: 'https://www.xiaohongshu.com/explore/68fb9898000000000402084f',
          cover,
          images: [cover],
          imageCandidates: [[cover, fallback]],
          video: 'https://sns-video.example.com/stream.mp4',
        })],
      },
    );

    const body = JSON.parse(calls[0][1].body);
    const packageBody = packagePayload(body);
    const note = packageBody.records[0].payload;
    assert.deepEqual(note.rawData.imageCandidates, [[cover, fallback]]);
    assert.deepEqual(packageBody.artifacts, []);
    assert.equal(result.meta.evidenceStatus, 'committed');
    assert.equal('mediaRegistrationConfirmed' in result.meta, false);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('manual note sync does not infer embedded comments as a second contract', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome();
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({ status: 'committed', receiptId: 'receipt-embedded' }), { status: 200 });
  };

  try {
    await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        notes: [strictXhsNote(1, {
          noteId: 'note_with_comments',
          platformContentId: 'note_with_comments',
          comments_count: 12,
          comments: [{ commentId: 'embedded_comment', text: '想看后续' }],
        })],
      },
    );
    const packageBody = packagePayload(JSON.parse(calls[0][1].body));

    assert.equal(packageBody.records.length, 1);
    assert.equal(packageBody.records[0].recordKind, 'note');
    assert.equal(packageBody.records[0].payload.comments, 12);
    assert.equal(packageBody.records.some((record) => record.recordKind === 'comment'), false);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('syncToFlywheel reads apiToken from flywheel config instead of hardcoded token', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const restoreChrome = installChrome({
    storage: {
      local: {
        async get() {
          return {};
        },
        async set() {
          return undefined;
        },
      },
    },
  });
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    if (calls.length === 1) {
      return new Response(JSON.stringify({
        dataToken: 'data_token_123',
        expiresAt: '2026-05-07T00:00:00.000Z',
        user: { email: 'user@example.com', name: '使用者' },
        workspaceId: 'user-workspace',
      }), { status: 200 });
    }
    return new Response(JSON.stringify({
      status: 'committed',
      receiptId: 'receipt-auth',
    }), { status: 200 });
  };

  try {
    await saveFlywheelConfig({
      serverUrl: 'http://localhost:3000',
      enabled: true,
      apiToken: 'token_123',
    });

    await syncToFlywheel([strictXhsNote(1, {
      url: 'https://www.xiaohongshu.com/explore/1',
      title: '标题',
    })]);

    assert.equal(calls.length, 2);
    assert.equal(calls[0][1].headers.Authorization, 'Bearer token_123');
    assert.equal(calls[0][1].credentials, 'include');
    assert.equal(calls[1][1].headers.Authorization, 'Bearer token_123');
    assert.equal(calls[1][1].headers['X-Plugin-Data-Token'], 'data_token_123');
    assert.equal(calls[1][1].credentials, 'include');
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('background workbench sync delegates to the shared authorized manual importer', async () => {
  const [backgroundSource, syncSource] = await Promise.all([
    fs.readFile(backgroundSourcePath, 'utf8'),
    fs.readFile(flywheelSyncSourcePath, 'utf8'),
  ]);

  assert.match(backgroundSource, /syncManualRecordsToWorkbench/);
  assert.match(syncSource, /ensureFlywheelDataSession/);
  assert.match(syncSource, /\/api\/execution-tasks\/manual-import/);
  assert.match(syncSource, /credentials:\s*'include'/);
  assert.match(syncSource, /X-Plugin-Data-Token/);
});

test('popup app uses shared flywheel mappers from utils instead of local duplicates', async () => {
  const [appSource, utilsSource] = await Promise.all([
    fs.readFile(popupAppSourcePath, 'utf8'),
    fs.readFile(popupUtilsSourcePath, 'utf8'),
  ]);

  assert.match(appSource, /mapNoteToFlywheel/);
  assert.match(appSource, /mapCommentToFlywheel/);
  assert.match(appSource, /mapAuthorToFlywheel/);
  assert.match(appSource, /from '\.\/utils\.js'/);
  assert.match(utilsSource, /export function mapNoteToFlywheel\(/);
  assert.doesNotMatch(appSource, /function mapNoteToFlywheel\(/);
  assert.doesNotMatch(appSource, /function mapCommentToFlywheel\(/);
  assert.doesNotMatch(appSource, /function mapAuthorToFlywheel\(/);
});
