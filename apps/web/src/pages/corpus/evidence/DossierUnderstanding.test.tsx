import { QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { DossierDocument } from './DossierDocument';
import { DossierUnderstanding } from './DossierUnderstanding';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
import { newClient } from './testKit';

const payload = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, {
  kind: 'body', blocks: [{ ref: 'original', kind: 'paragraph', text: '合成原始材料保持完整。' }],
})] }));
const work = payload.dossier.workRef;
function saved() {
  return { state: 'succeeded', failureCode: null, result: {
    card: { schemaVersion: 'content-card.v0.2', workRef: work,
      core: { summary: { text: '来自同一张卡片的整篇摘要。' }, knowledgeUnits: [
        { id: 'topic', unitType: 'discussion', statement: '同一Core中的议题。' },
        { id: 'step', unitType: 'provision', statement: '图片提供的具体步骤。' },
      ] }, sourceSnapshot: { coverageMode: 'partial' } },
    imageContributions: [{ inputCitationId: 'image_1', status: 'unclear', summary: '图片底部小字不清楚。' }],
  } };
}
const ready = { state: 'ready', kind: 'image_text', gaps: [], scopeNote: null };
const videoScope = '本次依据口播与已提取的画面文字，未覆盖整段视频的全部画面。';
function response(analysis: unknown, selected = work, materialReadiness: unknown = ready) {
  return new Response(JSON.stringify({ contract: 'content-understanding.read.v1', workRef: selected, analysis, materialReadiness }), { headers: { 'Content-Type': 'application/json' } });
}
function mount(active = true) {
  const client = newClient();
  const element = (domain = 'domain', revision = 'r1', asOf = payload.dossier.asOf) => <QueryClientProvider client={client}><DossierUnderstanding domain={domain} work={work} revision={revision} asOf={asOf} active={active}/></QueryClientProvider>;
  const view = render(element());
  return { ...view, change: (domain: string, revision: string, asOf = payload.dossier.asOf) => view.rerender(element(domain, revision, asOf)) };
}
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it('展示同一Card的摘要与知识单元，图片方法保留，打开与刷新只读取', async () => {
  const fetch = vi.fn(async () => response(saved())); vi.stubGlobal('fetch', fetch);
  mount();
  expect(await screen.findByText('来自同一张卡片的整篇摘要。')).toBeVisible();
  expect(screen.getByText('图片提供的具体步骤。')).toBeVisible();
  expect(screen.getByText(/本次按已取得的材料理解/)).toBeVisible();
  expect(screen.getByText(/图片底部小字不清楚/)).toBeVisible();
  expect(fetch).toHaveBeenCalledTimes(1);
  const [path, init] = fetch.mock.calls[0]! as unknown as [string, RequestInit];
  expect(path).toContain('/content-understanding/latest?');
  expect(new URL(path, 'http://localhost').searchParams.get('asOf')).toBe(payload.dossier.asOf);
  expect(init.method ?? 'GET').toBe('GET');
  expect(screen.queryByRole('button', { name: /分析|生成/ })).not.toBeInTheDocument();
});
it('未分析时不虚构结果，原始材料仍可完整阅读', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => response(null)));
  render(<QueryClientProvider client={newClient()}><DossierDocument payload={payload} domain="domain" active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/></QueryClientProvider>);
  expect(await screen.findByText(/这篇尚无当前材料的理解结果/)).toBeVisible();
  expect(screen.getByText('合成原始材料保持完整。')).toBeVisible();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it.each([
  ['failed', 'ai_output_invalid', '本次理解未完成，原始材料不受影响。'],
  ['failed', 'ai_dispatch_unknown', '本次处理结果尚未确认，以下仍可阅读原始材料。'],
  ['cancelled', 'batch_cancelled', '本次理解已取消，原始材料不受影响。'],
])('处理状态 %s/%s 不伪装成功', async (state, failureCode, message) => {
  vi.stubGlobal('fetch', vi.fn(async () => response({ state, failureCode, result: null })));
  mount(); expect(await screen.findByText(message)).toBeVisible();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it.each(['revision', 'domain'])('切换%s立即移除旧结果，再读取当前材料', async key => {
  let resolve!: (value: Response) => void;
  const fetch = vi.fn().mockResolvedValueOnce(response(saved())).mockImplementationOnce(() => new Promise<Response>(done => { resolve = done; }));
  vi.stubGlobal('fetch', fetch); const view = mount();
  await screen.findByText('来自同一张卡片的整篇摘要。');
  view.change(key === 'domain' ? 'other-domain' : 'domain', key === 'revision' ? 'r2' : 'r1');
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
  resolve(response(null));
  expect(await screen.findByText(/这篇尚无当前材料的理解结果/)).toBeVisible();
});
it('材料未变但底稿重新读取后，空理解重新读取为已保存结果', async () => {
  const fetch = vi.fn().mockResolvedValueOnce(response(null)).mockResolvedValueOnce(response(saved()));
  vi.stubGlobal('fetch', fetch);
  const view = mount();
  await screen.findByText(/这篇尚无当前材料的理解结果/);
  const asOf = '2026-11-10T15:00:00Z';
  view.change('domain', 'r1', asOf);
  expect(await screen.findByText('来自同一张卡片的整篇摘要。')).toBeVisible();
  expect(fetch).toHaveBeenCalledTimes(2);
  expect(new URL(fetch.mock.calls[1]![0], 'http://localhost').searchParams.get('asOf')).toBe(asOf);
});
it('理解读取失败后重试仍绑定原底稿截点，不改取另一版材料', async () => {
  const fetch = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(response(saved()));
  vi.stubGlobal('fetch', fetch); mount();
  fireEvent.click(await screen.findByRole('button', { name: '重试读取' }));
  await screen.findByText('来自同一张卡片的整篇摘要。');
  expect(fetch).toHaveBeenCalledTimes(2);
  for (const [path] of fetch.mock.calls) expect(new URL(path, 'http://localhost').searchParams.get('asOf')).toBe(payload.dossier.asOf);
});
it('请求错误只允许重试读取，不发起模型调用', async () => {
  const fetch = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(response(null));
  vi.stubGlobal('fetch', fetch); mount();
  fireEvent.click(await screen.findByRole('button', { name: '重试读取' }));
  await screen.findByText(/这篇尚无当前材料的理解结果/);
  expect(fetch).toHaveBeenCalledTimes(2);
  for (const [path, init] of fetch.mock.calls as [string, RequestInit][]) {
    expect(path).toContain('/latest?'); expect(init.method ?? 'GET').toBe('GET');
  }
});
it('别篇作品或成功状态缺失Card均不展示', async () => {
  const fetch = vi.fn().mockResolvedValueOnce(response(saved(), 'another-work')).mockResolvedValueOnce(response({ state: 'succeeded', result: null }));
  vi.stubGlobal('fetch', fetch); mount();
  fireEvent.click(await screen.findByRole('button', { name: '重试读取' }));
  await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
  expect(await screen.findByRole('button', { name: '重试读取' })).toBeVisible();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it('材料待补齐时逐项显示原因，原始材料仍可阅读，打开不生成', async () => {
  const fetch = vi.fn(async () => response(null, work, {
    ...ready, state: 'blocked', gaps: [
      { code: 'image_missing', state: 'blocked', message: '第 2 张正文原图尚未取得。', slotKey: 'image_2' },
      { code: 'image_count_unknown', state: 'blocked', message: '正文图片总数尚未确认。', slotKey: null },
    ],
  }));
  vi.stubGlobal('fetch', fetch);
  render(<QueryClientProvider client={newClient()}><DossierDocument payload={payload} domain="domain" active busy={false} refreshFailed={false} onRefresh={() => {}} onUnavailable={() => {}}/></QueryClientProvider>);
  expect(await screen.findByText('材料待补齐')).toBeVisible();
  expect(screen.getByText('第 2 张正文原图尚未取得。')).toBeVisible();
  expect(screen.getByText('正文图片总数尚未确认。')).toBeVisible();
  expect(screen.getByText('合成原始材料保持完整。')).toBeVisible();
  expect(screen.queryByRole('button', { name: /分析|生成/ })).not.toBeInTheDocument();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect((fetch.mock.calls[0] as unknown as [string, RequestInit])[1].method ?? 'GET').toBe('GET');
});
it('视频材料准备中显示口播与画面处理原因，不假装缺失或理解完成', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => response(null, work, {
    state: 'preparing', kind: 'video', scopeNote: videoScope, gaps: [
      { code: 'asr_pending', state: 'preparing', message: '口播转写正在处理。', slotKey: 'video' },
      { code: 'frames_pending', state: 'preparing', message: '画面文字正在提取。', slotKey: 'video' },
    ],
  })));
  mount();
  expect(await screen.findByText('材料准备中')).toBeVisible();
  expect(screen.getByText('口播转写正在处理。')).toBeVisible();
  expect(screen.getByText('画面文字正在提取。')).toBeVisible();
  expect(screen.queryByText(videoScope)).not.toBeInTheDocument();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it('材料已准备仍明确尚无理解结果，空缺口不加不适用或无字警告', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => response(null)));
  mount();
  expect(await screen.findByText('材料已准备')).toBeVisible();
  expect(screen.getByText(/这篇尚无当前材料的理解结果/)).toBeVisible();
  expect(screen.queryByRole('list')).not.toBeInTheDocument();
  expect(screen.queryByText(/不适用|没有文字|OCR/)).not.toBeInTheDocument();
});
it('已保存视频只说明一次实际理解范围，不重复泛化缺口提示', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => response(saved(), work, { ...ready, kind: 'video', scopeNote: videoScope })));
  mount();
  expect(await screen.findByText(videoScope)).toBeVisible();
  expect(screen.getAllByText(videoScope)).toHaveLength(1);
  expect(screen.queryByText(/本次按已取得的材料理解/)).not.toBeInTheDocument();
  expect(screen.getByText(/图片底部小字不清楚/)).toBeVisible();
});
it.each([
  null, {}, { ...ready, state: 'complete' }, { ...ready, kind: 'movie' },
  { ...ready, gaps: null }, { ...ready, scopeNote: 3 },
  { ...ready, scopeNote: undefined },
  { ...ready, gaps: [{ code: 'missing', state: 'ready', message: '缺图', slotKey: null }] },
  { ...ready, gaps: [{ code: 'missing', state: 'blocked', message: 3, slotKey: null }] },
  { ...ready, gaps: [{ code: 'missing', state: 'blocked', message: '缺图', slotKey: 2 }] },
])('材料合同异常不展示已准备或成功结果：%j', async readiness => {
  vi.stubGlobal('fetch', vi.fn(async () => response(saved(), work, readiness)));
  mount();
  expect(await screen.findByRole('button', { name: '重试读取' })).toBeVisible();
  expect(screen.queryByText('材料已准备')).not.toBeInTheDocument();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it('理解读取合同缺少材料就绪字段时拒绝展示成功结果', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ contract: 'content-understanding.read.v1', workRef: work, analysis: saved() }), { headers: { 'Content-Type': 'application/json' } })));
  mount();
  expect(await screen.findByRole('button', { name: '重试读取' })).toBeVisible();
  expect(screen.queryByText('来自同一张卡片的整篇摘要。')).not.toBeInTheDocument();
});
it('底稿不活跃时不请求', () => {
  const fetch = vi.fn(); vi.stubGlobal('fetch', fetch); mount(false);
  expect(fetch).not.toHaveBeenCalled();
});
