import assert from 'node:assert/strict';
import test from 'node:test';

import { createDashboardBridge } from '../src/content/dashboardBridge.js';

test('dashboard media action enters the registered Linggan media runtime without using a legacy download', async () => {
  const payloads = [];
  let noteReads = 0;
  let downloaderCalls = 0;
  const bridge = createDashboardBridge({
    noteStore: {
      async getById() {
        noteReads += 1;
        return { noteId: 'note-1' };
      },
    },
    commentStore: {},
    authorStore: {},
    downloadNoteMediaFromRecord: async (note, options) => {
      downloaderCalls += 1;
      assert.equal(note.noteId, 'note-1');
      assert.deepEqual(options, {});
      return { success: true, queued: true, mediaDelivery: 'pending' };
    },
    _testNonce: 'media-pending-nonce',
  });

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'downloadNoteMedia',
      nonce: 'media-pending-nonce',
      note: { noteId: 'note-1' },
    },
    ports: [{ postMessage(value) { payloads.push(value); } }],
  });

  assert.equal(noteReads, 0);
  assert.equal(downloaderCalls, 1);
  assert.equal(payloads[0].success, true);
  assert.equal(payloads[0].queued, true);
  assert.equal(payloads[0].mediaDelivery, 'pending');
});
