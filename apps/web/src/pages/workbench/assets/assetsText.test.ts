import { describe, expect, it } from 'vitest';
import { newDocument } from '../workbenchModel';
import type { Finding, Identity, Sentence } from './assetsModel';
import {
  applyRange, bridgeFromSentence, countByLevel, identityCommand, identitySummary, nextSentence, triLabel, withBridge, withoutTag,
} from './assetsText';

const identity = (patch: Partial<Identity> = {}): Identity => ({
  accountRef: '00000000-0000-4000-8000-000000000001', revision: 3, orgRelation: 'parent', bioDisclosesRelation: null,
  bioCheckedAt: null, bioCheckedBy: null, hasRealPerson: true, backingPersonRole: '妈妈', canProvideExperience: null,
  savedAt: 't', savedBy: 'local_machine', ...patch,
});
const sentence = (code: string): Sentence => ({
  code, revision: 2, group: 'parent', text: `句子${code}`, status: 'ok', statusNote: null, origin: 'manual',
  entryFindings: [], usedCount: 0, savedAt: 't', savedBy: 'local_machine',
});
const finding = (level: Finding['level']): Finding => ({ rule: 'R-X', level, message: 'm', location: { kind: 'slot' }, fixes: [] });

describe('asset labels and helpers', () => {
  it('keeps true, false and unknown as three different answers', () => {
    expect(triLabel(true, '已写明', '没写明')).toBe('已写明');
    expect(triLabel(false, '已写明', '没写明')).toBe('没写明');
    expect(triLabel(null, '已写明', '没写明')).toBe('不清楚');
  });

  it('summarises an identity without turning unknown into a no', () => {
    expect(identitySummary(null)).toBe('还没填写');
    expect(identitySummary(identity({ orgRelation: 'unknown' }))).toBe('关系还没确认');
    expect(identitySummary(identity({ orgRelation: 'none' }))).toBe('和机构无关');
    expect(identitySummary(identity())).toBe('陪跑营学员家长 · 简介未核对');
    expect(identitySummary(identity({ bioDisclosesRelation: true, orgRelation: 'coach' }))).toBe('陪跑老师 · 简介已写明');
    expect(identitySummary(identity({ bioDisclosesRelation: false }))).toBe('陪跑营学员家长 · 简介没写明');
  });

  it('counts findings per level, including levels with none', () => {
    expect(countByLevel([])).toEqual({ must: 0, warn: 0, hint: 0 });
    expect(countByLevel([finding('must'), finding('warn'), finding('warn')])).toEqual({ must: 1, warn: 2, hint: 0 });
  });

  it('applies a range fix in UTF-16 units and refuses ranges that do not fit', () => {
    const text = '😀孩子变好了，真好';
    const start = text.indexOf('变好了');
    expect(applyRange(text, start, start + 3, null)).toBe('😀孩子，真好');
    expect(applyRange(text, start, start + 3, '在进步')).toBe('😀孩子在进步，真好');
    expect(applyRange(text, 5, 2, null)).toBe(text);
    expect(applyRange(text, 0, text.length + 1, null)).toBe(text);
    expect(applyRange(text, -1, 1, null)).toBe(text);
    expect(applyRange(text, 1.5, 3, null)).toBe(text);
  });

  it('removes one topic token from the space separated draft', () => {
    expect(withoutTag('ADHD  放手 #adhd', 2)).toBe('ADHD 放手');
    expect(withoutTag('甲', 0)).toBe('');
    expect(withoutTag('甲 乙', 5)).toBe('甲 乙');
  });

  it('builds the bridge from a library sentence and merges into an undecided slot', () => {
    expect(bridgeFromSentence(sentence('XJ-0001'))).toEqual({ sentenceCode: 'XJ-0001', sentenceRevision: 2, group: 'parent', text: '句子XJ-0001', edited: false });
    const doc = newDocument('题');
    expect(withBridge(doc, { skip: true }).bridge).toEqual({ skip: true, sentence: null, afterParagraph: null });
    const picked = withBridge(doc, { sentence: bridgeFromSentence(sentence('XJ-0002')) });
    expect(withBridge(picked, { skip: true }).bridge?.sentence?.sentenceCode).toBe('XJ-0002');
    expect(doc.bridge).toBeNull();
  });

  it('cycles through the usable sentences and starts at the first when none is chosen', () => {
    const items = [sentence('XJ-0001'), sentence('XJ-0002'), sentence('XJ-0003')];
    expect(nextSentence([], null)).toBeNull();
    expect(nextSentence(items, null)?.code).toBe('XJ-0001');
    expect(nextSentence(items, 'XJ-0001')?.code).toBe('XJ-0002');
    expect(nextSentence(items, 'XJ-0003')?.code).toBe('XJ-0001');
    expect(nextSentence(items, 'XJ-9999')?.code).toBe('XJ-0001');
    expect(nextSentence([items[0]!], 'XJ-0001')?.code).toBe('XJ-0001');
  });

  it('starts an identity edit from the latest revision and keeps answers that were not changed', () => {
    const fresh = identityCommand(null, { orgRelation: 'coach' }, 'r1');
    expect(fresh).toEqual({ requestId: 'r1', expectedRevision: 0, orgRelation: 'coach', bioDisclosesRelation: null,
      hasRealPerson: null, backingPersonRole: null, canProvideExperience: null });
    const next = identityCommand(identity(), { bioDisclosesRelation: true }, 'r2');
    expect(next).toMatchObject({ expectedRevision: 3, orgRelation: 'parent', bioDisclosesRelation: true, hasRealPerson: true, backingPersonRole: '妈妈' });
  });
});
