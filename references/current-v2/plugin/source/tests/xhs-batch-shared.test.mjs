import test from 'node:test';
import assert from 'node:assert/strict';

import { getActiveCommentsContext, waitForNoteState } from '../src/platforms/xhs/batchShared.js';

test('waitForNoteState accepts embedded SSR note data after INITIAL_STATE cleanup', async () => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const noteId = '6a56177e000000000f01e0ee';
  globalThis.window = { __INITIAL_STATE__: undefined };
  globalThis.document = {
    scripts: [{
      textContent: `window.__INITIAL_STATE__={"note":{"noteDetailMap":{"${noteId}":{"note":{"noteId":"${noteId}"}}}},"global":{"detailMap":new Map([])}}`,
    }],
  };

  try {
    assert.equal(await waitForNoteState(noteId, 5), true);
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});

function createFakeEnv() {
  const defaultView = {
    getComputedStyle(el) {
      return el.__style || {
        display: 'block',
        visibility: 'visible',
        opacity: '1',
      };
    },
  };

  const document = {
    defaultView,
    querySelectorAll(selector) {
      return this.__queries?.[selector] || [];
    },
    __queries: {},
  };

  function createElement({
    width = 200,
    height = 200,
    style = null,
    text = '',
    queries = {},
    parent = null,
  } = {}) {
    const el = {
      ownerDocument: document,
      offsetWidth: width,
      offsetHeight: height,
      parentElement: parent,
      innerText: text,
      __style: style,
      __queries: queries,
      getBoundingClientRect() {
        return { width, height };
      },
      querySelectorAll(selector) {
        return this.__queries?.[selector] || [];
      },
    };
    return el;
  }

  return { document, createElement };
}

test('getActiveCommentsContext prefers visible comments container inside active detail root', () => {
  const { document, createElement } = createFakeEnv();

  const hiddenContainer = createElement({
    width: 0,
    height: 0,
    style: { display: 'none', visibility: 'hidden', opacity: '0' },
    text: '共 99 条评论',
  });
  const activeCommentItem = createElement({ width: 20, height: 20 });
  const activeContainer = createElement({
    text: '共 3 条评论',
    queries: {
      '.parent-comment, .comment-item': [activeCommentItem],
    },
  });
  const detailRoot = createElement({
    width: 320,
    height: 640,
    queries: {
      '.comments-container': [activeContainer],
      '[class*="comments"]': [activeContainer],
    },
  });
  activeContainer.parentElement = detailRoot;

  document.__queries = {
    '.note-detail': [detailRoot],
    '.comments-container': [hiddenContainer, activeContainer],
    '[class*="comments"]': [hiddenContainer, activeContainer],
  };

  const result = getActiveCommentsContext(document);

  assert.equal(result.root, detailRoot);
  assert.equal(result.container, activeContainer);
  assert.equal(result.hasCommentItems, true);
  assert.equal(result.hasCommentMeta, true);
});

test('getActiveCommentsContext detects explicit empty comments state', () => {
  const { document, createElement } = createFakeEnv();

  const emptyContainer = createElement({
    text: '还没有评论，快来抢沙发',
  });
  const detailRoot = createElement({
    width: 320,
    height: 640,
    queries: {
      '.comments-container': [emptyContainer],
      '[class*="comments"]': [emptyContainer],
    },
  });
  emptyContainer.parentElement = detailRoot;

  document.__queries = {
    '.note-detail': [detailRoot],
    '.comments-container': [emptyContainer],
    '[class*="comments"]': [emptyContainer],
  };

  const result = getActiveCommentsContext(document);

  assert.equal(result.container, emptyContainer);
  assert.equal(result.hasCommentItems, false);
  assert.equal(result.hasExplicitEmptyState, true);
});
