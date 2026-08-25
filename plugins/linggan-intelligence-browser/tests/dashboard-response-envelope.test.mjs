import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createDashboardBridge } from '../src/content/dashboardBridge.js';
import { sendToParent, unwrapParentResponseData } from '../src/dashboard/utils.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('dashboard bridge wraps raw collection payloads in a success/data envelope', async () => {
  const payloads = [];
  const TEST_NONCE = 'test-nonce-1';
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
      getAll: async () => [{ noteId: 'n1' }],
    },
    commentStore: {
      getAll: async () => [],
    },
    authorStore: {
      getAll: async () => [],
    },
    downloadNoteMediaFromRecord: async () => ({}),
    _testNonce: TEST_NONCE,
  });

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'getAllNotes',
      nonce: TEST_NONCE,
    },
    ports: [{
      postMessage(value) {
        payloads.push(value);
      },
    }],
  });

  assert.deepEqual(payloads[0], {
    success: true,
    data: [{ noteId: 'n1' }],
  });
});

test('dashboard bridge returns paged local records when a limit is provided', async () => {
  const payloads = [];
  const TEST_NONCE = 'test-nonce-page';
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
      count: async () => 3,
      getPage: async ({ offset, limit }) => [
        { noteId: `n${offset + 1}` },
        { noteId: `n${offset + 2}` },
      ].slice(0, limit),
    },
    commentStore: {},
    authorStore: {},
    downloadNoteMediaFromRecord: async () => ({}),
    _testNonce: TEST_NONCE,
  });

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'getAllNotes',
      nonce: TEST_NONCE,
      offset: 1,
      limit: 2,
    },
    ports: [{
      postMessage(value) {
        payloads.push(value);
      },
    }],
  });

  assert.deepEqual(payloads[0], {
    success: true,
    data: [{ noteId: 'n2' }, { noteId: 'n3' }],
    total: 3,
    offset: 1,
    limit: 2,
    hasMore: false,
  });
});

test('dashboard app keeps workbench and monitor records visible in local data table', () => {
  const appSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/App.jsx'), 'utf8');

  assert.doesNotMatch(
    appSource,
    /isMonitorGeneratedRecord/,
    'dashboard should not hide records created by workbench monitor tasks',
  );
  assert.match(
    appSource,
    /accumulated\s*=\s*accumulated\.concat\(data\)/,
    'dashboard should append every local record returned by the bridge',
  );
});

test('dashboard bridge preserves sync metadata while also exposing data envelope', async () => {
  const payloads = [];
  const TEST_NONCE = 'test-nonce-2';
  globalThis.chrome = {
    runtime: {
      async sendMessage() {
        return { success: true, imported: 3, skipped: 1 };
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

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'syncToWorkbench',
      nonce: TEST_NONCE,
      notes: [{ noteId: 'n1' }],
      comments: [],
      authors: [],
    },
    ports: [{
      postMessage(value) {
        payloads.push(value);
      },
    }],
  });

  assert.deepEqual(payloads[0], {
    success: true,
    imported: 3,
    skipped: 1,
    data: {
      imported: 3,
      skipped: 1,
    },
  });

  delete globalThis.chrome;
});

test('unwrapParentResponseData prefers data from a success envelope and falls back to raw payloads', () => {
  assert.deepEqual(
    unwrapParentResponseData({ success: true, data: [{ noteId: 'n1' }] }),
    [{ noteId: 'n1' }],
  );
  assert.deepEqual(
    unwrapParentResponseData([{ noteId: 'n2' }]),
    [{ noteId: 'n2' }],
  );
  assert.deepEqual(unwrapParentResponseData({ success: false, error: 'x' }, []), []);
});

test('sendToParent uses nonce from dashboard URL when storage has not caught up', async () => {
  const payloads = [];
  globalThis.chrome = {
    storage: {
      session: {
        async get() {
          return {};
        },
      },
      local: {
        async get() {
          return {};
        },
      },
    },
  };
  globalThis.window = {
    location: {
      href: 'chrome-extension://lgboom/dashboard.html?nonce=url-nonce-1',
      search: '?nonce=url-nonce-1',
      hash: '',
    },
    parent: {
      postMessage(payload, _targetOrigin, ports) {
        payloads.push(payload);
        ports[0].postMessage({ success: true, data: [{ noteId: 'n1' }] });
      },
    },
  };

  try {
    const response = await sendToParent('getAllNotes', {}, { timeoutMs: 500 });

    assert.equal(payloads[0].nonce, 'url-nonce-1');
    assert.deepEqual(response, { success: true, data: [{ noteId: 'n1' }] });
  } finally {
    delete globalThis.chrome;
    delete globalThis.window;
  }
});

test('sendToParent falls back to local storage nonce when session storage is empty', async () => {
  const payloads = [];
  globalThis.chrome = {
    storage: {
      session: {
        async get() {
          return {};
        },
      },
      local: {
        async get() {
          return { dashboardNonce: 'local-nonce-1' };
        },
      },
    },
  };
  globalThis.window = {
    location: {
      href: 'chrome-extension://lgboom/dashboard.html',
      search: '',
      hash: '',
    },
    parent: {
      postMessage(payload, _targetOrigin, ports) {
        payloads.push(payload);
        ports[0].postMessage({ success: true, data: [{ noteId: 'n1' }] });
      },
    },
  };

  try {
    const response = await sendToParent('getAllNotes', {}, { timeoutMs: 500 });

    assert.equal(payloads[0].nonce, 'local-nonce-1');
    assert.deepEqual(response, { success: true, data: [{ noteId: 'n1' }] });
  } finally {
    delete globalThis.chrome;
    delete globalThis.window;
  }
});

test('dashboard bridge keeps the nonce out of iframe URL for first load', async () => {
  const appended = [];
  const storageWrites = [];
  globalThis.chrome = {
    runtime: {
      getURL(path) {
        return `chrome-extension://lgboom/${path}`;
      },
    },
    storage: {
      session: {
        async set(value) {
          storageWrites.push({ area: 'session', value });
        },
        async remove() {},
      },
      local: {
        async set(value) {
          storageWrites.push({ area: 'local', value });
        },
        async remove() {},
      },
    },
  };
  globalThis.document = {
    body: {
      contains() {
        return false;
      },
      appendChild(node) {
        appended.push(node);
      },
    },
    createElement(tagName) {
      return {
        tagName,
        style: {},
        addEventListener() {},
        remove() {},
      };
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
  });

  try {
    await bridge.toggleDashboard();

    const iframe = appended.find((node) => node.tagName === 'iframe');
    assert.ok(iframe);
    assert.equal(iframe.src, 'chrome-extension://lgboom/dashboard.html');
    assert.deepEqual(storageWrites.map((write) => write.area).sort(), ['local', 'session']);
    assert.ok(storageWrites.every((write) => write.value.dashboardNonce));
  } finally {
    delete globalThis.chrome;
    delete globalThis.document;
  }
});

test('dashboard bridge rejects page-forged destructive messages even with a matching nonce', async () => {
  let clearCalls = 0;
  const trustedDashboardWindow = {};
  const pageWindow = {};
  const TEST_NONCE = 'leaked-nonce';
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
      clear: async () => {
        clearCalls += 1;
      },
    },
    commentStore: {},
    authorStore: {},
    downloadNoteMediaFromRecord: async () => ({}),
    _testNonce: TEST_NONCE,
    _testDashboardWindow: trustedDashboardWindow,
  });

  const handled = await bridge.handleDashboardMessageEvent({
    source: pageWindow,
    data: {
      source: 'lgboom-dashboard',
      action: 'clearAllNotes',
      nonce: TEST_NONCE,
    },
    ports: [{
      postMessage() {
        throw new Error('forged message should not receive a response');
      },
    }],
  });

  assert.equal(handled, false);
  assert.equal(clearCalls, 0);
});
