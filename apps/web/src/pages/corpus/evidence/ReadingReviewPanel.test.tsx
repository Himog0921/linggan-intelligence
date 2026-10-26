import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import type { DossierMaterial, ReadingCandidate, ReadingReviewRequest, ReadingReviewReceipt } from './dossierTypes';
import * as api from './dossierApi';
vi.mock('./dossierApi', async original => ({ ...await original<typeof api>(), fetchCandidates: vi.fn(), reviewReading: vi.fn() }));
afterEach(() => vi.resetAllMocks());
function unit(overrides: Record<string, unknown> = {}): DossierMaterial { return parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, overrides)] })).dossier.materials[0]!; }
function candidate(material: DossierMaterial, id = 'candidate-1'): ReadingCandidate { return { unitRef: material.ref, slotKey: material.slotKey, candidateRevisionRef: id, currentRevisionRef: material.currentRevisionRef, blocks: [{ ref: `${id}.0`, kind: 'paragraph', text: `全文 ${id}`, speaker: 'unconfirmed' }], issues: [], generatedBy: 'cloud_vision', verification: 'unchecked', createdAt: '2026-10-23T08:01:00Z', review: null }; }
function receipt(material: DossierMaterial, command: ReadingReviewRequest): ReadingReviewReceipt {
  const opinion = command.action === 'pending' || command.action === 'reject';
  return { commandRef: command.commandRef, unitRef: material.ref, candidateRevisionRef: command.candidateRevisionRef, action: command.action, currentRevisionRef: command.action === 'keep' || opinion ? material.currentRevisionRef : 'saved-reading', adoption: opinion ? material.reading?.adoption ?? null : command.action === 'edit' ? 'human_edited' : 'human_adopted', verification: opinion ? material.reading?.verification ?? null : command.verified ? 'verified' : 'unchecked', reviewRef: null };
}
function fillMetadata() { fireEvent.change(screen.getByLabelText('审核人', { exact: true }), { target: { value: '合成审核人' } }); fireEvent.change(screen.getByLabelText('核对依据', { exact: true }), { target: { value: '逐项对照合成原图，不发出外部请求' } }); }
function mount(material: DossierMaterial, applied = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return { ...render(<QueryClientProvider client={client}><ReadingReviewPanel unit={material} work={workRef(1)} domain={DOMAIN} onApplied={applied} /></QueryClientProvider>), applied, client };
}
it('无抽检记录也能采用明确选中的候选，生成/采用/核验分别表达', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([candidate(material), candidate(material, 'candidate-2')]);
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata();
  const versions = await screen.findByRole('combobox', { name: '核对版本' });
  expect(versions).toHaveAccessibleName('核对版本');
  expect(versions).toBe(screen.getByLabelText('核对版本', { exact: true }));
  await user.selectOptions(versions, 'candidate-2');
  expect(versions).toHaveAccessibleName('核对版本');
  expect(screen.getByText('全文 candidate-2')).toBeVisible(); expect(api.reviewReading).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: '采用此候选' })); await screen.findByText('候选读本已采用。');
  const command = vi.mocked(api.reviewReading).mock.calls[0]![3];
  expect(command).toMatchObject({ expectedCurrentRevisionRef: material.currentRevisionRef, candidateRevisionRef: 'candidate-2', action: 'adopt', verified: false, resolvedIssueKeys: [] }); expect(view.applied).toHaveBeenCalledTimes(1);
});
it('采用完成并刷新后，从新当前读本进入修正，不遗留候选选择或旧编辑缓冲', async () => {
  const material = unit(); const next = candidate(material); vi.mocked(api.fetchCandidates).mockResolvedValue([next]);
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata();
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
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata(); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '逐块修正' }));
  for (const label of ['第 1 项', '第 1 行，第 2 列', '第 1 轮发言', '标题文字']) expect(screen.getByLabelText(label, { exact: true })).toHaveAccessibleName(label);
  const full = '不是每天。'.repeat(20000); fireEvent.change(screen.getByLabelText('第 1 行，第 2 列'), { target: { value: full } });
  await user.click(screen.getByRole('checkbox', { name: /已对照来源解决：这里有一个字/ }));
  await user.click(screen.getByRole('checkbox', { name: /已对照来源解决：标题的一字待核对/ }));
  expect(screen.queryByRole('checkbox', { name: /来源缺了一页/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByRole('button', { name: '核对同一保存请求' });
  expect(screen.getByLabelText('第 1 行，第 2 列')).toBeDisabled();
  expect(screen.getByLabelText('审核人')).toBeDisabled(); expect(screen.getByLabelText('核对依据')).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '收起核对' })); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata();
  await user.click(screen.getByRole('button', { name: '核对同一保存请求' })); await screen.findByText('完整修正版已保存并采用。');
  const commands = vi.mocked(api.reviewReading).mock.calls.map(call => call[3]); expect(commands[0]).toEqual(commands[1]);
  expect(commands[0]).toMatchObject({ actor: '合成审核人', reason: '逐项对照合成原图，不发出外部请求' });
  expect(commands[0]?.resolvedIssueKeys).toEqual(['word', 'heading-word']); expect(commands[0]?.editedBlocks).toMatchObject([
    { kind: 'list', ordered: true, start: 2, items: ['不可忽略','100.25 ml'] },
    { kind: 'table', rows: [['不适用',full]], bbox: [0.1,0.2,0.3,0.4] },
    { kind: 'dialogue', turns: [{ speaker: 'other', text: '别人的意见' }] },
    { kind: 'heading', level: 2, text: '图中的次级标题' },
  ]); expect(view.applied).toHaveBeenCalledTimes(1);
});
it.each(['pending', 'reject'] as const)('没有当前读本时仍可提交 %s 意见，不编造采用或核验', async action => {
  const material = unit({ currentRevisionRef: null, reading: null, blocks: [] }); const next = candidate(material);
  vi.mocked(api.fetchCandidates).mockResolvedValue([next]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef);
  const button = screen.getByRole('button', { name: action === 'pending' ? '保持待核验' : '拒绝此候选' });
  expect(button).toBeDisabled(); fillMetadata();
  await user.click(screen.getByRole('checkbox', { name: '已对照来源核验所选候选读本' }));
  expect(screen.getByRole('button', { name: '保留当前读本' })).toBeDisabled();
  await user.click(button); await screen.findByText(action === 'pending' ? '已记录待核验意见，当前读本保持原样。' : '已拒绝此候选，当前读本保持原样。');
  const command = vi.mocked(api.reviewReading).mock.calls[0]![3];
  expect(command).toMatchObject({ action, expectedCurrentRevisionRef: null, candidateRevisionRef: next.candidateRevisionRef, verified: false, resolvedIssueKeys: [], actor: '合成审核人' }); expect(command).not.toHaveProperty('editedBlocks');
});
it('原图不可读时保留全文阅读，禁用新的人工决定', async () => {
  const material = unit({ originalUrl: null }); const next = candidate(material);
  vi.mocked(api.fetchCandidates).mockResolvedValue([next]); mount(material); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' })); await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef); fillMetadata();
  expect(screen.getByText('全文 candidate-1')).toBeVisible(); expect(screen.getByText(/原始媒体当前不可读，已有文字仍可阅读/)).toBeVisible();
  for (const name of ['采用此候选', '逐块修正', '保留当前读本', '保持待核验', '拒绝此候选']) expect(screen.getByRole('button', { name })).toBeDisabled();
  expect(api.reviewReading).not.toHaveBeenCalled();
});
it('没有独立图片 URL 的正常视频帧仍可修正，由后端核对真实媒体资格', async () => {
  const material = unit({ kind: 'video_frame', originalUrl: null, blocks: [{ ref: 'frame-block', kind: 'paragraph', text: '视频帧中的原文字', time: { startMs: 1250, endMs: 1250 } }] });
  vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); await screen.findByText(/当前没有其他候选版本/); fillMetadata();
  expect(screen.queryByText(/原始媒体当前不可读，已有文字仍可阅读/)).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '逐块修正' })); fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '对照视频后的完整修正' } });
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByText('完整修正版已保存并采用。');
  expect(vi.mocked(api.reviewReading).mock.calls[0]?.[3]).toMatchObject({ action: 'edit', expectedCurrentRevisionRef: material.currentRevisionRef, editedBlocks: [{ text: '对照视频后的完整修正' }] });
});
it('待核验保存后展示新审核意见，后续元数据刷新不替换草稿或未知保存命令', async () => {
  const material = unit({ currentRevisionRef: null, reading: null, blocks: [] }); const next = candidate(material);
  vi.mocked(api.fetchCandidates).mockResolvedValue([next]);
  vi.mocked(api.reviewReading).mockImplementationOnce(async (_w, _d, _u, command) => {
    vi.mocked(api.fetchCandidates).mockResolvedValue([{ ...next, review: { commandRef: command.commandRef, actor: command.actor, reason: command.reason, action: 'pending', reviewedAt: '2026-10-23T09:00:00Z' } }]);
    return receipt(material, command);
  });
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' }));
  await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef); fillMetadata();
  await user.click(screen.getByRole('button', { name: '保持待核验' })); await screen.findByText('已记录待核验意见，当前读本保持原样。');
  expect(await screen.findByText(/此候选最近意见：待核验 · 合成审核人/)).toHaveTextContent('核对依据：逐项对照合成原图，不发出外部请求');
  expect(screen.getByLabelText('核对版本')).toHaveValue(next.candidateRevisionRef);
  await user.click(screen.getByRole('button', { name: '逐块修正' }));
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '尚未取得保存回执的完整草稿' } });
  vi.mocked(api.reviewReading).mockRejectedValueOnce(new ApiError({ kind: 'http', status: 503, code: null })).mockImplementationOnce(async (_w, _d, _u, command) => receipt(material, command));
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByRole('button', { name: '核对同一保存请求' });
  await act(async () => { view.client.setQueryData(['dossier-candidates', DOMAIN, workRef(1)], [{ ...next, review: { commandRef: 'later-opinion', actor: '另一位核对人', reason: '补充核对意见', action: 'pending', reviewedAt: '2026-10-23T09:01:00Z' } }]); });
  expect(await screen.findByText(/此候选最近意见：待核验 · 另一位核对人/)).toHaveTextContent('核对依据：补充核对意见');
  expect(screen.getByLabelText('段落文字')).toHaveValue('尚未取得保存回执的完整草稿'); expect(screen.getByLabelText('段落文字')).toBeDisabled();
  expect(screen.getByLabelText('审核人')).toHaveValue('合成审核人'); expect(screen.getByLabelText('审核人')).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '核对同一保存请求' })); await screen.findByText('完整修正版已保存并采用。');
  const commands = vi.mocked(api.reviewReading).mock.calls.map(call => call[3]); expect(commands[2]).toEqual(commands[1]); expect(commands[2]).toMatchObject({ actor: '合成审核人', reason: '逐项对照合成原图，不发出外部请求', editedBlocks: [{ text: '尚未取得保存回执的完整草稿' }] });
});
it('可补回遗漏区块，新增区块没有虚构坐标，保留当前决定不发 editedBlocks', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata(); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '逐块修正' })); await user.click(screen.getByRole('button', { name: '添加区块' }));
  const fields = screen.getAllByLabelText('段落文字'); fireEvent.change(fields.at(-1)!, { target: { value: '原图中此前遗漏的段落' } });
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByText('完整修正版已保存并采用。');
  const added = vi.mocked(api.reviewReading).mock.calls[0]?.[3].editedBlocks?.at(-1); expect(added).toMatchObject({ kind: 'paragraph', text: '原图中此前遗漏的段落', speaker: 'unconfirmed' }); expect(added).not.toHaveProperty('bbox');
});
it('冲突不会宣布成功，保留内容供刷新后核对', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.reviewReading).mockRejectedValue(new ApiError({ kind: 'http', status: 409, code: 'reading_changed' }));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata(); await screen.findByText(/当前没有其他候选版本/);
  await user.click(screen.getByRole('button', { name: '保留当前读本' })); await screen.findByText(/当前读本或来源已变化/); expect(view.applied).not.toHaveBeenCalled();
  expect(vi.mocked(api.reviewReading).mock.calls[0]?.[3]).not.toHaveProperty('editedBlocks');
});
it('比较其他候选后保留当前读本，不把候选核验状态写给当前读本', async () => {
  const material = unit(); const next = candidate(material); next.verification = 'verified';
  vi.mocked(api.fetchCandidates).mockResolvedValue([next]); vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, _u, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata();
  await user.selectOptions(await screen.findByLabelText('核对版本'), next.candidateRevisionRef);
  expect(screen.getByRole('checkbox', { name: '已对照来源核验所选候选读本' })).toBeChecked();
  await user.click(screen.getByRole('button', { name: '保留当前读本' })); await screen.findByText('已记录保留当前读本。');
  expect(vi.mocked(api.reviewReading).mock.calls[0]?.[3]).toMatchObject({ action: 'keep', verified: false, candidateRevisionRef: next.candidateRevisionRef });
});
it('离开旧材料后迟到的保存回执不会修改新材料界面', async () => {
  const material = unit(); vi.mocked(api.fetchCandidates).mockResolvedValue([]);
  let release: (() => void) | undefined;
  vi.mocked(api.reviewReading).mockImplementation((_w, _d, _u, command) => new Promise(resolve => { release = () => resolve(receipt(material, command)); }));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '核对与修正' })); if (!screen.getByLabelText('审核人').hasAttribute('disabled')) fillMetadata(); await screen.findByText(/当前没有其他候选版本/); await user.click(screen.getByRole('button', { name: '保留当前读本' }));
  view.unmount(); const next = unit({ ref: 'new-unit' }); mount(next); await act(async () => release?.());
  expect(screen.queryByText('已记录保留当前读本。')).not.toBeInTheDocument(); expect(view.applied).not.toHaveBeenCalled();
  expect(within(document.body).getByRole('button', { name: '核对与修正' })).toBeInTheDocument();
  await waitFor(() => expect(api.reviewReading).toHaveBeenCalledTimes(1));
});

it('无当前读本和候选时不显示审核表单或无法执行的提交动作', async () => {
  const material = unit({ currentRevisionRef: null, reading: null, blocks: [], processing: { state: 'completed', reason: null } });
  vi.mocked(api.fetchCandidates).mockResolvedValue([]);
  mount(material); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' }));
  expect(await screen.findByText('尚无可核对的读本，请先完成图片识别。')).toBeVisible();
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('核对依据')).not.toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '逐块修正' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '保留当前读本' })).not.toBeInTheDocument();
  expect(screen.queryByText(/可直接对照来源修正/)).not.toBeInTheDocument();
  expect(api.reviewReading).not.toHaveBeenCalled();
});
it('无当前读本时须明确选中候选才出现审核表单，选择本身不采用', async () => {
  const material = unit({ currentRevisionRef: null, reading: null, blocks: [] });
  const next = candidate(material); vi.mocked(api.fetchCandidates).mockResolvedValue([next]);
  mount(material); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' }));
  const select = await screen.findByLabelText('核对版本');
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
  await user.selectOptions(select, next.candidateRevisionRef);
  expect(screen.getByLabelText('审核人')).toBeVisible();
  expect(screen.getByText('全文 candidate-1')).toBeVisible();
  expect(api.reviewReading).not.toHaveBeenCalled();
  await user.selectOptions(select, '');
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
});
it('无当前读本且候选读取失败时不给出可修正承诺', async () => {
  vi.mocked(api.fetchCandidates).mockRejectedValue(new ApiError({ kind: 'http', status: 503, code: null }));
  mount(unit({ currentRevisionRef: null, reading: null, blocks: [] })); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' }));
  expect(await screen.findByText('候选版本暂时不可读。')).toBeVisible();
  expect(screen.queryByText(/当前读本仍可核对|可直接对照来源修正/)).not.toBeInTheDocument();
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '重读候选' })).toBeEnabled();
});

it.each(['audio', 'video_frame'] as const)('%s 旧结果待接入时不指向图片整理或要求重新图片识别', async kind => {
  const material = unit({ kind, currentRevisionRef: null, reading: null, blocks: [], issues: [{ key: 'saved-result', code: 'existing_result_pending', message: '已有处理结果', resolution: 'source_refresh' }] });
  vi.mocked(api.fetchCandidates).mockResolvedValue([]); mount(material); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' }));
  expect(await screen.findByText('已有处理结果，尚未接入当前读本。')).toBeVisible();
  expect(screen.queryByRole('button', { name: '前往整理已有结果' })).not.toBeInTheDocument();
  expect(screen.queryByText(/请先完成图片识别/)).not.toBeInTheDocument();
  expect(api.reviewReading).not.toHaveBeenCalled();
});
it.each([
  ['audio', '尚无可核对的读本，请先取得音轨转写结果。'],
  ['video_frame', '尚无可核对的读本，请先取得画面识别结果。'],
] as const)('%s 没有读本或已有结果时使用对应来源提示', async (kind, message) => {
  vi.mocked(api.fetchCandidates).mockResolvedValue([]); mount(unit({ kind, currentRevisionRef: null, reading: null, blocks: [] })); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '核对与修正' }));
  expect(await screen.findByText(message)).toBeVisible();
  expect(screen.queryByText(/请先完成图片识别/)).not.toBeInTheDocument();
});
