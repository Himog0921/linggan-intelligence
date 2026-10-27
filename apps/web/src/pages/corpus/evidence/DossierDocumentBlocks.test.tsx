import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DossierGroup, TranscriptGroups } from './DossierDocumentBlocks';
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
it.each(['image', 'audio'] as const)('%s 连续同角色仅首处说明，other→author/unconfirmed 明确重置', kind => {
  const roles = ['other', 'other', 'author', 'author', 'unconfirmed', 'unconfirmed'] as const;
  const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { kind, blocks: roles.map((speaker, index) => ({ ref: `role-${index}`, kind: 'paragraph', speaker, text: `完整原文 ${index}` })) })] }));
  const groups = resolveReadingSections(payload.dossier)[0]!.groups;
  const view = render(kind === 'audio' ? <TranscriptGroups groups={groups} prefix="roles"/> : <>{groups.map((value, index) => <DossierGroup key={value.group.ref} value={value} prefix="roles" previousRole={endingGroupAttribution(groups[index - 1])}/>)}</>);
  for (const label of ['他人表述', '作品作者表述', '表达归属未确认']) expect(screen.getAllByText(label)).toHaveLength(1);
  const text = view.container.textContent ?? '';
  expect(text.indexOf('作品作者表述')).toBeLessThan(text.indexOf('完整原文 2'));
  expect(text.indexOf('表达归属未确认')).toBeLessThan(text.indexOf('完整原文 4'));
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
  const message = readingGaps(dossier).find(row => row.key === 'processing-audio')?.message;
  expect(message).toContain('既有已确认无文字的结果仍有效'); expect(message).not.toContain('无当前');
});
