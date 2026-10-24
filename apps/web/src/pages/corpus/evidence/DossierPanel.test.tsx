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
  mount();
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
  expect(screen.getAllByText('表达者未确认')).toHaveLength(1);
  expect(screen.getByText('〔局部无法辨认〕')).toBeVisible();
  expect(screen.queryByText('图片原始页序未知')).not.toBeInTheDocument();
});
