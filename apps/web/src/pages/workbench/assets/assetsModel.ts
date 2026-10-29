/**
 * Closed HTTP projection of CREATION-ASSETS-001 (S1a). Every response is decoded before the page
 * uses it; unknown stays unknown (`null`), never a default "no". Text offsets in findings are
 * UTF-16 code units, i.e. JavaScript string indexes.
 */
import { ApiError, isRecord } from '../../../shared/api';
import { ShapeError, bool, decode, field, list, num, obj, optBool, optObj, optStr, str, type Json } from '../../../shared/decode';
import type { BridgeGroup } from '../workbenchModel';

export type OrgRelation = 'parent' | 'coach' | 'none' | 'unknown';
export type BridgeState = 'no_account' | 'unconfirmed' | 'no_relation' | 'skipped' | 'bio_missing' | 'ready';
export type Level = 'must' | 'warn' | 'hint';
export type SentenceStatus = 'review' | 'ok' | 'off' | 'fix';
export type SentenceOrigin = 'manual' | 'saved_back';
export type ReviewAction = 'approve' | 'request_fix' | 'disable' | 'enable';
export interface Experience {
  id: string; code: string; accountRef: string; currentPerson: boolean;
  state: 'awaiting_subject' | 'confirmed' | 'disabled' | 'withdrawn'; revision: number;
  usedCount?: number;
  title: string | null; body: string | null; tags: string[] | null; origin: string | null;
  sendToModel: boolean | null; confirmedAt: string | null; confirmedSubjectRole: string | null;
}

export interface Identity {
  accountRef: string;
  revision: number;
  orgRelation: OrgRelation;
  bioDisclosesRelation: boolean | null;
  bioCheckedAt: string | null;
  bioCheckedBy: string | null;
  hasRealPerson: boolean | null;
  backingPersonRole: string | null;
  canProvideExperience: boolean | null;
  hasKnownPerson?: boolean | null;
  personRef?: string | null;
  savedAt: string;
  savedBy: string;
}
export interface AccountView {
  accountRef: string;
  accountName: string;
  platform: string;
  status: string;
  operatorName: string | null;
  identity: Identity | null;
}
export interface FactCard {
  code: string;
  revision: number;
  name: string;
  allowedNames: string[];
  bannedNames: string[];
  what: string;
  who: string;
  canSay: string;
  cannotSay: string;
  mustNote: string;
  confirmedBy: string | null;
  confirmedAt: string | null;
  savedAt: string;
  savedBy: string;
}
export interface FactCardView {
  code: string;
  latest: FactCard;
  confirmed: FactCard | null;
}

export type Location =
  | { kind: 'paragraph'; id: string; start: number; end: number }
  | { kind: 'title' | 'cover' | 'bridge' | 'text'; start: number; end: number }
  | { kind: 'tag'; index: number }
  | { kind: 'slot' };
export type FixAction =
  | { action: 'chooseAccount' | 'openIdentity' | 'skipBridge' | 'confirmBioWritten' | 'swapBridge' | 'autoPickBridge' | 'editByHand' | 'deleteMatch' | 'removeTag' }
  | { action: 'copyBioSuggestion'; text: string }
  | { action: 'replaceText'; replacement: string };
export type Fix = FixAction & { label: string };
export interface Finding {
  rule: string;
  level: Level;
  message: string;
  location: Location;
  fixes: Fix[];
}
export interface NotRun { rule: string; reason: string }
export interface CardRevision { code: string; revision: number }
export interface CheckReport {
  rulesetVersion: string;
  bridgeState: BridgeState;
  bridgeGroup: BridgeGroup | null;
  findings: Finding[];
  notRun: NotRun[];
  factCards: CardRevision[];
}
export interface Sentence {
  code: string;
  revision: number;
  group: BridgeGroup;
  text: string;
  status: SentenceStatus;
  statusNote: string | null;
  origin: SentenceOrigin;
  entryFindings: Finding[];
  usedCount: number;
  savedAt: string;
  savedBy: string;
}
export interface Candidates { state: BridgeState; group: BridgeGroup | null; items: Sentence[] }
export interface SentenceCheckReport { rulesetVersion: string; blocked: boolean; findings: Finding[]; notRun: NotRun[] }
export interface CopyReady {
  receiptRef: string;
  workId: string;
  revisionId: string;
  rulesetVersion: string;
  title: string;
  body: string;
  tags: string[];
  text: string;
  warnCount: number;
  hintCount: number;
  replayed: boolean;
}
export interface CopiedMark { receiptRef: string; revisionId: string; copiedAt: string }
export interface CopyStatus { copied: CopiedMark | null }

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
function uuid(record: Json, name: string): string {
  const value = str(record, name);
  if (!UUID.test(value)) throw new ShapeError(name);
  return value;
}
function oneOf<T extends string>(record: Json, name: string, values: readonly T[]): T {
  const value = str(record, name);
  const found = values.find(item => item === value);
  if (found === undefined) throw new ShapeError(name);
  return found;
}
function count(record: Json, name: string): number {
  const value = num(record, name);
  if (!Number.isSafeInteger(value) || value < 0) throw new ShapeError(name);
  return value;
}
function strings(record: Json, name: string): string[] {
  const value = field(record, name);
  if (!Array.isArray(value) || value.some(item => typeof item !== 'string')) throw new ShapeError(name);
  return value as string[];
}

const ORG: readonly OrgRelation[] = ['parent', 'coach', 'none', 'unknown'];
const GROUPS: readonly BridgeGroup[] = ['parent', 'coach'];
const STATES: readonly BridgeState[] = ['no_account', 'unconfirmed', 'no_relation', 'skipped', 'bio_missing', 'ready'];
const LEVELS: readonly Level[] = ['must', 'warn', 'hint'];
function parseExperience(r: Json): Experience {
  const tags = field(r, 'tags');
  if (tags !== null && (!Array.isArray(tags) || tags.some(value => typeof value !== 'string'))) throw new ShapeError('tags');
  return { id: uuid(r, 'id'), code: str(r, 'code'), accountRef: uuid(r, 'accountRef'), currentPerson: bool(r, 'currentPerson'),
    state: oneOf(r, 'state', ['awaiting_subject', 'confirmed', 'disabled', 'withdrawn'] as const), revision: count(r, 'revision'),
    usedCount: r.usedCount === undefined ? undefined : count(r, 'usedCount'),
    title: optStr(r, 'title'), body: optStr(r, 'body'), tags: tags as string[] | null, origin: optStr(r, 'origin'),
    sendToModel: optBool(r, 'sendToModel'), confirmedAt: optStr(r, 'confirmedAt'), confirmedSubjectRole: optStr(r, 'confirmedSubjectRole') };
}
export const decodeExperiences = (value: unknown) => decode(value, r => list(r, 'items', parseExperience));
export const decodeExperienceReceipt = (value: unknown) => decode(value, r => ({ experience: parseExperience(obj(r, 'experience')), replayed: bool(r, 'replayed') }));
export const decodeExperienceRequest = (value: unknown) => decode(value, r => { const item = optObj(r, 'result');
  return item ? { kind: oneOf(item, 'kind', ['create', 'edit', 'confirm', 'withdraw'] as const), experienceRef: uuid(item, 'experienceRef'), revision: count(item, 'revision'), state: str(item, 'state') } : null;
});
export const decodeExperienceCancellation = (value: unknown) => decode(value, r => oneOf(r, 'outcome', ['committed', 'cancelled'] as const));

export function parseIdentity(r: Json): Identity {
  return {
    accountRef: uuid(r, 'accountRef'), revision: count(r, 'revision'), orgRelation: oneOf(r, 'orgRelation', ORG),
    bioDisclosesRelation: optBool(r, 'bioDisclosesRelation'), bioCheckedAt: optStr(r, 'bioCheckedAt'),
    bioCheckedBy: optStr(r, 'bioCheckedBy'), hasRealPerson: optBool(r, 'hasRealPerson'),
    backingPersonRole: optStr(r, 'backingPersonRole'), canProvideExperience: optBool(r, 'canProvideExperience'),
    hasKnownPerson: optBool(r, 'hasKnownPerson'),
    personRef: r.personRef == null ? null : uuid(r, 'personRef'),
    savedAt: str(r, 'savedAt'), savedBy: str(r, 'savedBy'),
  };
}
function parseAccount(r: Json): AccountView {
  const identity = optObj(r, 'identity');
  return {
    accountRef: uuid(r, 'accountRef'), accountName: str(r, 'accountName'), platform: str(r, 'platform'),
    status: str(r, 'status'), operatorName: optStr(r, 'operatorName'), identity: identity ? parseIdentity(identity) : null,
  };
}
export function parseFactCard(r: Json): FactCard {
  return {
    code: str(r, 'code'), revision: count(r, 'revision'), name: str(r, 'name'),
    allowedNames: strings(r, 'allowedNames'), bannedNames: strings(r, 'bannedNames'),
    what: str(r, 'what'), who: str(r, 'who'), canSay: str(r, 'canSay'), cannotSay: str(r, 'cannotSay'),
    mustNote: str(r, 'mustNote'), confirmedBy: optStr(r, 'confirmedBy'), confirmedAt: optStr(r, 'confirmedAt'),
    savedAt: str(r, 'savedAt'), savedBy: str(r, 'savedBy'),
  };
}
function parseFactCardView(r: Json): FactCardView {
  const confirmed = optObj(r, 'confirmed');
  return { code: str(r, 'code'), latest: parseFactCard(obj(r, 'latest')), confirmed: confirmed ? parseFactCard(confirmed) : null };
}

function parseLocation(r: Json): Location {
  const kind = str(r, 'kind');
  const range = () => ({ start: count(r, 'start'), end: count(r, 'end') });
  switch (kind) {
    case 'paragraph': return { kind, id: uuid(r, 'id'), ...range() };
    case 'title': case 'cover': case 'bridge': case 'text': return { kind, ...range() };
    case 'tag': return { kind, index: count(r, 'index') };
    case 'slot': return { kind };
    default: throw new ShapeError('kind');
  }
}
function parseFix(r: Json): Fix {
  const label = str(r, 'label');
  const action = str(r, 'action');
  switch (action) {
    case 'chooseAccount': case 'openIdentity': case 'skipBridge': case 'confirmBioWritten': case 'swapBridge':
    case 'autoPickBridge': case 'editByHand': case 'deleteMatch': case 'removeTag':
      return { label, action };
    case 'copyBioSuggestion': return { label, action, text: str(r, 'text') };
    case 'replaceText': return { label, action, replacement: str(r, 'replacement') };
    default: throw new ShapeError('action');
  }
}
export function parseFinding(r: Json): Finding {
  return {
    rule: str(r, 'rule'), level: oneOf(r, 'level', LEVELS), message: str(r, 'message'),
    location: parseLocation(obj(r, 'location')), fixes: list(r, 'fixes', parseFix),
  };
}
const parseNotRun = (r: Json): NotRun => ({ rule: str(r, 'rule'), reason: str(r, 'reason') });
function parseBridgeGroup(r: Json): BridgeGroup | null {
  const value = r.bridgeGroup ?? r.group;
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string' || !GROUPS.includes(value as BridgeGroup)) throw new ShapeError('group');
  return value as BridgeGroup;
}
function parseCheckReport(r: Json): CheckReport {
  return {
    rulesetVersion: str(r, 'rulesetVersion'), bridgeState: oneOf(r, 'bridgeState', STATES), bridgeGroup: parseBridgeGroup(r),
    findings: list(r, 'findings', parseFinding), notRun: list(r, 'notRun', parseNotRun),
    factCards: list(r, 'factCards', c => ({ code: str(c, 'code'), revision: count(c, 'revision') })),
  };
}
export function parseSentence(r: Json): Sentence {
  return {
    code: str(r, 'code'), revision: count(r, 'revision'), group: oneOf(r, 'group', GROUPS), text: str(r, 'text'),
    status: oneOf(r, 'status', ['review', 'ok', 'off', 'fix']), statusNote: optStr(r, 'statusNote'),
    origin: oneOf(r, 'origin', ['manual', 'saved_back']), entryFindings: list(r, 'entryFindings', parseFinding),
    usedCount: count(r, 'usedCount'), savedAt: str(r, 'savedAt'), savedBy: str(r, 'savedBy'),
  };
}
function parseCandidates(r: Json): Candidates {
  return { state: oneOf(r, 'state', STATES), group: parseBridgeGroup(r), items: list(r, 'items', parseSentence) };
}
function parseSentenceCheck(r: Json): SentenceCheckReport {
  return {
    rulesetVersion: str(r, 'rulesetVersion'), blocked: bool(r, 'blocked'),
    findings: list(r, 'findings', parseFinding), notRun: list(r, 'notRun', parseNotRun),
  };
}
function parseCopyReady(r: Json): CopyReady {
  return {
    receiptRef: uuid(r, 'receiptRef'), workId: uuid(r, 'workId'), revisionId: uuid(r, 'revisionId'),
    rulesetVersion: str(r, 'rulesetVersion'), title: str(r, 'title'), body: str(r, 'body'), tags: strings(r, 'tags'),
    text: str(r, 'text'), warnCount: count(r, 'warnCount'), hintCount: count(r, 'hintCount'), replayed: bool(r, 'replayed'),
  };
}
function parseMark(r: Json): CopiedMark {
  return { receiptRef: uuid(r, 'receiptRef'), revisionId: uuid(r, 'revisionId'), copiedAt: str(r, 'copiedAt') };
}

export const decodeAccounts = (value: unknown) => decode(value, r => list(r, 'items', parseAccount));
export const decodeIdentityReceipt = (value: unknown) => decode(value, r => ({ identity: parseIdentity(obj(r, 'identity')), replayed: bool(r, 'replayed') }));
export const decodeCandidates = (value: unknown) => decode(value, parseCandidates);
export const decodeFactCards = (value: unknown) => decode(value, r => list(r, 'items', parseFactCardView));
export const decodeFactCardReceipt = (value: unknown) => decode(value, r => ({ card: parseFactCard(obj(r, 'card')), replayed: bool(r, 'replayed') }));
export const decodeSentences = (value: unknown) => decode(value, r => list(r, 'items', parseSentence));
export const decodeSentenceReceipt = (value: unknown) => decode(value, r => ({ sentence: parseSentence(obj(r, 'sentence')), replayed: bool(r, 'replayed') }));
export const decodeSentenceCheck = (value: unknown) => decode(value, parseSentenceCheck);
export const decodeCheckReport = (value: unknown) => decode(value, parseCheckReport);
export const decodeCopyReady = (value: unknown) => decode(value, parseCopyReady);
export const decodeCopiedMark = (value: unknown) => decode(value, parseMark);
export const decodeCopyStatus = (value: unknown) => decode(value, r => {
  const copied = optObj(r, 'copied');
  return { copied: copied ? parseMark(copied) : null };
});

/** Findings that came with a refusal (`creation_assets_blocked`), or null for any other failure. */
export function blockedFindings(error: unknown): Finding[] | null {
  if (!(error instanceof ApiError) || error.failure.kind !== 'http' || error.failure.code !== 'creation_assets_blocked') return null;
  const body = error.responseBody;
  if (!isRecord(body)) return null;
  try {
    return list(body, 'findings', parseFinding);
  } catch {
    return null;
  }
}
