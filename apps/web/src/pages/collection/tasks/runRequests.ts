import { ApiError, requestJson } from '../../../shared/api';
import { number, object, parseCreationReceipt, text, uuid } from './tasksApi';

export interface RunRequest { identity: string; fingerprint: string; sent: boolean }
export function definiteRejection(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && error.failure.status >= 400 && error.failure.status < 500;
}
/** Reserve before transport. A lost response only permits receipt reads, including after reload. */
export async function createRun(attempt: RunRequest, intent: Record<string, unknown>) {
  if (JSON.stringify(intent) !== attempt.fingerprint) throw new Error('请求内容已改变');
  const result = attempt.sent
    ? await requestJson(`/api/collection/runs/requests/${attempt.identity}`)
    : await (() => { attempt.sent = true; return requestJson('/api/collection/runs', { method: 'POST', body: { idempotencyKey: attempt.identity, ...intent } }); })();
  const receipt = parseCreationReceipt(result);
  if (receipt.domainRef !== intent.domainRef) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return receipt;
}
export type RunAction = 'pause' | 'continue' | 'stop';
export interface ControlIntent { run: string; domainRef: string; expectedRevision: number; action: RunAction }
export async function controlRun(attempt: RunRequest, intent: ControlIntent) {
  if (JSON.stringify(intent) !== attempt.fingerprint) throw new Error('请求内容已改变');
  const path = `/api/collection/runs/${uuid(intent.run)}/commands`;
  const result = attempt.sent
    ? await requestJson(`${path}/${attempt.identity}?domainRef=${encodeURIComponent(intent.domainRef)}`)
    : await (() => { attempt.sent = true; return requestJson(path, { method: 'POST', body: { commandRef: attempt.identity, domainRef: intent.domainRef, expectedRevision: intent.expectedRevision, action: intent.action } }); })();
  const r = object(result);
  const expected = intent.action === 'pause' ? 'paused' : intent.action === 'stop' ? 'stopped' : 'running';
  if (uuid(r.runRef) !== intent.run || uuid(r.commandRef) !== attempt.identity || number(r.revision) !== intent.expectedRevision + 1 || text(r.controlIntent) !== expected) throw new ApiError({ kind: 'invalid_response', status: 200 });
}
