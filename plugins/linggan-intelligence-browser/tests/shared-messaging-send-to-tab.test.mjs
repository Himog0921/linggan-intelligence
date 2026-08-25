import test from 'node:test';
import assert from 'node:assert/strict';

import { sendToTab } from '../src/shared/messaging.js';

test('sendToTab resolves recoverable tab context errors when allowContextError is enabled', async () => {
  const originalChrome = globalThis.chrome;
  const runtime = {
    id: 'extension-id',
    lastError: null,
  };
  globalThis.chrome = {
    runtime,
    tabs: {
      sendMessage(tabId, payload, callback) {
        runtime.lastError = { message: 'Receiving end does not exist.' };
        callback(undefined);
        runtime.lastError = null;
      },
    },
  };

  try {
    const result = await sendToTab(123, { action: 'PING' }, { allowContextError: true });
    assert.deepEqual(result, {
      success: false,
      skipped: true,
      recoverable: true,
      error: 'Receiving end does not exist.',
    });
  } finally {
    globalThis.chrome = originalChrome;
  }
});

test('sendToTab reinjects content script and retries recoverable tab context errors', async () => {
  const originalChrome = globalThis.chrome;
  const runtime = {
    id: 'extension-id',
    lastError: null,
  };
  let sendCount = 0;
  const injected = [];
  const cssInjected = [];
  globalThis.chrome = {
    runtime,
    tabs: {
      sendMessage(tabId, payload, callback) {
        sendCount += 1;
        if (sendCount === 1) {
          runtime.lastError = { message: 'Receiving end does not exist.' };
          callback(undefined);
          runtime.lastError = null;
          return;
        }
        callback({ success: true, retried: true, action: payload.action, tabId });
      },
    },
    scripting: {
      async insertCSS(options) {
        cssInjected.push(options);
      },
      async executeScript(options) {
        injected.push(options);
      },
    },
  };

  try {
    const result = await sendToTab(123, { action: 'PING' }, { autoReconnect: true });
    assert.deepEqual(result, { success: true, retried: true, action: 'PING', tabId: 123 });
    assert.equal(sendCount, 2);
    assert.deepEqual(cssInjected, [{ target: { tabId: 123 }, files: ['content.css'] }]);
    assert.deepEqual(injected, [{ target: { tabId: 123 }, files: ['vendor.js', 'content.js'] }]);
  } finally {
    globalThis.chrome = originalChrome;
  }
});
