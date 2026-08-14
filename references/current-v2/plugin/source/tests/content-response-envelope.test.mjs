import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { MSG } from '../src/shared/constants.js';
import { normalizeContentMessageResponse } from '../src/content/protocol/responseEnvelope.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('content list actions wrap raw arrays into success/data envelope', () => {
  const payload = [{ noteId: 'n1' }];
  assert.deepEqual(
    normalizeContentMessageResponse(MSG.GET_ALL_NOTES, payload),
    { success: true, data: payload },
  );
});

test('content page-context and stats actions expose direct data payloads', () => {
  assert.deepEqual(
    normalizeContentMessageResponse(MSG.GET_PAGE_CONTEXT, { success: true, context: { platform: 'xhs' } }),
    { success: true, context: { platform: 'xhs' }, data: { platform: 'xhs' } },
  );
  assert.deepEqual(
    normalizeContentMessageResponse(MSG.GET_STATS, { notes: 2, comments: 3, authors: 1 }),
    { success: true, notes: 2, comments: 3, authors: 1, data: { notes: 2, comments: 3, authors: 1 } },
  );
});

test('content runtime listener normalizes content envelopes before sendResponse', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/content/index.js'), 'utf8');
  const listenerSource = fs.readFileSync(path.join(projectRoot, 'src/content/messageListener.js'), 'utf8');

  assert.match(source, /createRuntimeMessageListener/);
  assert.match(listenerSource, /normalizeContentMessageResponse/);
  assert.match(listenerSource, /sendResponse\(normalizeRuntimeMessageResponse/);
});
