import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizePushSubscription,
  parseWorkbenchPushPayload,
  registerWorkbenchPushSubscription,
  shouldWakeForWorkbenchPush,
  urlBase64ToUint8Array,
} from '../src/workbench/runtime/workbenchPushSubscription.js';

test('urlBase64ToUint8Array decodes VAPID public keys', () => {
  const decoded = urlBase64ToUint8Array('AQIDBA');
  assert.deepEqual(Array.from(decoded), [1, 2, 3, 4]);
});

test('normalizePushSubscription keeps only endpoint and keys', () => {
  const normalized = normalizePushSubscription({
    endpoint: 'https://fcm.googleapis.com/fcm/send/abc',
    expirationTime: null,
    keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
  });
  assert.deepEqual(normalized, {
    endpoint: 'https://fcm.googleapis.com/fcm/send/abc',
    keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
  });
});

test('parseWorkbenchPushPayload and wake filter accept task/control push payloads', () => {
  const taskPayload = parseWorkbenchPushPayload({
    json() {
      return { type: 'collection_task_available', taskId: 'task-1' };
    },
  });
  const controlPayload = parseWorkbenchPushPayload({
    text() {
      return JSON.stringify({ type: 'collection_task_control', taskId: 'task-1' });
    },
  });

  assert.equal(shouldWakeForWorkbenchPush(taskPayload), true);
  assert.equal(shouldWakeForWorkbenchPush(controlPayload), true);
  assert.equal(shouldWakeForWorkbenchPush({ type: 'other' }), false);
});

test('registerWorkbenchPushSubscription subscribes and reports to workbench', async () => {
  const registrations = [];
  const subscription = {
    toJSON() {
      return {
        endpoint: 'https://fcm.googleapis.com/fcm/send/sub-1',
        keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
      };
    },
  };
  const result = await registerWorkbenchPushSubscription({
    registration: {
      pushManager: {
        async getSubscription() {
          return null;
        },
        async subscribe(options) {
          registrations.push(options);
          return subscription;
        },
      },
    },
    executionStationClient: {
      async fetchVapidPublicKey() {
        return { enabled: true, publicKey: 'AQIDBA' };
      },
      async registerPushSubscription(payload) {
        registrations.push(payload);
        return { ok: true };
      },
    },
  });

  assert.equal(result.registered, true);
  assert.equal(result.endpoint, 'https://fcm.googleapis.com/fcm/send/sub-1');
  assert.equal(registrations[0].userVisibleOnly, false);
  assert.deepEqual(Array.from(registrations[0].applicationServerKey), [1, 2, 3, 4]);
  assert.deepEqual(registrations[1], {
    subscription: {
      endpoint: 'https://fcm.googleapis.com/fcm/send/sub-1',
      keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
    },
  });
});

test('registerWorkbenchPushSubscription skips when server VAPID is disabled', async () => {
  const result = await registerWorkbenchPushSubscription({
    registration: { pushManager: {} },
    executionStationClient: {
      async fetchVapidPublicKey() {
        return { enabled: false, publicKey: null };
      },
    },
  });

  assert.deepEqual(result, { registered: false, reason: 'vapid_disabled' });
});

test('registerWorkbenchPushSubscription reports server registration skips', async () => {
  const result = await registerWorkbenchPushSubscription({
    registration: {
      pushManager: {
        async getSubscription() {
          return {
            toJSON() {
              return {
                endpoint: 'https://fcm.googleapis.com/fcm/send/sub-1',
                keys: { p256dh: 'p256dh-key', auth: 'auth-key' },
              };
            },
          };
        },
      },
    },
    executionStationClient: {
      async fetchVapidPublicKey() {
        return { enabled: true, publicKey: 'AQIDBA' };
      },
      async registerPushSubscription() {
        return { ok: false, skipped: true, reason: 'station_not_registered' };
      },
    },
  });

  assert.deepEqual(result, { registered: false, reason: 'station_not_registered' });
});
