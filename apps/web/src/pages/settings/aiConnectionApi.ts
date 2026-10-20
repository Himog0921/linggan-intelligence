import { requestJson } from '../../shared/api';
import { bool, integer, object, optional, text } from './aiSettingsModel';
const ROOT = '/api/local/model-settings';
export interface SaveConnectionRequest { versionRef: string; connectionRef: string; expectedRevision: number; name: string; api: string; baseUrl: string; apiKey: string; localEndpoint: boolean }
const connectionReceipt = object({ connectionRef: text, versionRef: text, revision: integer, state: optional(text), replayed: optional(bool) });
export async function saveConnection(request: SaveConnectionRequest) {
  return connectionReceipt(await requestJson(`${ROOT}/connections`, { method: 'POST', body: request }));
}
export async function saveModel(request: { modelRef: string; connectionVersionRef: string; modelId: string }) {
  return object({ modelRef: text, state: text })(await requestJson(`${ROOT}/models`, { method: 'POST', body: request }));
}
export async function setConnectionEnabled(request: { connectionRef: string; expectedRevision: number; enabled: boolean }) {
  return object({ enabled: bool, revision: integer })(await requestJson(`${ROOT}/connections/state`, { method: 'POST', body: request }));
}
