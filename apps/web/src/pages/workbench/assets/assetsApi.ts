/** Same-origin client for the S1a asset routes. Writes carry a request identity owned by the caller. */
import { ApiError, requestJson } from '../../../shared/api';
import type { BridgeGroup, WorkDocument } from '../workbenchModel';
import { newDocument } from '../workbenchModel';
import {
  decodeAccounts, decodeCandidates, decodeCheckReport, decodeCopiedMark, decodeCopyReady, decodeCopyStatus,
  decodeFactCardReceipt, decodeFactCards, decodeIdentityReceipt, decodeSentenceCheck, decodeSentenceReceipt, decodeExperiences, decodeExperienceReceipt, decodeExperienceRequest, decodeExperienceCancellation,
  decodePersonaFacts, decodePersonaReceipt, decodePersonaHistory, decodePersonaRequest,
  decodePublicationCandidates, decodePublicationLink, decodePublicationLinks, decodePublicationUnlink,
  decodeSentences, type OrgRelation, type ReviewAction, type SentenceOrigin, type SentenceStatus,
} from './assetsModel';

const base = '/ops/api/creation';
const send = (method: 'POST' | 'PUT', path: string, body: unknown) => requestJson(`${base}${path}`, { method, body });
const id = (value: string) => encodeURIComponent(value);

export async function listPersonaFacts(accountRef: string, signal?: AbortSignal) {
  return decodePersonaFacts(await requestJson(`${base}/persona-facts?accountRef=${id(accountRef)}`, { signal }));
}
export async function personaHistory(fact: string, signal?: AbortSignal) {
  return decodePersonaHistory(await requestJson(`${base}/persona-facts/${id(fact)}/history`, { signal }));
}
export async function putPersonaFact(command: { requestId: string; accountRef: string; expectedPersonRef: string; key: string; expectedVersion: number; text: string; source: string; conflictDecision: 'keep_old' | 'use_new' | null }) {
  return decodePersonaReceipt(await send('POST', '/persona-facts', command));
}
export async function confirmPersonaFact(fact: string, command: { requestId: string; expectedVersion: number; subjectConfirmed: true; confirmationMethod: string }) {
  return decodePersonaReceipt(await send('POST', `/persona-facts/${id(fact)}/confirm`, command));
}
export async function withdrawPersonaFact(fact: string, command: { requestId: string }) {
  return decodePersonaReceipt(await send('POST', `/persona-facts/${id(fact)}/withdraw`, command));
}
export async function personaRequestStatus(requestId: string) {
  return decodePersonaRequest(await requestJson(`${base}/persona-requests/${id(requestId)}`));
}
export async function cancelPersonaRequest(requestId: string) {
  return decodeExperienceCancellation(await send('POST', `/persona-requests/${id(requestId)}`, {}));
}

export async function listExperiences(accountRef: string, signal?: AbortSignal) {
  return decodeExperiences(await requestJson(`${base}/experiences?accountRef=${id(accountRef)}`, { signal }));
}
export async function experienceRequestStatus(requestId: string) {
  return decodeExperienceRequest(await requestJson(`${base}/experience-requests/${id(requestId)}`));
}
export async function cancelExperienceRequest(requestId: string) {
  return decodeExperienceCancellation(await send('POST', `/experience-requests/${id(requestId)}`, {}));
}
export async function createExperience(command: { requestId: string; accountRef: string; expectedPersonRef: string; title: string; body: string; tags: string[]; origin: string }) {
  return decodeExperienceReceipt(await send('POST', '/experiences', command));
}
export async function editExperience(experience: string, command: { requestId: string; expectedRevision: number; title: string; body: string; tags: string[]; origin: string }) {
  return decodeExperienceReceipt(await send('PUT', `/experiences/${id(experience)}`, command));
}
export async function confirmExperience(experience: string, command: { requestId: string; expectedRevision: number; subjectConfirmed: true; draftUse: true; sendToModel: null; confirmationMethod: string }) {
  return decodeExperienceReceipt(await send('POST', `/experiences/${id(experience)}/confirm`, command));
}
export async function withdrawExperience(experience: string, command: { requestId: string }) {
  return decodeExperienceReceipt(await send('POST', `/experiences/${id(experience)}/withdraw`, command));
}

export interface IdentityCommand {
  requestId: string;
  expectedRevision: number;
  orgRelation: OrgRelation;
  bioDisclosesRelation: boolean | null;
  hasRealPerson: boolean | null;
  backingPersonRole: string | null;
  canProvideExperience: boolean | null;
  personChange?: 'correction' | 'replacement';
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
 * bridge slot. Only direct comment references attached to paragraphs accompany the text,
 * so the grade check can distinguish a quoted comment from the account person's own facts.
 */
export function checkDocument(document: WorkDocument): WorkDocument {
  const commentIds = new Set(document.excerpts.filter(item => item.citation?.sourceKind === 'comment').map(item => item.id));
  const uses = document.uses.filter(item => item.target?.kind === 'paragraph' && item.contentOrigin === 'excerpt' && item.sourceSpan && item.excerptId && commentIds.has(item.excerptId));
  const canvasIds = new Set(uses.map(item => item.canvasId));
  const canvas = document.canvas.filter(item => canvasIds.has(item.id)).map(item => ({ ...item, note: '', expression: '' }));
  const excerptIds = new Set([...uses.map(item => item.excerptId), ...canvas.map(item => item.excerptId)]);
  const excerpts = document.excerpts.filter(item => excerptIds.has(item.id));
  return {
    ...newDocument(), title: document.title, accountRef: document.accountRef, tags: document.tags,
    paragraphs: document.paragraphs, experienceSlots: document.experienceSlots, experienceGaps: document.experienceGaps,
    pages: document.pages.filter(page => page.kind === 'cover').slice(0, 1),
    bridge: document.bridge, coverAi: document.coverAi,
    excerpts, canvas, uses,
  };
}
export async function checkDraft(document: WorkDocument, signal?: AbortSignal, workRef?: string) {
  return decodeCheckReport(await requestJson(`${base}/checks`, { method: 'POST', body: { document: checkDocument(document), ...(workRef ? { workRef: id(workRef) } : {}) }, signal }));
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
export async function publicationCandidates(workId: string, signal?: AbortSignal) {
  return decodePublicationCandidates(await requestJson(`${base}/works/${id(workId)}/publication-candidates`, { signal }));
}
export async function linkPublication(workId: string, command: { requestId: string; expectedRevision: string; publicationRef: string }) {
  return decodePublicationLink(await send('POST', `/works/${id(workId)}/publication-link`, command));
}
export async function publicationLinks(workId: string, signal?: AbortSignal) {
  return decodePublicationLinks(await requestJson(`${base}/works/${id(workId)}/publication-links`, { signal }));
}
export async function unlinkPublication(workId: string, linkRef: string, requestId: string) {
  return decodePublicationUnlink(await send('POST', `/works/${id(workId)}/publication-links/${id(linkRef)}/unlink`, { requestId }));
}

const messages: Record<string, string> = {
  creation_persona_conflict_decision: '新旧事实不一致，请选择保留原来的或以新的为准。',
  creation_assets_stale: '别处刚改过这里，请刷新后再试，当前输入仍保留。',
  creation_person_change_required: '请说明是更正原来的真人信息，还是换了另一位真人。',
  creation_assets_source_unavailable: '稿件引用的来源当前不可使用，请刷新稿件并处理后再复制。',
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
