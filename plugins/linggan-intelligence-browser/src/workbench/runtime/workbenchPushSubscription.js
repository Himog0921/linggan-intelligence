function normalizeString(value = '') {
  return String(value || '').trim();
}

export function urlBase64ToUint8Array(base64String = '') {
  const normalized = normalizeString(base64String);
  const padding = '='.repeat((4 - (normalized.length % 4)) % 4);
  const base64 = `${normalized}${padding}`.replace(/-/g, '+').replace(/_/g, '/');
  const rawData = globalThis.atob(base64);
  return new Uint8Array([...rawData].map((char) => char.charCodeAt(0)));
}

export function normalizePushSubscription(subscription = null) {
  const payload = typeof subscription?.toJSON === 'function'
    ? subscription.toJSON()
    : subscription;
  if (!payload || typeof payload !== 'object') return null;
  const endpoint = normalizeString(payload.endpoint);
  const p256dh = normalizeString(payload.keys?.p256dh);
  const auth = normalizeString(payload.keys?.auth);
  if (!endpoint || !p256dh || !auth) return null;
  return { endpoint, keys: { p256dh, auth } };
}

export function parseWorkbenchPushPayload(eventData = null) {
  try {
    if (typeof eventData?.json === 'function') return eventData.json();
  } catch {
    return null;
  }
  try {
    const text = typeof eventData?.text === 'function' ? eventData.text() : '';
    return text ? JSON.parse(text) : null;
  } catch {
    return null;
  }
}

export function shouldWakeForWorkbenchPush(payload = null) {
  const type = normalizeString(payload?.type);
  return type === 'collection_task_available' || type === 'collection_task_control';
}

export async function registerWorkbenchPushSubscription({
  registration = globalThis.registration,
  executionStationClient,
} = {}) {
  if (!registration?.pushManager || !executionStationClient) {
    return { registered: false, reason: 'push_unavailable' };
  }

  const keyPayload = await executionStationClient.fetchVapidPublicKey();
  if (!keyPayload?.enabled || !keyPayload?.publicKey) {
    return { registered: false, reason: 'vapid_disabled' };
  }

  const existing = await registration.pushManager.getSubscription?.();
  const subscription = existing || await registration.pushManager.subscribe({
    userVisibleOnly: false,
    applicationServerKey: urlBase64ToUint8Array(keyPayload.publicKey),
  });
  const normalized = normalizePushSubscription(subscription);
  if (!normalized) {
    return { registered: false, reason: 'invalid_subscription' };
  }

  const result = await executionStationClient.registerPushSubscription({ subscription: normalized });
  if (result?.ok !== true) {
    return {
      registered: false,
      reason: normalizeString(result?.reason) || 'register_push_subscription_failed',
    };
  }
  return { registered: true, endpoint: normalized.endpoint };
}
