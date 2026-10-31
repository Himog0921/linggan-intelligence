import { expect, it } from 'vitest';
import { parseDossierPayload } from './dossierDecode';
import { groupText, resolveReadingSections } from './dossierDocumentModel';
import { hasReadingSourceText, missingImageTextPages } from './dossierReadingContent';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
import type { ReadingGroup, ReadingSection } from './dossierTypes';
const group = (blockRefs: string[], patch: Record<string, unknown> = {}) => ({ ref: `group:${blockRefs.join(',')}`, mode: 'blocks', blockRefs, continuation: { fromPrevious: false, toNext: false }, ...patch }) as ReadingGroup;
function fixture(materials: unknown[], groups: ReadingGroup[], kind: ReadingSection['kind'] = 'images') {
  return dossierJson(1, { materials, readingView: { mainBlockRefs: groups.flatMap(value => value.blockRefs), supplementBlockRefs: [], sections: [{ ref: `section:${kind}`, kind, groups, gaps: [] }] } });
}
it('sections 固定章节顺序，旧主补数组保持 Works 原相对次序', () => {
  const materials = [dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'audio', kind: 'paragraph', text: '音频' }] }), dossierMaterialJson(2, { kind: 'video_frame', blocks: [{ ref: 'frame', kind: 'paragraph', text: '画面' }] }), dossierMaterialJson(3, { blocks: [{ ref: 'image', kind: 'paragraph', text: '图片' }] })];
  const raw = dossierJson(1, { materials, readingView: { mainBlockRefs: ['audio', 'frame', 'image'], supplementBlockRefs: [] } });
  const parsed = parseDossierPayload(raw).dossier;
  expect(parsed.readingView.mainBlockRefs).toEqual(['audio', 'frame', 'image']);
  expect(parsed.readingView.sections.map(section => section.kind)).toEqual(['images', 'audio', 'frames']);
  const invalid = structuredClone(raw); invalid.dossier.readingView.sections.reverse();
  expect(() => parseDossierPayload(invalid)).toThrow();
});
it('引用组必需、完整、唯一、对应来源种类，不能缺字段按 kind 猜读法', () => {
  const raw = dossierJson(1);
  for (const mutate of [
    (value: typeof raw) => { delete (value.dossier.readingView as Partial<typeof value.dossier.readingView>).sections; },
    (value: typeof raw) => { value.dossier.readingView.sections[0]!.groups = []; },
    (value: typeof raw) => { value.dossier.readingView.sections[0]!.groups[0]!.blockRefs = ['missing']; },
    (value: typeof raw) => { value.dossier.readingView.sections[1]!.groups[0]!.ref = value.dossier.readingView.sections[0]!.groups[0]!.ref; },
    (value: typeof raw) => { value.dossier.readingView.sections[1]!.kind = 'audio'; },
    (value: typeof raw) => { value.dossier.readingView.sections[1]!.groups.push(value.dossier.readingView.sections[1]!.groups[0]!); },
  ]) { const value = structuredClone(raw); mutate(value); expect(() => parseDossierPayload(value)).toThrow(); }
});
it('paragraph 仅按服务端 joiners 拼接，角色不同与结构块不能强拼', () => {
  const material = dossierMaterialJson(1, { blocks: [{ ref: 'a', kind: 'paragraph', text: '前半', speaker: 'author' }, { ref: 'b', kind: 'paragraph', text: '后半', speaker: 'author' }] });
  const raw = fixture([material], [group(['a', 'b'], { mode: 'paragraph', joiners: [''] })]);
  const resolved = resolveReadingSections(parseDossierPayload(raw).dossier)[0]!.groups[0]!;
  expect(groupText(resolved)).toBe('前半后半');
  for (const joiners of [[], ['自行填字'], [' ', '\n']]) expect(() => parseDossierPayload(fixture([material], [group(['a', 'b'], { mode: 'paragraph', joiners })]))).toThrow();
  for (const second of [{ ref: 'b', kind: 'paragraph', text: '后半' }, { ref: 'b', kind: 'gap', text: '缺字', speaker: 'author' }]) {
    expect(() => parseDossierPayload(fixture([{ ...material, blocks: [material.blocks[0], second] }], raw.dossier.readingView.sections[0]!.groups))).toThrow();
  }
});
it('口播组保持毫秒和单材料边界，segment continuation 保留', () => {
  const material = dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'a', kind: 'paragraph', text: '完整口播', time: { startMs: 173000, endMs: 190000 } }] });
  const raw = fixture([material], [group(['a'], { mode: 'transcript_paragraph', joiners: [], continuation: { fromPrevious: true, toNext: true } })], 'audio');
  const dto = parseDossierPayload(raw).dossier;
  expect(dto.materials[0]!.blocks[0]!.time).toEqual({ startMs: 173000, endMs: 190000 });
  expect(dto.readingView.sections[0]!.groups[0]!.continuation).toEqual({ fromPrevious: true, toNext: true });
  const second = dossierMaterialJson(2, { kind: 'audio', blocks: [{ ref: 'b', kind: 'paragraph', text: '另一视频' }] });
  expect(() => parseDossierPayload(fixture([material, second], [group(['a', 'b'], { mode: 'transcript_paragraph', joiners: [' '] })], 'audio'))).toThrow();
});
it('已确认标签对和两种单边 segment 保留顺序与重复，正文不变', () => {
  const body = dossierMaterialJson(1, { kind: 'body', sourceRef: 'same', blocks: [{ ref: 'a', kind: 'paragraph', text: '#A[话题]# #A# #B#' }] });
  const tags = dossierMaterialJson(2, { kind: 'tags', sourceRef: 'same', blocks: [{ ref: 'b', kind: 'list', ordered: false, items: ['A', 'A', 'B[话题]'] }] });
  for (const [materials, refs] of [[[body, tags], ['a', 'b']], [[body], ['a']], [[tags], ['b']]] as const) {
    const raw = fixture([...materials], [group([...refs], { mode: 'tags' })], 'publication');
    const dto = parseDossierPayload(raw).dossier;
    expect(groupText(resolveReadingSections(dto)[0]!.groups[0]!)).toBe('A\nA\nB');
    expect(dto.raw).toBe(raw.dossier);
  }
  for (const text of ['#A# 独立文字', '#未闭合', '#[话题]#', '# #']) expect(() => parseDossierPayload(fixture([{ ...body, blocks: [{ ref: 'a', kind: 'paragraph', text }] }], [group(['a'], { mode: 'tags' })], 'publication'))).toThrow();
});
it('有序列表与表格只接受兼容的真实结构', () => {
  const list = (ref: string, start: number) => ({ ref, kind: 'list', ordered: true, start, items: ['一项', '二项'] });
  const raw = fixture([dossierMaterialJson(1, { blocks: [list('a', 3), list('b', 5)] })], [group(['a', 'b'], { mode: 'list' })]);
  expect(() => parseDossierPayload(raw)).not.toThrow();
  expect(() => parseDossierPayload(fixture([dossierMaterialJson(1, { blocks: [list('a', 3), list('b', 9)] })], [group(['a', 'b'], { mode: 'list' })]))).toThrow();
  expect(() => parseDossierPayload(fixture([dossierMaterialJson(1, { blocks: [{ ref: 'a', kind: 'table', columns: ['A'], rows: [['x']] }, { ref: 'b', kind: 'table', columns: ['B'], rows: [['y']] }] })], [group(['a', 'b'], { mode: 'table' })]))).toThrow();
});
it('图片缺口必须本节定位并有正整数页码，其他章节不接收图片缺口', () => {
  const raw = dossierJson(1); const image = raw.dossier.readingView.sections[1]!;
  image.gaps = [{ beforeGroupRef: image.groups[0]!.ref, missingPages: [2] }];
  expect(parseDossierPayload(raw).dossier.readingView.sections[1]!.gaps).toEqual(image.gaps);
  for (const gap of [{ beforeGroupRef: 'outside-section', missingPages: [2] }, { beforeGroupRef: null, missingPages: [0] }, { beforeGroupRef: null, missingPages: [] }]) {
    const bad = structuredClone(raw); bad.dossier.readingView.sections[1]!.gaps = [gap]; expect(() => parseDossierPayload(bad)).toThrow();
  }
  const bad = structuredClone(raw); bad.dossier.readingView.sections[0]!.gaps = [{ beforeGroupRef: null, missingPages: [2] }]; expect(() => parseDossierPayload(bad)).toThrow();
});
it('待补文字页集合同时覆盖识读失败与源未取得，原图片清单语义不变', () => {
  const raw = dossierJson(1, { materials: [dossierMaterialJson(10), dossierMaterialJson(11, { blocks: [], reading: null, currentRevisionRef: null, processing: { state: 'failed', reason: 'ocr_failed' } }), dossierMaterialJson(12, { sourceState: 'not_acquired', blocks: [], reading: null, currentRevisionRef: null }), dossierMaterialJson(9, { sourceState: 'known_empty', blocks: [] })], coverage: { imageInventory: { basis: 'platform_image_list', expectedPages: 12, acquiredPages: 11, processedPages: 10, missingPages: [12], unknownOrder: false } } });
  raw.dossier.readingView.sections[0]!.gaps = [{ beforeGroupRef: null, missingPages: [11, 12] }];
  const dossier = parseDossierPayload(raw).dossier;
  expect(dossier.coverage.imageInventory.missingPages).toEqual([12]);
  expect(missingImageTextPages(dossier.materials, dossier.coverage.imageInventory)).toEqual([11, 12]);
  const bad = structuredClone(raw); bad.dossier.readingView.sections[0]!.gaps[0]!.missingPages = [9, 11, 12]; expect(() => parseDossierPayload(bad)).toThrow();
  expect(hasReadingSourceText({ ref: 'gap', kind: 'gap', text: '不是来源文字' })).toBe(false);
  expect(hasReadingSourceText({ ref: 'empty', kind: 'paragraph', text: ' \n ' })).toBe(false);
  expect(hasReadingSourceText({ ref: 'list', kind: 'list', ordered: false, items: [''] })).toBe(true);
  expect(hasReadingSourceText({ ref: 'table', kind: 'table', columns: [''], rows: [] })).toBe(true);
  expect(hasReadingSourceText({ ref: 'dialogue', kind: 'dialogue', turns: [{ speaker: 'unconfirmed', text: '' }] })).toBe(true);
});

it('同图大字号正文按服务端 paragraph 读法连接，原始 heading 块保持不变', () => {
  const material = dossierMaterialJson(1, { blocks: [{ ref: 'a', kind: 'heading', text: '大字号换行', speaker: 'author' }, { ref: 'b', kind: 'paragraph', text: '完整结尾', speaker: 'author' }] });
  const plan = [group(['a', 'b'], { mode: 'paragraph', joiners: [''] })];
  const parsed = parseDossierPayload(fixture([material], plan)).dossier;
  expect(groupText(resolveReadingSections(parsed)[0]!.groups[0]!)).toBe('大字号换行完整结尾');
  expect(parsed.materials[0]!.blocks[0]!.kind).toBe('heading');
  expect(() => parseDossierPayload(fixture([{ ...material, kind: 'audio' }], plan, 'audio'))).toThrow();
});
