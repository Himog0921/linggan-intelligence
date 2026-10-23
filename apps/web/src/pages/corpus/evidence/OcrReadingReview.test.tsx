import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, it, vi } from 'vitest';
import { OcrRecognitionResults } from './OcrRecognitionResults';
import { parseDossierPayload } from './dossierDecode';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import type { DossierBlock, ReadingCandidate } from './dossierTypes';
import type { OcrRecognitionImage, OcrRecognitionRun } from './ocrRecognitionTypes';
import * as api from './dossierApi';

vi.mock('./dossierApi', async original => ({ ...await original<typeof api>(), fetchCandidates: vi.fn(), fetchDossier: vi.fn(), fetchReadingVersion: vi.fn(), reviewReading: vi.fn() }));
afterEach(() => vi.resetAllMocks());
function setup(blocks: DossierBlock[], originalUrl: string | null = '/api/local/media/original') {
  const candidateVersion = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { currentRevisionRef: 'candidate', blocks, originalUrl })] })).dossier.materials[0]!;
  const candidate: ReadingCandidate = { unitRef: candidateVersion.ref, slotKey: candidateVersion.slotKey, candidateRevisionRef: 'candidate', currentRevisionRef: null, blocks, issues: [], generatedBy: 'cloud_vision', verification: 'unchecked', createdAt: '2026-10-23T08:01:00Z', review: null };
  const image: OcrRecognitionImage = { imageRef: 'saved-image', ordinal: 1, role: 'cover', state: 'succeeded', slotKey: candidate.slotKey, aliases: [], originalUrl, candidateRevisionRef: candidate.candidateRevisionRef, blocks, issues: [], rawResultRef: 'raw', rawResultSha256: null, rawJson: '{"regions":[]}', invocationRef: 'call', finishReason: 'stop', usage: null, estimatedCostUsd: null };
  const run: OcrRecognitionRun = { runRef: 'same-run', workRef: workRef(1), state: 'succeeded', completedImages: 1, totalImages: 1, currentStage: '已完成', modelLabel: '合成模型', unavailableReason: null, images: [image], pendingCount: 0, failureCode: null, batchRef: 'batch', batchRevision: 1, cancelRequested: false, scope: null };
  // Only the requested source/current versions are needed, even when the dossier hides this unit.
  vi.mocked(api.fetchDossier).mockResolvedValue(parseDossierPayload(dossierJson(1, { materials: [] })));
  vi.mocked(api.fetchCandidates).mockResolvedValue([candidate]); vi.mocked(api.fetchReadingVersion).mockResolvedValue({ workRef: workRef(1), currentRevisionRef: null, material: candidateVersion });
  const onSaved = vi.fn(); const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><OcrRecognitionResults run={run} domain={DOMAIN} onSaved={onSaved} /></QueryClientProvider>);
  return { onSaved, candidate, candidateVersion, image, client };
}
function metadata() {
  fireEvent.change(screen.getByLabelText('审核人'), { target: { value: '合成核对人' } });
  fireEvent.change(screen.getByLabelText('核对依据'), { target: { value: '已对照受控原图逐项核验' } });
}
it.each(['title_only', 'empty'] as const)('saved %s cover enters the same canonical panel and persists the full explicit decision', async kind => {
  const blocks: DossierBlock[] = kind === 'title_only' ? [{ ref: 'title-block', kind: 'heading', text: '仅有封面主标题' }] : [];
  const view = setup(blocks); const user = userEvent.setup();
  vi.mocked(api.reviewReading).mockImplementation(async (_work, _domain, unit, command) => {
    const current = { ...view.candidateVersion, currentRevisionRef: 'saved', blocks: command.editedBlocks ?? blocks, reading: { generatedBy: 'human_edit' as const, adoption: 'human_edited' as const, verification: 'unchecked' as const, ruleVersion: 'dossier-content.v2', review: { commandRef: command.commandRef, actor: command.actor, reason: command.reason, action: command.action, reviewedAt: '2026-10-23T09:00:00Z' } } };
    vi.mocked(api.fetchCandidates).mockResolvedValue([]); vi.mocked(api.fetchReadingVersion).mockImplementation(async (_w, _d, reading) => ({ workRef: workRef(1), currentRevisionRef: 'saved', material: reading === 'saved' ? current : view.candidateVersion }));
    return { commandRef: command.commandRef, unitRef: unit, candidateRevisionRef: command.candidateRevisionRef, action: command.action, currentRevisionRef: 'saved', adoption: 'human_edited', verification: 'unchecked', reviewRef: null };
  });
  const entry = screen.getByRole('button', { name: '人工核对与采用' });
  expect(api.fetchDossier).not.toHaveBeenCalled(); await user.click(entry);
  expect(await screen.findByRole('combobox', { name: '核对版本' })).toHaveValue('candidate');
  expect(within(screen.getByRole('region', { name: '当前采用读本' })).getByText('尚未采用读本。')).toBeVisible();
  expect(api.fetchReadingVersion).toHaveBeenCalledWith(workRef(1), DOMAIN, 'candidate', expect.any(AbortSignal));
  expect(screen.getByRole('button', { name: '保留当前读本' })).toBeDisabled();
  await user.click(screen.getByText('对照原图 · 第 1 页')); fireEvent.load(screen.getByRole('img', { name: '第 1 页原图' }));
  await user.click(screen.getByRole('button', { name: '逐块修正' }));
  if (kind === 'empty') await user.click(screen.getByRole('button', { name: '添加区块' }));
  const text = `${kind}：原图完整核对文本。${'不得截断。'.repeat(4000)}`;
  const field = screen.getByLabelText(kind === 'empty' ? '段落文字' : '标题文字');
  expect(field).not.toHaveAttribute('maxlength'); fireEvent.change(field, { target: { value: text } }); metadata();
  await user.click(screen.getByRole('button', { name: '保存完整修正版' })); await screen.findByText('完整修正版已保存并采用。');
  const call = vi.mocked(api.reviewReading).mock.calls[0]!;
  expect(call.slice(0, 3)).toEqual([workRef(1), DOMAIN, view.candidate.unitRef]);
  expect(call[3]).toMatchObject({ action: 'edit', expectedCurrentRevisionRef: null, candidateRevisionRef: 'candidate', actor: '合成核对人', reason: '已对照受控原图逐项核验', editedBlocks: [{ text }] });
  await waitFor(() => expect(view.onSaved).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(within(screen.getByRole('region', { name: '当前采用读本' })).getByText(text)).toBeVisible());
  expect(screen.getByText(/当前读本审核：合成核对人/)).toBeVisible();
  expect(api.fetchDossier).not.toHaveBeenCalled();
});
it('a failed original image keeps preview text but disables review actions', async () => {
  setup([{ ref: 'heading', kind: 'heading', text: '可保留阅读的标题' }]); const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '人工核对与采用' })); await screen.findByLabelText('核对版本'); metadata();
  await user.click(screen.getByText('对照原图 · 第 1 页'));
  fireEvent.error(screen.getByRole('img', { name: '第 1 页原图' }));
  expect(screen.getByRole('button', { name: '采用此候选' })).toBeDisabled(); expect(screen.getByRole('button', { name: '保持待核验' })).toBeDisabled();
  expect(screen.getAllByText('可保留阅读的标题').length).toBeGreaterThan(0); expect(api.reviewReading).not.toHaveBeenCalled();
});
it('a hidden source unit fetches its actual current version without presenting the candidate as current', async () => {
  const view = setup([{ ref: 'candidate-heading', kind: 'heading', text: '尚未采用的标题' }]);
  vi.mocked(api.fetchCandidates).mockResolvedValue([{ ...view.candidate, currentRevisionRef: 'real-current' }]);
  vi.mocked(api.fetchReadingVersion).mockImplementation(async (_work, _domain, ref) => ({ workRef: workRef(1), currentRevisionRef: 'real-current', material: ref === 'real-current' ? { ...view.candidateVersion, currentRevisionRef: ref, blocks: [{ ref: 'current-block', kind: 'paragraph', text: '真正当前正文' }] } : view.candidateVersion }));
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '人工核对与采用' }));
  const current = await screen.findByRole('region', { name: '当前采用读本' });
  expect(within(current).getByText('真正当前正文')).toBeVisible(); expect(within(current).queryByText('尚未采用的标题')).not.toBeInTheDocument();
  expect(api.fetchReadingVersion).toHaveBeenCalledWith(workRef(1), DOMAIN, 'real-current', expect.any(AbortSignal)); expect(api.reviewReading).not.toHaveBeenCalled();
});
it('an external current B supersedes saved receipt A, including a unit absent from the deduplicated dossier', async () => {
  const view = setup([{ ref: 'candidate-heading', kind: 'heading', text: '先采用的候选 A' }]);
  let currentRef: string | null = null;
  vi.mocked(api.fetchReadingVersion).mockImplementation(async (_work, _domain, ref) => ({ workRef: workRef(1), currentRevisionRef: currentRef, material: ref === 'external-b' ? { ...view.candidateVersion, currentRevisionRef: ref, blocks: [{ ref: 'external-block', kind: 'paragraph', text: '随后由其他审核人采用的 B' }] } : view.candidateVersion }));
  vi.mocked(api.reviewReading).mockImplementation(async (_w, _d, unit, command) => {
    if (command.action === 'adopt') { currentRef = 'candidate'; vi.mocked(api.fetchCandidates).mockResolvedValue([]); }
    return { commandRef: command.commandRef, unitRef: unit, candidateRevisionRef: command.candidateRevisionRef, action: command.action, currentRevisionRef: currentRef, adoption: 'human_adopted', verification: 'unchecked', reviewRef: null };
  });
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '人工核对与采用' })); await screen.findByLabelText('核对版本'); metadata();
  await user.click(screen.getByRole('button', { name: '采用此候选' })); await screen.findByText('候选读本已采用。');
  const current = screen.getByRole('region', { name: '当前采用读本' }); await within(current).findByText('先采用的候选 A');
  currentRef = 'external-b';
  await user.click(screen.getByRole('button', { name: '刷新核对版本' }));
  expect(await within(current).findByText('随后由其他审核人采用的 B')).toBeVisible(); expect(within(current).queryByText('先采用的候选 A')).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '保留当前读本' })).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '使用最新读本' })); await user.click(screen.getByRole('button', { name: '保留当前读本' })); await screen.findByText('已记录保留当前读本。');
  expect(vi.mocked(api.reviewReading).mock.calls[1]?.[3]).toMatchObject({ action: 'keep', expectedCurrentRevisionRef: 'external-b', candidateRevisionRef: 'external-b' });
  expect(api.fetchDossier).not.toHaveBeenCalled();
});
