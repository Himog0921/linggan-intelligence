import { describe, expect, it } from 'vitest';
import { newDocument } from '../workbenchModel';
import type { Experience, Finding, Identity, Sentence } from './assetsModel';
import {
  adoptTitle, applyRange, bridgeFromSentence, countByLevel, dedupeTags, identityCommand, identitySummary, insertExperience, nextSentence, previewParts, triLabel, usableExperiences, withBridge, withCoverTitle, withoutTag,
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

describe('experience insertion', () => {
  const exp = (patch: Partial<Experience> = {}): Experience => ({
    id: '00000000-0000-4000-8000-0000000000e1', code: 'JL-0001', accountRef: 'a', currentPerson: true, state: 'confirmed', revision: 2,
    title: '一次小事', body: '那天我们慢慢来。', tags: null, origin: null, sendToModel: null, confirmedAt: 't', confirmedSubjectRole: '妈妈', ...patch,
  });
  it('lists only confirmed current-person experiences that the draft does not already carry at that revision', () => {
    const carried = exp({ id: '00000000-0000-4000-8000-0000000000e2' });
    const document = { ...newDocument('t'), experienceSlots: [{ paragraphId: 'p', experienceRef: carried.id, experienceRevision: carried.revision }] };
    const items = [exp(), carried, exp({ id: 'x3', state: 'awaiting_subject' }), exp({ id: 'x4', currentPerson: false }), exp({ id: 'x5', body: null }), { ...carried, revision: 3 }];
    expect(usableExperiences(document, items).map(item => `${item.id}:${item.revision}`)).toEqual([`${exp().id}:2`, `${carried.id}:3`]);
  });
  it('appends a paragraph when there is no empty experience slot, and records where the experience came from', () => {
    const next = insertExperience(newDocument('t'), exp());
    expect(next?.paragraphs.map(p => p.text)).toEqual(['那天我们慢慢来。']);
    expect(next?.experienceSlots).toEqual([{ paragraphId: next?.paragraphs[0]?.id, experienceRef: exp().id, experienceRevision: 2 }]);
  });
  it('fills the first empty experience slot instead of appending', () => {
    const base = { ...newDocument('t'), paragraphs: [{ id: 'g1', text: '' }, { id: 'p2', text: '后一段' }], experienceGaps: ['g1'] };
    const next = insertExperience(base, exp());
    expect(next?.paragraphs).toEqual([{ id: 'g1', text: '那天我们慢慢来。' }, { id: 'p2', text: '后一段' }]);
    expect(next?.experienceGaps).toEqual([]);
    expect(next?.experienceSlots[0]?.paragraphId).toBe('g1');
  });
  it('refuses an experience whose body is not readable', () => {
    expect(insertExperience(newDocument('t'), exp({ body: null }))).toBeNull();
  });
});

describe('phone preview text', () => {
  const para = (id: string, text: string) => ({ id, text });
  const withBridge = (patch: Partial<NonNullable<ReturnType<typeof newDocument>['bridge']>> = {}) => ({
    skip: false, afterParagraph: null, sentence: { sentenceCode: 'XJ-1', sentenceRevision: 1, group: 'parent' as const, text: '我也是学员家长', edited: false }, ...patch,
  });
  const base = () => ({ ...newDocument('t'), paragraphs: [para('a', '第一段'), para('b', '第二段'), para('c', '收尾')] });
  it('puts the bridge before the last paragraph unless a paragraph is chosen', () => {
    expect(previewParts({ ...base(), bridge: withBridge() }).map(p => p.text)).toEqual(['第一段', '第二段', '我也是学员家长', '收尾']);
    expect(previewParts({ ...base(), bridge: withBridge({ afterParagraph: 'a' }) }).map(p => p.text)).toEqual(['第一段', '我也是学员家长', '第二段', '收尾']);
    expect(previewParts({ ...base(), bridge: withBridge() }).map(p => p.bridge)).toEqual([false, false, true, false]);
  });
  it('leaves the bridge out when the draft skips it, has none, or the chosen paragraph is gone', () => {
    expect(previewParts({ ...base(), bridge: withBridge({ skip: true }) }).map(p => p.text)).toEqual(['第一段', '第二段', '收尾']);
    expect(previewParts({ ...base(), bridge: null }).map(p => p.text)).toEqual(['第一段', '第二段', '收尾']);
    expect(previewParts({ ...base(), bridge: withBridge({ afterParagraph: 'gone' }) }).map(p => p.text)).toEqual(['第一段', '第二段', '我也是学员家长', '收尾']);
  });
  it('drops empty paragraphs and still shows a bridge on an otherwise empty draft', () => {
    expect(previewParts({ ...newDocument('t'), paragraphs: [para('a', '  '), para('b', '内容')] }).map(p => p.text)).toEqual(['内容']);
    expect(previewParts({ ...newDocument('t'), bridge: withBridge() }).map(p => p.text)).toEqual(['我也是学员家长']);
  });
  it('reads a repeated tag once, whatever its case', () => {
    expect(dedupeTags('#A娃 #adhd  #ADHD #A娃 #陪跑')).toEqual(['#A娃', '#adhd', '#陪跑']);
    expect(dedupeTags('   ')).toEqual([]);
  });
});

describe('cover title', () => {
  it('sets the title of the existing cover, leaving other pages alone', () => {
    const base = newDocument('t');
    const withCover = withCoverTitle(base, '先做一步');
    expect(withCover.pages).toHaveLength(1);
    expect(withCover.pages[0]).toMatchObject({ kind: 'cover', title: '先做一步' });
    const again = withCoverTitle(withCover, '第一步变小');
    expect(again.pages).toHaveLength(1);
    expect(again.pages[0]?.title).toBe('第一步变小');
    expect(again.pages[0]?.id).toBe(withCover.pages[0]?.id);
  });
  it('makes no cover when the draft is already at the page limit', () => {
    const page = (id: string) => ({ ...withCoverTitle(newDocument('t'), 'x').pages[0]!, id, kind: 'opinion' as const });
    const full = { ...newDocument('t'), pages: Array.from({ length: 30 }, (_, index) => page(`p${index}`)) };
    expect(withCoverTitle(full, '新封面')).toBe(full);
  });
  it('adopting one of our own titles says no when the cover would need a 31st page, and still sets the note title', () => {
    const page = (id: string) => ({ ...withCoverTitle(newDocument('t'), 'x').pages[0]!, id, kind: 'opinion' as const });
    const full = { ...newDocument('t'), pages: Array.from({ length: 30 }, (_, index) => page(`p${index}`)) };
    expect(adoptTitle(full, 'cover', '新封面')).toBeNull();
    expect(adoptTitle(full, 'note', '新标题')).toMatchObject({ title: '新标题' });
    expect(adoptTitle(newDocument('t'), 'cover', '新封面')?.pages[0]).toMatchObject({ kind: 'cover', title: '新封面' });
  });
});
