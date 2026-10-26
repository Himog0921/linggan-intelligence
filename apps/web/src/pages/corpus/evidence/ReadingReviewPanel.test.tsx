import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { MaterialsSection } from './DossierSections';
import { ReadingReviewPanel } from './ReadingReviewPanel';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import type { DossierMaterial, ReadingReviewRequest, ReadingReviewReceipt } from './dossierTypes';
import * as api from './dossierApi';
vi.mock('./dossierApi', async original => ({ ...await original<typeof api>(), reviewReading: vi.fn() }));
afterEach(() => { vi.resetAllMocks(); vi.unstubAllGlobals(); });
function unit(overrides: Record<string, unknown> = {}): DossierMaterial { return parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, overrides)] })).dossier.materials[0]!; }
function receipt(material: DossierMaterial, command: ReadingReviewRequest): ReadingReviewReceipt {
  return { commandRef: command.commandRef, unitRef: material.ref, action: 'edit', currentRevisionRef: 'saved-reading', adoption: 'human_edited', verification: command.verified ? 'verified' : 'unchecked', reviewRef: null };
}
function fillMetadata() {
  fireEvent.change(screen.getByLabelText('修正人', { exact: true }), { target: { value: '合成修正人' } });
  fireEvent.change(screen.getByLabelText('修正说明', { exact: true }), { target: { value: '逐项对照原图修正文字' } });
}
function mount(material: DossierMaterial, applied = vi.fn(), originalReadable = true) {
  return { ...render(<ReadingReviewPanel unit={material} work={workRef(1)} domain={DOMAIN} onApplied={applied} originalReadable={originalReadable} />), applied };
}
it('按需编辑当前完整读本，不读取候选或增加采用流程，保存带来源与基底', async () => {
  const material = unit(); vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  const fetch = vi.fn(); vi.stubGlobal('fetch', fetch);
  const view = mount(material); const user = userEvent.setup();
  expect(screen.queryByLabelText('段落文字')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '修正文字' }));
  expect(screen.getByLabelText('段落文字')).toHaveValue(material.blocks[1]?.kind === 'paragraph' ? material.blocks[1].text : '');
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '已核对的完整修正版' } }); fillMetadata();
  await user.click(screen.getByRole('checkbox', { name: '已对照原件核验文字' }));
  await user.click(screen.getByRole('button', { name: '保存修正' })); await screen.findByText('修正已保存。');
  const command = vi.mocked(api.reviewReading).mock.calls[0]![3];
  expect(command).toMatchObject({ unitRef: material.ref, sourceVersion: material.sourceVersion, expectedCurrentRevisionRef: material.currentRevisionRef, action: 'edit', verified: true });
  expect(command).not.toHaveProperty('candidateRevisionRef'); expect(command.editedBlocks).toHaveLength(2);
  expect(command.editedBlocks[1]).toMatchObject({ kind: 'paragraph', text: '已核对的完整修正版' });
  expect(view.applied).toHaveBeenCalledTimes(1); expect(fetch).not.toHaveBeenCalled();
  expect(screen.queryByRole('combobox', { name: '核对版本' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '采用此候选' })).not.toBeInTheDocument();
});
it.each([new TypeError('lost receipt'), new ApiError({ kind: 'http', status: 503, code: null })])('回执未确认时锁定原稿并复用同一请求：%s', async failure => {
  const material = unit(); vi.mocked(api.reviewReading).mockRejectedValueOnce(failure).mockImplementationOnce(async (_work, _domain, _unit, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '固定的人工修正' } });
  await user.click(screen.getByRole('button', { name: '保存修正' }));
  await screen.findByRole('button', { name: '核对同一保存请求' });
  expect(screen.getByLabelText('段落文字')).toBeDisabled(); expect(screen.getByLabelText('修正人')).toBeDisabled();
  expect(screen.queryByRole('button', { name: '取消' })).not.toBeInTheDocument();
  const frozen = structuredClone(vi.mocked(api.reviewReading).mock.calls[0]![3]);
  await user.click(screen.getByRole('button', { name: '核对同一保存请求' })); await screen.findByText('修正已保存。');
  expect(vi.mocked(api.reviewReading).mock.calls[1]![3]).toEqual(frozen);
});
it('回执身份不匹配不能宣布保存成功，只能核对同一请求', async () => {
  const material = unit(); vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => ({ ...receipt(material, command), unitRef: 'other-unit' }));
  const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  await user.click(screen.getByRole('button', { name: '保存修正' })); await screen.findByRole('button', { name: '核对同一保存请求' });
  expect(view.applied).not.toHaveBeenCalled(); expect(screen.queryByText('修正已保存。')).not.toBeInTheDocument();
});
it('来源或当前读本变化时保留人工草稿，显式更新后才可提交', async () => {
  const material = unit(); const view = mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '我的未提交修正' } });
  const updated = { ...material, sourceVersion: 'source-new', currentRevisionRef: 'reading-new', blocks: [{ ref: 'new', kind: 'paragraph' as const, text: '最新来源文字' }] };
  view.rerender(<ReadingReviewPanel unit={updated} work={workRef(1)} domain={DOMAIN} onApplied={view.applied} />);
  expect(screen.getByLabelText('段落文字')).toHaveValue('我的未提交修正'); expect(screen.getByRole('button', { name: '保存修正' })).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '使用最新读本' }));
  expect(screen.getByLabelText('段落文字')).toHaveValue('最新来源文字'); expect(screen.getByRole('button', { name: '保存修正' })).toBeEnabled();
});
it('空当前读本可按需人工录入，仍提交空基底而不选择候选', async () => {
  const material = unit({ currentRevisionRef: null, reading: null, blocks: [] });
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, _unit, command) => receipt(material, command));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  await user.click(screen.getByRole('button', { name: '添加区块' })); fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '人工对照录入' } });
  await user.click(screen.getByRole('button', { name: '保存修正' })); await screen.findByText('修正已保存。');
  expect(vi.mocked(api.reviewReading).mock.calls[0]![3]).toMatchObject({ expectedCurrentRevisionRef: null });
});
it('原件不可读禁止新增修正；受限时不显示缓存编辑稿', async () => {
  const material = unit(); const view = mount(material, vi.fn(), false); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' }));
  expect(screen.getByLabelText('段落文字')).toBeDisabled(); expect(screen.getByRole('button', { name: '保存修正' })).toBeDisabled();
  view.rerender(<ReadingReviewPanel unit={{ ...material, sourceState: 'restricted' }} work={workRef(1)} domain={DOMAIN} onApplied={view.applied} />);
  expect(screen.getByRole('alert')).toHaveTextContent('来源当前受限'); expect(screen.queryByLabelText('段落文字')).not.toBeInTheDocument();
});
it('明确冲突不覆盖新版本，保留草稿；双击保存只发一次', async () => {
  const material = unit(); let reject!: (value: unknown) => void;
  vi.mocked(api.reviewReading).mockImplementation(() => new Promise((_resolve, rejectFn) => { reject = rejectFn; }));
  mount(material); const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  fireEvent.click(screen.getByRole('button', { name: '保存修正' })); fireEvent.click(screen.getByRole('button', { name: '保存修正' }));
  expect(api.reviewReading).toHaveBeenCalledTimes(1);
  await act(async () => reject(new ApiError({ kind: 'http', status: 409, code: null })));
  await waitFor(() => expect(screen.getByText(/读本或来源已变化/)).toBeVisible());
  expect(screen.getByLabelText('段落文字')).not.toBeDisabled(); expect(screen.queryByRole('button', { name: '核对同一保存请求' })).not.toBeInTheDocument();
});

it('首次空读本保存回包未知，材料引用变为UUID后仍保留原请求；确认后以新规范区块继续编辑', async () => {
  const original = unit({ ref: `${workRef(1)}.image.xhs:dossier-note:image:1`, slotKey: 'xhs:dossier-note:image:1', currentRevisionRef: null, reading: null, blocks: [] });
  const payload = (material: DossierMaterial) => { const p = parseDossierPayload(dossierJson(1)); p.dossier.materials = [material]; return p; };
  vi.mocked(api.reviewReading).mockRejectedValueOnce(new TypeError('receipt lost after commit')).mockImplementation(async (_work, _domain, locator, command) => ({ ...receipt(original, command), unitRef: locator }));
  const props = { work: workRef(1), domain: DOMAIN, onOpenImage: vi.fn(), onApplied: vi.fn() };
  const view = render(<MaterialsSection payload={payload(original)} {...props} />);
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '修正文字' })); fillMetadata();
  await user.click(screen.getByRole('button', { name: '添加区块' }));
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '人工首次录入' } });
  await user.click(screen.getByRole('button', { name: '保存修正' })); await screen.findByRole('button', { name: '核对同一保存请求' });
  const frozen = structuredClone(vi.mocked(api.reviewReading).mock.calls[0]![3]);
  const persisted: DossierMaterial = { ...original, ref: '00000000-0000-4000-8000-000000000789', currentRevisionRef: 'saved-reading', blocks: [{ ref: 'server-normalized-block', kind: 'paragraph', text: '人工首次录入' }] };
  view.rerender(<MaterialsSection payload={payload(persisted)} {...props} />);
  expect(screen.getByRole('button', { name: '核对同一保存请求' })).toBeVisible();
  expect(screen.getByLabelText('段落文字')).toHaveValue('人工首次录入'); expect(screen.getByLabelText('段落文字')).toBeDisabled();
  expect(api.reviewReading).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole('button', { name: '核对同一保存请求' })); await screen.findByText('修正已保存。');
  expect(vi.mocked(api.reviewReading).mock.calls[1]!.slice(2)).toEqual([original.ref, frozen]);
  await waitFor(() => expect(screen.getByLabelText('段落文字')).toBeEnabled());
  fireEvent.change(screen.getByLabelText('段落文字'), { target: { value: '继续修正' } });
  await user.click(screen.getByRole('button', { name: '保存修正' }));
  const next = vi.mocked(api.reviewReading).mock.calls[2]!;
  expect(next[2]).toBe(persisted.ref); expect(next[3].commandRef).not.toBe(frozen.commandRef);
  expect(next[3]).toMatchObject({ expectedCurrentRevisionRef: 'saved-reading', editedBlocks: [{ ref: 'server-normalized-block', text: '继续修正' }] });
});
