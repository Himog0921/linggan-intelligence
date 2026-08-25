import assert from 'node:assert/strict';
import test from 'node:test';

import {
  LINGGAN_LOCAL_ORIGIN,
  LINGGAN_PENDING_MESSAGE,
  createLingganPendingResult,
  readLingganLocalReadiness,
} from '../src/linggan/adapter.js';

test('pending capability is explicit and has no success-shaped result', () => {
  const result = createLingganPendingResult('xhs_detail');
  assert.equal(result.success, false);
  assert.equal(result.code, 'linggan_adapter_pending');
  assert.equal(result.capability, 'xhs_detail');
  assert.match(result.message, /没有访问平台/);
});

test('local readiness probes only Linggan loopback without credentials', async () => {
  let received = null;
  const result = await readLingganLocalReadiness(async (url, options) => {
    received = { url, options };
    return { ok: true };
  });
  assert.equal(result.connected, true);
  assert.equal(received.url, `${LINGGAN_LOCAL_ORIGIN}/health`);
  assert.equal(received.options.credentials, 'omit');
  assert.match(LINGGAN_PENDING_MESSAGE, /尚未接通/);
});
