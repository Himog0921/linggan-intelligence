import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, requestText } from '../../../shared/api';
import { parseBlock } from './dossierBlockDecode';
import { parseCandidates, parseDossierComments, parseDossierPayload, parseReadingReceipt } from './dossierDecode';
import { fetchDossier, fetchDossierMarkdown, fetchReadingVersion, reviewReading } from './dossierApi';
import { nestComments } from './dossierPresent';
import { DOMAIN, dossierCommentsJson, dossierJson, dossierMaterialJson, mediaHandle, workRef } from './evidenceFixtures';
afterEach(() => vi.unstubAllGlobals());
const response = (value: unknown) => new Response(JSON.stringify(value), { headers: { 'Content-Type': 'application/json' } });
describe('research-dossier.v2', () => {
  it('完整内容只在 materials.blocks 存在一次，原始导出不增加 UI 字段', () => {
    const long = '不是每天都需要。123.50 毫克\n'.repeat(6000);
    const raw = dossierJson(1, { bodyText: long }); const dto = parseDossierPayload(raw);
    expect(dto.dossier.materials[0]?.blocks[0]).toMatchObject({ kind: 'paragraph', text: long });
    expect(dto.dossier.raw).toBe(raw.dossier);
    expect(dto.dossier).not.toHaveProperty('fragments'); expect(dto.dossier).not.toHaveProperty('media');
    expect(dto).not.toHaveProperty('markdown'); expect(dto).not.toHaveProperty('inputs');
    expect(dto.dossier.note).not.toHaveProperty('externalId');
    expect(dto.dossier.coverage.imageInventory.missingPages).toEqual([2]);
  });
  it('直接拒绝旧合同、重复区块与错误来源状态', () => {
    for (const patch of [{ contract: 'research-dossier.v1' }, { fragments: [] }, { media: [] }, { materials: [dossierMaterialJson(1), dossierMaterialJson(1)] }, { materials: [dossierMaterialJson(1, { sourceState: 'UNKNOWN' })] }]) {
      const value = dossierJson(1); expect(() => parseDossierPayload({ ...value, dossier: { ...value.dossier, ...patch } })).toThrow(ApiError);
    }
  });
  it('未知总页数与已知零分别保留，不推算进度或页码', () => {
    const base = dossierJson(1);
    base.dossier.coverage.imageInventory = { basis: 'observed_set', expectedPages: null, acquiredPages: 0, processedPages: 0, missingPages: [], unknownOrder: true } as unknown as typeof base.dossier.coverage.imageInventory;
    expect(parseDossierPayload(base).dossier.coverage.imageInventory).toMatchObject({ expectedPages: null, acquiredPages: 0, unknownOrder: true });
  });
  it('原图只使用本地媒体句柄，不发起 CDN 或派生物回退', () => {
    const value = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1), dossierMaterialJson(2, { originalUrl: 'https://cdn.example/a.png' }), dossierMaterialJson(3, { originalUrl: '/api/local/derivative/a' })] }));
    expect(value.dossier.materials.map(unit => unit.originalUrl)).toEqual([mediaHandle(1), null, null]);
  });
  it('同图确认的页序保留独立来源，不替换封面读本或区块身份', () => {
    const original = dossierMaterialJson(1); const pageSource = { basis: 'same_verified_bytes', sourceRef: 'page-one-source', sourceVersion: 'verified-byte-version', slotKey: 'page-one-slot' };
    const material = parseDossierPayload(dossierJson(1, { materials: [{ ...original, pageSource }] })).dossier.materials[0]!;
    expect(material).toMatchObject({ ref: original.ref, sourceRef: original.sourceRef, currentRevisionRef: original.currentRevisionRef, page: 1, pageSource });
    expect(material.blocks.map(block => block.ref)).toEqual(original.blocks.map(block => block.ref));
    expect(() => parseDossierPayload(dossierJson(1, { materials: [{ ...original, pageSource: { ...pageSource, basis: 'guessed_from_cover' } }] }))).toThrow();
    expect(() => parseDossierPayload(dossierJson(1, { materials: [{ ...original, pageSource: { ...pageSource, sourceVersion: '' } }] }))).toThrow();
  });
  it.each([
    { ref: 'block', kind: 'paragraph', text: '文本', items: ['重复副本'] },
    { ref: 'block', kind: 'table', columns: ['A','B'], rows: [['缺列']] },
    { ref: 'block', kind: 'paragraph', text: '文本', bbox: [0.8,0.1,0.5,0.2] },
    { ref: 'block', kind: 'paragraph', text: '文本', time: { startMs: 300, endMs: 100 } },
    { ref: 'block', kind: 'dialogue', turns: [{ speaker: 'owner', text: '没有证据的归属' }] },
    { ref: 'block', kind: 'list', items: ['项目'], ordered: 'yes' },
    { ref: 'block', kind: 'list', items: [], ordered: false },
    { ref: 'block', kind: 'heading', text: '标题', level: 7 },
    { ref: 'block', kind: 'paragraph', text: '段落', ordered: true },
    { ref: 'block', kind: 'paragraph', text: '帧', time: { startMs: 1.5, endMs: 2 } },
  ])('拒绝不完整或矛盾结构 %#', block => expect(() => parseBlock(block)).toThrow());
  it('表格、列表、对话与真实坐标逐字段保留', () => {
    expect(parseBlock({ ref: 'list', kind: 'list', ordered: true, start: 3, items: ['不得停用', '100.5 ml'] })).toMatchObject({ start: 3, items: ['不得停用', '100.5 ml'] });
    expect(parseBlock({ ref: 'table', kind: 'table', columns: ['剂量','条件'], rows: [['0','未知\n第二行']] })).toMatchObject({ rows: [['0','未知\n第二行']] });
    expect(parseBlock({ ref: 'dialogue', kind: 'dialogue', turns: [{ speaker: 'other', text: '他人的看法' }] })).toMatchObject({ turns: [{ speaker: 'other' }] });
    expect(parseBlock({ ref: 'box', kind: 'paragraph', text: '文字', bbox: [0.1,0.2,0.3,0.4] }).bbox).toEqual([0.1,0.2,0.3,0.4]);
    expect(parseBlock({ ref: 'point', kind: 'paragraph', text: '实际采样帧', time: { startMs: 1250, endMs: 1250 } }).time).toEqual({ startMs: 1250, endMs: 1250 });
    expect(parseBlock({ ref: 'heading', kind: 'heading', text: '二级标题', level: 2 })).toMatchObject({ level: 2 });
  });
  it('候选与当前版本分别读出，不把生成当作采用', () => {
    const unit = parseDossierPayload(dossierJson(1)).dossier.materials[1]!;
    const candidate = { unitRef: unit.ref, slotKey: unit.slotKey, candidateRevisionRef: 'new', currentRevisionRef: unit.currentRevisionRef, blocks: unit.blocks, issues: [], generatedBy: 'cloud_vision', verification: 'unchecked', createdAt: '2026-10-23T08:01:00Z', review: null };
    expect(parseCandidates({ candidates: [candidate] })[0]).toMatchObject({ candidateRevisionRef: 'new', currentRevisionRef: unit.currentRevisionRef });
    expect(() => parseCandidates({ candidates: [{ ...candidate, generatedBy: 'legacy' }] })).toThrow(ApiError);
    const review = { commandRef: 'review-command', actor: '审核人', reason: '还需核对原图中的一字', action: 'pending', reviewedAt: '2026-10-23T09:00:00Z' };
    expect(parseCandidates({ candidates: [{ ...candidate, review }] })[0]?.review).toEqual(review);
    expect(() => parseCandidates({ candidates: [{ ...candidate, review: undefined }] })).toThrow(ApiError);
    expect(() => parseCandidates({ candidates: [{ ...candidate, review: { ...review, actor: ' ' } }] })).toThrow(ApiError);
  });
  it('意见回执区分没有当前读本与原有采用状态，拒绝伪造已采用或空缺核验状态', () => {
    const receipt = { commandRef: 'command', unitRef: 'unit', candidateRevisionRef: 'candidate', action: 'pending', currentRevisionRef: null, adoption: null, verification: null, reviewRef: 'review' };
    expect(parseReadingReceipt(receipt)).toMatchObject({ action: 'pending', currentRevisionRef: null, adoption: null, verification: null });
    expect(parseReadingReceipt({ ...receipt, action: 'reject', currentRevisionRef: 'current', adoption: 'human_edited', verification: 'verified' })).toMatchObject({ adoption: 'human_edited', verification: 'verified' });
    for (const patch of [{ action: 'adopt' }, { adoption: 'automatic' }, { currentRevisionRef: 'current', verification: 'verified' }]) expect(() => parseReadingReceipt({ ...receipt, ...patch })).toThrow(ApiError);
  });
});
describe('on-demand reads and scoped writes', () => {
  it('GET 默认只要 JSON，Markdown 带相同快照截止时间单独读取', async () => {
    const mock = vi.fn().mockResolvedValueOnce(response(dossierJson(1))).mockResolvedValueOnce(new Response('# 完整底稿', { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } })); vi.stubGlobal('fetch', mock);
    await fetchDossier(workRef(1), DOMAIN); expect(mock.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}/research-dossier?domain=${DOMAIN}`);
    expect(await fetchDossierMarkdown(workRef(1), DOMAIN, '2026-10-23T08:00:00Z')).toBe('# 完整底稿');
    const url = new URL(String(mock.mock.calls[1]?.[0]), 'http://localhost'); expect(url.searchParams.get('asOf')).toBe('2026-10-23T08:00:00Z'); expect(url.searchParams.get('format')).toBe('markdown');
  });
  it('文本导出仍执行同源边界并拒绝 HTML', async () => {
    const mock = vi.fn().mockResolvedValue(new Response('<html>登录</html>', { headers: { 'Content-Type': 'text/html' } })); vi.stubGlobal('fetch', mock);
    await expect(requestText('https://external.example/path')).rejects.toThrow(); expect(mock).not.toHaveBeenCalled();
    await expect(requestText('/api/local/test')).rejects.toBeInstanceOf(ApiError);
  });
  it('人工修正请求携带当前版本、原命令与显式问题解决清单', async () => {
    const command = { commandRef: 'command', expectedCurrentRevisionRef: 'current', candidateRevisionRef: 'candidate', action: 'edit' as const, editedBlocks: [{ ref: 'b', kind: 'paragraph' as const, text: '完整修正' }], verified: true, resolvedIssueKeys: ['legible'], actor: '审核人', reason: '对照来源完成修正' };
    const mock = vi.fn().mockResolvedValue(response({ ...command, unitRef: 'unit', currentRevisionRef: 'corrected', adoption: 'human_edited', verification: 'verified' })); vi.stubGlobal('fetch', mock);
    const receipt = await reviewReading(workRef(1), DOMAIN, 'unit', command); expect(receipt.currentRevisionRef).toBe('corrected'); expect(JSON.parse(mock.mock.calls[0]?.[1].body)).toEqual(command);
  });
  it('精确版本读取校验作品与读本身份，不能用别的作品或版本替换', async () => {
    const material = dossierMaterialJson(1); const mock = vi.fn().mockResolvedValueOnce(response({ workRef: workRef(1), currentRevisionRef: 'new-current', material })).mockResolvedValueOnce(response({ workRef: workRef(2), currentRevisionRef: null, material })).mockResolvedValueOnce(response({ workRef: workRef(1), currentRevisionRef: null, material: { ...material, currentRevisionRef: 'another-reading' } })).mockResolvedValueOnce(response({ workRef: workRef(1), material })); vi.stubGlobal('fetch', mock);
    await expect(fetchReadingVersion(workRef(1), DOMAIN, material.currentRevisionRef)).resolves.toMatchObject({ currentRevisionRef: 'new-current', material: { ref: material.ref, currentRevisionRef: material.currentRevisionRef } });
    await expect(fetchReadingVersion(workRef(1), DOMAIN, material.currentRevisionRef)).rejects.toBeInstanceOf(ApiError);
    await expect(fetchReadingVersion(workRef(1), DOMAIN, material.currentRevisionRef)).rejects.toBeInstanceOf(ApiError);
    await expect(fetchReadingVersion(workRef(1), DOMAIN, material.currentRevisionRef)).rejects.toBeInstanceOf(ApiError);
  });
  it('评论完整分页与未取得父评论保留独立', () => {
    const page = parseDossierComments(dossierCommentsJson(31)); expect(page.items).toHaveLength(31);
    const nodes = nestComments([{ sourceRef: 'r', parentSourceRef: null, relation: 'ROOT' as const }, { sourceRef: 'a', parentSourceRef: 'missing', relation: 'REPLY' as const }]);
    expect(nodes[1]?.orphan).toBe(true);
  });
});
