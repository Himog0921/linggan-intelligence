import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import type { DossierMaterial, ReadingCandidate, ReadingReviewRequest } from './dossierTypes';
import * as api from './dossierApi';
vi.mock('./dossierApi', async original => ({ ...await original<typeof api>(), fetchCandidates: vi.fn(), reviewReading: vi.fn() }));
afterEach(() => vi.resetAllMocks());
function unit(overrides: Record<string, unknown> = {}): DossierMaterial { return parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, overrides)] })).dossier.materials[0]!; }
function candidate(material: DossierMaterial, id = 'candidate-1'): ReadingCandidate { return { unitRef: material.ref, slotKey: material.slotKey, candidateRevisionRef: id, currentRevisionRef: material.currentRevisionRef, blocks: [{ ref: `${id}.0`, kind: 'paragraph', text: `全文 ${id}`, speaker: 'unconfirmed' }], issues: [], generatedBy: 'cloud_vision', verification: 'unchecked', createdAt: '2026-10-23T08:01:00Z' }; }
function receipt(material: DossierMaterial, command: ReadingReviewRequest) { return { commandRef: command.commandRef, unitRef: material.ref, candidateRevisionRef: command.candidateRevisionRef, action: command.action, currentRevisionRef: command.action === 'keep' ? material.currentRevisionRef : 'saved-reading', adoption: command.action === 'edit' ? 'human_edited' as const : 'human_adopted' as const, verification: command.verified ? 'verified' as const : 'unchecked' as const, reviewRef: null }; }
function mount(material: DossierMaterial, applied = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return { ...render(<QueryClientProvider client={client}><ReadingReviewPanel unit={material} work={workRef(1)} domain={DOMAIN} onApplied={applied} /></QueryClientProvider>), applied, client };
}
it('无抽检记录也能采用明确选中的候选，生成/采用/核验分别表达', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([candidate(material), candidate(material, 'candidate-2')]);
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.selectOptions(await screen.findByLabelText('核对版本'), 'candidate-2');
  expect(screen.getByText('全文 candidate-2')).toBeVisible(); expect(api.reviewReading).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '采用此候选' })); await screen.findByText('候选读本已采用。');
  const command = vi.mocked(api.reviewReading).mock.calls[0]![3];
  expect(command).toMatchObject({ expectedCurrentRevisionRef: material.currentRevisionRef, candidateRevisionRef: 'candidate-2', action: 'adopt', verified: false, resolvedIssueKeys: [] }); expect(view.applied).toHaveBeenCalledTimes(1);
});
it('采用完成并刷新后，从新当前读本进入修正，不遗留候选选择或旧编辑缓冲', async () => {
  const material = unit(); const next = candidate(material); vi.mocked(api.fetchCandidates).mockResolvedValue([next]);
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef);
  await user.click(screen.getByRole('button', { name: '采用此候选' })); await screen.findByText('候选读本已采用。');
  const updated = { ...material, currentRevisionRef: 'saved-reading', blocks: [{ ref: 'canonical-new-block', kind: 'paragraph' as const, text: '服务端采用后刷新到的当前正文' }] };
  view.rerender(<QueryClientProvider client={view.client}><ReadingReviewPanel unit={updated} work={workRef(1)} domain={DOMAIN} onApplied={view.applied} /></QueryClientProvider>);
  await user.click(screen.getByRole('button', { name: '逐块修正' }));
  expect(screen.getByLabelText('段落文字')).toHaveValue('服务端采用后刷新到的当前正文');
  expect(screen.getByLabelText('核对版本')).toHaveValue('');
});
it.each([new TypeError('lost receipt'), new ApiError({ kind: 'http', status: 503, code: 'reading_storage_unavailable' })])('结构化修正保存全部内容，只显式解决文字问题，回执不确定后沿用原命令（%s）', async failure => {
  const material = unit({ blocks: [
    { ref: 'list', kind: 'list', items: ['不可忽略','100.25 ml'], ordered: true, start: 2 },
    { ref: 'table', kind: 'table', columns: ['条件','读数'], rows: [['不适用','0.5']], bbox: [0.1,0.2,0.3,0.4] },
    { ref: 'talk', kind: 'dialogue', turns: [{ speaker: 'other', text: '别人的意见' }] },
    { ref: 'heading', kind: 'heading', level: 2, text: '图中的次级标题', issues: [{ key: 'heading-word', code: 'glyph', message: '标题的一字待核对', resolution: 'reading_edit' }] },
  ], issues: [
    { key: 'word', code: 'unreadable', message: '这里有一个字待核对', resolution: 'reading_edit', blockRef: 'table' },
    { key: 'page', code: 'missing_source', message: '来源缺了一页', resolution: 'source_refresh', blockRef: null },
  ] });
  vi.mocked(api.fetchCandidates).mockResolvedValue([]);
  vi.mocked(api.reviewReading).mockRejectedValueOnce(failure).mockImplementationOnce(async (_w, _d, _u, command) => receipt(material, command));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '逐块修正' }));
  const full = '不是每天。'.repeat(20000); fireEvent.change(screen.getByLabelText('第 1 行，第 2 列'), { target: { value: full } });
  await user.click(screen.getByRole('checkbox', { name: /已对照来源解决：这里有一个字/ }));
  await user.click(screen.getByRole('checkbox', { name: /已对照来源解决：标题的一字待核对/ }));
  expect(screen.queryByRole('checkbox', { name: /来源缺了一页/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByRole('button', { name: '核对同一保存请求' });
  expect(screen.getByLabelText('第 1 行，第 2 列')).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '收起核对' })); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.click(screen.getByRole('button', { name: '核对同一保存请求' })); await screen.findByText('完整修正版已保存并采用。');
  const commands = vi.mocked(api.reviewReading).mock.calls.map(call => call[3]); expect(commands[0]).toEqual(commands[1]);
  expect(commands[0]?.resolvedIssueKeys).toEqual(['word', 'heading-word']); expect(commands[0]?.editedBlocks).toMatchObject([
    { kind: 'list', ordered: true, start: 2, items: ['不可忽略','100.25 ml'] },
    { kind: 'table', rows: [['不适用',full]], bbox: [0.1,0.2,0.3,0.4] },
    { kind: 'dialogue', turns: [{ speaker: 'other', text: '别人的意见' }] },
    { kind: 'heading', level: 2, text: '图中的次级标题' },
  ]); expect(view.applied).toHaveBeenCalledTimes(1);
});
it('可补回遗漏区块，新增区块没有虚构坐标，保留当前决定不发 editedBlocks', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '逐块修正' })); await user.click(screen.getByRole('button', { name: '添加区块' }));
  const fields = screen.getAllByLabelText('段落文字'); fireEvent.change(fields.at(-1)!, { target: { value: '原图中此前遗漏的段落' } });
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByText('完整修正版已保存并采用。');
  const added = vi.mocked(api.reviewReading).mock.calls[0]?.[3].editedBlocks?.at(-1); expect(added).toMatchObject({ kind: 'paragraph', text: '原图中此前遗漏的段落', speaker: 'unconfirmed' }); expect(added).not.toHaveProperty('bbox');
});
it('冲突不会宣布成功，保留内容供刷新后核对', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.reviewReading).mockRejectedValue(new ApiError({ kind: 'http', status: 409, code: 'reading_changed' }));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '保留当前读本' })); await screen.findByText(/当前读本或来源已变化/); expect(view.applied).not.toHaveBeenCalled();
  expect(vi.mocked(api.reviewReading).mock.calls[0]?.[3]).not.toHaveProperty('editedBlocks');
});
it('比较其他候选后保留当前读本，不把候选核验状态写给当前读本', async () => {
  const material = unit(); const next = candidate(material); next.verification = 'verified';
  vi.mocked(api.fetchCandidates).mockResolvedValue([next]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef);
  expect(screen.getByRole('checkbox', { name: '已对照来源核验所选候选读本' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: '保留当前读本' })); await screen.findByText('已记录保留当前读本。');
  expect(vi.mocked(api.reviewReading).mock.calls[0]?.[3]).toMatchObject({ action: 'keep', verified: false, candidateRevisionRef: next.candidateRevisionRef });
});
it('离开旧材料后迟到的保存回执不会修改新材料界面', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]);
  let release: (() => void) | undefined;
  vi.mocked(api.reviewReading).mockImplementation((_w, _d, _u, command) => new Promise(resolve => { release = () => resolve(receipt(material, command)); }));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); await screen.findByText(/当前没有其他候选版本/); await user.click(screen.getByRole('button', { name: '保留当前读本' }));
  view.unmount(); const next = unit({ ref: 'new-unit' }); mount(next); await act(async () => release?.());
  expect(screen.queryByText('已记录保留当前读本。')).not.toBeInTheDocument(); expect(view.applied).not.toHaveBeenCalled();
  expect(within(document.body).getByRole('button', { name: '核对与修正' })).toBeInTheDocument();
  await waitFor(() => expect(api.reviewReading).toHaveBeenCalledTimes(1));
});
