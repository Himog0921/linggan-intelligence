import test from 'node:test';
import assert from 'node:assert/strict';

import { syncManualRecordsToWorkbench } from '../src/sync/flywheelSync.js';

test('manual workbench sync sends source media only through the import endpoint', async () => {
  const calls = [];
  const originalFetch = globalThis.fetch;
  const originalChrome = globalThis.chrome;
  globalThis.chrome = { runtime: { getManifest: () => ({ version: '2.0.92' }) } };
  globalThis.fetch = async (url, options = {}) => {
    calls.push([url, options]);
    if (String(url).endsWith('/api/plugin-data-workspace')) {
      return new Response(JSON.stringify({ dataToken: 'data_token_123' }), { status: 200 });
    }
    return new Response(JSON.stringify({
      status: 'committed',
      receiptId: 'receipt-media-evidence',
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
        platformContentId: 'note_1',
        type: 'video',
        collectedAt: Date.parse('2026-08-12T03:04:05.000Z'),
        cover: 'https://sns-img.example.com/cover.webp',
        video: 'https://sns-video.example.com/video.mp4',
      }],
    });

    assert.equal(calls.length, 2);
    assert.match(calls[1][0], /\/api\/execution-tasks\/manual-import$/);
    const body = JSON.parse(calls[1][1].body);
    const payload = JSON.parse(Buffer.from(body.capturePackage.packagePayload, 'base64').toString('utf8'));
    assert.equal(payload.records[0].payload.coverUrl, 'https://sns-img.example.com/cover.webp');
    assert.equal(payload.records[0].payload.rawData.video, 'https://sns-video.example.com/video.mp4');
    assert.deepEqual(payload.artifacts, []);
    assert.equal(result.meta.evidenceStatus, 'committed');
    assert.equal('mediaRegistrationConfirmed' in result.meta, false);
  } finally {
    globalThis.fetch = originalFetch;
    globalThis.chrome = originalChrome;
  }
});
