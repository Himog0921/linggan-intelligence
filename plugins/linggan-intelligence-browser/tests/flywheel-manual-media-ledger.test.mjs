import test from 'node:test';
import assert from 'node:assert/strict';

import { syncManualRecordsToWorkbench } from '../src/sync/flywheelSync.js';

test('manual workbench sync sends source media only through the import endpoint', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url, options = {}) => {
    calls.push([url, options]);
    if (String(url).endsWith('/api/plugin-data-workspace')) {
      return new Response(JSON.stringify({ dataToken: 'data_token_123' }), { status: 200 });
    }
    return new Response(JSON.stringify({
      success: true,
      imported: 1,
      skipped: 0,
      meta: {
        mediaRegistrationConfirmed: true,
        mediaObserved: 2,
        mediaEnqueued: 2,
        mediaRequiredMissing: 0,
        mediaUnlinked: 0,
      },
    }), { status: 200 });
  };

  try {
    const result = await syncManualRecordsToWorkbench({
      serverUrl: 'https://workbench.example',
      apiToken: 'token_123',
      dataToken: 'data_token_123',
    }, {
      notes: [{
        platform: 'xhs',
        noteId: 'note_1',
        type: 'video',
        cover: 'https://sns-img.example.com/cover.webp',
        video: 'https://sns-video.example.com/video.mp4',
      }],
    });

    assert.equal(calls.length, 2);
    assert.match(calls[1][0], /\/api\/execution-tasks\/manual-import$/);
    const payload = JSON.parse(calls[1][1].body);
    assert.equal(payload.result.records.notes[0].coverUrl, 'https://sns-img.example.com/cover.webp');
    assert.equal(payload.result.records.notes[0].videoUrl, 'https://sns-video.example.com/video.mp4');
    assert.equal(payload.result.records.notes[0].cover, undefined);
    assert.equal(payload.result.records.notes[0].video, undefined);
    assert.equal(result.meta.mediaObserved, 2);
    assert.equal(result.meta.mediaEnqueued, 2);
    assert.equal(result.meta.mediaRegistrationConfirmed, true);
  } finally {
    globalThis.fetch = originalFetch;
  }
});
