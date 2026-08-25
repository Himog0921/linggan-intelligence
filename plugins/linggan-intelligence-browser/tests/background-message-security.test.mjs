import test from 'node:test';
import assert from 'node:assert/strict';

import {
  authorizeBackgroundMessage,
  createSensitiveActionSet,
} from '../src/background/messageSecurity.js';
import { MSG } from '../src/shared/constants.js';

test('background message security treats destructive and credential actions as sensitive', () => {
  const sensitiveActions = createSensitiveActionSet(MSG);

  assert.equal(sensitiveActions.has(MSG.DELETE_NOTE), true);
  assert.equal(sensitiveActions.has(MSG.CLEAR_ALL_NOTES), true);
  assert.equal(sensitiveActions.has(MSG.UPDATE_ACCOUNT), true);
  assert.equal(sensitiveActions.has(MSG.SAVE_FLYWHEEL_CONFIG), true);
  assert.equal(sensitiveActions.has(MSG.GET_PLATFORM_COOKIES), true);
  assert.equal(sensitiveActions.has(MSG.SYNC_TO_WORKBENCH), true);
});

test('background message security rejects sensitive messages from non-extension senders', () => {
  const sensitiveActions = createSensitiveActionSet(MSG);

  assert.deepEqual(
    authorizeBackgroundMessage({
      action: MSG.GET_PLATFORM_COOKIES,
      sender: { id: 'web-page' },
      runtimeId: 'extension-id',
      sensitiveActions,
    }),
    { allowed: false, error: 'unauthorized_sender' },
  );

  assert.deepEqual(
    authorizeBackgroundMessage({
      action: MSG.GET_PLATFORM_COOKIES,
      sender: { id: 'extension-id' },
      runtimeId: 'extension-id',
      sensitiveActions,
    }),
    { allowed: true },
  );
});
