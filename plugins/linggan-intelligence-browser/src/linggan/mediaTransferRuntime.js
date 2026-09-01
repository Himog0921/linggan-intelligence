import { LINGGAN_LOCAL_ORIGIN } from './adapter.js';
import { prioritizeMediaUploads } from './mediaAcquisitionExecution.js';

const MAX_MEDIA_BYTES = 256 * 1024 * 1024;
const MEDIA_CHUNK_BYTES = 1024 * 1024;
const PLATFORM_MEDIA_HOST_SUFFIXES = Object.freeze([
  '.xhscdn.com', '.xiaohongshu.com', '.byteimg.com', '.bytevcloudcdn.com',
  '.douyinpic.com', '.douyinstatic.com', '.douyinvod.com', '.amemv.com',
]);

export function allowedMediaCandidateUri(value) {
  try {
    const uri = new URL(String(value || '').trim());
    if (uri.protocol !== 'https:' || uri.username || uri.password || uri.port) return false;
    return PLATFORM_MEDIA_HOST_SUFFIXES.some((suffix) => uri.hostname.toLowerCase().endsWith(suffix));
  } catch {
    return false;
  }
}

/**
 * XHS occasionally publishes otherwise valid CDN candidates with an http scheme.  The producer
 * never sends credentials or bytes over that scheme: only an exact platform-host candidate may
 * be upgraded to https, after which the ordinary strict allow-list still governs the request and
 * every redirect target.
 */
export function normalizeMediaCandidateUri(value) {
  try {
    const uri = new URL(String(value || '').trim());
    if (!['http:', 'https:'].includes(uri.protocol) || uri.username || uri.password || uri.port) return '';
    if (!PLATFORM_MEDIA_HOST_SUFFIXES.some((suffix) => uri.hostname.toLowerCase().endsWith(suffix))) return '';
    uri.protocol = 'https:';
    return allowedMediaCandidateUri(uri.href) ? uri.href : '';
  } catch {
    return '';
  }
}

function normalizedCandidates(values) {
  return [...new Set((Array.isArray(values) ? values : [values])
    .map(normalizeMediaCandidateUri)
    .filter(Boolean))]
    .slice(0, 6);
}

/**
 * A Live Photo is one logical slot but two byte resources. Direct detail collection must enqueue
 * the same independently addressable still/motion units that the server scheduler would claim;
 * otherwise one task's media completion depends on unrelated global backlog.
 */
export function mediaUploadUnitsForRecord(record = {}) {
  const observation = record?.observation && typeof record.observation === 'object'
    ? record.observation
    : {};
  const components = observation?.components && typeof observation.components === 'object'
    ? observation.components
    : {};
  const componentUnits = ['still', 'motion'].map((componentKind) => ({
    componentKind,
    candidateUris: normalizedCandidates(components?.[componentKind]?.candidateUris),
  })).filter((unit) => unit.candidateUris.length > 0);
  if (componentUnits.length > 0) return componentUnits;
  const candidateUris = normalizedCandidates(
    Array.isArray(observation.candidateUris)
      ? observation.candidateUris
      : observation.externalUri,
  );
  return candidateUris.length > 0 ? [{ componentKind: 'single', candidateUris }] : [];
}

async function fetchMediaCandidate(candidateUris, fetchImpl) {
  let lastError = new Error('media_candidate_unavailable');
  for (const sourceCandidate of candidateUris) {
    const candidate = normalizeMediaCandidateUri(sourceCandidate);
    if (!allowedMediaCandidateUri(candidate)) continue;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20000);
    try {
      const response = await fetchImpl(candidate, {
        method: 'GET', credentials: 'omit', redirect: 'follow', signal: controller.signal,
      });
      if (!response.ok) throw new Error(`media_http_${response.status}`);
      if (!allowedMediaCandidateUri(response.url || candidate)) throw new Error('media_redirect_origin_not_allowed');
      const mimeType = String(response.headers.get('content-type') || '').split(';')[0].trim();
      if (!/^(image|video|audio)\//.test(mimeType)) throw new Error('media_mime_not_allowed');
      const declaredSize = Number(response.headers.get('content-length'));
      if (Number.isFinite(declaredSize) && declaredSize > MAX_MEDIA_BYTES) throw new Error('media_size_limit_exceeded');
      const bytes = await response.blob();
      if (bytes.size > MAX_MEDIA_BYTES) throw new Error('media_size_limit_exceeded');
      return { bytes, mimeType };
    } catch (error) {
      lastError = controller.signal.aborted ? new Error('media_candidate_timeout') : error;
    } finally {
      clearTimeout(timer);
    }
  }
  throw lastError;
}

async function sha256Hex(arrayBuffer, cryptoImpl) {
  const digest = new Uint8Array(await cryptoImpl.subtle.digest('SHA-256', arrayBuffer));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

async function uploadMediaInChunks({ mediaObservationRef, blob, mimeType, sha256 }, fetchImpl) {
  const start = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-observations/${encodeURIComponent(mediaObservationRef)}/uploads`, {
    method: 'POST', credentials: 'omit',
    headers: {
      'x-linggan-media-sha256': sha256,
      'x-linggan-media-mime': mimeType,
      'x-linggan-media-size': String(blob.size),
    },
  });
  const startPayload = await start.json().catch(() => ({}));
  if (!start.ok) throw new Error(startPayload.code || 'media_upload_not_started');
  const sessionRef = String(startPayload.sessionRef || '').trim();
  let offset = Number(startPayload.nextOffset);
  if (!sessionRef || !Number.isInteger(offset) || offset < 0 || offset > blob.size) throw new Error('media_upload_resume_invalid');
  while (offset < blob.size) {
    const chunk = blob.slice(offset, Math.min(blob.size, offset + MEDIA_CHUNK_BYTES));
    const written = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-uploads/${encodeURIComponent(sessionRef)}/chunks`, {
      method: 'PATCH', credentials: 'omit',
      headers: { 'content-type': 'application/octet-stream', 'x-linggan-media-offset': String(offset) },
      body: chunk,
    });
    const writtenPayload = await written.json().catch(() => ({}));
    if (!written.ok) throw new Error(writtenPayload.code || 'media_upload_chunk_failed');
    const nextOffset = Number(writtenPayload.nextOffset);
    if (!Number.isInteger(nextOffset) || nextOffset <= offset || nextOffset > blob.size) throw new Error('media_upload_offset_invalid');
    offset = nextOffset;
  }
  const finalized = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-uploads/${encodeURIComponent(sessionRef)}/finalize`, {
    method: 'POST', credentials: 'omit',
  });
  const payload = await finalized.json().catch(() => ({}));
  if (!finalized.ok) throw new Error(payload.code || 'media_upload_finalize_failed');
  return payload;
}

export async function recordMediaDownloadFailure(upload, error, fetchImpl = fetch) {
  const attemptedUri = String(upload?.candidateUris?.[0] || '').trim();
  const observationRef = String(upload?.mediaObservationRef || '').trim();
  if (!attemptedUri || !observationRef) return null;
  const message = String(error?.message || error || 'unknown');
  const terminalReason = /mime/i.test(message) ? 'mime_mismatch'
    : /size/i.test(message) ? 'size_limit'
      : /http_404|expired/i.test(message) ? 'expired_url'
        : /cancel/i.test(message) ? 'cancelled' : 'network_error';
  const response = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/api/local/producer/media-observations/${encodeURIComponent(observationRef)}/download-failures`, {
    method: 'POST', credentials: 'omit', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      attemptedUri,
      terminalReason,
      ...(upload?.serverWorkRef ? {
        workRef: upload.serverWorkRef,
        claimGeneration: upload.claimGeneration,
        installKey: upload.installKey,
      } : {}),
    }),
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) throw new Error(payload?.code || 'media_download_failure_not_recorded');
  return payload;
}

export async function flushMediaOutboxInExecutionContext({
  outbox,
  preferredUploadId = '',
  fetchImpl = fetch,
  cryptoImpl = crypto,
  limit = 12,
} = {}) {
  const preferred = preferredUploadId ? await outbox.dueById(preferredUploadId) : null;
  const due = await outbox.due({ limit });
  const uploads = prioritizeMediaUploads(preferred, due, limit);
  const results = [];
  for (const upload of uploads) {
    if (upload.slotSubmissionId) {
      const dependency = await outbox.producerDependency(upload.slotSubmissionId);
      if (!dependency || dependency.status === 'terminal') {
        await outbox.terminal(upload.uploadId, 'media_slot_delivery_not_accepted');
        results.push({ uploadId: upload.uploadId, status: 'terminal' });
        continue;
      }
      if (dependency.status !== 'acknowledged') continue;
    }
    await outbox.markInFlight(upload.uploadId);
    try {
      const response = await fetchMediaCandidate(upload.candidateUris, fetchImpl);
      const sha256 = await sha256Hex(await response.bytes.arrayBuffer(), cryptoImpl);
      const uploaded = await uploadMediaInChunks({
        mediaObservationRef: upload.mediaObservationRef,
        blob: response.bytes,
        mimeType: response.mimeType,
        sha256,
      }, fetchImpl);
      await outbox.acknowledge(upload.uploadId, uploaded);
      results.push({ uploadId: upload.uploadId, status: 'acknowledged' });
    } catch (error) {
      const failure = await recordMediaDownloadFailure(upload, error, fetchImpl).catch(() => null);
      if (upload.serverWorkRef) {
        await outbox.terminal(upload.uploadId, failure?.work?.state || 'media_acquisition_generation_finished');
        results.push({ uploadId: upload.uploadId, status: 'terminal' });
      } else {
        await outbox.retry(upload.uploadId, error?.message || error);
        results.push({ uploadId: upload.uploadId, status: 'retryable' });
      }
    }
  }
  return { success: true, processed: results.length, results };
}
