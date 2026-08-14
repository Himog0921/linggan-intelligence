import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import test from 'node:test';

const require = createRequire(import.meta.url);
const contractModulePath = require.resolve('../src/workbench/protocol/v2/xhs-contracts.cjs');

test('XHS V2 contracts initialize and encode without the Node Buffer global', () => {
  const originalBuffer = globalThis.Buffer;
  try {
    globalThis.Buffer = undefined;
    delete require.cache[contractModulePath];
    const contracts = require(contractModulePath);
    const bytes = new TextEncoder().encode('browser-service-worker');
    assert.equal(contracts.encodeBase64(bytes), 'YnJvd3Nlci1zZXJ2aWNlLXdvcmtlcg==');
    assert.equal(Object.keys(contracts.FIXTURES).length, 6);
  } finally {
    globalThis.Buffer = originalBuffer;
    delete require.cache[contractModulePath];
  }
});
