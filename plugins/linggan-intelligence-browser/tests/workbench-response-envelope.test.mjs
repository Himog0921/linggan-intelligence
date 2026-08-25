import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { normalizeWorkbenchMessageResponse } from '../src/workbench/protocol/responseEnvelope.js';
import { MSG } from '../src/shared/constants.js';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('workbench message responses gain a data envelope while preserving legacy fields', () => {
  const result = normalizeWorkbenchMessageResponse(MSG.WORKBENCH_CAPABILITY_CHECK, {
    success: true,
    accepted: true,
    report: { mode: 'detail' },
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.deepEqual(result.report, { mode: 'detail' });
  assert.deepEqual(result.data, {
    accepted: true,
    report: { mode: 'detail' },
  });
});

test('workbench message errors normalize into the same envelope contract', () => {
  const result = normalizeWorkbenchMessageResponse(MSG.WORKBENCH_RECORD_DELTA, {
    error: 'no_active_workbench_task',
    skipped: true,
  });

  assert.equal(result.success, false);
  assert.equal(result.error, 'no_active_workbench_task');
  assert.equal(result.skipped, true);
  assert.deepEqual(result.data, {
    skipped: true,
  });
});

test('non-workbench responses stay untouched for legacy raw callers', () => {
  const raw = [{ noteId: 'note_1' }];
  assert.equal(normalizeWorkbenchMessageResponse(MSG.GET_ALL_NOTES, raw), raw);
});

test('content and background runtime listeners normalize workbench responses before sendResponse', () => {
  const contentSource = fs.readFileSync(path.join(projectRoot, 'src/content/index.js'), 'utf8');
  const contentListenerSource = fs.readFileSync(path.join(projectRoot, 'src/content/messageListener.js'), 'utf8');
  const backgroundSource = fs.readFileSync(path.join(projectRoot, 'src/background/index.js'), 'utf8');

  assert.match(contentSource, /createRuntimeMessageListener/);
  assert.match(contentListenerSource, /normalizeWorkbenchMessageResponse/);
  assert.match(backgroundSource, /normalizeWorkbenchMessageResponse/);
});
