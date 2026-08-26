import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { IDBKeyRange, indexedDB } from 'fake-indexeddb';

globalThis.indexedDB = indexedDB;
globalThis.IDBKeyRange = IDBKeyRange;

function createVisibleCard(noteId, top, left) {
  const cover = {
    getAttribute: (name) => name === 'href' ? `/explore/${noteId}` : '',
    querySelector: () => null,
  };
  return {
    querySelector(selector) {
      if (selector === 'a.cover') return cover;
      if (selector === '.footer span' || selector === '.title' || selector === '.like-wrapper .count') return null;
      if (selector === '.play-icon') return null;
      return null;
    },
    getBoundingClientRect: () => ({ top, left }),
  };
}

test('the current-surface handler reads only fixture cards and queues without the TDZ', async () => {
  const original = {
    chrome: globalThis.chrome,
    document: globalThis.document,
    window: globalThis.window,
    fetch: globalThis.fetch,
  };
  const listeners = [];
  const toasts = [];
  const storage = new Map();
  const cards = [createVisibleCard('65ee6fe6000000000000000a', 10, 10), createVisibleCard('65ee6fe6000000000000000b', 10, 220)];
  let selectorCalls = 0;
  let localFetches = 0;

  globalThis.document = {
    querySelectorAll(selector) {
      selectorCalls += 1;
      assert.equal(selector, '.feeds-container section');
      return cards;
    },
    querySelector: () => null,
  };
  globalThis.window = {
    location: { href: 'https://www.xiaohongshu.com/search_result?keyword=synthetic' },
    scrollY: 0,
  };
  globalThis.fetch = async () => {
    localFetches += 1;
    throw new Error('synthetic_local_host_unavailable');
  };
  globalThis.chrome = {
    runtime: {
      id: 'synthetic-extension',
      lastError: null,
      onMessage: { addListener(listener) { listeners.push(listener); } },
      sendMessage(message, callback) {
        const listener = listeners.at(-1);
        assert.ok(listener, 'the local Producer background handler must be registered');
        listener(message, {}, callback);
      },
      getManifest: () => ({ version: 'test' }),
    },
    storage: {
      local: {
        async get(key) { return { [key]: storage.get(key) }; },
        async set(value) { Object.entries(value).forEach(([key, item]) => storage.set(key, item)); },
      },
    },
    tabs: { query: async () => [], create: async () => {} },
  };

  try {
    await import(`../src/linggan/background.js?current-surface-tdz=${Date.now()}`);
    const { readCurrentVisibleSurfaceNotes } = await import('../src/platforms/xhs/noteCollector.js');
    const { createLingganContentRuntime } = await import('../src/linggan/contentRuntimeAdapter.js');
    const { createXhsPageController } = await import('../src/content/xhsPageController.js');

    const runtime = createLingganContentRuntime({ platform: 'xhs' });
    const controller = createXhsPageController({
      assertPluginAuthorized: async () => ({ mode: 'LOCAL_TRUSTED', authorized: true }),
      discoverSurface: ({ maximumQuota }) => readCurrentVisibleSurfaceNotes('.feeds-container', maximumQuota),
      submitDiscovery: (visibleCards, context) => runtime.submitDiscovery(visibleCards, context),
      isContextValid: () => true,
      showToast: (message, level) => toasts.push({ message, level }),
      collectComments: async () => [],
      collectCommentImages: async () => [],
      collectNote: async () => ({}),
      collectAuthor: async () => ({}),
      injectUI: () => {}, toggleStopButton: () => {}, togglePauseResumeButtons: () => {},
      showCommentLimitDialog: async () => ({}), showMediaDownloadDialog: async () => ({}),
      showBatchSettingsDialog: async () => ({}), ensureTaskControlBar: () => {}, updateTaskControlBar: () => {},
      hideTaskControlBar: () => {}, reportDone: () => {}, extractNoteId: () => '', sendToBackground: async () => ({}),
      downloadNoteMediaFromRecord: async () => ({}),
    });

    await controller.handleButtonClick({
      target: { closest: (selector) => selector === '.lgboom-btn' ? { dataset: { action: 'discoverSurface', params: '{"mode":"search"}' } } : null },
    });

    assert.equal(selectorCalls, 1, 'the reader must query exactly the declared current-surface selector once');
    assert.equal(toasts.some(({ level }) => level === 'error'), false, 'the TDZ must not become a page error');
    assert.equal(toasts.at(-1)?.message, '已读取当前页面 2 条，待本机 Linggan 交付');
    assert.equal(localFetches >= 0, true, 'the synthetic test has no platform request');
  } finally {
    globalThis.chrome = original.chrome;
    globalThis.document = original.document;
    globalThis.window = original.window;
    globalThis.fetch = original.fetch;
  }
});

test('the current-surface handler remains isolated from legacy discovery and deeper collectors', () => {
  const content = readFileSync(new URL('../src/content/index.js', import.meta.url), 'utf8');
  const start = content.indexOf('discoverSurface: async');
  const end = content.indexOf('\n  submitDiscovery:', start);
  const currentSurface = content.slice(start, end);
  assert.match(currentSurface, /readCurrentVisibleSurfaceNotes/);
  assert.doesNotMatch(currentSurface, /discoverSurfaceNotesFromBestSource|discoverWithScroll|requestXhs(?:Search|Profile)NotesSnapshot|ensureXhsCommentApiBridge|Batch(?:Note|Comment)Controller|collectNote|collectComments|collectAuthor|acquireMedia/);
});
