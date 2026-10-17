import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOMAIN, listJson, shellJson, workRef } from './evidenceFixtures';
import { LIST_PATH, SHELL_PATH, detailPathOf, items, pageRoutes, pageUrl, renderEvidencePage, stubViewport } from './pageKit';
import { createFetchMock, fail, ok, type FetchKit } from './testKit';

let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.install();
  sessionStorage.clear();
  localStorage.clear();
  stubViewport();
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const listCalls = () => kit.of('GET', LIST_PATH);
const aside = () => screen.getByRole('complementary', { name: /合成作品|请选择/ });
const rowButton = (n: number) => screen.getByRole('button', { name: `合成作品 ${n}` });
const addressAfterFirstSelection = (extra = '') => `/corpus/evidence?domain=${DOMAIN}${extra}&work=${workRef(1)}`;

async function opened(search?: string) {
  const view = renderEvidencePage(pageUrl(search));
  await screen.findByRole('button', { name: '合成作品 1' });
  return view;
}

describe('打开页面', () => {
  it('读外壳与列表，自动选中第一项并替换地址（不新增历史），再读它的详情；全程只有读取请求', async () => {
    pageRoutes(kit);
    const view = await opened();
    await waitFor(() => expect(view.location()).toBe(addressAfterFirstSelection()));
    expect(view.router.state.historyAction).toBe('REPLACE');
    await waitFor(() => expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 1' })).toBeInTheDocument());
    expect(kit.of('GET', detailPathOf(1))).toHaveLength(1);
    expect(kit.of('GET', detailPathOf(1))[0]?.search.get('domain')).toBe(DOMAIN);
    expect(kit.calls.every(call => call.method === 'GET')).toBe(true);
    expect(document.title).toBe('证据库 · 语料');
  });

  it('列表请求只带领域；没有排序、读取窗口这类已移除的参数（DR-1）', async () => {
    pageRoutes(kit);
    await opened();
    const call = listCalls()[0]!;
    expect([...call.search.keys()]).toEqual(['domain']);
    expect(call.search.get('domain')).toBe(DOMAIN);
  });

  it('页头读数是读取时间；侧栏徽标是本次读取的作品数，悬停说明里是三个读数', async () => {
    pageRoutes(kit, { list: ok(listJson(items(3), { asOf: '2026-10-16 13:28:09.89457+08' })) });
    await opened();
    expect(screen.getByText('读取于 2026-10-16 13:28')).toBeInTheDocument();
    const badge = document.getElementById('ev-rail-count')!;
    expect(badge).toHaveTextContent('3');
    expect(badge).not.toHaveAttribute('hidden');
    expect(badge.title).toMatch(/^本次读取 3 个作品 · 已留存评论 .* · 本地材料 \d+ · 部分或缺口 \d+$/);
  });

  it('页面标题、侧栏页脚与语料导航保持', async () => {
    pageRoutes(kit);
    await opened();
    expect(screen.getByRole('heading', { level: 1, name: '证据库' })).toBeInTheDocument();
    expect(screen.getByText('只读本机材料投影／列表与详情不触发采集')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /证据库/ })).toHaveAttribute('href', `/corpus/evidence?domain=${DOMAIN}`);
  });

  it('读取中先说在读取，列表到了再换成作品', async () => {
    let release!: () => void;
    const gate = new Promise<void>(resolve => {
      release = resolve;
    });
    pageRoutes(kit, {
      list: async () => {
        await gate;
        return { status: 200, body: listJson(items(2)) };
      },
    });
    renderEvidencePage();
    expect(await screen.findByText('正在读取本机材料投影')).toBeInTheDocument();
    expect(screen.getByText('读取中…')).toBeInTheDocument();
    release();
    expect(await screen.findByRole('button', { name: '合成作品 2' })).toBeInTheDocument();
    expect(screen.queryByText('正在读取本机材料投影')).not.toBeInTheDocument();
  });
});

describe('没有领域', () => {
  it('弹出领域选择，不读列表、不改地址；控件置灰并写明原因', async () => {
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: null })) });
    const view = renderEvidencePage('/corpus/evidence');
    const dialog = await screen.findByRole('dialog', { name: '选择研究领域' });
    expect(within(dialog).getAllByRole('link')).toHaveLength(2);
    expect(listCalls()).toHaveLength(0);
    expect(view.location()).toBe('/corpus/evidence');
    const search = screen.getByRole('searchbox', { name: '检索作品与证据' });
    expect(search).toBeDisabled();
    expect(search).toHaveAttribute('title', '请先选择研究领域，再读取统一作品材料。');
    expect(screen.getByRole('button', { name: /筛选/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: '读取材料' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '部分取得' })).toBeDisabled();
  });

  it('关闭对话框只回到「请选择研究领域」空态，不代选、不读材料', async () => {
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: null })) });
    const { user } = renderEvidencePage('/corpus/evidence');
    await user.click(await screen.findByRole('button', { name: '关闭领域选择' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: '请选择研究领域' })).toBeInTheDocument();
    expect(listCalls()).toHaveLength(0);
  });

  it('地址带作品但没有领域：每个领域链接都带着这件作品，选定后继续定位它（A12）', async () => {
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: null })) });
    renderEvidencePage(`/corpus/evidence?work=${workRef(7)}`);
    const dialog = await screen.findByRole('dialog', { name: '选择研究领域' });
    for (const link of within(dialog).getAllByRole('link')) expect(link.getAttribute('href')).toMatch(new RegExp(`&work=${workRef(7)}$`));
    expect(kit.of('GET', detailPathOf(7))).toHaveLength(0);
  });

  it('一个领域也没有：写明，并给出前往领域管理的出口', async () => {
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: null, domains: [] })) });
    renderEvidencePage('/corpus/evidence');
    const dialog = await screen.findByRole('dialog', { name: '选择研究领域' });
    expect(within(dialog).getByText('还没有可选择的研究领域。')).toBeInTheDocument();
    expect(within(dialog).getByRole('link', { name: '前往领域管理 →' })).toHaveAttribute('href', '/collection/domains');
  });

  it('地址里的筛选条件在没有领域时被忽略，不上送', async () => {
    pageRoutes(kit, { shell: ok(shellJson({ selectedDomain: null })) });
    renderEvidencePage('/corpus/evidence?q=abc&lane=comments&view=risk');
    await screen.findByRole('dialog', { name: '选择研究领域' });
    expect(screen.getByRole('searchbox', { name: '检索作品与证据' })).toHaveValue('');
    expect(screen.getByRole('button', { name: '全部材料' })).toHaveAttribute('aria-pressed', 'true');
  });
});

describe('读取失败要如实说', () => {
  it('列表读取失败：说明、错误码、回执条，不显示成空列表；「重试读取」重新读取', async () => {
    pageRoutes(kit, { list: fail(503, 'read_model_not_connected') });
    const { user } = renderEvidencePage();
    expect(await screen.findByText('本机材料读取暂时不可用')).toBeInTheDocument();
    expect(screen.getAllByText('read_model_not_connected').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('本机数据库没有连接，页面没有回退到其他数据。')).toBeInTheDocument();
    expect(screen.getByText('当前未读取任何材料，不能据此判断库为空或来源不存在。')).toBeInTheDocument();
    expect(screen.queryByText('当前查询没有匹配的作品材料')).not.toBeInTheDocument();
    expect(kit.of('GET', detailPathOf(1))).toHaveLength(0);

    pageRoutes(kit);
    await user.click(screen.getByRole('button', { name: '重试读取' }));
    expect(await screen.findByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
    expect(screen.queryByText('本机材料读取暂时不可用')).not.toBeInTheDocument();
  });

  it('外壳读取失败：不读列表；「重试读取」之后恢复', async () => {
    pageRoutes(kit, { shell: fail(503, 'domain_read_unavailable') });
    const { user } = renderEvidencePage();
    expect(await screen.findByRole('heading', { name: '读取失败' })).toBeInTheDocument();
    expect(listCalls()).toHaveLength(0);
    pageRoutes(kit);
    await user.click(screen.getByRole('button', { name: '重试读取' }));
    expect(await screen.findByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
  });

  it('空结果：读取成功但没有匹配，并说明这不证明现实里没有', async () => {
    pageRoutes(kit, { list: ok(listJson([])) });
    renderEvidencePage();
    expect(await screen.findByText('当前查询没有匹配的作品材料')).toBeInTheDocument();
    expect(screen.getByText('读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。')).toBeInTheDocument();
    expect(document.getElementById('ev-rail-count')).toHaveAttribute('hidden');
  });

  it('地址里的采集运行编号无效：不发请求，说明是地址的问题；清除之后才读取（B20）', async () => {
    pageRoutes(kit);
    const { user, location } = renderEvidencePage(pageUrl(`domain=${DOMAIN}&collectionRunRef=not-a-uuid`));
    expect(await screen.findByText('地址里的筛选条件无效')).toBeInTheDocument();
    expect(listCalls()).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: '清除这个筛选条件' }));
    expect(await screen.findByRole('button', { name: '合成作品 1' })).toBeInTheDocument();
    expect(listCalls()[0]?.search.has('collectionRunRef')).toBe(false);
    await waitFor(() => expect(location()).not.toContain('collectionRunRef'));
  });

  it('创作者筛选条件不是合法 JSON：同样不发请求', async () => {
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&creatorFilter=${encodeURIComponent('[1]')}&creatorKey=x`));
    expect(await screen.findByText('地址里的筛选条件无效')).toBeInTheDocument();
    expect(listCalls()).toHaveLength(0);
  });
});

describe('读取回执条', () => {
  it('扫描预算触发：说当前结果不是全部匹配，并给出扫描条数', async () => {
    pageRoutes(kit, { list: ok(listJson(items(2), { scanLimited: true, scannedCount: 200, cursor: 'c-next' })) });
    await opened();
    expect(screen.getByText('扫描预算已触发，当前结果不是全部匹配；可继续读取')).toBeInTheDocument();
    expect(screen.getByText('扫描 200')).toBeInTheDocument();
  });

  it('已截断却没有游标：如实说来源没有给出继续读取的游标', async () => {
    pageRoutes(kit, { list: ok(listJson(items(2), { truncated: true, scannedCount: 12 })) });
    await opened();
    expect(screen.getByText('本次读取已截断，但来源没有给出继续读取的游标')).toBeInTheDocument();
    expect(screen.getByText('扫描 12')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '继续读取作品' })).not.toBeInTheDocument();
  });

  it('平常的成功读取不出现回执条', async () => {
    pageRoutes(kit);
    await opened();
    expect(screen.queryByText(/扫描预算已触发/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^扫描 /)).not.toBeInTheDocument();
  });
});

describe('地址直达一件不在页内的作品', () => {
  it('按领域直读详情，不回退到第一项、不改写地址（E20）', async () => {
    pageRoutes(kit, { count: 3 });
    kit.route('GET', detailPathOf(9), ok((await import('./evidenceFixtures')).detailJson(9)));
    const view = renderEvidencePage(pageUrl(`domain=${DOMAIN}&work=${workRef(9)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    await waitFor(() => expect(kit.of('GET', detailPathOf(9))).toHaveLength(1));
    expect(kit.of('GET', detailPathOf(9))[0]?.search.get('domain')).toBe(DOMAIN);
    expect(kit.of('GET', detailPathOf(1))).toHaveLength(0);
    expect(view.location()).toBe(pageUrl(`domain=${DOMAIN}&work=${workRef(9)}`));
    expect(await within(aside()).findByRole('heading', { level: 2, hidden: true, name: '合成作品 9' })).toBeInTheDocument();
  });

  it('带着本次采集的固定作品集合直读：详情请求保持同一个 collectionRunRef', async () => {
    const run = '30000000-0000-4000-8000-000000000001';
    pageRoutes(kit, { count: 1 });
    kit.route('GET', detailPathOf(9), ok((await import('./evidenceFixtures')).detailJson(9)));
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&collectionRunRef=${run}&work=${workRef(9)}`));
    await waitFor(() => expect(kit.of('GET', detailPathOf(9))).toHaveLength(1));
    expect(kit.of('GET', detailPathOf(9))[0]?.search.get('collectionRunRef')).toBe(run);
    expect(listCalls()[0]?.search.get('collectionRunRef')).toBe(run);
  });

  it('列表为空时地址里的作品仍按详情直读', async () => {
    pageRoutes(kit, { list: ok(listJson([])), count: 0 });
    kit.route('GET', detailPathOf(9), ok((await import('./evidenceFixtures')).detailJson(9)));
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&work=${workRef(9)}`));
    expect(await screen.findByText('当前查询没有匹配的作品材料')).toBeInTheDocument();
    await waitFor(() => expect(kit.of('GET', detailPathOf(9))).toHaveLength(1));
  });

  it('领域里读不到这件作品：写「当前领域无法读取这篇作品」并建议切换领域', async () => {
    pageRoutes(kit, { count: 1 });
    kit.route('GET', detailPathOf(9), fail(404, 'material_not_found'));
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&work=${workRef(9)}`));
    expect(await screen.findByText('当前领域无法读取这篇作品')).toBeInTheDocument();
    expect(screen.getAllByText('material_not_found').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/请切换领域或从作品列表重新选择/)).toBeInTheDocument();
  });

  it('详情读取失败：说明、错误码与「重试读取」；列表仍可用', async () => {
    pageRoutes(kit, { count: 1 });
    kit.route('GET', detailPathOf(1), fail(503, 'material_detail_unavailable'));
    const { user } = renderEvidencePage();
    expect(await screen.findByText('当前作品详情读取失败')).toBeInTheDocument();
    expect(screen.getAllByText('material_detail_unavailable').length).toBeGreaterThanOrEqual(1);
    expect(rowButton(1)).toBeEnabled();
    kit.route('GET', detailPathOf(1), ok((await import('./evidenceFixtures')).detailJson(1)));
    await user.click(within(aside()).getByRole('button', { name: '重试读取' }));
    await waitFor(() => expect(screen.queryByText('当前作品详情读取失败')).not.toBeInTheDocument());
  });

  it('列表没有给出可用的同源详情地址：写来源信息不完整，不猜路由', async () => {
    pageRoutes(kit, { list: ok(listJson(items(1, () => ({ detailUrl: 'https://example.invalid/work/1' })))) });
    await opened();
    expect(await screen.findByText('来源信息不完整')).toBeInTheDocument();
    expect(kit.of('GET', detailPathOf(1))).toHaveLength(0);
  });
});

describe('接口只被访问一次', () => {
  it('同一领域同一条件下，重新渲染不会重复读取列表与外壳', async () => {
    pageRoutes(kit);
    const { rerender } = await opened();
    rerender(<></>);
    expect(kit.of('GET', SHELL_PATH)).toHaveLength(1);
    expect(listCalls()).toHaveLength(1);
  });
});
