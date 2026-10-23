import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { parseOcrRecognition } from './ocrRecognitionTypes';

const root = (workRef: string) => '/api/local/work-resources/' + encodeURIComponent(workRef) + '/ocr-recognition';
export async function readOcrRecognition(workRef: string, signal?: AbortSignal) {
  const result = parseOcrRecognition(await requestJson(root(workRef), { signal }));
  if (result && result.workRef !== workRef) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return result;
}
/** Only internal references cross the browser boundary. The server freezes/rechecks the image list. */
export async function startOcrRecognition(workRef: string, commandRef: string, expectedScopeSha256: string) {
  const receipt = await requestJson(root(workRef), { method: 'POST', body: { commandRef, expectedScopeSha256 } });
  if (!isRecord(receipt) || typeof receipt.runRef !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 });
  const result = await readOcrRecognition(workRef);
  if (!result || result.runRef !== receipt.runRef) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return result;
}
export async function stopOcrRecognition(batchRef: string, expectedRevision: number, commandRef: string) {
  const receipt = await requestJson('/api/local/ai-settings/batches/cancel', { method: 'POST', body: { batchRef, expectedRevision, commandRef, reason: '用户在当前作品停止图片文字识别' } });
  if (!isRecord(receipt) || receipt.batchRef !== batchRef || receipt.cancelRequested !== true) throw new ApiError({ kind: 'invalid_response', status: 200 });
}
export async function adoptManualOcr(workRef: string, payload: { commandRef: string; adoptionRef: string; expectedRevision: number; decision: 'adopt' | 'reject' | 'pending'; field: 'adopted_body'; text: string | null; actor: string; reason: string }) {
  const receipt = await requestJson(root(workRef) + '/adoptions', { method: 'POST', body: payload });
  if (!isRecord(receipt) || typeof receipt.reviewRef !== 'string' || typeof receipt.revision !== 'number') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return receipt;
}
