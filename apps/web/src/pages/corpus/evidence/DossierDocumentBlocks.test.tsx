import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DossierDocument } from './DossierDocument';
import { DossierGroup, EmptySpeechNotes, ReadingIssues, TranscriptGroups } from './DossierDocumentBlocks';
import { parseDossierPayload } from './dossierDecode';
import { endingGroupAttribution, readingGaps, resolveReadingSections } from './dossierDocumentModel';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
import type { DossierBlock, ReadingGroup } from './dossierTypes';
function mount(blocks: DossierBlock[], mode: ReadingGroup['mode'] = 'blocks') {
  const fixture = dossierJson(1, { materials: [dossierMaterialJson(1, { blocks })] });
  fixture.dossier.readingView.sections[0]!.groups = [{ ref: 'group', mode, blockRefs: blocks.map(block => block.ref), continuation: { fromPrevious: false, toNext: false }, ...(mode === 'paragraph' || mode === 'transcript_paragraph' ? { joiners: blocks.slice(1).map(() => '') } : {}) }] as ReadingGroup[];
  const value = resolveReadingSections(parseDossierPayload(fixture).dossier)[0]!.groups[0]!;
  return render(<DossierGroup value={value} prefix="test"/>);
}
it('确切术语/解释三行表可读作定义列表，全部原文和表头语义保留', () => {
  const view = mount([{ ref: 'definitions', kind: 'table', columns: ['术语', '解释'], rows: [['理解','说清楚为什么'],['应用','放进真实情境'],['迁移','换个场景也能用']] }]);
  const list = screen.getByLabelText('术语与解释');
  expect(list.tagName).toBe('DL');
  expect(list.querySelectorAll('dt')).toHaveLength(3); expect(list.querySelectorAll('dd')).toHaveLength(3);
  expect(list.textContent).toBe('理解说清楚为什么应用放进真实情境迁移换个场景也能用');
  expect(view.container.querySelector('table')).toBeNull();
});
it.each([
  ['普通二列表', ['项目','说明'], [['A','一'],['B','二'],['C','三']]],
  ['两行定义表', ['术语','解释'], [['A','一'],['B','二']]],
  ['带额外列的表', ['术语','解释','来源'], [['A','一','x'],['B','二','y'],['C','三','z']]],
] as const)('%s 保留原表格，不根据条数套定义样式', (_name, columns, rows) => {
  const view = mount([{ ref: 'table', kind: 'table', columns: [...columns], rows: rows.map(row => [...row]) }]);
  expect(screen.getByRole('table')).toBeVisible(); expect(view.container.querySelector('dl')).toBeNull();
});
it('普通三条列表继续是列表，合并段落/列表/表格不丢失 other 归属', () => {
  const list = mount([{ ref: 'items', kind: 'list', ordered: false, items: ['一','二','三'], speaker: 'other' }], 'list');
  expect(screen.getByRole('list')).toBeVisible(); expect(screen.getByText('他人表述')).toBeVisible(); expect(list.container.querySelector('dl')).toBeNull(); list.unmount();
  const paragraph = mount([{ ref: 'a', kind: 'paragraph', text: '前半', speaker: 'other' }, { ref: 'b', kind: 'paragraph', text: '后半', speaker: 'other' }], 'paragraph');
  expect(screen.getByText('前半后半')).toBeVisible(); expect(screen.getAllByText('他人表述')).toHaveLength(1); paragraph.unmount();
  mount([{ ref: 'table', kind: 'table', columns: ['A'], rows: [['值']], speaker: 'other' }], 'table');
  expect(screen.getByText('他人表述')).toBeVisible();
});
it.each(['image', 'audio'] as const)('%s 连续同角色仅首处说明，已知归属重置且未知归属不加解释', kind => {
  const roles = ['other', 'other', 'author', 'author', 'unconfirmed', 'unconfirmed'] as const;
  const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind, blocks: roles.map((speaker, index) => ({ ref: `role-${index}`, kind: 'paragraph', speaker, text: `完整原文 ${index}` })) })] }));
  const groups = resolveReadingSections(payload.dossier)[0]!.groups;
  const view = render(kind === 'audio' ? <TranscriptGroups groups={groups} prefix="roles"/> : <>{groups.map((value, index) => <DossierGroup key={value.group.ref} value={value} prefix="roles" previousRole={endingGroupAttribution(groups[index - 1])}/>)}</>);
  for (const label of ['他人表述', '作品作者表述']) expect(screen.getAllByText(label)).toHaveLength(1);
  const text = view.container.textContent ?? '';
  expect(text.indexOf('作品作者表述')).toBeLessThan(text.indexOf('完整原文 2'));
  expect(screen.queryByText('表达归属未确认')).not.toBeInTheDocument();
  expect(screen.getByText('完整原文 4')).toBeVisible();
  expect(text).not.toMatch(/讲述者|说话人|\d+ 人/);
});
it('同一个 blocks 组内及对话之后也重置表达归属', () => {
  mount([{ ref: 'other', kind: 'paragraph', speaker: 'other', text: '他人原话' }, { ref: 'author', kind: 'paragraph', speaker: 'author', text: '作者回应' }, { ref: 'dialogue', kind: 'dialogue', turns: [{ speaker: 'other', text: '明确对话' }] }, { ref: 'after', kind: 'paragraph', speaker: 'author', text: '对话后的作者正文' }]);
  expect(screen.getAllByText('作品作者表述')).toHaveLength(2);
  expect(screen.getByText('对话后的作者正文')).toBeVisible();
});
it.each([['running', '处理中'], ['failed', '处理失败'], ['cancelled', '已取消'], ['invalidated', '已失效']] as const)('无读本的 %s 保留真实执行状态', (state, expected) => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', blocks: [], reading: null, currentRevisionRef: null, processing: { state, reason: null } })], coverage: { sources: { body: { state: 'known_empty', units: 0, readings: 0 }, images: { state: 'not_applicable', units: 0, readings: 0 }, audio: { state: 'not_processed', units: 1, readings: 0 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } } } })).dossier;
  expect(readingGaps(dossier).find(row => row.key === 'audio')?.state).toBe(expected);
});
it('known_empty 的有效空读本在再次处理失败时继续有效，不误说无读本', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', sourceState: 'known_empty', blocks: [], processing: { state: 'failed', reason: 'retry_failed' } })], coverage: { sources: { body: { state: 'known_empty', units: 0, readings: 0 }, images: { state: 'not_applicable', units: 0, readings: 0 }, audio: { state: 'known_empty', units: 1, readings: 1 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } } } })).dossier;
  expect(readingGaps(dossier).some(row => row.key.includes('audio'))).toBe(false);
  expect(dossier.materials[0]?.processing?.state).toBe('failed');
});


it('后续空识别状态不否定保留的可用口播', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'kept-speech', kind: 'paragraph', text: '既有口播' }], processing: { state: 'completed', reason: 'no_speech_observed' } })] })).dossier;
  const view = render(<EmptySpeechNotes materials={dossier.materials}/>);
  expect(view.container.textContent).toBe('');
  expect(dossier.materials[0]?.blocks).toHaveLength(1);
});


it('人工确认无口播的空读本不成为待补项', () => {
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', sourceState: 'present', blocks: [],
    reading: { generatedBy: 'human_edit', adoption: 'human_edited', verification: 'verified', ruleVersion: 'human.v1', review: null },
    processing: { state: 'completed', reason: null } })], coverage: { sources: { body: { state: 'not_applicable', units: 0, readings: 0 }, images: { state: 'not_applicable', units: 0, readings: 0 }, audio: { state: 'available', units: 1, readings: 1 }, videoFrames: { state: 'not_applicable', units: 0, readings: 0 } } } })).dossier;
  expect(readingGaps(dossier).some(row => row.key === 'audio')).toBe(false);
});


it('结构块的真实时间范围保留毫秒，连续口播保持紧凑起点和精确定位', () => {
  const dialogue = mount([{ ref: 'timed-dialogue', kind: 'dialogue', turns: [{ speaker: 'other', text: '有来源定位的对话' }], time: { startMs: 100, endMs: 200 } }]);
  const range = screen.getByText('00:00.100–00:00.200');
  expect(range).toHaveAttribute('data-time-start-ms', '100');
  expect(range).toHaveAttribute('data-time-end-ms', '200');
  dialogue.unmount();
  const dossier = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'precise-speech', kind: 'paragraph', text: '完整的口播', time: { startMs: 173125, endMs: 179250 } }] })] })).dossier;
  render(<TranscriptGroups groups={resolveReadingSections(dossier)[0]!.groups} prefix="precise"/>);
  const time = screen.getByText('02:53');
  expect(time).toHaveAttribute('title', '02:53.125–02:59.250');
  expect(time).toHaveAttribute('data-time-start-ms', '173125');
  expect(time).toHaveAttribute('data-time-end-ms', '179250');
});

it.each(['blocks', 'paragraph'] as const)('在线资料在%s模式保留原文与视觉类型', mode => {
  mount([{ ref: 'observation', kind: 'paragraph', text: '两栏以不同底色区分。', speaker: 'unconfirmed', contentType: 'visual_description', regionRef: 'r1', regionLabel: '整图' }], mode);
  expect(screen.getByText('视觉描述 · 模型观察，非原文')).toBeVisible();
  expect(screen.queryByText('整图')).toBeNull();
  expect(screen.queryByText('作品作者表述')).toBeNull();
});

it('图片资料提供所属原图入口并保留未知页码', () => {
  const originalUrl = '/api/local/media/20000000-0000-4000-8000-000000000001/' + 'a'.repeat(64);
  const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { page: null, originalUrl })] }));
  render(<DossierDocument payload={payload} domain="test" active={true} busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/>);
  expect(screen.getByRole('link', { name: '查看原图' })).toHaveAttribute('href', originalUrl);
  expect(screen.getByText(/图片 · 页码未确认/)).toBeVisible();
});

it('核对说明默认收起，通用提示不展示，具体问题展开后仍可读', () => {
  const issue = { code: 'image_limitation', key: 'cut', displayKey: 'cut', message: '右栏末尾被裁切', resolution: 'reading_edit' as const };
  const { container } = render(<ReadingIssues issues={[issue, { ...issue, key: 'duplicate', displayKey: 'duplicate' }, { ...issue, key: 'general', displayKey: 'general', code: 'cloud_image_unchecked', message: '通用未核对提醒' }]}/>);
  expect(container.querySelector('details')).not.toHaveAttribute('open');
  expect(screen.getByText('右栏末尾被裁切')).not.toBeVisible();
  expect(screen.queryByText('通用未核对提醒')).toBeNull();
  expect(screen.getAllByText('右栏末尾被裁切')).toHaveLength(1);
  fireEvent.click(screen.getByText('核对说明'));
  expect(screen.getByText('右栏末尾被裁切')).toBeVisible();
});

it('同图多个区域只保留一个核对入口，展开后保留疑点位置', () => {
  const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { issues: [], blocks: [
    { ref: 'left', kind: 'paragraph', text: '左栏原文', contentType: 'extracted_text', regionRef: 'r1', regionLabel: '左栏', issues: [{ key: 'left', code: 'region_uncertainty', message: '末字模糊', blockRef: 'left', resolution: 'reading_edit' }] },
    { ref: 'right', kind: 'paragraph', text: '右栏原文', contentType: 'extracted_text', regionRef: 'r2', regionLabel: '右栏', issues: [{ key: 'right', code: 'region_uncertainty', message: '数值模糊', blockRef: 'right', resolution: 'reading_edit' }] }
  ] })] }));
  render(<DossierDocument payload={payload} domain="test" active={true} busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/>);
  expect(screen.getAllByText(/^核对说明/)).toHaveLength(1);
  expect(screen.getByText('左栏：末字模糊')).not.toBeVisible();
  fireEvent.click(screen.getByText(/^核对说明/));
  expect(screen.getByText('左栏：末字模糊')).toBeVisible();
  expect(screen.getByText('右栏：数值模糊')).toBeVisible();
});

it('口播段落的具体疑点默认收起并可展开核对', () => {
  const issue = { key: 'unclear-number', code: 'glyph', message: '第二句数字需核对', resolution: 'reading_edit' };
  const raw = dossierJson(1, { materials: [dossierMaterialJson(1, { kind: 'audio', blocks: [{ ref: 'speech', kind: 'paragraph', text: '练习持续十五分钟。', issues: [issue] }], issues: [issue] })] });
  raw.dossier.readingView.sections[0]!.groups[0]!.mode = 'transcript_paragraph';
  const groups = resolveReadingSections(parseDossierPayload(raw).dossier)[0]!.groups;
  render(<TranscriptGroups groups={groups} prefix="audio-review"/>);
  expect(screen.getByText(issue.message)).not.toBeVisible();
  fireEvent.click(screen.getByText(/^核对说明/));
  expect(screen.getByText(issue.message)).toBeVisible();
});

it('在线读本缺口明确标注待在线识读，不把旧处理完成当作正文可用', () => {
  const pending = dossierMaterialJson(1, { blocks: [], currentRevisionRef: null, reading: null,
    issues: [{ key: 'online_image_pending', code: 'online_image_pending', message: '历史 OCR 已保留', resolution: 'source_refresh' }],
    processing: { state: 'completed', reason: null } });
  const dossier = parseDossierPayload(dossierJson(1, { materials: [pending] })).dossier;
  expect(readingGaps(dossier)).toContainEqual({ key: 'images', label: '图片资料', state: '待在线识读', message: '1 张图片尚未在线识读。 第 1、2 页文字待补全。' });
});

it('在线过渡中的未识读和失败未知页全部计数', () => {
  const issue = { key: 'online_image_pending', code: 'online_image_pending', message: '历史OCR留存', resolution: 'source_refresh' };
  const payload = dossierJson(1, { materials: [
    dossierMaterialJson(1, { page: null, blocks: [], currentRevisionRef: null, reading: null, issues: [issue] }),
    dossierMaterialJson(2, { page: null, blocks: [], currentRevisionRef: null, reading: null, issues: [], processing: { state: 'failed', reason: 'ocr_failed' } }),
  ] });
  const dossier = parseDossierPayload(payload).dossier;
  dossier.coverage.imageInventory.expectedPages = null;
  dossier.coverage.imageInventory.missingPages = [];
  expect(readingGaps(dossier)).toContainEqual({ key: 'images', label: '图片资料', state: '待在线识读', message: '2 张图片尚未在线识读。其中 1 张处理失败。' });
});
