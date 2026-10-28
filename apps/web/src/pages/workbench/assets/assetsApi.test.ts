import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { newDocument } from '../workbenchModel';
import {
  assetsErrorText, candidates, checkDocument, checkDraft, checkSentence, confirmCopy, confirmFactCard, copyStatus, createSentence,
  editSentence, listAccounts, listFactCards, listSentences, prepareCopy, putIdentity, reviewSentence, saveFactCard,
} from './assetsApi';
import { blockedFindings, decodeCheckReport, decodeCandidates, decodeSentences } from './assetsModel';

const id = '00000000-0000-4000-8000-000000000001';
const other = '00000000-0000-4000-8000-000000000002';

const identity = {
  accountRef: id, revision: 1, orgRelation: 'parent', bioDisclosesRelation: true, bioCheckedAt: '2026-10-28 10:00:00+08',
  bioCheckedBy: 'local_machine', hasRealPerson: true, backingPersonRole: '妈妈', canProvideExperience: null,
  savedAt: '2026-10-28 10:00:00+08', savedBy: 'local_machine',
};
const finding = {
  rule: 'R-M2', level: 'must', message: '账号简介还没写明和机构的关系，不能放衔接句', location: { kind: 'slot' },
  fixes: [{ label: '复制建议简介', action: 'copyBioSuggestion', text: '我是示例营的学员家长' }, { label: '这篇不植入', action: 'skipBridge' }],
};
const sentence = {
  code: 'XJ-0001', revision: 1, group: 'parent', text: '我也是学员家长', status: 'ok', statusNote: null, origin: 'manual',
  entryFindings: [], usedCount: 2, savedAt: 't', savedBy: 'local_machine',
};
const report = {
  rulesetVersion: 'creation-checks.v1', bridgeState: 'bio_missing', bridgeGroup: 'parent', findings: [finding],
  notRun: [{ rule: 'R-W3', reason: '产品事实卡还没有确认，产品叫法没有检查' }], factCards: [{ code: 'CP-0001', revision: 2 }],
};

function reply(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
}
function stub(...responses: Response[]) {
  const fetchMock = vi.fn(async () => responses.shift() ?? reply({}));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}
function call(fetchMock: ReturnType<typeof stub>, index = 0) {
  const [path, init] = fetchMock.mock.calls[index] as unknown as [string, RequestInit];
  return { path, method: init.method, body: init.body ? JSON.parse(String(init.body)) : undefined };
}
afterEach(() => vi.unstubAllGlobals());

describe('asset responses', () => {
  it('decode the shapes the server sends, including the flattened fix parameters', () => {
    const parsed = decodeCheckReport(report);
    expect(parsed.findings[0]?.fixes).toEqual([
      { label: '复制建议简介', action: 'copyBioSuggestion', text: '我是示例营的学员家长' }, { label: '这篇不植入', action: 'skipBridge' },
    ]);
    expect(parsed).toMatchObject({ bridgeState: 'bio_missing', bridgeGroup: 'parent', notRun: [{ rule: 'R-W3' }], factCards: [{ code: 'CP-0001', revision: 2 }] });
    expect(decodeCheckReport({ ...report, bridgeGroup: null }).bridgeGroup).toBeNull();
    expect(decodeCandidates({ state: 'ready', group: 'parent', items: [sentence] }).items[0]?.usedCount).toBe(2);
    expect(decodeSentences({ items: [] })).toEqual([]);
  });

  it('keep unknown as null and refuse anything outside the closed shapes', () => {
    const located = (location: unknown) => ({ ...report, findings: [{ ...finding, location }] });
    expect(decodeCheckReport(located({ kind: 'paragraph', id, start: 0, end: 3 })).findings[0]?.location).toEqual({ kind: 'paragraph', id, start: 0, end: 3 });
    expect(decodeCheckReport(located({ kind: 'tag', index: 2 })).findings[0]?.location).toEqual({ kind: 'tag', index: 2 });
    for (const broken of [
      { ...report, bridgeState: 'maybe' },
      { ...report, findings: [{ ...finding, level: 'fatal' }] },
      { ...report, findings: [{ ...finding, fixes: [{ label: '改', action: 'sendEmail' }] }] },
      { ...report, findings: [{ ...finding, fixes: [{ label: '改', action: 'replaceText' }] }] },
      located({ kind: 'galaxy' }),
      located({ kind: 'paragraph', id: 'nope', start: 0, end: 1 }),
      located({ kind: 'title', start: -1, end: 1 }),
      { ...report, factCards: [{ code: 'CP-0001' }] },
      { rulesetVersion: 'v1' },
    ]) {
      expect(() => decodeCheckReport(broken)).toThrow(ApiError);
    }
    expect(() => decodeSentences({ items: [{ ...sentence, group: 'both' }] })).toThrow(ApiError);
    expect(() => decodeSentences({ items: [{ ...sentence, usedCount: -1 }] })).toThrow(ApiError);
  });

  it('read the findings that came with a refusal and nothing else', () => {
    const refused = new ApiError({ kind: 'http', status: 409, code: 'creation_assets_blocked' }, { code: 'creation_assets_blocked', findings: [finding] });
    expect(blockedFindings(refused)?.[0]?.rule).toBe('R-M2');
    expect(blockedFindings(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_stale' }, { findings: [finding] }))).toBeNull();
    expect(blockedFindings(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_blocked' }, { findings: 'x' }))).toBeNull();
    expect(blockedFindings(new Error('x'))).toBeNull();
  });
});

describe('asset requests', () => {
  it('reads accounts, candidates, cards and sentences with the documented paths', async () => {
    const f = stub(
      reply({ items: [{ accountRef: id, accountName: '甲', platform: 'xhs', status: 'active', operatorName: null, identity }] }),
      reply({ state: 'ready', group: 'parent', items: [sentence] }),
      reply({ items: [] }),
      reply({ items: [sentence] }),
      reply({ items: [] }),
    );
    expect((await listAccounts(other))[0]?.identity?.backingPersonRole).toBe('妈妈');
    expect((await candidates(id)).items).toHaveLength(1);
    expect(await listFactCards()).toEqual([]);
    expect(await listSentences({ group: 'parent', status: 'ok' })).toHaveLength(1);
    await listSentences();
    expect([0, 1, 2, 3, 4].map(i => call(f, i).path)).toEqual([
      `/ops/api/creation/accounts?domainRef=${other}`, `/ops/api/creation/accounts/${id}/candidates`, '/ops/api/creation/fact-cards',
      '/ops/api/creation/bridge-sentences?group=parent&status=ok', '/ops/api/creation/bridge-sentences',
    ]);
  });

  it('writes identity, cards and sentences with the right verbs and bodies', async () => {
    const card = { code: 'CP-0001', revision: 1, name: '示例营', allowedNames: [], bannedNames: [], what: '', who: '', canSay: '', cannotSay: '',
      mustNote: '', confirmedBy: null, confirmedAt: null, savedAt: 't', savedBy: 'local_machine' };
    const f = stub(
      reply({ identity, replayed: false }), reply({ card, replayed: false }), reply({ card, replayed: false }), reply({ card, replayed: false }),
      reply({ sentence, replayed: false }), reply({ sentence, replayed: false }), reply({ sentence, replayed: true }),
    );
    const command = { requestId: 'r1', expectedRevision: 0, orgRelation: 'parent' as const, bioDisclosesRelation: true, hasRealPerson: null, backingPersonRole: null, canProvideExperience: null };
    expect((await putIdentity(id, command)).identity.revision).toBe(1);
    const cardCommand = { requestId: 'r2', expectedRevision: 0, name: '示例营', allowedNames: [], bannedNames: [], what: '', who: '', canSay: '', cannotSay: '', mustNote: '' };
    await saveFactCard(null, cardCommand);
    await saveFactCard('CP-0001', { ...cardCommand, expectedRevision: 1 });
    await confirmFactCard('CP-0001', { requestId: 'r3', expectedRevision: 1 });
    await createSentence({ requestId: 'r4', group: 'parent', text: '一句话', origin: 'manual' });
    await editSentence('XJ-0001', { requestId: 'r5', expectedRevision: 1, group: 'parent', text: '改一句' });
    expect((await reviewSentence('XJ-0001', { requestId: 'r6', expectedRevision: 1, action: 'approve', note: null })).replayed).toBe(true);
    expect([0, 1, 2, 3, 4, 5, 6].map(i => `${call(f, i).method} ${call(f, i).path}`)).toEqual([
      `PUT /ops/api/creation/accounts/${id}/identity`, 'POST /ops/api/creation/fact-cards', 'PUT /ops/api/creation/fact-cards/CP-0001',
      'POST /ops/api/creation/fact-cards/CP-0001/confirm', 'POST /ops/api/creation/bridge-sentences',
      'PUT /ops/api/creation/bridge-sentences/XJ-0001', 'POST /ops/api/creation/bridge-sentences/XJ-0001/review',
    ]);
    expect(call(f, 0).body).toEqual(command);
    expect(call(f, 6).body).toEqual({ requestId: 'r6', expectedRevision: 1, action: 'approve', note: null });
  });

  it('sends only what the check needs: the account, the words, the topics and the bridge slot', async () => {
    const heavy = { ...newDocument('标题'), accountRef: id, tags: ['ADHD'], paragraphs: [{ id, text: '正文' }],
      excerpts: [{ unrelated: true }] as never, canvas: [{ unrelated: true }] as never, uses: [{ unrelated: true }] as never,
      bridge: { skip: false, sentence: null, afterParagraph: null }, coverAi: true,
      pages: [{ id, kind: 'voice' }, { id: other, kind: 'cover', title: '封面大字' }, { id, kind: 'cover', title: '第二个封面' }] as never };
    const minimal = checkDocument(heavy);
    expect(minimal).toMatchObject({ title: '标题', accountRef: id, tags: ['ADHD'], coverAi: true, bridge: { skip: false }, excerpts: [], canvas: [], uses: [], materials: [] });
    expect(minimal.pages.map(page => page.id)).toEqual([other]);
    const f = stub(reply(report));
    await checkDraft(heavy);
    expect(call(f).method).toBe('POST');
    expect(call(f).path).toBe('/ops/api/creation/checks');
    expect(Object.keys(call(f).body)).toEqual(['document']);
    expect(call(f).body.document.excerpts).toEqual([]);
  });

  it('copies in two steps and reads the copied mark', async () => {
    const ready = { receiptRef: other, workId: id, revisionId: id, rulesetVersion: 'creation-checks.v1', title: '标题', body: '正文', tags: ['#ADHD'],
      text: '正文\n#ADHD', warnCount: 1, hintCount: 0, replayed: false };
    const f = stub(reply(ready), reply({ receiptRef: other, revisionId: id, copiedAt: 't' }), reply({ copied: null }), reply({ copied: { receiptRef: other, revisionId: id, copiedAt: 't' } }), reply({ state: 'x' }));
    expect((await prepareCopy(id, { requestId: 'c1', expectedRevision: id })).text).toBe('正文\n#ADHD');
    expect((await confirmCopy(id, other, { requestId: 'c2' })).copiedAt).toBe('t');
    expect((await copyStatus(id)).copied).toBeNull();
    expect((await copyStatus(id)).copied?.revisionId).toBe(id);
    expect([0, 1, 2].map(i => `${call(f, i).method ?? 'GET'} ${call(f, i).path}`)).toEqual([
      `POST /ops/api/creation/works/${id}/copy`, `POST /ops/api/creation/works/${id}/copy/${other}/confirm`, `GET /ops/api/creation/works/${id}/copy-status`,
    ]);
    stub(reply({ nope: true }));
    await expect(checkSentence('一句')).rejects.toBeInstanceOf(ApiError);
  });

  it('turns failures into plain sentences without blaming the user or hiding that the input is kept', () => {
    const http = (code: string | null) => assetsErrorText(new ApiError({ kind: 'http', status: 409, code }));
    expect(http('creation_assets_stale')).toContain('刷新');
    expect(http('creation_assets_blocked')).toContain('规则');
    expect(http('something_new')).toContain('当前输入仍保留');
    expect(http(null)).toContain('当前输入仍保留');
    expect(assetsErrorText(new ApiError({ kind: 'network' }))).toContain('结果尚未确认');
    expect(assetsErrorText(new ApiError({ kind: 'invalid_response', status: 200 }))).toContain('结果尚未确认');
    expect(assetsErrorText(new Error('boom'))).toContain('当前输入仍保留');
  });
});
