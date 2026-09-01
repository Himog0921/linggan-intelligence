import assert from 'node:assert/strict';
import test from 'node:test';

import {
  allowedMediaCandidateUri,
  flushMediaOutboxInExecutionContext,
  mediaUploadUnitsForRecord,
  normalizeMediaCandidateUri,
} from '../src/linggan/mediaTransferRuntime.js';
import {
  MEDIA_WORKER_PORT,
  registerMediaWorkerPort,
  requestMediaWorker,
} from '../src/linggan/mediaWorkerChannel.js';

function listenerSet() {
  const listeners = new Set();
  return {
    addListener(listener) { listeners.add(listener); },
    removeListener(listener) { listeners.delete(listener); },
    emit(...args) { for (const listener of [...listeners]) listener(...args); },
    size() { return listeners.size; },
  };
}

function connectedRuntime() {
  const onConnect = listenerSet();
  const runtime = {
    lastError: null,
    onConnect,
    connect({ name }) {
      const clientMessage = listenerSet();
      const workerMessage = listenerSet();
      const clientDisconnect = listenerSet();
      const workerDisconnect = listenerSet();
      let disconnected = false;
      const disconnect = () => {
        if (disconnected) return;
        disconnected = true;
        clientDisconnect.emit();
        workerDisconnect.emit();
      };
      const client = {
        name,
        onMessage: clientMessage,
        onDisconnect: clientDisconnect,
        postMessage(message) { queueMicrotask(() => workerMessage.emit(message)); },
        disconnect,
      };
      const worker = {
        name,
        onMessage: workerMessage,
        onDisconnect: workerDisconnect,
        postMessage(message) { queueMicrotask(() => clientMessage.emit(message)); },
        disconnect,
      };
      onConnect.emit(worker);
      return client;
    },
  };
  return runtime;
}

test('media transfer policy accepts only approved HTTPS platform hosts', () => {
  assert.equal(allowedMediaCandidateUri('https://sns-webpic-qc.xhscdn.com/one.webp'), true);
  assert.equal(allowedMediaCandidateUri('http://sns-webpic-qc.xhscdn.com/one.webp'), false);
  assert.equal(allowedMediaCandidateUri('https://xhscdn.com.attacker.example/one.webp'), false);
});

test('media transfer normalizes only approved platform HTTP candidates to HTTPS', () => {
  assert.equal(
    normalizeMediaCandidateUri('http://sns-webpic-qc.xhscdn.com/one.webp'),
    'https://sns-webpic-qc.xhscdn.com/one.webp',
  );
  assert.equal(
    normalizeMediaCandidateUri('https://sns-webpic-qc.xhscdn.com/one.webp'),
    'https://sns-webpic-qc.xhscdn.com/one.webp',
  );
  assert.equal(normalizeMediaCandidateUri('http://xhscdn.com.attacker.example/one.webp'), '');
  assert.equal(normalizeMediaCandidateUri('http://user@sns-webpic-qc.xhscdn.com/one.webp'), '');
  assert.equal(normalizeMediaCandidateUri('http://sns-webpic-qc.xhscdn.com:8080/one.webp'), '');
});

test('one Live Photo slot becomes independently queued still and motion byte units', () => {
  assert.deepEqual(mediaUploadUnitsForRecord({
    observation: {
      components: {
        still: { candidateUris: ['http://sns-webpic-qc.xhscdn.com/still.webp'] },
        motion: { candidateUris: [
          'http://sns-video-qc.xhscdn.com/motion.mp4',
          'http://sns-video-qc.xhscdn.com/motion.mp4',
        ] },
      },
    },
  }), [
    { componentKind: 'still', candidateUris: ['https://sns-webpic-qc.xhscdn.com/still.webp'] },
    { componentKind: 'motion', candidateUris: ['https://sns-video-qc.xhscdn.com/motion.mp4'] },
  ]);
});

test('ordinary media remains one normalized single byte unit', () => {
  assert.deepEqual(mediaUploadUnitsForRecord({
    observation: { candidateUris: ['http://sns-webpic-qc.xhscdn.com/image.webp'] },
  }), [{
    componentKind: 'single',
    candidateUris: ['https://sns-webpic-qc.xhscdn.com/image.webp'],
  }]);
});

test('offscreen media uses one named Port and leaves one-shot capture messages to the service worker', async () => {
  const runtime = connectedRuntime();
  const seen = [];
  const unregister = registerMediaWorkerPort({
    runtime,
    processMedia: async ({ preferredUploadId }) => {
      seen.push(preferredUploadId);
      return { success: true, processed: 7 };
    },
  });

  const result = await requestMediaWorker({
    runtime,
    preferredUploadId: 'media-7',
    requestId: 'request-7',
    timeoutMs: 1000,
  });

  assert.equal(MEDIA_WORKER_PORT, 'linggan-media-worker-v1');
  assert.deepEqual(seen, ['media-7']);
  assert.equal(result.processed, 7);
  assert.equal(result.requestId, 'request-7');
  unregister();
  assert.equal(runtime.onConnect.size(), 0);
});

test('offscreen media Port returns the exact worker failure without timing out capture delivery', async () => {
  const runtime = connectedRuntime();
  registerMediaWorkerPort({
    runtime,
    processMedia: async () => { throw new Error('media_fetch_failed'); },
  });

  await assert.rejects(
    requestMediaWorker({ runtime, requestId: 'request-failed', timeoutMs: 1000 }),
    /media_fetch_failed/,
  );
});

test('one offscreen wake drains a bounded detail media set larger than two items', async () => {
  const uploads = Array.from({ length: 7 }, (_, index) => ({
    uploadId: `detail-media-${index + 1}`,
    mediaObservationRef: `observation-${index + 1}`,
    candidateUris: [`https://sns-webpic-qc.xhscdn.com/${index + 1}.webp`],
    status: 'pending',
  }));
  const acknowledged = [];
  const outbox = {
    async dueById() { return null; },
    async due({ limit }) { assert.equal(limit, 12); return uploads; },
    async producerDependency() { return null; },
    async markInFlight() {},
    async acknowledge(id) { acknowledged.push(id); },
    async retry() { throw new Error('retry_not_expected'); },
    async terminal() { throw new Error('terminal_not_expected'); },
  };
  const offsets = new Map();
  const fetchImpl = async (url) => {
    const value = String(url);
    if (value.includes('xhscdn.com')) {
      return new Response(new Blob(['media']), {
        status: 200,
        headers: { 'content-type': 'image/webp', 'content-length': '5' },
      });
    }
    if (value.endsWith('/uploads')) {
      const sessionRef = `session-${offsets.size + 1}`;
      offsets.set(sessionRef, 0);
      return Response.json({ sessionRef, nextOffset: 0 });
    }
    if (value.endsWith('/chunks')) return Response.json({ nextOffset: 5 });
    if (value.endsWith('/finalize')) return Response.json({ materializationRef: 'materialized' });
    throw new Error(`unexpected_fetch:${url}`);
  };

  const result = await flushMediaOutboxInExecutionContext({ outbox, fetchImpl, cryptoImpl: crypto });

  assert.equal(result.processed, 7);
  assert.equal(acknowledged.length, 7);
});

test('offscreen execution upgrades an approved raw HTTP candidate before fetching bytes', async () => {
  const upload = {
    uploadId: 'http-upgrade',
    mediaObservationRef: 'observation-http',
    candidateUris: ['http://sns-webpic-qc.xhscdn.com/one.webp'],
    status: 'pending',
  };
  const requested = [];
  const outbox = {
    async dueById() { return upload; },
    async due() { return []; },
    async producerDependency() { return null; },
    async markInFlight() {},
    async acknowledge() {},
    async retry() { throw new Error('retry_not_expected'); },
    async terminal() { throw new Error('terminal_not_expected'); },
  };
  const fetchImpl = async (url) => {
    requested.push(String(url));
    if (requested.length === 1) {
      return new Response(new Blob(['media']), {
        status: 200,
        headers: { 'content-type': 'image/webp', 'content-length': '5' },
      });
    }
    if (String(url).endsWith('/uploads')) return Response.json({ sessionRef: 'session-http', nextOffset: 0 });
    if (String(url).endsWith('/chunks')) return Response.json({ nextOffset: 5 });
    if (String(url).endsWith('/finalize')) return Response.json({ materializationRef: 'material-http' });
    throw new Error(`unexpected_fetch:${url}`);
  };

  const result = await flushMediaOutboxInExecutionContext({
    outbox,
    preferredUploadId: upload.uploadId,
    fetchImpl,
    cryptoImpl: crypto,
  });

  assert.equal(result.results[0].status, 'acknowledged');
  assert.equal(requested[0], 'https://sns-webpic-qc.xhscdn.com/one.webp');
});

test('offscreen execution completes one claimed generation through resumable upload', async () => {
  const upload = {
    uploadId: 'work-1:1',
    serverWorkRef: 'work-1',
    claimGeneration: 1,
    installKey: 'installation-1',
    mediaObservationRef: 'observation-1',
    candidateUris: ['https://sns-webpic-qc.xhscdn.com/one.webp'],
    status: 'pending',
  };
  const events = [];
  const outbox = {
    async dueById(id) { return id === upload.uploadId ? upload : null; },
    async due() { return [upload]; },
    async producerDependency() { return null; },
    async markInFlight(id) { events.push(`in-flight:${id}`); },
    async acknowledge(id, receipt) { events.push(`ack:${id}:${receipt.materializationRef}`); },
    async retry() { throw new Error('retry_not_expected'); },
    async terminal() { throw new Error('terminal_not_expected'); },
  };
  const calls = [];
  const fetchImpl = async (url, options = {}) => {
    calls.push({ url: String(url), method: options.method || 'GET' });
    if (String(url).includes('xhscdn.com')) {
      return new Response(new Blob(['real-media']), {
        status: 200,
        headers: { 'content-type': 'image/webp', 'content-length': '10' },
      });
    }
    if (String(url).endsWith('/uploads')) {
      return Response.json({ sessionRef: 'session-1', nextOffset: 0 });
    }
    if (String(url).endsWith('/chunks')) {
      return Response.json({ nextOffset: 10 });
    }
    if (String(url).endsWith('/finalize')) {
      return Response.json({ materializationRef: 'materialization-1' });
    }
    throw new Error(`unexpected_fetch:${url}`);
  };

  const result = await flushMediaOutboxInExecutionContext({
    outbox,
    preferredUploadId: upload.uploadId,
    fetchImpl,
    cryptoImpl: crypto,
  });

  assert.equal(result.success, true);
  assert.deepEqual(result.results, [{ uploadId: upload.uploadId, status: 'acknowledged' }]);
  assert.deepEqual(events, [
    `in-flight:${upload.uploadId}`,
    `ack:${upload.uploadId}:materialization-1`,
  ]);
  assert.deepEqual(calls.map((call) => call.method), ['GET', 'POST', 'PATCH', 'POST']);
});

test('offscreen execution reports failure against the exact server generation', async () => {
  const upload = {
    uploadId: 'work-2:4',
    serverWorkRef: 'work-2',
    claimGeneration: 4,
    installKey: 'installation-2',
    mediaObservationRef: 'observation-2',
    candidateUris: ['https://sns-webpic-qc.xhscdn.com/missing.webp'],
    status: 'pending',
  };
  const events = [];
  let failureBody = null;
  const outbox = {
    async dueById() { return upload; },
    async due() { return []; },
    async producerDependency() { return null; },
    async markInFlight() {},
    async acknowledge() { throw new Error('ack_not_expected'); },
    async retry() { throw new Error('retry_not_expected'); },
    async terminal(id, reason) { events.push(`${id}:${reason}`); },
  };
  const fetchImpl = async (url, options = {}) => {
    if (String(url).includes('xhscdn.com')) return new Response('', { status: 404 });
    failureBody = JSON.parse(options.body);
    return Response.json({ work: { state: 'retry_wait' } });
  };

  const result = await flushMediaOutboxInExecutionContext({
    outbox,
    preferredUploadId: upload.uploadId,
    fetchImpl,
    cryptoImpl: crypto,
  });

  assert.deepEqual(failureBody, {
    attemptedUri: upload.candidateUris[0],
    terminalReason: 'expired_url',
    workRef: 'work-2',
    claimGeneration: 4,
    installKey: 'installation-2',
  });
  assert.deepEqual(result.results, [{ uploadId: upload.uploadId, status: 'terminal' }]);
  assert.deepEqual(events, ['work-2:4:retry_wait']);
});
