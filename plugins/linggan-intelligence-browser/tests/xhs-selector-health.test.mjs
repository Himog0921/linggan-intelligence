import test from 'node:test';
import assert from 'node:assert/strict';

import {
  runXhsSelectorBootstrapProbe,
  runXhsSelectorPreflight,
} from '../src/platforms/xhs/selectorHealth.js';

function createDocument(queryMap = {}) {
  return {
    querySelector(selector) {
      const value = queryMap[selector];
      if (Array.isArray(value)) return value[0] || null;
      return value || null;
    },
    querySelectorAll(selector) {
      const value = queryMap[selector];
      if (Array.isArray(value)) return value;
      return value ? [value] : [];
    },
  };
}

test('xhs selector preflight blocks batch notes when expected feed container is missing', () => {
  const result = runXhsSelectorPreflight('batchNotes', {
    params: { mode: 'profile' },
    document: createDocument({}),
    win: { location: { href: 'https://www.xiaohongshu.com/user/profile/abc' } },
  });

  assert.equal(result.ok, false);
  assert.equal(result.code, 'selector_missing');
  assert.match(result.message, /批量笔记|页面结构/);
});

test('xhs selector preflight allows profile batch notes when profile feed container exists', () => {
  const result = runXhsSelectorPreflight('batchNotes', {
    params: { mode: 'profile' },
    document: createDocument({
      '#userPostedFeeds': { nodeType: 1 },
    }),
    win: { location: { href: 'https://www.xiaohongshu.com/user/profile/abc' } },
  });

  assert.equal(result.ok, true);
  assert.equal(result.checks[0].name, 'profile_feed_container');
  assert.match(result.checks[0].verifiedAt, /^20\d{2}-\d{2}-\d{2}T/);
});

test('xhs selector preflight blocks comment image download when comments container is missing', () => {
  const result = runXhsSelectorPreflight('collectCommentImages', {
    document: createDocument({}),
    win: { location: { href: 'https://www.xiaohongshu.com/explore/note_1' } },
  });

  assert.equal(result.ok, false);
  assert.equal(result.code, 'selector_missing');
  assert.match(result.message, /评论图片区|页面结构/);
});

test('xhs selector bootstrap probe inspects current search route without blocking tasks', () => {
  const win = {
    location: {
      href: 'https://www.xiaohongshu.com/search_result/coffee?keyword=%E5%92%96%E5%95%A1',
      pathname: '/search_result/coffee',
    },
  };
  const result = runXhsSelectorBootstrapProbe({
    document: createDocument({
      '.feeds-container': { nodeType: 1 },
    }),
    win,
  });

  assert.equal(result.action, 'bootstrap');
  assert.equal(result.ok, true);
  assert.equal(result.checks[0].name, 'feed_container');
});

test('xhs selector bootstrap keeps an unfinished SPA diagnostic instead of a blocked preflight', () => {
  const result = runXhsSelectorBootstrapProbe({
    document: createDocument({}),
    win: {
      location: {
        href: 'https://www.xiaohongshu.com/search_result/coffee?keyword=%E5%92%96%E5%95%A1',
        pathname: '/search_result/coffee',
      },
    },
  });

  assert.equal(result.ok, true);
  assert.equal(result.kind, 'diagnostic');
  assert.equal(result.diagnosticState, 'not_ready');
  assert.equal(result.code, 'not_ready');
  assert.equal(result.checks[0].ok, false);
});

test('xhs selector bootstrap accepts a signed detail route backed by SSR note state', () => {
  const noteId = '6a8e86de00000000240043b0';
  const win = {
    location: {
      href: `https://www.xiaohongshu.com/explore/${noteId}?xsec_token=SIGNED`,
      pathname: `/explore/${noteId}`,
    },
  };
  const document = createDocument({});
  document.scripts = [{
    textContent: `window.__INITIAL_STATE__={"note":{"noteDetailMap":{"${noteId}":{"note":{"noteId":"${noteId}","title":"真实详情"}}}}}`,
  }];

  const result = runXhsSelectorBootstrapProbe({ document, win });

  assert.equal(result.action, 'bootstrap');
  assert.equal(result.ok, true);
  assert.equal(result.checks[0].name, 'note_detail_ssr_state');
});
