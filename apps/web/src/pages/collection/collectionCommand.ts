import { ApiError, requestJson } from '../../shared/api';

export interface CollectionCommandReceipt {
  requestRef: string;
  action: string;
  outcome: 'applied' | 'rejected';
  code: string | null;
  location: string | null;
}
interface Pending {
  intent: string;
  initial: Promise<CollectionCommandReceipt> | null;
}
// Transport recovery only. No original material or credentials are persisted in browser storage.
const requests = new Map<string, Pending>();

function record(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value as Record<string, unknown>;
}
function receipt(value: unknown, action: string, requestRef: string): CollectionCommandReceipt {
  const v = record(value);
  if (v.requestRef !== requestRef || v.action !== action) throw new ApiError({ kind: 'invalid_response', status: 200 });
  if (v.outcome === 'pending' || v.outcome === 'unconfirmed') throw new ApiError({ kind: 'network' }, v);
  if (!['applied', 'rejected'].includes(String(v.outcome)) || (v.code !== null && typeof v.code !== 'string') || (v.location !== null && typeof v.location !== 'string')) throw new ApiError({ kind: 'invalid_response', status: 200 });
  if (typeof v.location === 'string' && (!v.location.startsWith('/collection/') || v.location.startsWith('//') || /[\\\r\n]/.test(v.location))) throw new ApiError({ kind: 'invalid_response', status: 200 });
  if (v.outcome === 'applied' && typeof window !== 'undefined') window.dispatchEvent(new Event('collection:changed'));
  return { requestRef, action, outcome: v.outcome as CollectionCommandReceipt['outcome'], code: v.code as string | null, location: v.location as string | null };
}

export async function readCollectionCommandReceipt(action: string, requestRef: string): Promise<CollectionCommandReceipt> {
  try {
    const result = await requestJson(`/api/collection/commands/receipts/${encodeURIComponent(requestRef)}?action=${encodeURIComponent(action)}`);
    return receipt(result, action, requestRef);
  } catch (error) {
    if (error instanceof ApiError && error.answered) return receipt(error.responseBody, action, requestRef);
    throw error;
  }
}

/** A repeated call with the same identity is a read, never another POST. */
export async function executeCollectionCommand(action: string, fields: Record<string, unknown>, requestRef: string): Promise<CollectionCommandReceipt> {
  const intent = JSON.stringify({ action, fields });
  const previous = requests.get(requestRef);
  if (previous) {
    if (previous.intent !== intent) throw new ApiError({ kind: 'http', status: 409, code: 'request_identity_conflict' });
    return previous.initial ?? readCollectionCommandReceipt(action, requestRef);
  }
  const pending: Pending = { intent, initial: null };
  // Reserve before the first await, including StrictMode/concurrent user events.
  requests.set(requestRef, pending);
  pending.initial = (async () => {
    let context: Record<string, unknown>;
    try { context = record(await requestJson('/api/collection/commands/context')); if (typeof context.epoch !== 'string') throw new Error('invalid context'); }
    catch { requests.delete(requestRef); throw new ApiError({ kind: 'http', status: 503, code: 'command_not_sent' }); }
    try {
      const result = await requestJson(`/api/collection/commands/${encodeURIComponent(action)}`, {
        method: 'POST', body: { requestRef, epoch: context.epoch, fields },
      });
      return receipt(result, action, requestRef);
    } catch (error) {
      if (error instanceof ApiError && error.answered) {
        const answer = receipt(error.responseBody, action, requestRef);
        if (answer.outcome === 'rejected') throw error;
        return answer;
      }
      throw error;
    }
  })();
  try { return await pending.initial; }
  finally { pending.initial = null; }
}
