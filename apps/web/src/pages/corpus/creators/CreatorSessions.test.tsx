import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DOMAIN,
  OTHER_DOMAIN,
  createFetchMock,
  creatorKeyFor,
  defaultRoutes,
  deferred,
  itemJson,
  listJson,
  ok,
  renderCreatorsPage,
  workRef,
  type FetchKit,
  type Reply,
} from './testKit';

/**
 * 页面会话内的作者上下文：切换作者、关闭、翻页、换领域都只隐藏检查器，不重开请求。
 * 这些用例专门盯住「A 的请求与回执不会被 B 或换页吞掉」。
 */

const RUN = '40000000-0000-4000-8000-000000000001';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const previewPath = (n: number) => `/api/local/creators/${encodeURIComponent(creatorKeyFor(`author-${n}`))}/preview`;

function card(n: number, over: Record<string, unknown> = {}) {
  return { workRef: workRef(100 + n), ordinal: n, inDomain: false, title: `主页卡片 ${n}`, likes: n * 10, comments: 1, collects: 1, ...over };
}
function snapshot(over: Record<string, unknown> = {}) {
  return {
    state: 'complete', runRef: RUN, revision: 2, requestedLimit: 30, sampleCount: 3, frozenCount: 3,
    scopeFrozenAt: '2026-10-28 10:00:00+08', stopReason: 'surface_ended', works: [card(1), card(2), card(3)], ...over,
  };
}
const WITH_KNOWN = () => snapshot({ works: [card(1), card(2, { inDomain: true }), card(3)] });
const NOT_REQUESTED = { state: 'not_requested', runRef: null, requestedLimit: 30, sampleCount: 0, works: [] };

let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.install();
  sessionStorage.clear();
});
afterEach(() => {
  vi.unstubAllGlobals();
});

type View = ReturnType<typeof renderCreatorsPage>;

async function open(view: View, name: string) {
  await view.user.click(await screen.findByRole('button', { name }));
  const panel = await screen.findByRole('complementary', { name });
  return { panel };
}

async function openWorks(view: View, name: string) {
  const { panel } = await open(view, name);
  await view.user.click(within(panel).getByRole('tab', { name: '主页作品' }));
  return { panel, tab: within(panel).getByRole('tabpanel', { name: '主页作品' }) };
}

/** 两页列表：第一页 1、2 号；第二页 3 号。按地址里的 page 回答，重读同一页也稳定。 */
function pagedList() {
  kit.route('GET', '/api/local/creators', call => {
    const page = Number(call.search.get('page') ?? '0');
    return {
      status: 200,
      body: page === 0
        ? listJson({ items: [itemJson(1), itemJson(2)], total: 60, page: 0 })
        : listJson({ items: [itemJson(3)], total: 60, page: 1 }),
    };
  });
}

describe('页面会话 · 切换到另一位作者', () => {
  it('提交 A 后切到 B：A 的迟到回执只写进 A，B 的内容与焦点不受影响；回到 A 可用同一请求重试', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2)] })));
    kit.route('GET', previewPath(1),
      ok(NOT_REQUESTED),
      ok(snapshot({ state: 'running', scopeFrozenAt: null, works: [card(1)], sampleCount: 1, frozenCount: 1 })));
    kit.route('GET', previewPath(2), ok(NOT_REQUESTED));
    const gate = deferred<Reply>();
    kit.route('POST', previewPath(1), () => gate.promise);
    const view = renderCreatorsPage();

    const { panel: panelA, tab: tabA } = await openWorks(view, '合成作者 1');
    await view.user.click(within(tabA).getByRole('button', { name: '读取主页（最多 30 篇）' }));
    await waitFor(() => expect(kit.of('POST', previewPath(1))).toHaveLength(1));
    expect(within(tabA).getByRole('button', { name: '读取主页（最多 30 篇）' })).toBeDisabled();

    // 切到 B：A 只隐藏，不卸载；B 拿到焦点，B 的页签是它自己的空态。
    const { panel: panelB, tab: tabB } = await openWorks(view, '合成作者 2');
    expect(panelA).toHaveAttribute('hidden');
    expect(panelB.contains(document.activeElement)).toBe(true);
    expect(within(tabB).getByText('尚无主页样本。')).toBeVisible();

    // A 的失败回执迟到：只回到 A 名下。
    gate.resolve('network');
    await waitFor(() => expect(within(tabA).getByText(/读取请求尚未确认/)).toBeInTheDocument());
    expect(within(tabB).queryByText(/读取请求尚未确认/)).toBeNull();
    expect(within(tabB).getByText('尚无主页样本。')).toBeVisible();
    expect(within(tabB).queryByText(/正在读取/)).toBeNull();

    // 回到 A：失败提示还在，重试沿用同一个请求标识。
    await open(view, '合成作者 1');
    expect(within(tabA).getByText(/读取请求尚未确认/)).toBeVisible();
    await view.user.click(within(tabA).getByRole('button', { name: '读取主页（最多 30 篇）' }));
    await waitFor(() => expect(kit.of('POST', previewPath(1))).toHaveLength(2));
    const [first, second] = kit.of('POST', previewPath(1));
    expect((first!.body as { commandRef: string }).commandRef).toMatch(UUID);
    expect((second!.body as { commandRef: string }).commandRef).toBe((first!.body as { commandRef: string }).commandRef);
  });
});

describe('页面会话 · 关闭再打开', () => {
  it('同一位作者：精选选择、确认面板与未确认的请求身份都还在，重试请求体不变', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2)] })));
    kit.route('GET', previewPath(1), ok(WITH_KNOWN()));
    kit.route('POST', '/api/collection/runs',
      () => 'network',
      ok({ runRef: '40000000-0000-4000-8000-000000000002', domainRef: DOMAIN, replayed: false }));
    const view = renderCreatorsPage();

    const { panel, tab } = await openWorks(view, '合成作者 1');
    await within(tab).findByText('本次样本 3 篇');
    await view.user.click(within(tab).getByRole('button', { name: '移除 主页卡片 2' }));
    expect(within(tab).getByRole('button', { name: '研究已选 2 篇' })).toBeEnabled();
    await view.user.click(within(tab).getByRole('button', { name: '研究已选 2 篇' }));
    const confirm = within(tab).getByRole('region', { name: '精选研究确认' });
    await view.user.click(within(confirm).getByRole('button', { name: /^仅补文字资料/ }));
    await view.user.click(within(confirm).getByRole('button', { name: '确认精选范围' }));
    expect(await within(tab).findByText(/精选任务未得到确定回执/)).toBeVisible();

    // 关闭：只隐藏。再点同一位作者，原实例带着页签、选择与确认面板回来。
    await view.user.click(within(panel).getByRole('button', { name: '关闭作者检查器' }));
    await waitFor(() => expect(panel).toHaveAttribute('hidden'));
    await open(view, '合成作者 1');
    const reopenedTab = within(panel).getByRole('tabpanel', { name: '主页作品' });
    expect(within(reopenedTab).getByRole('region', { name: '精选研究确认' })).toBeInTheDocument();
    expect(within(reopenedTab).getByText(/精选任务未得到确定回执/)).toBeVisible();
    expect(within(reopenedTab).getByRole('button', { name: '研究已选 2 篇' })).toBeEnabled();

    await view.user.click(within(reopenedTab).getByRole('button', { name: '确认精选范围' }));
    expect(await within(reopenedTab).findByText(/已建立精选任务/)).toBeVisible();
    const [first, second] = kit.of('POST', '/api/collection/runs');
    expect(first!.body).toEqual(second!.body);
    expect(second!.body).toMatchObject({ domainRef: DOMAIN, workLimit: 2, commentMode: 'none' });
  });
});

describe('页面会话 · 翻页关闭检查器后找回作者', () => {
  it('提交 A 的精选请求后翻到第二页再回来：确认面板与请求身份都保留，重试请求体不变', async () => {
    defaultRoutes(kit);
    pagedList();
    kit.route('GET', previewPath(1), ok(WITH_KNOWN()));
    kit.route('POST', '/api/collection/runs',
      () => 'network',
      ok({ runRef: '40000000-0000-4000-8000-000000000003', domainRef: DOMAIN, replayed: false }));
    const view = renderCreatorsPage();

    const { tab } = await openWorks(view, '合成作者 1');
    await within(tab).findByText('本次样本 3 篇');
    await view.user.click(within(tab).getByRole('button', { name: '研究已选 3 篇' }));
    await view.user.click(within(within(tab).getByRole('region', { name: '精选研究确认' })).getByRole('button', { name: '确认精选范围' }));
    expect(await within(tab).findByText(/精选任务未得到确定回执/)).toBeVisible();

    // 翻页只关闭检查器，不删除仍在页面会话里的作者上下文。
    await view.user.click(screen.getByRole('button', { name: '下一页' }));
    await screen.findByRole('button', { name: '合成作者 3' });
    expect(screen.queryByRole('complementary', { name: '合成作者 1' })).toBeNull();

    await view.user.click(screen.getByRole('button', { name: '上一页' }));
    await screen.findByRole('button', { name: '合成作者 1' });
    await open(view, '合成作者 1');
    const panel = await screen.findByRole('complementary', { name: '合成作者 1' });
    const reopenedTab = within(panel).getByRole('tabpanel', { name: '主页作品' });
    expect(within(reopenedTab).getByRole('region', { name: '精选研究确认' })).toBeInTheDocument();
    expect(within(reopenedTab).getByText(/精选任务未得到确定回执/)).toBeVisible();

    await view.user.click(within(reopenedTab).getByRole('button', { name: '确认精选范围' }));
    expect(await within(reopenedTab).findByText(/已建立精选任务/)).toBeVisible();
    const [first, second] = kit.of('POST', '/api/collection/runs');
    expect(first!.body).toEqual(second!.body);
    expect(second!.body).toMatchObject({
      domainRef: DOMAIN,
      workLimit: 3,
      source: { kind: 'creator_selected_works', previewRef: RUN, workRefs: [workRef(101), workRef(102), workRef(103)] },
    });
  });

  it('提交 A 的主页读取后翻页再回来：重试还是同一个 commandRef', async () => {
    defaultRoutes(kit);
    pagedList();
    kit.route('GET', previewPath(1), ok(NOT_REQUESTED));
    kit.route('POST', previewPath(1),
      () => 'network',
      ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    const view = renderCreatorsPage();

    const { tab } = await openWorks(view, '合成作者 1');
    await view.user.click(await within(tab).findByRole('button', { name: '读取主页（最多 30 篇）' }));
    expect(await within(tab).findByText(/读取请求尚未确认/)).toBeVisible();

    await view.user.click(screen.getByRole('button', { name: '下一页' }));
    await screen.findByRole('button', { name: '合成作者 3' });
    expect(screen.queryByRole('complementary', { name: '合成作者 1' })).toBeNull();
    await view.user.click(screen.getByRole('button', { name: '上一页' }));
    await screen.findByRole('button', { name: '合成作者 1' });

    await open(view, '合成作者 1');
    const panel = await screen.findByRole('complementary', { name: '合成作者 1' });
    const reopenedTab = within(panel).getByRole('tabpanel', { name: '主页作品' });
    expect(within(reopenedTab).getByText(/读取请求尚未确认/)).toBeVisible();
    await view.user.click(within(reopenedTab).getByRole('button', { name: '读取主页（最多 30 篇）' }));
    await waitFor(() => expect(kit.of('POST', previewPath(1))).toHaveLength(2));
    const [first, second] = kit.of('POST', previewPath(1));
    expect((second!.body as { commandRef: string }).commandRef).toBe((first!.body as { commandRef: string }).commandRef);
  });
});

describe('页面会话 · 上一位 / 下一位', () => {
  it('只按当前页已加载顺序走，边界禁用，切人后焦点留在翻页按钮上；不多读列表', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2), itemJson(3)] })));
    for (const n of [1, 2, 3]) kit.route('GET', previewPath(n), ok(NOT_REQUESTED));
    const view = renderCreatorsPage();

    await view.user.click(await screen.findByRole('button', { name: '合成作者 1' }));
    const first = await screen.findByRole('complementary', { name: '合成作者 1' });
    expect(within(first).getByText('本页 1/3')).toBeInTheDocument();
    expect(within(first).getByRole('button', { name: '上一位' })).toBeDisabled();
    expect(within(first).getByRole('button', { name: '下一位' })).toBeEnabled();

    await view.user.click(within(first).getByRole('button', { name: '下一位' }));
    const second = await screen.findByRole('complementary', { name: '合成作者 2' });
    expect(within(second).getByText('本页 2/3')).toBeInTheDocument();
    expect(within(second).getByRole('button', { name: '下一位' })).toHaveFocus();

    await view.user.click(within(second).getByRole('button', { name: '下一位' }));
    const third = await screen.findByRole('complementary', { name: '合成作者 3' });
    expect(within(third).getByText('本页 3/3')).toBeInTheDocument();
    expect(within(third).getByRole('button', { name: '下一位' })).toBeDisabled();
    // 末位没有下一位：焦点落到仍可用的「上一位」，不离开检查器。
    expect(within(third).getByRole('button', { name: '上一位' })).toHaveFocus();
    expect(third.contains(document.activeElement)).toBe(true);

    await view.user.click(within(third).getByRole('button', { name: '上一位' }));
    await screen.findByRole('complementary', { name: '合成作者 2' });
    expect(kit.of('GET', '/api/local/creators')).toHaveLength(1);
  });

  it('进入未访问过的作者时沿用当前页签，连续浏览不回到概览', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2)] })));
    for (const n of [1, 2]) kit.route('GET', previewPath(n), ok(NOT_REQUESTED));
    const view = renderCreatorsPage();

    const { panel } = await open(view, '合成作者 1');
    await view.user.click(within(panel).getByRole('tab', { name: '研究线索' }));
    expect(within(panel).getByRole('tab', { name: '研究线索' })).toHaveAttribute('aria-selected', 'true');

    await view.user.click(within(panel).getByRole('button', { name: '下一位' }));
    const second = await screen.findByRole('complementary', { name: '合成作者 2' });
    expect(within(second).getByRole('tab', { name: '研究线索' })).toHaveAttribute('aria-selected', 'true');
    expect(within(second).getByRole('tabpanel', { name: '研究线索' })).toBeVisible();
  });

  it('切换作者或关闭再打开都保留原来的阅读位置，不回到顶部', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2)] })));
    for (const n of [1, 2]) kit.route('GET', previewPath(n), ok(NOT_REQUESTED));
    const view = renderCreatorsPage();

    const { panel } = await open(view, '合成作者 1');
    const body = panel.querySelector('[role="tabpanel"]')?.parentElement as HTMLElement;
    body.scrollTop = 250;
    fireEvent.scroll(body);

    await view.user.click(within(panel).getByRole('button', { name: '下一位' }));
    const second = await screen.findByRole('complementary', { name: '合成作者 2' });
    await view.user.click(within(second).getByRole('button', { name: '上一位' }));
    await waitFor(() => expect(body.scrollTop).toBe(250));

    body.scrollTop = 200;
    fireEvent.scroll(body);
    await view.user.click(within(panel).getByRole('button', { name: '关闭作者检查器' }));
    await waitFor(() => expect(panel).toHaveAttribute('hidden'));
    await open(view, '合成作者 1');
    await waitFor(() => expect(body.scrollTop).toBe(200));
  });
});

describe('页面会话 · 同一作者在不同领域', () => {
  it('两份会话互不串状态：另一个领域从空态开始，回到原领域才恢复原请求上下文', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2)] })));
    kit.route('GET', '/api/local/corpus/shell', call => ({
      status: 200,
      body: {
        domains: [
          { domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 3, sampleCount: 80 },
          { domainRef: OTHER_DOMAIN, name: '合成领域二', status: 'active', targetCount: 1, sampleCount: null },
        ],
        selectedDomain: call.search.get('domain') ?? DOMAIN,
        collectionState: '观察中',
      },
    }));
    kit.route('GET', '/api/local/creators', call => {
      const other = call.search.get('domain') === OTHER_DOMAIN;
      const base = listJson({ items: other ? [itemJson(1)] : [itemJson(1), itemJson(2)] });
      return {
        status: 200,
        body: other ? { ...base, domain: { ...base.domain, domainRef: OTHER_DOMAIN, name: '合成领域二' } } : base,
      };
    });
    kit.route('GET', previewPath(1), call => ({
      status: 200,
      body: call.search.get('domain') === OTHER_DOMAIN ? NOT_REQUESTED : WITH_KNOWN(),
    }));
    kit.route('POST', '/api/collection/runs', ok({ runRef: '40000000-0000-4000-8000-000000000004', domainRef: DOMAIN, replayed: false }));
    const view = renderCreatorsPage();

    // 领域一：做出选择并打开确认面板（尚未提交）。
    const { tab } = await openWorks(view, '合成作者 1');
    await within(tab).findByText('本次样本 3 篇');
    await view.user.click(within(tab).getByRole('button', { name: '移除 主页卡片 2' }));
    await view.user.click(within(tab).getByRole('button', { name: '研究已选 2 篇' }));
    expect(within(tab).getByRole('region', { name: '精选研究确认' })).toBeInTheDocument();

    // 换到领域二：同一位作者是另一份会话，主页样本从空态开始，不带走确认面板。
    await view.user.click(screen.getByLabelText('切换当前观察领域'));
    await view.user.click(await screen.findByRole('link', { name: /合成领域二/ }));
    const { tab: tabTwo } = await openWorks(view, '合成作者 1');
    expect(within(tabTwo).getByText('尚无主页样本。')).toBeVisible();
    expect(within(tabTwo).queryByRole('region', { name: '精选研究确认' })).toBeNull();
    expect(within(tabTwo).queryByText(/已选 \d+ 篇/)).toBeNull();
    expect(kit.of('POST', '/api/collection/runs')).toHaveLength(0);

    // 回到领域一：原会话还在，选择与确认面板原样回来，提交用的是领域一。
    await view.user.click(screen.getByLabelText('切换当前观察领域'));
    await view.user.click(await screen.findByRole('link', { name: /合成领域一/ }));
    await open(view, '合成作者 1');
    const panel = await screen.findByRole('complementary', { name: '合成作者 1' });
    const reopenedTab = within(panel).getByRole('tabpanel', { name: '主页作品' });
    expect(within(reopenedTab).getByRole('region', { name: '精选研究确认' })).toBeInTheDocument();
    await view.user.click(within(reopenedTab).getByRole('button', { name: '确认精选范围' }));
    await waitFor(() => expect(kit.of('POST', '/api/collection/runs')).toHaveLength(1));
    expect(kit.of('POST', '/api/collection/runs')[0]!.body).toMatchObject({ domainRef: DOMAIN, workLimit: 2 });
  });
});
