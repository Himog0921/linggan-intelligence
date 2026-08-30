import test from 'node:test';
import assert from 'node:assert/strict';

import { readXhsSsrNoteDetailMap } from '../src/platforms/xhs/ssrNoteMap.js';
import { waitForNoteState } from '../src/platforms/xhs/batchShared.js';

const NOTE_ID = '6a8e86de00000000240043b0';

function documentWithScripts(...texts) {
  return {
    scripts: texts.map((textContent) => ({ textContent })),
  };
}

test('reads the XHS SSR noteDetailMap after the global initial state has been removed', () => {
  const note = {
    noteId: NOTE_ID,
    title: '不要浪费 ADHD 最耀眼的天赋',
    desc: '详情正文',
    imageList: [{ urlDefault: 'https://sns-webpic-qc.xhscdn.com/detail.webp' }],
    interactInfo: { likedCount: '89', collectedCount: '12', commentCount: '7' },
  };
  const doc = documentWithScripts(
    `window.__INITIAL_STATE__={"note":{"noteDetailMap":${JSON.stringify({ [NOTE_ID]: { note } })}},"user":{}}`,
  );

  assert.deepEqual(readXhsSsrNoteDetailMap(doc), {
    [NOTE_ID]: { note },
  });
});

test('does not execute page script text while recovering the SSR map', () => {
  globalThis.__xhsSsrProbeExecuted = false;
  const doc = documentWithScripts(
    `window.__INITIAL_STATE__={"note":{"noteDetailMap":{"${NOTE_ID}":{"note":{"noteId":"${NOTE_ID}"}}}},"tail":(globalThis.__xhsSsrProbeExecuted=true)}`,
  );

  try {
    assert.deepEqual(readXhsSsrNoteDetailMap(doc), {
      [NOTE_ID]: { note: { noteId: NOTE_ID } },
    });
    assert.equal(globalThis.__xhsSsrProbeExecuted, false);
  } finally {
    delete globalThis.__xhsSsrProbeExecuted;
  }
});

test('returns an empty map for malformed, unrelated, or oversized script text', () => {
  assert.deepEqual(readXhsSsrNoteDetailMap(documentWithScripts('window.foo={"noteDetailMap":{}}')), {});
  assert.deepEqual(readXhsSsrNoteDetailMap(documentWithScripts('window.__INITIAL_STATE__={"note":{"noteDetailMap":{')), {});
  assert.deepEqual(
    readXhsSsrNoteDetailMap(documentWithScripts(`window.__INITIAL_STATE__=${' '.repeat((5 * 1024 * 1024) + 1)}"noteDetailMap":{}`)),
    {},
  );
});

test('detail readiness accepts SSR state without waiting for the removed global state', async () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  globalThis.window = { __INITIAL_STATE__: undefined };
  globalThis.document = documentWithScripts(
    `window.__INITIAL_STATE__={"note":{"noteDetailMap":{"${NOTE_ID}":{"note":{"noteId":"${NOTE_ID}","title":"真实详情"}}}}}`,
  );

  try {
    assert.equal(await waitForNoteState(NOTE_ID, 20), true);
  } finally {
    globalThis.window = originalWindow;
    globalThis.document = originalDocument;
  }
});
