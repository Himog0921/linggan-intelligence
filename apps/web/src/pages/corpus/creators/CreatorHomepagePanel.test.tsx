import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOMAIN, createFetchMock, creatorKeyFor, defaultRoutes, ok, renderCreatorsPage, workRef, type FetchKit } from './testKit';

/** 「主页作品」分区：浅看 → 冻结样本 → 精选研究。形状照 `GET /api/local/creators/{key}/preview` 的真实输出。 */
// 请求路径里的作者标识带冒号，浏览器按 encodeURIComponent 发出，假 fetch 看到的是百分号编码后的路径。
const PREVIEW = `/api/local/creators/${encodeURIComponent(creatorKeyFor('author-1'))}/preview`;
const RUN = '40000000-0000-4000-8000-000000000001';
const RUN_NEXT = '40000000-0000-4000-8000-000000000008';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function card(n: number, over: Record<string, unknown> = {}) {
  return { workRef: workRef(100 + n), ordinal: n, inDomain: false, title: `主页卡片 ${n}`, likes: n * 10, comments: 1, collects: 1, ...over };
}
function snapshot(over: Record<string, unknown> = {}) {
  return {
    state: 'complete', runRef: RUN, revision: 2, requestedLimit: 30, sampleCount: 3, frozenCount: 3,
    scopeFrozenAt: '2026-10-28 10:00:00+08', stopReason: 'surface_ended', works: [card(1), card(2), card(3)], ...over,
  };
}
// 中间一张是本领域已有的研究材料：默认建议 = 已见的 + 点赞最高的 + 一张对照，共 3 篇。
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

async function openHomepage() {
  const view = renderCreatorsPage();
  await view.user.click(await screen.findByRole('button', { name: '合成作者 1' }));
  const dialog = await screen.findByRole('dialog', { name: '合成作者 1' });
  await view.user.click(within(dialog).getByRole('tab', { name: '主页作品' }));
  return { ...view, dialog, panel: within(dialog).getByRole('tabpanel', { name: '主页作品' }) };
}

describe('主页作品 · 浅看入口', () => {
  it('没展开过：只给「浅看主页」；点一次发一个带请求标识的请求，随后读到「正在浅看」', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(NOT_REQUESTED), ok(snapshot({ state: 'running', scopeFrozenAt: null, works: [], sampleCount: 0, frozenCount: 0 })));
    kit.route('POST', PREVIEW, ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    const { user, panel } = await openHomepage();
    expect(await within(panel).findByText('先展开这位作者的主页')).toBeVisible();
    expect(panel).toHaveTextContent('本次不自动打开全部详情、不采集评论，也不会自动加入长期观察。');
    await user.click(within(panel).getByRole('button', { name: '浅看主页' }));
    expect(await within(panel).findByText(/正在浅看/)).toBeVisible();
    const posts = kit.of('POST', PREVIEW);
    expect(posts).toHaveLength(1);
    expect(posts[0]!.body).toEqual({ domain: DOMAIN, commandRef: expect.stringMatching(UUID) });
    // 没有冻结样本之前不能发起精选；运行中给停止入口。
    expect(within(panel).queryByRole('button', { name: /研究已选/ })).toBeNull();
    expect(within(panel).getByRole('button', { name: '停止这次浅看' })).toBeEnabled();
  });

  it('请求没得到确定回执：提示可再点，再点沿用同一个请求标识', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(NOT_REQUESTED));
    kit.route('POST', PREVIEW, () => 'network', ok({ runRef: RUN, domainRef: DOMAIN, replayed: true }));
    const { user, panel } = await openHomepage();
    await user.click(await within(panel).findByRole('button', { name: '浅看主页' }));
    expect(await within(panel).findByText(/浅看请求尚未确认/)).toBeVisible();
    await user.click(within(panel).getByRole('button', { name: '浅看主页' }));
    await waitFor(() => expect(kit.of('POST', PREVIEW)).toHaveLength(2));
    const [first, second] = kit.of('POST', PREVIEW);
    expect((first!.body as { commandRef: string }).commandRef).toBe((second!.body as { commandRef: string }).commandRef);
  });

  it('停止、范围不完整、或一篇也没取到之后，给「重新浅看」，点击发出一个带请求标识的新请求', async () => {
    for (const [label, body] of [
      ['已停止', snapshot({ state: 'stopped', works: [card(1)], sampleCount: 1, frozenCount: 1 })],
      ['范围不完整', snapshot({ state: 'partial', works: [card(1)], sampleCount: 1, frozenCount: 1 })],
      ['0 篇', snapshot({ state: 'complete', works: [], sampleCount: 0, frozenCount: 0 })],
    ] as const) {
      kit = createFetchMock();
      kit.install();
      defaultRoutes(kit);
      kit.route('GET', PREVIEW, ok(body), ok(snapshot({ state: 'running', scopeFrozenAt: null, works: [], sampleCount: 0, frozenCount: 0 })));
      kit.route('POST', PREVIEW, ok({ runRef: '40000000-0000-4000-8000-000000000009', domainRef: DOMAIN, replayed: false }));
      const { user, panel, unmount } = await openHomepage();
      const retry = await within(panel).findByRole('button', { name: '重新浅看' });
      expect(retry, label).toBeEnabled();
      await user.click(retry);
      await waitFor(() => expect(kit.of('POST', PREVIEW), label).toHaveLength(1));
      expect(kit.of('POST', PREVIEW)[0]!.body).toEqual({ domain: DOMAIN, commandRef: expect.stringMatching(UUID) });
      expect(await within(panel).findByText(/正在浅看/), label).toBeVisible();
      unmount();
    }
  });

  it('上一次浅看已得到回执之后再「重新浅看」，请求标识与上一次不同（否则会被当作重试重放成同一次）', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW,
      ok(NOT_REQUESTED),
      ok(snapshot({ state: 'stopped', works: [card(1)], sampleCount: 1, frozenCount: 1 })),
      ok(snapshot({ state: 'running', runRef: RUN_NEXT, scopeFrozenAt: null, works: [], sampleCount: 0, frozenCount: 0 })));
    kit.route('POST', PREVIEW,
      ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }),
      ok({ runRef: RUN_NEXT, domainRef: DOMAIN, replayed: false }));
    const { user, panel } = await openHomepage();
    await user.click(await within(panel).findByRole('button', { name: '浅看主页' }));
    await user.click(await within(panel).findByRole('button', { name: '重新浅看' }));
    await waitFor(() => expect(kit.of('POST', PREVIEW)).toHaveLength(2));
    const [first, second] = kit.of('POST', PREVIEW).map(call => (call.body as { commandRef: string }).commandRef);
    expect(first).toMatch(UUID);
    expect(second).toMatch(UUID);
    expect(second).not.toBe(first);
  });

  it('完成且有卡片时没有「重新浅看」（完成后刷新样本是产品取舍，不在这里偷偷开放）', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(snapshot()));
    const { panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    expect(within(panel).queryByRole('button', { name: '重新浅看' })).toBeNull();
  });

  it('运行中：已有卡片可先看，但不能发起精选；停止发给服务端带当前版本号', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(snapshot({ state: 'running', revision: 5, scopeFrozenAt: null, works: [card(1)], sampleCount: 1, frozenCount: 1 })));
    kit.route('POST', `/api/collection/runs/${RUN}/commands`, ok({}));
    const { user, panel } = await openHomepage();
    await within(panel).findByText(/正在浅看/);
    expect(within(panel).getByRole('button', { name: /研究已选/ })).toBeDisabled();
    await user.click(within(panel).getByRole('button', { name: '停止这次浅看' }));
    await waitFor(() => expect(kit.of('POST', `/api/collection/runs/${RUN}/commands`)).toHaveLength(1));
    expect(kit.of('POST', `/api/collection/runs/${RUN}/commands`)[0]!.body).toEqual({
      domainRef: DOMAIN, commandRef: expect.stringMatching(UUID), expectedRevision: 5, action: 'stop',
    });
  });
});

describe('主页作品 · 冻结样本与精选研究', () => {
  it('卡片上的「本领域已见」来自服务端，默认建议最多 3 篇：已见的一篇、点赞最高的一篇、另一篇对照', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(snapshot({
      works: [card(1, { likes: 5 }), card(2, { inDomain: true, likes: 20 }), card(3, { likes: 300 }), card(4, { likes: 50 })],
      sampleCount: 4, frozenCount: 4,
    })));
    const { panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 4 篇');
    expect(within(panel).getByRole('button', { name: '研究已选 3 篇' })).toBeEnabled();
    // 已见的（2）、点赞最高的（3）、对照（1 号位）被选；4 号没选。
    expect(within(panel).getByRole('button', { name: '移除 主页卡片 2' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(panel).getByRole('button', { name: '移除 主页卡片 3' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(panel).getByRole('button', { name: '移除 主页卡片 1' })).toHaveAttribute('aria-pressed', 'true');
    expect(within(panel).getByRole('button', { name: '选择 主页卡片 4' })).toHaveAttribute('aria-pressed', 'false');
    expect(panel.querySelectorAll('[data-selected="true"]')).toHaveLength(3);
    expect(panel).toHaveTextContent('本领域已见');
    expect(panel).toHaveTextContent('主页新遇见');
    // 页面的作品筛选不影响这个标签：只看服务端给的 inDomain，所以只有 2 号是「已见」。
    expect(within(panel).getAllByText(/本领域已见/)).toHaveLength(1);
  });

  it('文字版精选：确认之前不发请求；请求体只带所选作品与服务端固定的范围', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(WITH_KNOWN()));
    kit.route('POST', '/api/collection/runs', ok({ runRef: '40000000-0000-4000-8000-000000000002', domainRef: DOMAIN, replayed: false }));
    const { user, panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    await user.click(within(panel).getByRole('button', { name: '移除 主页卡片 2' }));
    await user.click(within(panel).getByRole('button', { name: '研究已选 2 篇' }));
    const confirm = within(panel).getByRole('region', { name: '精选研究确认' });
    expect(confirm).toHaveTextContent('精确选定的 2 篇');
    expect(kit.of('POST', '/api/collection/runs')).toHaveLength(0);
    await user.click(within(confirm).getByRole('button', { name: /^仅补文字资料/ }));
    await user.click(within(confirm).getByRole('button', { name: '确认精选范围' }));
    expect(await within(panel).findByText('精选任务已建立')).toBeVisible();
    expect(kit.of('POST', '/api/collection/runs')[0]!.body).toEqual({
      idempotencyKey: expect.stringMatching(UUID), domainRef: DOMAIN,
      source: { kind: 'creator_selected_works', previewRef: RUN, workRefs: [workRef(101), workRef(103)] },
      ranking: null, workLimit: 2, commentMode: 'none', commentTarget: null,
    });
  });

  it('标准版精选带评论样本；没有确定回执时保留选择，重试沿用同一个请求标识且不扩大范围', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(WITH_KNOWN()));
    kit.route('POST', '/api/collection/runs', () => 'network', ok({ runRef: '40000000-0000-4000-8000-000000000003', domainRef: DOMAIN, replayed: false }));
    const { user, panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    await user.click(within(panel).getByRole('button', { name: '研究已选 3 篇' }));
    const confirm = within(panel).getByRole('region', { name: '精选研究确认' });
    await user.click(within(confirm).getByRole('button', { name: '确认精选范围' }));
    expect(await within(panel).findByText(/精选任务未得到确定回执/)).toBeVisible();
    expect(within(panel).getByRole('button', { name: '研究已选 3 篇' })).toBeEnabled();
    await user.click(within(confirm).getByRole('button', { name: '确认精选范围' }));
    expect(await within(panel).findByText('精选任务已建立')).toBeVisible();
    const [first, second] = kit.of('POST', '/api/collection/runs');
    expect(first!.body).toEqual(second!.body);
    expect(second!.body).toMatchObject({ commentMode: 'sample_30', workLimit: 3, source: { kind: 'creator_selected_works', previewRef: RUN } });
  });

  it('精选请求没得到回执时样本换成了新的一次：旧请求标识不沿用，范围是新样本的建议而不是旧选择', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW,
      ok(WITH_KNOWN()),
      ok(snapshot({ runRef: RUN_NEXT, works: [card(4, { inDomain: true }), card(5), card(6)] })));
    kit.route('POST', '/api/collection/runs', () => 'network', ok({ runRef: '40000000-0000-4000-8000-000000000003', domainRef: DOMAIN, replayed: false }));
    const { user, panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    await user.click(within(panel).getByRole('button', { name: '研究已选 3 篇' }));
    const confirm = within(panel).getByRole('region', { name: '精选研究确认' });
    await user.click(within(confirm).getByRole('button', { name: '确认精选范围' }));
    expect(await within(panel).findByText(/精选任务未得到确定回执/)).toBeVisible();
    // 样本被换成新的一次（例如另一个页签重新浅看过）。
    await user.click(within(panel).getByRole('button', { name: '更新' }));
    await within(panel).findByRole('button', { name: '移除 主页卡片 4' });
    await user.click(within(panel).getByRole('button', { name: '确认精选范围' }));
    await waitFor(() => expect(kit.of('POST', '/api/collection/runs')).toHaveLength(2));
    const [first, second] = kit.of('POST', '/api/collection/runs').map(call => call.body as {
      idempotencyKey: string; source: { previewRef: string; workRefs: string[] };
    });
    expect(second!.idempotencyKey).not.toBe(first!.idempotencyKey);
    expect(second!.source).toEqual({ kind: 'creator_selected_works', previewRef: RUN_NEXT, workRefs: [workRef(104), workRef(105), workRef(106)] });
  });

  it('新的一次浅看还没有卡片时，「已选研究」不沿用旧样本的选择', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW,
      ok(WITH_KNOWN()),
      ok(snapshot({ state: 'running', runRef: RUN_NEXT, scopeFrozenAt: null, works: [], sampleCount: 0, frozenCount: 0 })));
    const { user, panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    expect(panel).toHaveTextContent(/已选研究\s*3/);
    await user.click(within(panel).getByRole('button', { name: '更新' }));
    await within(panel).findByText(/正在浅看/);
    expect(panel).toHaveTextContent(/已选研究\s*0/);
    expect(panel).not.toHaveTextContent(/已选研究\s*3/);
  });

  it('有几篇暂时读不出来时如实说，不把读不出的当作没有', async () => {
    defaultRoutes(kit);
    kit.route('GET', PREVIEW, ok(snapshot({ frozenCount: 5 })));
    const { panel } = await openHomepage();
    await within(panel).findByText('本次主页样本 3 篇');
    expect(panel).toHaveTextContent('另有 2 篇暂时无法读取，仍算在本次样本里');
  });
});
