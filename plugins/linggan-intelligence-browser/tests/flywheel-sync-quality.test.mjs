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

test('popup flywheel mappers preserve quality fields and collectionRunId', () => {
  const note = mapNoteToFlywheel({
    noteId: 'note_1',
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

  assert.equal(comment.dataQuality, 'full');
  assert.equal(comment.qualityReason, '');
  assert.equal(comment.sourceTier, 'api');
  assert.equal(comment.collectionRunId, 'run_comment_1');

  assert.equal(author.dataQuality, 'degraded');
  assert.equal(author.qualityReason, 'inject_failed_dom_only');
  assert.equal(author.sourceTier, 'dom');
  assert.equal(author.collectionRunId, 'run_author_1');
});

test('syncToFlywheel posts quality fields and collectionRunId in batch payload', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      imported: 1,
      skipped: 0,
      sources: [],
    }), { status: 200 });
  };

  try {
    const result = await syncToFlywheel([{
      noteId: 'note_1',
      platform: 'douyin',
      url: 'https://www.douyin.com/video/1',
      title: '标题',
      content: '正文',
      dataQuality: 'seed',
      qualityReason: 'search_summary_seed',
      sourceTier: 'seed',
      collectionRunId: 'run_note_1',
    }]);

    assert.equal(result.success, true);
    assert.equal(calls.length, 1);

    const payload = JSON.parse(calls[0][1].body);
    assert.equal(payload.result.records.notes[0].dataQuality, 'seed');
    assert.equal(payload.result.records.notes[0].qualityReason, 'search_summary_seed');
    assert.equal(payload.result.records.notes[0].sourceTier, 'seed');
    assert.equal(payload.result.records.notes[0].collectionRunId, 'run_note_1');
    assert.equal(payload.result.records.notes[0].rawData.collectionRunId, 'run_note_1');
    assert.match(calls[0][0], /\/api\/execution-tasks\/manual-import$/);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('syncToFlywheel keeps partial batch success when a later batch fails', async () => {
  const originalFetch = globalThis.fetch;
  let callCount = 0;
  globalThis.fetch = async () => {
    callCount += 1;
    if (callCount === 1) {
      return new Response(JSON.stringify({
        imported: 50,
        skipped: 0,
      }), { status: 200 });
    }
    return new Response(JSON.stringify({
      error: 'batch failed',
    }), { status: 500 });
  };

  try {
    const payload = Array.from({ length: 51 }, (_, index) => ({
      noteId: `note_${index + 1}`,
      platform: 'xhs',
      url: `https://www.xiaohongshu.com/explore/${index + 1}`,
      title: `标题 ${index + 1}`,
    }));
    const result = await syncToFlywheel(payload);

    assert.equal(result.success, true);
    assert.equal(result.imported, 50);
    assert.equal(result.skipped, 0);
    assert.deepEqual(result.details, []);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('manual workbench sync carries comment-only and author-only records', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      imported: 2,
      skipped: 0,
      meta: {
        commentsReceived: 1,
        commentsRegistered: 1,
        authorsReceived: 1,
        authorsIngested: 1,
      },
    }), { status: 200 });
  };

  try {
    const result = await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        comments: [{ commentId: 'comment_1', text: '求方法', platform: 'xhs' }],
        authors: [{ userId: 'author_1', name: '作者', platform: 'xhs' }],
      },
    );
    const payload = JSON.parse(calls[0][1].body);

    assert.equal(result.success, true);
    assert.equal(payload.result.records.notes.length, 0);
    assert.equal(payload.result.records.comments[0].commentId, 'comment_1');
    assert.equal(payload.result.records.authors[0].userId, 'author_1');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('manual workbench sync sends one image source for each candidate group', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const cover = 'https://sns-img.example.com/original-cover.webp';
  const fallback = 'https://sns-img.example.com/original-cover-fallback.webp';
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      success: true,
      imported: 1,
      skipped: 0,
      meta: { mediaRegistrationConfirmed: true },
    }), { status: 200 });
  };

  try {
    await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        notes: [{
          noteId: '68fb9898000000000402084f',
          platform: 'xhs',
          type: 'video',
          url: 'https://www.xiaohongshu.com/explore/68fb9898000000000402084f',
          cover,
          images: [cover],
          imageCandidates: [[cover, fallback]],
          video: 'https://sns-video.example.com/stream.mp4',
        }],
      },
    );

    const payload = JSON.parse(calls[0][1].body);
    const note = payload.result.records.notes[0];
    assert.deepEqual(note.imageUrls, [cover]);
    assert.equal(note.imageCandidates, undefined);
    assert.equal(note.images, undefined);
    assert.deepEqual(note.rawData.imageCandidates, [[cover, fallback]]);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('manual workbench sync separates embedded comment records from the note comment count', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({ imported: 2, skipped: 0, meta: {} }), { status: 200 });
  };

  try {
    await syncManualRecordsToWorkbench(
      { serverUrl: 'http://localhost:3000' },
      {
        notes: [{
          noteId: 'note_with_comments',
          platform: 'xhs',
          comments_count: 12,
          comments: [{ commentId: 'embedded_comment', text: '想看后续' }],
        }],
      },
    );
    const payload = JSON.parse(calls[0][1].body);

    assert.equal(payload.result.records.notes[0].comments, 12);
    assert.equal(payload.result.records.comments[0].commentId, 'embedded_comment');
    assert.equal(payload.result.records.comments[0].noteId, 'note_with_comments');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('syncToFlywheel reads apiToken from flywheel config instead of hardcoded token', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const originalChrome = globalThis.chrome;
  globalThis.chrome = {
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
  };
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
      imported: 1,
      skipped: 0,
      sources: [],
    }), { status: 200 });
  };

  try {
    await saveFlywheelConfig({
      serverUrl: 'http://localhost:3000',
      enabled: true,
      apiToken: 'token_123',
    });

    await syncToFlywheel([{
      noteId: 'note_1',
      platform: 'xhs',
      url: 'https://www.xiaohongshu.com/explore/1',
      title: '标题',
    }]);

    assert.equal(calls.length, 2);
    assert.equal(calls[0][1].headers.Authorization, 'Bearer token_123');
    assert.equal(calls[0][1].credentials, 'include');
    assert.equal(calls[1][1].headers.Authorization, 'Bearer token_123');
    assert.equal(calls[1][1].headers['X-Plugin-Data-Token'], 'data_token_123');
    assert.equal(calls[1][1].credentials, 'include');
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.chrome = originalChrome;
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
