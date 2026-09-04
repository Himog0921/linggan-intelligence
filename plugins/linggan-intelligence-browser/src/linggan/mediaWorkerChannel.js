export const MEDIA_WORKER_PORT = 'linggan-media-worker-v1';
export const PROCESS_MEDIA_OUTBOX = 'process-media-outbox';

function asErrorMessage(value, fallback) {
  return String(value?.message || value || fallback).trim() || fallback;
}

/**
 * Keep the offscreen executor off the one-shot runtime message bus used by content capture.
 *
 * An offscreen document is a second extension context. Giving it a generic onMessage listener
 * makes page -> service-worker delivery share a receiver set with media execution. A named Port
 * is an explicit point-to-point lifetime: it also keeps the MV3 worker alive until the bounded
 * media drain has replied.
 */
export function requestMediaWorker({
  runtime = chrome.runtime,
  preferredUploadId = '',
  installationCredential = '',
  timeoutMs = 120000,
  requestId = crypto.randomUUID(),
} = {}) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const port = runtime.connect({ name: MEDIA_WORKER_PORT });
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      port.onMessage.removeListener(onMessage);
      port.onDisconnect.removeListener(onDisconnect);
      try { port.disconnect(); } catch {}
      callback(value);
    };
    const onMessage = (message = {}) => {
      if (String(message.requestId || '') !== requestId) return;
      if (message.success === true) finish(resolve, message);
      else finish(reject, new Error(asErrorMessage(message.code, 'media_worker_failed')));
    };
    const onDisconnect = () => {
      finish(reject, new Error(asErrorMessage(runtime.lastError, 'media_worker_disconnected')));
    };
    const timer = setTimeout(() => {
      finish(reject, new Error('media_worker_timeout'));
    }, Math.max(1000, Number(timeoutMs) || 120000));
    port.onMessage.addListener(onMessage);
    port.onDisconnect.addListener(onDisconnect);
    port.postMessage({
      action: PROCESS_MEDIA_OUTBOX,
      requestId,
      preferredUploadId: String(preferredUploadId || ''),
      installationCredential: String(installationCredential || ''),
    });
  });
}

export function registerMediaWorkerPort({ runtime = chrome.runtime, processMedia } = {}) {
  if (typeof processMedia !== 'function') throw new Error('media_worker_processor_required');
  const onConnect = (port) => {
    if (port?.name !== MEDIA_WORKER_PORT) return;
    const onMessage = (message = {}) => {
      if (message.action !== PROCESS_MEDIA_OUTBOX) return;
      const requestId = String(message.requestId || '');
      Promise.resolve(processMedia({
        preferredUploadId: String(message.preferredUploadId || ''),
      })).then((response) => {
        port.postMessage({ ...response, success: response?.success === true, requestId });
      }).catch((error) => {
        port.postMessage({
          success: false,
          code: asErrorMessage(error, 'media_worker_failed'),
          requestId,
        });
      });
    };
    const cleanup = () => port.onMessage.removeListener(onMessage);
    port.onMessage.addListener(onMessage);
    port.onDisconnect.addListener(cleanup);
  };
  runtime.onConnect.addListener(onConnect);
  return () => runtime.onConnect.removeListener(onConnect);
}
