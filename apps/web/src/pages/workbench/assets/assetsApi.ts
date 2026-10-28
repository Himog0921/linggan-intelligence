/** Same-origin client for the S1a asset routes. Writes carry a request identity owned by the caller. */
import { ApiError, requestJson } from '../../../shared/api';
import type { BridgeGroup, WorkDocument } from '../workbenchModel';
import { newDocument } from '../workbenchModel';
import {
  decodeAccounts, decodeCandidates, decodeCheckReport, decodeCopiedMark, decodeCopyReady, decodeCopyStatus,
  decodeFactCardReceipt, decodeFactCards, decodeIdentityReceipt, decodeSentenceCheck, decodeSentenceReceipt,
  decodeSentences, type OrgRelation, type ReviewAction, type SentenceOrigin, type SentenceStatus,
} from './assetsModel';

const base = '/ops/api/creation';
const send = (method: 'POST' | 'PUT', path: string, body: unknown) => requestJson(`${base}${path}`, { method, body });
const id = (value: string) => encodeURIComponent(value);

export interface IdentityCommand {
  requestId: string;
  expectedRevision: number;
  orgRelation: OrgRelation;
  bioDisclosesRelation: boolean | null;
  hasRealPerson: boolean | null;
  backingPersonRole: string | null;
  canProvideExperience: boolean | null;
}
export interface FactCardCommand {
  requestId: string;
  expectedRevision: number;
  name: string;
  allowedNames: string[];
  bannedNames: string[];
  what: string;
  who: string;
  canSay: string;
  cannotSay: string;
  mustNote: string;
}

export async function listAccounts(domainRef: string, signal?: AbortSignal) {
  return decodeAccounts(await requestJson(`${base}/accounts?domainRef=${id(domainRef)}`, { signal }));
}
export async function putIdentity(accountRef: string, command: IdentityCommand) {
  return decodeIdentityReceipt(await send('PUT', `/accounts/${id(accountRef)}/identity`, command));
}
export async function candidates(accountRef: string, signal?: AbortSignal) {
  return decodeCandidates(await requestJson(`${base}/accounts/${id(accountRef)}/candidates`, { signal }));
}

export async function listFactCards(signal?: AbortSignal) {
  return decodeFactCards(await requestJson(`${base}/fact-cards`, { signal }));
}
export async function saveFactCard(code: string | null, command: FactCardCommand) {
  return decodeFactCardReceipt(code === null
    ? await send('POST', '/fact-cards', command)
    : await send('PUT', `/fact-cards/${id(code)}`, command));
}
export async function confirmFactCard(code: string, command: { requestId: string; expectedRevision: number }) {
  return decodeFactCardReceipt(await send('POST', `/fact-cards/${id(code)}/confirm`, command));
}

export async function listSentences(filter: { group?: BridgeGroup; status?: SentenceStatus } = {}, signal?: AbortSignal) {
  const query = new URLSearchParams();
  if (filter.group) query.set('group', filter.group);
  if (filter.status) query.set('status', filter.status);
  const text = query.toString();
  return decodeSentences(await requestJson(`${base}/bridge-sentences${text ? `?${text}` : ''}`, { signal }));
}
export async function createSentence(command: { requestId: string; group: BridgeGroup; text: string; origin: SentenceOrigin }) {
  return decodeSentenceReceipt(await send('POST', '/bridge-sentences', command));
}
export async function editSentence(code: string, command: { requestId: string; expectedRevision: number; group: BridgeGroup; text: string }) {
  return decodeSentenceReceipt(await send('PUT', `/bridge-sentences/${id(code)}`, command));
}
export async function reviewSentence(code: string, command: { requestId: string; expectedRevision: number; action: ReviewAction; note: string | null }) {
  return decodeSentenceReceipt(await send('POST', `/bridge-sentences/${id(code)}/review`, command));
}
export async function checkSentence(text: string, signal?: AbortSignal) {
  return decodeSentenceCheck(await requestJson(`${base}/bridge-sentences/check`, { method: 'POST', body: { text }, signal }));
}

/**
 * The check looks at the account, the title, the cover headline, the body, the topics and the
 * bridge slot, so only those travel; excerpts, canvas and materials stay out of every keystroke.
 */
export function checkDocument(document: WorkDocument): WorkDocument {
  return {
    ...newDocument(), title: document.title, accountRef: document.accountRef, tags: document.tags,
    paragraphs: document.paragraphs, pages: document.pages.filter(page => page.kind === 'cover').slice(0, 1),
    bridge: document.bridge, coverAi: document.coverAi,
  };
}
export async function checkDraft(document: WorkDocument, signal?: AbortSignal) {
  return decodeCheckReport(await requestJson(`${base}/checks`, { method: 'POST', body: { document: checkDocument(document) }, signal }));
}
export async function prepareCopy(workId: string, command: { requestId: string; expectedRevision: string }) {
  return decodeCopyReady(await send('POST', `/works/${id(workId)}/copy`, command));
}
export async function confirmCopy(workId: string, receiptRef: string, command: { requestId: string }) {
  return decodeCopiedMark(await send('POST', `/works/${id(workId)}/copy/${id(receiptRef)}/confirm`, command));
}
export async function copyStatus(workId: string, signal?: AbortSignal) {
  return decodeCopyStatus(await requestJson(`${base}/works/${id(workId)}/copy-status`, { signal }));
}

const messages: Record<string, string> = {
  creation_assets_stale: '别处刚改过这里，请刷新后再试，当前输入仍保留。',
  creation_assets_request_conflict: '同一个请求被用在了另一件事上，请刷新后重试。',
  creation_assets_parameter_invalid: '填写的内容不符合要求，请检查后再提交。',
  creation_assets_not_found: '没有找到要改的对象，请刷新后再试。',
  creation_assets_blocked: '被规则挡住了，请先处理列出的问题。',
  creation_assets_unavailable: '服务暂时读不到，当前输入仍保留，请稍后重试。',
  creation_not_connected: '创作服务没有连上数据库。',
};
export function assetsErrorText(error: unknown): string {
  if (error instanceof ApiError) {
    if (error.failure.kind === 'network' || error.failure.kind === 'invalid_response') {
      return '结果尚未确认，当前输入仍保留。请核实后用原请求重试。';
    }
    const code = error.failure.code;
    return (code && messages[code]) || '操作未完成，当前输入仍保留。请稍后重试。';
  }
  return '操作未完成，当前输入仍保留。请稍后重试。';
}
