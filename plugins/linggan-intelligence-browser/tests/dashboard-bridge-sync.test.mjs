import test from 'node:test';
import assert from 'node:assert/strict';

import { createDashboardBridge } from '../src/content/dashboardBridge.js';

test('dashboard bridge forwards notes, comments, and authors to background sync handler', async () => {
  const sentMessages = [];
  const TEST_NONCE = 'test-nonce-sync';
  globalThis.chrome = {
    runtime: {
      async sendMessage(payload) {
        sentMessages.push(payload);
        return { success: true, imported: 3, skipped: 0 };
      },
    },
    storage: {
      session: {
        async set() {},
        async get() { return {}; },
        async remove() {},
      },
    },
  };

  const bridge = createDashboardBridge({
    MSG: {
      GET_ALL_NOTES: 'getAllNotes',
      GET_ALL_COMMENTS: 'getAllComments',
      GET_ALL_AUTHORS: 'getAllAuthors',
      DOWNLOAD_NOTE_MEDIA: 'downloadNoteMedia',
      CLEAR_ALL_NOTES: 'clearAllNotes',
      CLEAR_ALL_COMMENTS: 'clearAllComments',
      CLEAR_ALL_AUTHORS: 'clearAllAuthors',
      DELETE_NOTE: 'deleteNote',
      DELETE_COMMENT: 'deleteComment',
      DELETE_AUTHOR: 'deleteAuthor',
      SYNC_TO_WORKBENCH: 'syncToWorkbench',
    },
    noteStore: {},
    commentStore: {},
    authorStore: {},
    downloadNoteMediaFromRecord: async () => ({}),
    _testNonce: TEST_NONCE,
  });

  const result = await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'syncToWorkbench',
      nonce: TEST_NONCE,
      notes: [{ noteId: 'n1' }],
      comments: [{ commentId: 'c1' }],
      authors: [{ userId: 'u1' }],
    },
    ports: [{
      postMessage(payload) {
        sentMessages.push({ portPayload: payload });
      },
    }],
  });

  assert.equal(result, true);
  assert.deepEqual(sentMessages[0], {
    action: 'syncToWorkbench',
    notes: [{ noteId: 'n1' }],
    comments: [{ commentId: 'c1' }],
    authors: [{ userId: 'u1' }],
  });
  assert.deepEqual(sentMessages[1], {
    portPayload: {
      success: true,
      imported: 3,
      skipped: 0,
      data: {
        imported: 3,
        skipped: 0,
      },
    },
  });

  delete globalThis.chrome;
});

test('dashboard bridge forwards selected media types into note media download', async () => {
  const payloads = [];
  const downloadCalls = [];
  const TEST_NONCE = 'test-nonce-media-types';

  const bridge = createDashboardBridge({
    MSG: {
      GET_ALL_NOTES: 'getAllNotes',
      GET_ALL_COMMENTS: 'getAllComments',
      GET_ALL_AUTHORS: 'getAllAuthors',
      DOWNLOAD_NOTE_MEDIA: 'downloadNoteMedia',
      CLEAR_ALL_NOTES: 'clearAllNotes',
      CLEAR_ALL_COMMENTS: 'clearAllComments',
      CLEAR_ALL_AUTHORS: 'clearAllAuthors',
      DELETE_NOTE: 'deleteNote',
      DELETE_COMMENT: 'deleteComment',
      DELETE_AUTHOR: 'deleteAuthor',
      SYNC_TO_WORKBENCH: 'syncToWorkbench',
    },
    noteStore: {
      getById: async () => ({ noteId: 'n1', title: '测试笔记' }),
    },
    commentStore: {},
    authorStore: {},
    downloadNoteMediaFromRecord: async (note, options) => {
      downloadCalls.push({ note, options });
      return { total: 1, success: 1, failed: 0 };
    },
    _testNonce: TEST_NONCE,
  });

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'downloadNoteMedia',
      nonce: TEST_NONCE,
      noteId: 'n1',
      mediaTypes: ['cover'],
    },
    ports: [{
      postMessage(value) {
        payloads.push(value);
      },
    }],
  });

  assert.equal(downloadCalls.length, 1);
  assert.equal(downloadCalls[0].note.noteId, 'n1');
  assert.deepEqual(downloadCalls[0].options, { mediaTypes: ['cover'] });
  assert.deepEqual(payloads[0], {
    success: true,
    summary: { total: 1, success: 1, failed: 0 },
    data: {
      summary: { total: 1, success: 1, failed: 0 },
    },
  });
});

test('dashboard bridge registers one window listener and can unregister it', () => {
  const originalWindow = globalThis.window;
  const added = [];
  const removed = [];
  globalThis.window = {
    addEventListener(type, handler) {
      added.push({ type, handler });
    },
    removeEventListener(type, handler) {
      removed.push({ type, handler });
    },
  };

  try {
    const bridge = createDashboardBridge({
      MSG: {},
      noteStore: {},
      commentStore: {},
      authorStore: {},
      downloadNoteMediaFromRecord: async () => ({}),
      _testNonce: 'listener-test',
    });

    bridge.registerDashboardBridge();
    bridge.registerDashboardBridge();
    assert.equal(added.length, 1);
    assert.equal(added[0].type, 'message');

    bridge.unregisterDashboardBridge();
    bridge.unregisterDashboardBridge();
    assert.equal(removed.length, 1);
    assert.deepEqual(removed[0], added[0]);
  } finally {
    globalThis.window = originalWindow;
  }
});
