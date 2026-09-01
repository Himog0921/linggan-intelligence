function text(value) {
  return String(value || '').trim();
}

export function prioritizeMediaUploads(preferred = null, due = [], limit = 2) {
  const preferredUploadId = text(preferred?.uploadId);
  const preferredEligible = preferredUploadId && ['pending', 'retryable'].includes(preferred?.status);
  return [
    ...(preferredEligible ? [preferred] : []),
    ...(Array.isArray(due) ? due : []).filter((upload) => text(upload?.uploadId) !== preferredUploadId),
  ].slice(0, Math.max(0, Number(limit) || 0));
}

export function claimedMediaUpload({
  claim = {},
  installKey = '',
  allowCandidate = () => false,
  normalizeCandidate = text,
} = {}) {
  const candidateUris = (Array.isArray(claim?.candidateUris) ? claim.candidateUris : [])
    .map((value) => text(normalizeCandidate(value)))
    .filter((value) => value && allowCandidate(value))
    .slice(0, 6);
  const workRef = text(claim?.workRef);
  const mediaObservationRef = text(claim?.mediaObservationRef);
  const claimGeneration = Number(claim?.claimGeneration);
  if (!workRef || !mediaObservationRef || !Number.isInteger(claimGeneration)
      || claimGeneration < 1 || candidateUris.length === 0) {
    throw new Error('media_claim_invalid');
  }
  return {
    uploadId: `${workRef}:${claimGeneration}`,
    serverWorkRef: workRef,
    claimGeneration,
    installKey: text(installKey),
    mediaObservationRef,
    componentKind: text(claim?.componentKind) || 'single',
    candidateUris,
  };
}

/**
 * Crosses the dangerous gap between a server lease and the durable browser outbox.
 * Every post-claim exception is reported against the exact work generation; an alarm is also
 * scheduled before IndexedDB work so an interrupted MV3 worker is woken before the lease ends.
 */
export async function executeClaimedMediaAcquisition({
  claim,
  installKey,
  allowCandidate,
  normalizeCandidate,
  outbox,
  flush,
  recordFailure,
  scheduleRecovery = async () => {},
} = {}) {
  let upload = null;
  try {
    upload = claimedMediaUpload({ claim, installKey, allowCandidate, normalizeCandidate });
    await scheduleRecovery(60);
    await outbox.enqueue(upload);
    // Give the exact newly leased generation priority over unrelated historical rows.
    await flush(upload.uploadId);
    const row = typeof outbox.get === 'function' ? await outbox.get(upload.uploadId) : null;
    const acknowledged = row?.status === 'acknowledged';
    const terminal = row?.status === 'terminal';
    return {
      success: acknowledged || (!row && !terminal),
      state: acknowledged
        ? 'media_generation_acknowledged'
        : (terminal ? 'media_generation_failed' : 'media_generation_queued'),
      executed: true,
      workRef: upload.serverWorkRef,
      nextPollAfterSeconds: acknowledged ? 0 : 60,
    };
  } catch (error) {
    if (upload && typeof recordFailure === 'function') {
      await recordFailure(upload, error).catch(() => null);
      if (typeof outbox?.terminal === 'function') {
        await outbox.terminal(upload.uploadId, 'media_generation_setup_failed').catch(() => null);
      }
    }
    return {
      success: false,
      state: text(error?.message) || 'media_generation_setup_failed',
      executed: Boolean(upload),
      workRef: text(upload?.serverWorkRef || claim?.workRef),
      nextPollAfterSeconds: 60,
    };
  }
}
