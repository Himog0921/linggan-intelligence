import Dexie from 'dexie';

const database = new Dexie('LingganLocalProducerOutbox');
database.version(1).stores({
  submissions: '&submissionId, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
});

const RETRYABLE = ['pending', 'retryable', 'in_flight'];

function currentTime() { return Date.now(); }
function retryDelay(attempts) { return Math.min(60000, 1000 * (2 ** Math.min(6, attempts))); }

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
    async enqueue(envelope) {
      if (!envelope?.submissionId || !envelope?.attemptId || !envelope?.taskId || !envelope?.producerInstanceId) {
        throw new Error('invalid_local_producer_submission');
      }
      const existing = await table.get(envelope.submissionId);
      if (existing) return existing;
      const createdAt = now();
      const row = { ...envelope, status: 'pending', attempts: 0, nextAttemptAt: createdAt, createdAt, updatedAt: createdAt };
      await table.add(row);
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

    async terminal(submissionId, error, { at = now() } = {}) {
      await table.update(submissionId, { status: 'terminal', updatedAt: at, error: String(error || 'contract_rejected') });
    },

    async pendingCount() {
      const counts = await Promise.all(RETRYABLE.map((status) => table.where('status').equals(status).count()));
      return counts.reduce((total, value) => total + value, 0);
    },
  };
}

export const localProducerOutbox = createLocalProducerOutbox();
