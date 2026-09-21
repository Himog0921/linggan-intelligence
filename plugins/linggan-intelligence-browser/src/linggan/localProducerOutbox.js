import Dexie from 'dexie';

const database = new Dexie('LingganLocalProducerOutbox');
database.version(1).stores({
  submissions: '&submissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
});
database.version(2).stores({
  submissions: '&submissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  mediaUploads: '&uploadId, slotSubmissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
});
database.version(3).stores({
  submissions: '&submissionId, &idempotencyKey, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  mediaUploads: '&uploadId, slotSubmissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
});

const RETRYABLE = ['pending', 'retryable', 'in_flight'];

function currentTime() { return Date.now(); }
function retryDelay(attempts) { return Math.min(60000, 1000 * (2 ** Math.min(6, attempts))); }

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort()
      .filter((key) => value[key] !== undefined).map((key) => [key, canonicalJson(value[key])]));
  }
  return value;
}

function replayEnvelope(existing, incoming) {
  const immutable = (row) => JSON.stringify(canonicalJson({
    producerInstanceId: row.producerInstanceId, taskId: row.taskId, attemptId: row.attemptId,
    contractVersion: row.contractVersion, taskSpec: row.taskSpec, attempt: row.attempt,
    capturePackage: row.capturePackage, idempotencyKey: row.idempotencyKey || '',
  }));
  if (immutable(existing) !== immutable(incoming)) throw new Error('local_producer_identity_content_conflict');
  return existing;
}

export function createLocalProducerOutbox(table = database.submissions, now = currentTime) {
  async function restoreExpired({ at = now(), limit = 20 } = {}) {
    const rows = await table
      .where('[status+nextAttemptAt+createdAt]')
      .between(['in_flight', Dexie.minKey, Dexie.minKey], ['in_flight', at, Dexie.maxKey])
      .limit(limit)
      .toArray();
    await Promise.all(rows.map((row) => table.update(row.submissionId, {
      status: 'retryable', nextAttemptAt: at, updatedAt: at, error: 'delivery_timeout',
    })));
    return rows.length;
  }

  return {
    async get(submissionId) { return table.get(submissionId); },
    async getByIdempotencyKey(key) { return table.where('idempotencyKey').equals(key).first(); },
    async releaseForClaim(submissionId) {
      const row = await table.get(submissionId);
      if (row && ['pending', 'retryable'].includes(row.status)) {
        await table.update(submissionId, { deliveryNotBefore: null, nextAttemptAt: now(), updatedAt: now() });
      }
    },
    async enqueue(envelope) {
      if (!envelope?.submissionId || !envelope?.attemptId || !envelope?.taskId || !envelope?.producerInstanceId) {
        throw new Error('invalid_local_producer_submission');
      }
      const existing = await table.get(envelope.submissionId);
      if (existing) return replayEnvelope(existing, envelope);
      const idempotencyKey = String(envelope?.idempotencyKey || '').trim();
      if (idempotencyKey) {
        const replay = await table.where('idempotencyKey').equals(idempotencyKey).first();
        if (replay) return replayEnvelope(replay, envelope);
      }
      const createdAt = now();
      const nextAttemptAt = Math.max(createdAt, Number(envelope.deliveryNotBefore) || 0);
      const row = { ...envelope, status: 'pending', attempts: 0, nextAttemptAt, createdAt, updatedAt: createdAt };
      try {
        await table.add(row);
      } catch (error) {
        // Two Service Worker wakeups may both observe an empty idempotency-key lane before
        // either IndexedDB add commits. The unique index is the final arbiter; the loser must
        // reuse the committed envelope instead of turning an intentional replay into a failure.
        if (idempotencyKey && error?.name === 'ConstraintError') {
          const replay = await table.where('idempotencyKey').equals(idempotencyKey).first();
          if (replay) return replayEnvelope(replay, envelope);
        }
        throw error;
      }
      return row;
    },

    async due({ at = now(), limit = 5 } = {}) {
      await restoreExpired({ at, limit });
      const rows = [];
      for (const status of ['pending', 'retryable']) {
        const found = await table
          .where('[status+nextAttemptAt+createdAt]')
          .between([status, Dexie.minKey, Dexie.minKey], [status, at, Dexie.maxKey])
          .limit(limit)
          .toArray();
        rows.push(...found);
      }
      return rows.sort((left, right) => left.createdAt - right.createdAt).slice(0, limit);
    },

    async markInFlight(submissionId, { at = now(), timeoutMs = 30000 } = {}) {
      await table.update(submissionId, { status: 'in_flight', nextAttemptAt: at + timeoutMs, updatedAt: at, error: '' });
    },

    async acknowledge(submissionId, receipt, { at = now() } = {}) {
      await table.update(submissionId, { status: 'acknowledged', receipt, updatedAt: at, error: '' });
    },

    async retry(submissionId, error, { at = now() } = {}) {
      const row = await table.get(submissionId);
      if (!row) return;
      const attempts = Number(row.attempts || 0) + 1;
      await table.update(submissionId, {
        status: 'retryable', attempts, nextAttemptAt: at + retryDelay(attempts), updatedAt: at,
        error: String(error || 'delivery_not_acknowledged'),
      });
    },

    async terminal(submissionId, error, { at = now(), dispatchFailureCode = '' } = {}) {
      const row = await table.get(submissionId);
      if (!row) return;
      const scheduledDelivery = row?.taskSpec?.source === 'scheduled';
      const dispatchFailure = scheduledDelivery && String(dispatchFailureCode).trim()
        ? {
          dispatchFailureCode: String(dispatchFailureCode).trim(),
          // submissionId is already a durable UUID unique to this server task.
          // Reusing it makes a lost failure response safely idempotent.
          dispatchFailureId: row.submissionId,
          dispatchFailureReportedAt: null,
        }
        : {};
      await table.update(submissionId, {
        status: 'terminal', updatedAt: at, error: String(error || 'contract_rejected'), ...dispatchFailure,
      });
    },

    async terminalDispatchFailures({ limit = 5 } = {}) {
      const rows = await table.where('status').equals('terminal').toArray();
      return rows
        .map((row) => {
          // v0.8.52 persisted this exact server rejection before it knew how
          // to close the server task. This is a one-way terminal-state
          // migration, not a second delivery or a platform retry.
          const legacyRejectedPackage = !row?.dispatchFailureCode
            && String(row?.error || '').trim() === 'submission_invalid';
          return legacyRejectedPackage
            ? {
              ...row,
              dispatchFailureCode: 'capture_delivery_rejected',
              dispatchFailureId: row.submissionId,
            }
            : row;
        })
        .filter((row) => row?.taskSpec?.source === 'scheduled'
          && String(row?.taskId || '').trim()
          && String(row?.producerInstanceId || '').trim()
          && String(row?.dispatchFailureId || '').trim()
          && String(row?.dispatchFailureCode || '').trim()
          && !row?.dispatchFailureReportedAt)
        .sort((left, right) => Number(left.createdAt) - Number(right.createdAt))
        .slice(0, limit);
    },

    async markTerminalDispatchFailureReported(submissionId, { at = now() } = {}) {
      await table.update(submissionId, { dispatchFailureReportedAt: at, updatedAt: at });
    },

    async pendingCount() {
      const counts = await Promise.all(RETRYABLE.map((status) => table.where('status').equals(status).count()));
      return counts.reduce((total, value) => total + value, 0);
    },
  };
}

export const localProducerOutbox = createLocalProducerOutbox();

export function createLocalMediaOutbox(table = database.mediaUploads, now = currentTime) {
  function isValidEnvelope(envelope) {
    const dependency = String(envelope?.slotSubmissionId || envelope?.serverWorkRef || '').trim();
    const candidateUris = Array.isArray(envelope?.candidateUris) ? envelope.candidateUris : [];
    const serverGenerationValid = !envelope?.serverWorkRef
      || (Number.isInteger(Number(envelope?.claimGeneration))
        && Number(envelope.claimGeneration) >= 1
        && String(envelope?.installKey || '').trim());
    return Boolean(
      String(envelope?.uploadId || '').trim()
      && dependency
      && String(envelope?.mediaObservationRef || '').trim()
      && candidateUris.length > 0
      && candidateUris.every((candidate) => typeof candidate === 'string' && candidate.trim())
      && serverGenerationValid,
    );
  }

  function isValidStoredRow(row) {
    return isValidEnvelope(row)
      && ['pending', 'retryable', 'in_flight', 'acknowledged', 'terminal'].includes(row?.status)
      && Number.isFinite(Number(row?.nextAttemptAt))
      && Number.isFinite(Number(row?.createdAt));
  }

  async function rejectInvalidRows(rows = [], at = now()) {
    const valid = [];
    for (const row of rows) {
      if (isValidStoredRow(row)) valid.push(row);
      else if (row?.uploadId) {
        await table.update(row.uploadId, {
          status: 'terminal', updatedAt: at, error: 'invalid_local_media_upload',
        });
      }
    }
    return valid;
  }

  async function restoreExpired({ at = now(), limit = 20 } = {}) {
    const rows = await table
      .where('[status+nextAttemptAt+createdAt]')
      .between(['in_flight', Dexie.minKey, Dexie.minKey], ['in_flight', at, Dexie.maxKey])
      .limit(limit)
      .toArray();
    await Promise.all(rows.map((row) => table.update(row.uploadId, {
      status: 'retryable', nextAttemptAt: at, updatedAt: at, error: 'media_delivery_timeout',
    })));
    return rows.length;
  }

  return {
    async get(uploadId) { return table.get(uploadId); },
    async enqueue(envelope) {
      if (!isValidEnvelope(envelope)) throw new Error('invalid_local_media_upload');
      const existing = await table.get(envelope.uploadId);
      if (existing) return existing;
      const createdAt = now();
      const row = { ...envelope, status: 'pending', attempts: 0, nextAttemptAt: createdAt, createdAt, updatedAt: createdAt };
      await table.add(row);
      return row;
    },
    async due({ at = now(), limit = 2 } = {}) {
      await restoreExpired({ at, limit });
      const rows = [];
      for (const status of ['pending', 'retryable']) {
        rows.push(...await table.where('[status+nextAttemptAt+createdAt]').between([status, Dexie.minKey, Dexie.minKey], [status, at, Dexie.maxKey]).limit(limit).toArray());
      }
      const valid = await rejectInvalidRows(rows, at);
      return valid.sort((left, right) => left.createdAt - right.createdAt).slice(0, limit);
    },
    async dueById(uploadId, { at = now() } = {}) {
      await restoreExpired({ at, limit: 20 });
      const row = await table.get(uploadId);
      if (!row || !['pending', 'retryable'].includes(row.status) || Number(row.nextAttemptAt) > at) return null;
      const [valid] = await rejectInvalidRows([row], at);
      return valid || null;
    },
    async markInFlight(uploadId, { at = now(), timeoutMs = 120000 } = {}) { await table.update(uploadId, { status: 'in_flight', nextAttemptAt: at + timeoutMs, updatedAt: at }); },
    async acknowledge(uploadId, receipt, { at = now() } = {}) { await table.update(uploadId, { status: 'acknowledged', receipt, updatedAt: at, error: '' }); },
    async retry(uploadId, error, { at = now() } = {}) {
      const row = await table.get(uploadId); if (!row) return;
      const attempts = Number(row.attempts || 0) + 1;
      if (attempts >= 3) {
        await table.update(uploadId, { status: 'terminal', attempts, updatedAt: at, error: String(error || 'media_upload_attempt_limit') });
        return;
      }
      await table.update(uploadId, { status: 'retryable', attempts, nextAttemptAt: at + retryDelay(attempts), updatedAt: at, error: String(error || 'media_upload_not_acknowledged') });
    },
    async terminal(uploadId, error, { at = now() } = {}) { await table.update(uploadId, { status: 'terminal', updatedAt: at, error: String(error || 'media_upload_terminal') }); },
    async pendingCount() {
      const counts = await Promise.all(RETRYABLE.map((status) => table.where('status').equals(status).count()));
      return counts.reduce((total, value) => total + value, 0);
    },
  };
}

export const localMediaOutbox = createLocalMediaOutbox();
