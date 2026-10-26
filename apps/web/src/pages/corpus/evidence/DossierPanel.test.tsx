import { screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DossierPanel } from './DossierPanel';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';
const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
let kit: FetchKit;
beforeEach(() => { kit = createFetchMock(); kit.install(); });
afterEach(() => vi.unstubAllGlobals());
function mount() { return renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active onOpenImage={vi.fn()} onApplied={vi.fn()} />); }

it.each([401,403,404,410])('刷新得到 %i 后撤下旧内容与导出入口', async status => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '已有缓存但随后失去权限的正文' })), fail(status, 'source_restricted'));
  const view = mount(); await screen.findByText('已有缓存但随后失去权限的正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText('研究底稿暂时不可读。');
  expect(screen.queryByText('已有缓存但随后失去权限的正文')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '导出 JSON' })).not.toBeInTheDocument(); expect(screen.queryByRole('button', { name: '导出 Markdown' })).not.toBeInTheDocument();
});
it('临时服务故障保留并标记最后读到的版本，不把故障表示为无内容', async () => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '最后成功读取的完整正文' })), fail(503, 'temporarily_unavailable'));
  const view = mount(); await screen.findByText('最后成功读取的完整正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText(/刷新未完成，当前保留上一次读到的版本/);
  expect(screen.getByText('最后成功读取的完整正文')).toBeVisible(); expect(screen.getByRole('button', { name: '导出 JSON' })).toBeEnabled();
});

it('总页数未知时仍展示已处理页数，未知页码图片有独立编号', async () => {
  kit.route('GET', path, ok(dossierJson(1, {
    materials: [dossierMaterialJson(1, { page: null }), dossierMaterialJson(2, { page: null })],
    coverage: { imageInventory: { basis: 'observed_set', expectedPages: null, acquiredPages: 2, processedPages: 1, missingPages: [], unknownOrder: true } },
  })));
  const view = mount();
  await screen.findByRole('heading', { name: '图片 1 · 页码未知' });
  await view.user.click(screen.getByText('材料详情'));
  expect(await screen.findByText('已取得 2 张图片，已有读本 1 页，原作总页数未知。')).toBeVisible();
  expect(screen.getByRole('heading', { name: '图片 1 · 页码未知' })).toBeVisible();
  expect(screen.getByRole('heading', { name: '图片 2 · 页码未知' })).toBeVisible();
  expect(screen.queryByRole('heading', { name: '第 1 页' })).not.toBeInTheDocument();
});
it('同一问题就近展示一次，原文默认可见，来源诊断保持折叠', async () => {
  const issue = { key: 'word', code: 'glyph', message: '数字需要对照原图核验', resolution: 'reading_edit' };
  kit.route('GET', path, ok(dossierJson(1, {
    materials: [dossierMaterialJson(1, { blocks: [{ ref: 'paragraph', kind: 'paragraph', text: '完整的原文阅读内容', issues: [issue] }], issues: [issue] })],
    coverage: { issues: [issue] },
  })));
  mount();
  expect(await screen.findByText('完整的原文阅读内容')).toBeVisible();
  expect(screen.getAllByText(issue.message)).toHaveLength(1);
  expect(screen.getByText('来源与读本版本').closest('details')).not.toHaveAttribute('open');
});
it('与检查器标题相同时不复读平台标题，空材料只保留一处来源状态', async () => {
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1, { sourceState: 'not_acquired', blocks: [], reading: null, currentRevisionRef: null, originalUrl: null })] })));
  renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} displayedTitle="合成作品标题" active onOpenImage={vi.fn()} onApplied={vi.fn()} />);
  expect(await screen.findByText('来源尚未取得，不能判断原作是否没有内容。')).toBeVisible();
  expect(screen.queryByText('合成作品标题')).not.toBeInTheDocument();
  expect(screen.queryByText('尚未取得', { exact: true })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '核对与修正' })).not.toBeInTheDocument();
});

it('重复缺口、页码提示与连续表达者标签不挤占正文', async () => {
  kit.route('GET', path, ok(dossierJson(1, {
    materials: [dossierMaterialJson(1, { page: null, blocks: [
      { ref: 'a', kind: 'paragraph', text: '第一段原文', speaker: 'unconfirmed' },
      { ref: 'b', kind: 'paragraph', text: '第二段原文', speaker: 'unconfirmed' },
      { ref: 'c', kind: 'gap', text: '〔局部无法辨认〕' },
    ], issues: [{ key: 'order', code: 'page_order_unknown', message: '图片原始页序未知', resolution: 'source_refresh' }] })],
  })));
  mount();
  await screen.findByText('第一段原文');
  expect(screen.queryByText('表达者未确认')).not.toBeInTheDocument();
  expect(document.querySelectorAll('[data-speaker="unconfirmed"]')).toHaveLength(2);
  expect(screen.getByText('〔局部无法辨认〕')).toBeVisible();
  expect(screen.queryByText('图片原始页序未知')).not.toBeInTheDocument();
});

it.each(['audio', 'video_frame'] as const)('%s 的原视频使用音视频链接，不作为图片对照加载', async kind => {
  const originalUrl = '/api/local/media/synthetic-original-video.mp4';
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1, {
    kind, page: null, originalUrl,
    blocks: [{ ref: 'motion-text', kind: 'paragraph', text: '已取得的音视频文字完整保留' }],
  })] })));
  const view = mount();
  expect(await screen.findByText('已取得的音视频文字完整保留')).toBeVisible();
  const link = screen.getByRole('link', { name: '查看音视频原件' });
  expect(link).toHaveAttribute('href', originalUrl);
  expect(link).toHaveAttribute('target', '_blank');
  expect(screen.queryByText(/^对照原图/)).not.toBeInTheDocument();
  expect(screen.queryByText('放大查看原图')).not.toBeInTheDocument();
  expect(view.container.querySelector('img')).toBeNull();
  expect(screen.getByRole('button', { name: '核对与修正' })).toBeVisible();
});

it('已有合格版面时导航展开既有整理面板，原件待取得与待识字独立，导航不执行写入', async () => {
  kit.route('GET', path, ok(dossierJson(1, { materials: [
    dossierMaterialJson(1, { currentRevisionRef: null, reading: null, blocks: [], processing: { state: 'completed', reason: null }, issues: [{ key: 'saved-layout', code: 'existing_result_pending', message: '已有识别结果，待整理。', resolution: 'source_refresh' }] }),
    dossierMaterialJson(2, { sourceState: 'not_acquired', currentRevisionRef: null, reading: null, blocks: [], originalUrl: null }),
  ], plan: { layoutCount: 1, pendingLayouts: 1, needsOcr: 0, needsAcquisition: 1 } })));
  kit.route('GET', `${path}/candidates`, ok({ candidates: [] }));
  const view = mount();
  expect(await screen.findByText('已有识别结果，待整理。')).toBeVisible();
  expect(screen.getAllByText('已有识别结果，待整理。')).toHaveLength(1);
  expect(screen.queryByText('处理完成，暂无可读文字。')).not.toBeInTheDocument();
  const summary = screen.getByText('整理已有识别版面'); const scroll = vi.fn(); summary.scrollIntoView = scroll;
  const maintenance = summary.closest('details'); const details = screen.getByText('材料详情').closest('details');
  expect(maintenance).not.toHaveAttribute('open'); expect(details).not.toHaveAttribute('open');
  await view.user.click(screen.getByRole('button', { name: '核对与修正' }));
  const go = await screen.findByRole('button', { name: '前往整理已有结果' });
  expect(screen.queryByText('尚无可核对的读本，请先完成图片识别。')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
  const reads = kit.calls.length;
  await view.user.click(go);
  expect(maintenance).toHaveAttribute('open'); expect(details).toHaveAttribute('open');
  expect(summary).toHaveFocus(); expect(scroll).toHaveBeenCalledWith({ block: 'center' });
  expect(screen.getByRole('button', { name: '重新整理' })).toBeVisible();
  expect(screen.getByText(/待取得 1 · 待识字 0/)).toBeVisible();
  expect(kit.calls).toHaveLength(reads); expect(kit.calls.every(call => call.method === 'GET')).toBe(true);
});
it('cloud预览仍须显式选择与采用，不因为处理完成而进入当前读本', async () => {
  const material = dossierMaterialJson(1, { currentRevisionRef: null, reading: null, blocks: [], processing: { state: 'completed', reason: null } });
  kit.route('GET', path, ok(dossierJson(1, { materials: [material] })));
  kit.route('GET', `${path}/candidates`, ok({ candidates: [{ unitRef: material.ref, slotKey: material.slotKey, candidateRevisionRef: 'cloud-preview', currentRevisionRef: null,
    blocks: [{ ref: 'preview-block', kind: 'paragraph', text: '仅在候选预览中的文字' }], issues: [], generatedBy: 'cloud_vision', verification: 'unchecked', createdAt: '2026-10-26T00:00:00Z', review: null }] }));
  const view = mount(); await screen.findByText('处理完成，尚无当前读本。');
  expect(screen.queryByText('仅在候选预览中的文字')).not.toBeInTheDocument();
  await view.user.click(screen.getByRole('button', { name: '核对与修正' }));
  const select = await screen.findByLabelText('核对版本');
  expect(screen.queryByRole('button', { name: '前往整理已有结果' })).not.toBeInTheDocument();
  expect(screen.queryByLabelText('审核人')).not.toBeInTheDocument();
  await view.user.selectOptions(select, 'cloud-preview');
  expect(screen.getByText('仅在候选预览中的文字')).toBeVisible();
  expect(screen.getByRole('button', { name: '采用此候选' })).toBeDisabled();
  expect(kit.calls.every(call => call.method === 'GET')).toBe(true);
});
