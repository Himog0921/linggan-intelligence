import test from 'node:test';
import assert from 'node:assert/strict';

import { syncManualRecordsToWorkbench, syncToFlywheel } from '../src/sync/flywheelSync.js';

const OBSERVED_AT = '2026-08-12T03:04:05.000Z';

function strictNote(overrides = {}) {
  return {
    platform: 'xhs',
    noteId: 'note-manual-1',
    platformContentId: 'note-manual-1',
    type: 'normal',
    title: 'manual note',
    collectedAt: Date.parse(OBSERVED_AT),
    ...overrides,
  };
}

function installRuntimeVersion() {
  const originalChrome = globalThis.chrome;
  globalThis.chrome = {
    runtime: { getManifest: () => ({ version: '2.0.92' }) },
  };
  return () => { globalThis.chrome = originalChrome; };
}

test('syncToFlywheel sends one strict manual_import CaptureSubmissionV2 for XHS', async () => {
  const restoreChrome = installRuntimeVersion();
  const originalFetch = globalThis.fetch;
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push([url, options]);
    return new Response(JSON.stringify({
      status: 'committed',
      captureId: 'capture-manual-note',
      receiptId: 'receipt-manual-note',
    }), { status: 200 });
  };

  try {
    const result = await syncToFlywheel([strictNote()], 'evaluate', 'operator-1');
    assert.equal(result.success, true);
    assert.equal(calls.length, 1);
    assert.match(calls[0][0], /\/api\/execution-tasks\/manual-import$/);

    const body = JSON.parse(calls[0][1].body);
    assert.deepEqual(Object.keys(body).sort(), ['capturePackage', 'header']);
    assert.equal(body.header.protocolVersion, 'capture-submission/v2');
    assert.equal(body.header.ingressKind, 'manual_import');
    assert.equal(body.header.contractId, 'xhs.list-scan');
    assert.equal(body.header.collectorVersion, '2.0.92');
    assert.equal(body.header.observedAt, OBSERVED_AT);
    assert.equal(body.header.report.counters.emitted, 1);
    assert.equal(body.header.report.slots[0].status, 'observed');
    assert.equal('jobId' in body.header, false);
    assert.equal('result' in body, false);

    const payload = JSON.parse(Buffer.from(body.capturePackage.packagePayload, 'base64').toString('utf8'));
    assert.deepEqual(payload.header, body.header);
    assert.equal(payload.records.length, 1);
    assert.equal(payload.records[0].recordKind, 'note');
    assert.equal(payload.records[0].payload.noteId, 'note-manual-1');
    assert.equal(payload.records[0].payload.platformContentId, 'note-manual-1');
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('manual XHS sync rejects legacy-shaped identity before network access', async () => {
  const restoreChrome = installRuntimeVersion();
  const originalFetch = globalThis.fetch;
  let fetchCalls = 0;
  globalThis.fetch = async () => { fetchCalls += 1; throw new Error('network must not be called'); };
  try {
    const result = await syncManualRecordsToWorkbench({}, {
      notes: [{ platform: 'xhs', noteId: 'legacy-only-id', collectedAt: Date.parse(OBSERVED_AT) }],
    });
    assert.equal(result.success, false);
    assert.match(result.error, /identity_missing|content_type_missing/);
    assert.equal(fetchCalls, 0);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('Douyin manual records fail closed and never enter the XHS V2 route', async () => {
  const restoreChrome = installRuntimeVersion();
  const originalFetch = globalThis.fetch;
  let fetchCalls = 0;
  globalThis.fetch = async () => { fetchCalls += 1; throw new Error('network must not be called'); };
  try {
    const result = await syncToFlywheel([{
      platform: 'douyin',
      noteId: 'dy-1',
      collectedAt: Date.parse(OBSERVED_AT),
    }]);
    assert.deepEqual(result, {
      success: false,
      imported: 0,
      skipped: 0,
      details: [],
      error: 'manual_import_platform_unsupported:douyin',
    });
    assert.equal(fetchCalls, 0);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});

test('manual author sync never duplicates a legacy alias into the two required identities', async () => {
  const restoreChrome = installRuntimeVersion();
  const originalFetch = globalThis.fetch;
  let fetchCalls = 0;
  globalThis.fetch = async () => { fetchCalls += 1; throw new Error('network must not be called'); };
  try {
    const result = await syncManualRecordsToWorkbench({}, {
      authors: [{
        platform: 'xhs',
        userId: 'legacy-author-alias',
        collectedAt: Date.parse(OBSERVED_AT),
      }],
    });
    assert.equal(result.success, false);
    assert.match(result.error, /identity_missing/);
    assert.equal(fetchCalls, 0);
  } finally {
    globalThis.fetch = originalFetch;
    restoreChrome();
  }
});
