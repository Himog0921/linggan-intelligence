import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const probeNames = [
  'probe-xhs-search-context-v2.js',
  'probe-xhs-search-results-v2.js',
  'probe-xhs-note-detail-v2.js',
  'probe-xhs-profile-v2.js',
];

test('XHS V2 probes only emit sanitized page capability facts', async () => {
  for (const probeName of probeNames) {
    const source = await readFile(new URL(`../scripts/${probeName}`, import.meta.url), 'utf8');
    assert.match(source, /sanitized:\s*true/, `${probeName} must mark its output sanitized`);
    assert.doesNotMatch(source, /\bfetch\s*\(/, `${probeName} must not call a platform API`);
    assert.doesNotMatch(source, /credentials\s*:/, `${probeName} must not forward credentials`);
    assert.doesNotMatch(source, /document\.cookie|localStorage|sessionStorage/i, `${probeName} must not read browser secrets`);
    assert.doesNotMatch(source, /innerHTML|outerHTML/i, `${probeName} must not emit source fragments`);
  }
});

test('XHS V2 probes cover the page structures observed by AUD-XHS-001', async () => {
  const searchContext = await readFile(new URL('../scripts/probe-xhs-search-context-v2.js', import.meta.url), 'utf8');
  const noteDetail = await readFile(new URL('../scripts/probe-xhs-note-detail-v2.js', import.meta.url), 'utf8');

  assert.match(searchContext, /sug-item/, 'search suggestions must cover the observed sug-* structure');
  assert.match(noteDetail, /comments-el/, 'detail probe must cover the observed comments-el root');
  assert.match(noteDetail, /no-comments/, 'detail probe must preserve an explicit empty-comment state');
  assert.match(noteDetail, /cap:\s*30/, 'detail probe must retain the 30-comment cap');
});
