/** Plain-Chinese labels and small pure helpers shared by the asset pages and the check panel. */
import type { BridgeChoice, BridgeGroup, BridgeUse, WorkDocument } from '../workbenchModel';
import type { BridgeState, Finding, Identity, Level, OrgRelation, Sentence, SentenceStatus } from './assetsModel';
import type { IdentityCommand } from './assetsApi';

export const relationLabel: Record<OrgRelation, string> = {
  parent: '陪跑营学员家长', coach: '陪跑老师', none: '和机构无关', unknown: '还没确认',
};
export const groupLabel: Record<BridgeGroup, string> = { parent: '陪跑营学员家长', coach: '陪跑老师' };
export const levelLabel: Record<Level, string> = { must: '必改', warn: '提醒', hint: '提示' };
export const levelHeading: Record<Level, string> = { must: '必须先改', warn: '提醒', hint: '发布时记得' };
export const sentenceStatusLabel: Record<SentenceStatus, string> = { review: '待审', ok: '可用', off: '停用', fix: '待改' };
export const bridgeStateText: Record<BridgeState, string> = {
  no_account: '先选一个账号，才知道能不能放衔接句。',
  unconfirmed: '这个号和机构的关系还没确认，不放衔接句。',
  no_relation: '这个号和机构无关，不提产品，没有衔接句。',
  skipped: '这篇不植入。',
  bio_missing: '账号简介还没写明和机构的关系，不放衔接句。',
  ready: '这个号可以放一句如实的衔接。',
};

/** True, false and not-known stay three different answers. */
export function triLabel(value: boolean | null, yes: string, no: string, unknown = '不清楚'): string {
  return value === null ? unknown : value ? yes : no;
}

export function identitySummary(identity: Identity | null): string {
  if (!identity) return '还没填写';
  if (identity.orgRelation === 'unknown') return '关系还没确认';
  if (identity.orgRelation === 'none') return '和机构无关';
  return `${relationLabel[identity.orgRelation]} · ${triLabel(identity.bioDisclosesRelation, '简介已写明', '简介没写明', '简介未核对')}`;
}

export function countByLevel(findings: readonly Finding[]): Record<Level, number> {
  return findings.reduce<Record<Level, number>>((sum, finding) => ({ ...sum, [finding.level]: sum[finding.level] + 1 }), { must: 0, warn: 0, hint: 0 });
}

/** Replaces `[start, end)` (UTF-16 units) with `replacement`, or deletes it when `replacement` is null. */
export function applyRange(text: string, start: number, end: number, replacement: string | null): string {
  if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end < start || end > text.length) return text;
  return text.slice(0, start) + (replacement ?? '') + text.slice(end);
}

/** The topic input is a space-separated string; drop its `index`-th token. */
export function withoutTag(draft: string, index: number): string {
  const tokens = draft.split(/\s+/u).filter(Boolean);
  return tokens.filter((_, position) => position !== index).join(' ');
}

export function bridgeFromSentence(sentence: Sentence): BridgeUse {
  return { sentenceCode: sentence.code, sentenceRevision: sentence.revision, group: sentence.group, text: sentence.text, edited: false };
}
export function withBridge(document: WorkDocument, patch: Partial<BridgeChoice>): WorkDocument {
  const current: BridgeChoice = document.bridge ?? { skip: false, sentence: null, afterParagraph: null };
  return { ...document, bridge: { ...current, ...patch } };
}

/** The next usable sentence after the current one, wrapping around; the first when none is chosen. */
export function nextSentence(items: readonly Sentence[], currentCode: string | null): Sentence | null {
  if (items.length === 0) return null;
  const at = items.findIndex(item => item.code === currentCode);
  return items[(at + 1) % items.length] ?? null;
}

/** An identity revision with some answers changed; the rest carry over, unknown stays unknown. */
export function identityCommand(identity: Identity | null, patch: Partial<Omit<IdentityCommand, 'requestId' | 'expectedRevision'>>, requestId: string): IdentityCommand {
  return {
    requestId,
    expectedRevision: identity?.revision ?? 0,
    orgRelation: identity?.orgRelation ?? 'unknown',
    bioDisclosesRelation: identity?.bioDisclosesRelation ?? null,
    hasRealPerson: identity?.hasRealPerson ?? null,
    backingPersonRole: identity?.backingPersonRole ?? null,
    canProvideExperience: identity?.canProvideExperience ?? null,
    ...patch,
  };
}
