import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

import { syncToFlywheel } from '../src/sync/flywheelSync.js';

const pluginRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const workbenchRoot = process.env.CONTENT_WORKBENCH_REPO
  || path.resolve(pluginRoot, '../content-workbench/v2-b3-projection-readiness');
const validatorProgram = [
  "import fs from 'node:fs';",
  "import * as validatorModule from './src/lib/evidence/ingress/validator.ts';",
  "const validateCaptureSubmissionBody = validatorModule.validateCaptureSubmissionBody || validatorModule.default?.validateCaptureSubmissionBody;",
  "const raw = JSON.parse(fs.readFileSync(0, 'utf8'));",
  "const result = validateCaptureSubmissionBody(raw, 'manual_import');",
  "process.stdout.write(JSON.stringify(result.ok ? { ok: true } : result));",
].join('\n');

function validateAtWorkbenchRoute(body) {
  const result = spawnSync(process.execPath, ['--import', 'tsx', '--input-type=module', '-e', validatorProgram], {
    cwd: workbenchRoot,
    input: JSON.stringify(body),
    encoding: 'utf8',
    timeout: 30_000,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

test('real XHS manual producer emits a body accepted by the current public route validator', async () => {
  const originalChrome = globalThis.chrome;
  const originalFetch = globalThis.fetch;
  let validatedBody = null;
  globalThis.chrome = { runtime: { getManifest: () => ({ version: '2.0.92' }) } };
  globalThis.fetch = async (url, options) => {
    assert.match(String(url), /\/api\/execution-tasks\/manual-import$/);
    validatedBody = JSON.parse(options.body);
    const validation = validateAtWorkbenchRoute(validatedBody);
    assert.deepEqual(validation, { ok: true });
    return new Response(JSON.stringify({ status: 'committed', receiptId: 'cross-repo-receipt' }), {
      status: 200,
    });
  };

  try {
    const result = await syncToFlywheel([{
      platform: 'xhs',
      noteId: 'cross-repo-note',
      platformContentId: 'cross-repo-note',
      type: 'normal',
      collectedAt: Date.parse('2026-08-12T03:04:05.000Z'),
      title: 'route-consumable fixture',
    }]);
    assert.equal(result.success, true);
    assert.equal(validatedBody.header.ingressKind, 'manual_import');
  } finally {
    globalThis.chrome = originalChrome;
    globalThis.fetch = originalFetch;
  }
});

test('current public route validator rejects the retired legacy manual body', () => {
  const validation = validateAtWorkbenchRoute({
    source: 'plugin_manual_sync',
    result: { records: { notes: [], comments: [], authors: [] } },
    metadata: { tag: 'evaluate' },
  });
  assert.deepEqual(validation, { ok: false, reason: 'invalid_submission' });
});
