import test from 'node:test';
import assert from 'node:assert/strict';

import { resolveCollectionProfile } from '../src/workbench/runtime/collectionProfile.js';

test('resolves an explicit collection profile before task type fallback', () => {
  assert.equal(
    resolveCollectionProfile({
      collectionProfile: 'author_links',
      taskType: 'xhs.note_full',
    }),
    'author_links',
  );
});

test('maps author link task type when the top-level profile is missing', () => {
  assert.equal(
    resolveCollectionProfile({ taskType: 'xhs.authorNoteLinks' }),
    'author_links',
  );
});

test('uses persisted profile before payload fallback', () => {
  assert.equal(
    resolveCollectionProfile(
      { taskType: 'xhs.note_full', payload: { collectionProfile: 'list_scan' } },
      { collectionProfile: 'author_links' },
    ),
    'author_links',
  );
});
