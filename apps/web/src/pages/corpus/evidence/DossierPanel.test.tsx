import { screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { DossierPanel } from './DossierPanel';
import { DOMAIN, dossierJson, dossierMaterialJson, workRef } from './evidenceFixtures';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';
const path = `/api/local/work-resources/${workRef(1)}/research-dossier`;
let kit: FetchKit;
beforeEach(() => { kit = createFetchMock(); kit.install(); HTMLElement.prototype.scrollIntoView = vi.fn(); });
afterEach(() => vi.unstubAllGlobals());
function mount() { return renderWithProviders(<DossierPanel work={workRef(1)} domain={DOMAIN} active onOpenImage={vi.fn()} onApplied={vi.fn()} />); }

it.each([401,403,404,410])('刷新得到 %i 后撤下旧内容与导出入口', async status => {
  kit.route('GET', path, ok(dossierJson(1, { bodyText: '已有缓存但随后失去权限的正文' })), fail(status, 'source_restricted'));
  const view = mount(); await screen.findByText('已有缓存但随后失去权限的正文');
  await view.user.click(screen.getByRole('button', { name: '刷新底稿' })); await screen.findByText('研究底稿暂时不可读。');
  expect(screen.queryByText('已有缓存但随后失去权限的正文')).not.toBeInTheDocument();
  await waitFor(() => { expect(screen.queryByRole('button', { name: '导出 JSON' })).not.toBeInTheDocument(); expect(screen.queryByRole('button', { name: '导出 Markdown' })).not.toBeInTheDocument(); });
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
  expect(screen.queryByRole('button', { name: '修正文字' })).not.toBeInTheDocument();
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
  await view.user.click(screen.getByRole('button', { name: '查看来源' }));
  const link = screen.getByRole('link', { name: '查看音视频原件' });
  expect(link).toHaveAttribute('href', originalUrl);
  expect(link).toHaveAttribute('target', '_blank');
  expect(screen.queryByText(/^对照原图/)).not.toBeInTheDocument();
  expect(screen.queryByText('放大查看原图')).not.toBeInTheDocument();
  expect(view.container.querySelector('img')).toBeNull();
  expect(screen.getByRole('button', { name: '修正文字' })).toBeVisible();
});

it('页面不再读取旧候选或版面整理，缺少读本仍保留原图入口', async () => {
  const material = dossierMaterialJson(1, { blocks: [], currentRevisionRef: null, reading: null });
  kit.route('GET', path, ok(dossierJson(1, { materials: [material] })));
  mount(); await screen.findByText('对照原图 · 第 1 页');
  expect(screen.queryByRole('button', { name: '前往整理已有结果' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '采用此候选' })).not.toBeInTheDocument();
  expect(kit.of('GET', `${path}/candidates`)).toHaveLength(0);
  expect(kit.of('GET', `${path}/relayer-plan`)).toHaveLength(0);
  expect(kit.of('POST', `${path}/relayer`)).toHaveLength(0);
});
it('主文优先，材料状态和原件默认收起，查看来源会展开并聚焦原图', async () => {
  const gap = dossierMaterialJson(2, { blocks: [], currentRevisionRef: null, reading: null, issues: [{ key: 'wait', code: 'not_processed', message: '此页尚未处理', resolution: 'source_refresh' }] });
  kit.route('GET', path, ok(dossierJson(1, { materials: [dossierMaterialJson(1), gap] })));
  const view = mount(); await screen.findByText('第0行连续的图片正文，完整可用。');
  expect(screen.getByText('材料状态 · 1 项').closest('details')).not.toHaveAttribute('open');
  const originals = screen.getByText('原件与修正 · 2 项').closest('details'); expect(originals).not.toHaveAttribute('open');
  await view.user.click(screen.getAllByRole('button', { name: '查看来源' })[0]!);
  expect(originals).toHaveAttribute('open');
  expect(screen.getByText('对照原图 · 第 1 页').closest('details')).toHaveAttribute('open');
  expect(document.activeElement).toHaveAttribute('data-source-unit-ref', dossierMaterialJson(1).ref);
});
it('作者、发布时间原始表达和精度、观察时间及四类互动按需可查', async () => {
  kit.route('GET', path, ok(dossierJson(1)));
  kit.route('GET', `${path}/metadata`, ok({ metadata: { author: { displayName: '合成作者' }, interaction: { likes: { value: 0 }, comments: { value: null }, collects: { value: 2 }, shares: { value: 3 } }, publication: { publishedAt: '2026-10-23T04:00:00Z', localDate: '2026-10-23', sourceText: '3小时前', sourceField: 'publishTime', sourceKind: 'source_text', precision: 'minute', observedAt: '2026-10-23T07:00:00Z', referenceObservedAt: '2026-10-23T07:00:00Z', sourceObservedAt: '2026-10-23T07:00:00Z', sourceRef: null, parserVersion: 'v1' } } }));
  const view = mount(); await screen.findByText('合成作品标题');
  expect(kit.of('GET', `${path}/metadata`)).toHaveLength(0);
  await view.user.click(screen.getByText('作者、时间与互动'));
  expect(await screen.findByText('合成作者')).toBeVisible();
  expect(screen.getByText('发布时间：3小时前 · 精度：分钟')).toBeVisible();
  expect(screen.getByText('观察时间：2026-10-23 15:00')).toBeVisible();
  expect(screen.getByText('点赞 0 · 评论 未知 · 收藏 2 · 分享 3')).toBeVisible();
  expect(screen.getByText('观察与来源诊断').closest('details')).not.toHaveAttribute('open');
  expect(kit.of('GET', `${path}/metadata`)[0]!.search.get('sections')).toBe('author,interaction,publication');
});
