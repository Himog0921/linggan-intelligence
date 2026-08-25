import assert from 'node:assert/strict';
import test from 'node:test';

import { createDashboardBridge } from '../src/content/dashboardBridge.js';

test('dashboard media action returns Linggan pending without reading or downloading platform media', async () => {
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
    // A legacy caller may still provide this value; the active bridge must
    // never invoke it before Linggan's media contract is available.
    downloadNoteMediaFromRecord: async () => {
      downloaderCalls += 1;
      throw new Error('must not download');
    },
    _testNonce: 'media-pending-nonce',
  });

  await bridge.handleDashboardMessageEvent({
    data: {
      source: 'lgboom-dashboard',
      action: 'downloadNoteMedia',
      nonce: 'media-pending-nonce',
      noteId: 'note-1',
    },
    ports: [{ postMessage(value) { payloads.push(value); } }],
  });

  assert.equal(noteReads, 0);
  assert.equal(downloaderCalls, 0);
  assert.equal(payloads[0].success, false);
  assert.equal(payloads[0].code, 'linggan_adapter_pending');
  assert.equal(payloads[0].capability, 'media_download');
  assert.match(payloads[0].message, /没有下载媒体/);
});
