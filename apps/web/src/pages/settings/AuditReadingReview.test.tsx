import { act, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { AuditReadingReview } from './AuditReadingReview';
import { fetchAuditDetail } from './ocrAuditApi';
import { createFetchMock, deferred, fail, ok, renderWithProviders, type FetchKit, type Reply } from '../corpus/evidence/testKit';

const reading = '80000000-0000-4000-8000-000000000001';
const sample = '81000000-0000-4000-8000-000000000001';
const base = '/api/local/ocr-audit';
let kit: FetchKit;
function detail() { return { sampleRef: sample, originalAssetUrl: null, modelId: 'synthetic-vision', modelOpinion: { regions: [{ text: '抽检模型的第二意见' }] }, issues: [{ issue_ref: '82000000-0000-4000-8000-000000000001', suggestion: { discrepancy: 'synthetic' } }], reviews: [] }; }
function receipt(body: unknown) { const command = body as { decision: string; materialReviewCommandRef: string | null }; return { reviewRef: '83000000-0000-4000-8000-000000000001', readingRef: reading, revision: 1, decision: command.decision, materialReviewCommandRef: command.materialReviewCommandRef }; }
beforeEach(() => { kit = createFetchMock(); kit.install(); kit.route('GET', `${base}/readings/${reading}`, ok(detail())); });
afterEach(() => vi.unstubAllGlobals());

it('最新意见恢复待核验时，不沿用旧确认版本的核验点数', async () => {
  kit.route('GET', `${base}/readings/${reading}`, ok({ ...detail(), reviews: [
    { revision: 1, decision: 'confirmed', actor: '合成审核人', reason: '早先确认', metrics: { bodyMissing: { numerator: 2, denominator: 5 } } },
    { revision: 2, decision: 'pending', actor: '合成审核人', reason: '新证据需要重新核对', metrics: { bodyMissing: { numerator: 2, denominator: 5 } } },
  ] }));
  const result = await fetchAuditDetail(reading);
  expect(result.revision).toBe(2);
  expect(result.humanCounts.bodyMissing).toEqual({ numerator: 0, denominator: 0 });
  expect(result.reviews.map(review => review.decision)).toEqual(['confirmed', 'pending']);
});

it.each(['confirmed', 'dismissed', 'pending'])('仅记录 %s 抽检意见；不产生读本替换或文本写入', async decision => {
  kit.route('POST', `${base}/reviews`, call => ({ status: 200, body: receipt(call.body) }));
  const view = renderWithProviders(<AuditReadingReview readingRef={reading} />);
  expect(kit.calls).toHaveLength(0);
  await view.user.click(screen.getByRole('button', { name: '审阅抽检意见' }));
  await screen.findByText('抽检模型的第二意见');
  await view.user.selectOptions(screen.getByLabelText('审阅决定'), decision);
  await view.user.type(screen.getByLabelText('审核人'), '合成审核人');
  await view.user.type(screen.getByLabelText('核对依据'), '已对照原图');
  expect(screen.queryByLabelText(/替换文字|修正版|段落文字/)).not.toBeInTheDocument();
  await view.user.click(screen.getByRole('button', { name: '保存审阅意见' }));
  await screen.findByText('抽检审阅已保存，第 1 版。');
  const writes = kit.calls.filter(call => call.method === 'POST');
  expect(writes).toHaveLength(1); expect(writes[0]?.path).toBe(`${base}/reviews`);
  expect(writes[0]?.body).toMatchObject({ sampleRef: sample, expectedRevision: 0, decision, issueRef: null, materialReviewCommandRef: null, actor: '合成审核人', reason: '已对照原图' });
  expect(writes[0]?.body).not.toHaveProperty('field'); expect(writes[0]?.body).not.toHaveProperty('text'); expect(writes[0]?.body).not.toHaveProperty('editedBlocks');
});

it.each(['network', 503])('抽检审阅回执不确定（%s）后，收起再打开仍复用完整的同一请求', async failure => {
  kit.route('POST', `${base}/reviews`, () => failure === 'network' ? 'network' : { status: 503, body: { error: 'storage_unavailable' } }, call => ({ status: 200, body: receipt(call.body) }));
  const view = renderWithProviders(<AuditReadingReview readingRef={reading} />);
  await view.user.click(screen.getByRole('button', { name: '审阅抽检意见' })); await screen.findByText('抽检模型的第二意见');
  await view.user.type(screen.getByLabelText('审核人'), '合成审核人'); await view.user.type(screen.getByLabelText('核对依据'), '确认不了的地方保持待核验');
  await view.user.click(screen.getByRole('button', { name: '保存审阅意见' })); await screen.findByRole('button', { name: '核对同一审阅请求' });
  expect(screen.getByLabelText('审核人')).toBeDisabled();
  await view.user.click(screen.getByRole('button', { name: '收起抽检意见' })); await view.user.click(screen.getByRole('button', { name: '审阅抽检意见' }));
  await view.user.click(screen.getByRole('button', { name: '核对同一审阅请求' })); await screen.findByText('抽检审阅已保存，第 1 版。');
  const commands = kit.of('POST', `${base}/reviews`).map(call => call.body); expect(commands).toHaveLength(2); expect(commands[0]).toEqual(commands[1]);
});

it('已知来源受限后撤下已缓存的第二意见与审阅表单', async () => {
  kit.route('GET', `${base}/readings/${reading}`, ok(detail()), fail(403, 'source_restricted'));
  const view = renderWithProviders(<AuditReadingReview readingRef={reading} />);
  await view.user.click(screen.getByRole('button', { name: '审阅抽检意见' })); await screen.findByText('抽检模型的第二意见');
  await act(async () => { await view.client.invalidateQueries({ queryKey: ['ocr-audit-detail', reading] }); });
  await screen.findByRole('alert'); expect(screen.queryByText('抽检模型的第二意见')).not.toBeInTheDocument(); expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
});

it('迟到的旧读本审阅回执不进入下一读本界面', async () => {
  const pending = deferred<Reply>(); kit.route('POST', `${base}/reviews`, () => pending.promise);
  const view = renderWithProviders(<AuditReadingReview readingRef={reading} />);
  await view.user.click(screen.getByRole('button', { name: '审阅抽检意见' })); await screen.findByText('抽检模型的第二意见');
  await view.user.type(screen.getByLabelText('审核人'), '合成审核人'); await view.user.type(screen.getByLabelText('核对依据'), '合成依据');
  await view.user.click(screen.getByRole('button', { name: '保存审阅意见' })); await waitFor(() => expect(kit.of('POST', `${base}/reviews`)).toHaveLength(1));
  view.unmount(); renderWithProviders(<AuditReadingReview readingRef="next-reading" />);
  await act(async () => pending.resolve({ status: 200, body: receipt(kit.of('POST', `${base}/reviews`)[0]?.body) }));
  expect(screen.queryByText('抽检审阅已保存，第 1 版。')).not.toBeInTheDocument(); expect(screen.getByRole('button', { name: '审阅抽检意见' })).toBeInTheDocument();
});
