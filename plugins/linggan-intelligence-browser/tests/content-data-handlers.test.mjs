import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createDataHandlers } from '../src/content/messageHandlers/dataHandlers.js';

function createDataHandlersWithStores({ notes = [], comments = [], authors = [], csvCalls }) {
  return createDataHandlers({
    MSG,
    ensurePluginAuthorized: async () => null,
    noteStore: {
      count: async () => notes.length,
      getAll: async () => notes,
      deleteById: async () => true,
      clear: async () => true,
    },
    commentStore: {
      count: async () => comments.length,
      getAll: async () => comments,
      deleteById: async () => true,
      clear: async () => true,
    },
    authorStore: {
      count: async () => authors.length,
      getAll: async () => authors,
      deleteById: async () => true,
      clear: async () => true,
    },
    generateCsv: (headers, rows) => {
      csvCalls.push({ headers, rows });
      return 'csv-content';
    },
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'xhs', pageType: 'detail' }),
  });
}

test('data handlers export csv includes quality fields for notes comments and authors', async () => {
  const csvCalls = [];
  const handlers = createDataHandlersWithStores({
    csvCalls,
    notes: [{
      platform: 'douyin',
      noteId: 'note_1',
      dataQuality: 'degraded',
      qualityReason: 'search_summary_seed',
      sourceTier: 'seed',
    }],
    comments: [{
      platform: 'xhs',
      commentId: 'comment_1',
      dataQuality: 'full',
      qualityReason: 'api_snapshot_partial',
      sourceTier: 'api',
    }],
    authors: [{
      platform: 'xhs',
      userId: 'author_1',
      handle: 'alice',
      dataQuality: 'degraded',
      qualityReason: 'inject_failed_dom_only',
      sourceTier: 'dom',
    }],
  });

  await handlers[MSG.EXPORT_CSV]({ type: 'notes' });
  await handlers[MSG.EXPORT_CSV]({ type: 'comments' });
  await handlers[MSG.EXPORT_CSV]({ type: 'authors' });

  assert.equal(csvCalls.length, 3);

  const [noteCsv, commentCsv, authorCsv] = csvCalls;

  assert.ok(noteCsv.headers.includes('dataQuality'));
  assert.ok(noteCsv.headers.includes('qualityReason'));
  assert.ok(noteCsv.headers.includes('sourceTier'));
  assert.deepEqual(
    noteCsv.rows[0].slice(-3),
    ['degraded', 'search_summary_seed', 'seed'],
  );

  assert.ok(commentCsv.headers.includes('dataQuality'));
  assert.ok(commentCsv.headers.includes('qualityReason'));
  assert.ok(commentCsv.headers.includes('sourceTier'));
  assert.deepEqual(
    commentCsv.rows[0].slice(-3),
    ['full', 'api_snapshot_partial', 'api'],
  );

  assert.ok(authorCsv.headers.includes('dataQuality'));
  assert.ok(authorCsv.headers.includes('qualityReason'));
  assert.ok(authorCsv.headers.includes('sourceTier'));
  assert.deepEqual(
    authorCsv.rows[0].slice(-3),
    ['degraded', 'inject_failed_dom_only', 'dom'],
  );
});

test('data handlers protect local record reads with plugin authorization', async () => {
  const handlers = createDataHandlersWithStores({
    csvCalls: [],
    notes: [{ noteId: 'note_1' }],
  });

  assert.deepEqual(await handlers[MSG.GET_STATS](), {
    notes: 1,
    comments: 0,
    authors: 0,
  });
  assert.deepEqual(await handlers[MSG.GET_ALL_NOTES](), [{ noteId: 'note_1' }]);
});
