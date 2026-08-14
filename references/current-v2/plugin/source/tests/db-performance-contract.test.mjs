import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

test('latest db schema indexes collectionRunId for notes and authors', () => {
  const schema = readFileSync(new URL('../src/db/index.js', import.meta.url), 'utf8');

  assert.match(schema, /db\.version\(13\)\.stores/);
  assert.match(schema, /notes: 'noteId,[^']*collectionRunId/);
  assert.match(schema, /authors: 'userId,[^']*collectionRunId/);
});

test('note and author stores query collectionRunId through indexed lookups', () => {
  const noteStore = readFileSync(new URL('../src/db/noteStore.js', import.meta.url), 'utf8');
  const authorStore = readFileSync(new URL('../src/db/authorStore.js', import.meta.url), 'utf8');

  assert.match(noteStore, /\.where\('collectionRunId'\)\.equals\(id\)/);
  assert.match(authorStore, /\.where\('collectionRunId'\)\.equals\(id\)/);
});
