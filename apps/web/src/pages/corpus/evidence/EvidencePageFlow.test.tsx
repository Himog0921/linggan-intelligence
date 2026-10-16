import { act, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOMAIN, listJson, workRef } from './evidenceFixtures';
import { LIST_PATH, detailPathOf, items, pageRoutes, pageUrl, renderEvidencePage, stubViewport } from './pageKit';
import { createFetchMock, fail, type Call, type FetchKit, type Reply } from './testKit';

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
const lastList = () => listCalls().at(-1)!;
/** 检查器（语料侧栏也是 complementary，所以按检查器的标题认）。 */
const aside = () => screen.getByRole('complementary', { name: /^(合成作品 \d+|请选择)/ });
const rowButton = (n: number) => screen.getByRole('button', { name: `合成作品 ${n}` });
const withWork = (n: number, search = `domain=${DOMAIN}`) => `/corpus/evidence?${search}&work=${workRef(n)}`;

/** 按 `q` 给两套结果：没有检索词是 1–3，检索词 abc 是 4–5。 */
function searchable(call: Call): Reply {
  return { status: 200, body: call.search.get('q') === 'abc' ? listJson(items(2, () => ({}), 4)) : listJson(items(3)) };
}

async function opened(url?: string) {
  const view = renderEvidencePage(url);
  await screen.findByRole('button', { name: '合成作品 1' });
  await waitFor(() => expect(view.location()).toContain('work='));
  return view;
}

describe('选中与历史', () => {
  it('点选另一行：新增一条历史、地址带上它、读它的详情；后退回到上一件作品', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    expect(view.location()).toBe(withWork(2));
    await waitFor(() => expect(kit.of('GET', detailPathOf(2))).toHaveLength(1));
    await waitFor(() => expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 2' })).toBeInTheDocument());
    expect(rowButton(2)).toHaveAttribute('aria-current', 'true');
    expect(rowButton(1)).not.toHaveAttribute('aria-current');

    await act(() => view.router.navigate(-1));
    expect(view.location()).toBe(withWork(1));
    await waitFor(() => expect(rowButton(1)).toHaveAttribute('aria-current', 'true'));
    expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 1' })).toBeInTheDocument();
  });

  it('方向键移动选中是替换历史，不新增（后退不会逐行倒退）', async () => {
    pageRoutes(kit);
    const view = await opened();
    rowButton(1).focus();
    await view.user.keyboard('{ArrowDown}');
    expect(view.location()).toBe(withWork(2));
    expect(view.router.state.historyAction).toBe('REPLACE');
    expect(rowButton(2)).toHaveFocus();
    await view.user.keyboard('{End}');
    expect(view.location()).toBe(withWork(3));
    expect(view.router.state.historyAction).toBe('REPLACE');
  });

  it('点勾选框只勾选，不触发行选择（F07）', async () => {
    pageRoutes(kit);
    const view = await opened();
    const boxes = screen.getAllByRole('checkbox', { name: '勾选作品用于批量操作' });
    await view.user.click(boxes[1]!);
    expect(boxes[1]).toBeChecked();
    expect(view.location()).toBe(withWork(1));
    expect(screen.getByText('已勾选 1 篇作品')).toBeInTheDocument();
  });

  it('从地址整体还原：排版、标签、宽度、作品都来自地址', async () => {
    pageRoutes(kit);
    const view = renderEvidencePage(pageUrl(`domain=${DOMAIN}&layout=table&tab=trace&panel=wide&work=${workRef(2)}`));
    await screen.findByRole('table');
    expect(screen.getByRole('button', { name: '表格' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('tab', { name: '来源轨迹' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: '加宽' })).toHaveAttribute('aria-pressed', 'true');
    await waitFor(() => expect(kit.of('GET', detailPathOf(2))).toHaveLength(1));
    expect(view.location()).toBe(pageUrl(`domain=${DOMAIN}&layout=table&tab=trace&panel=wide&work=${workRef(2)}`));
  });

  it('无法识别的参数值按默认处理，不上送、不把页面带坏', async () => {
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&layout=zzz&tab=zzz&panel=zzz&view=zzz&lane=zzz&laneState=RETIRED&mediaKind=zzz&sort=relevance&window=last_7_days&work=not-a-uuid`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect([...lastList().search.keys()]).toEqual(['domain']);
    expect(screen.getByRole('tab', { name: '概览' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: '研读' })).toHaveAttribute('aria-pressed', 'true');
  });
});

describe('检索与筛选', () => {
  it('提交检索：新增历史、请求带检索词、选中复位到新结果的第一项；后退回到原来的结果', async () => {
    pageRoutes(kit, { count: 5, list: searchable });
    const view = await opened();
    await view.user.type(screen.getByRole('searchbox', { name: '检索作品与证据' }), 'abc{Enter}');
    await screen.findByRole('button', { name: '合成作品 4' });
    expect(lastList().search.get('q')).toBe('abc');
    expect(view.location()).toBe(withWork(4, `domain=${DOMAIN}&q=abc`));
    await waitFor(() => expect(kit.of('GET', detailPathOf(4))).toHaveLength(1));
    expect(screen.queryByRole('button', { name: '合成作品 1' })).not.toBeInTheDocument();

    await act(() => view.router.navigate(-1));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(screen.getByRole('searchbox', { name: '检索作品与证据' })).toHaveValue('');
    expect(view.location()).toBe(withWork(1));
  });

  it('检索只在提交时进入地址：输入过程中不读取、不改地址', async () => {
    pageRoutes(kit);
    const view = await opened();
    const before = listCalls().length;
    await view.user.type(screen.getByRole('searchbox', { name: '检索作品与证据' }), 'abc');
    expect(listCalls()).toHaveLength(before);
    expect(view.location()).toBe(withWork(1));
  });

  it('提交会把系统视图复位成「全部材料」', async () => {
    pageRoutes(kit);
    const view = await opened(pageUrl(`domain=${DOMAIN}&view=risk`));
    expect(screen.getByRole('button', { name: '风险停止' })).toHaveAttribute('aria-pressed', 'true');
    await view.user.type(screen.getByRole('searchbox', { name: '检索作品与证据' }), 'x{Enter}');
    await waitFor(() => expect(screen.getByRole('button', { name: '全部材料' })).toHaveAttribute('aria-pressed', 'true'));
    expect(view.location()).not.toContain('view=');
  });

  it('提交没有变化的条件也重新读取（旧页的行为）', async () => {
    pageRoutes(kit);
    const view = await opened();
    expect(listCalls()).toHaveLength(1);
    await view.user.click(screen.getByRole('button', { name: '读取材料' }));
    await waitFor(() => expect(listCalls()).toHaveLength(2));
    await screen.findByRole('button', { name: '合成作品 1' });
  });

  it('筛选：改一个下拉就新增历史并重新读取，徽标计数；「清空筛选」全部复位', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: /^筛选/ }));
    await view.user.selectOptions(screen.getByRole('combobox', { name: '材料通道' }), 'comments');
    await waitFor(() => expect(lastList().search.get('lane')).toBe('comments'));
    expect(view.location()).toContain('lane=comments');
    expect(screen.getByLabelText('已选 1 项筛选')).toBeInTheDocument();

    await view.user.selectOptions(screen.getByRole('combobox', { name: '通道状态' }), 'PARTIAL');
    await waitFor(() => expect(lastList().search.get('laneState')).toBe('PARTIAL'));
    expect(screen.getByLabelText('已选 2 项筛选')).toBeInTheDocument();

    await view.user.click(screen.getByRole('button', { name: '清空筛选' }));
    await waitFor(() => expect(lastList().search.has('lane')).toBe(false));
    expect(lastList().search.has('laneState')).toBe(false);
    expect(screen.getByLabelText('已选 0 项筛选')).toBeInTheDocument();
  });

  it('改一个下拉新增一条历史：后退回到筛选之前的结果', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: /^筛选/ }));
    await view.user.selectOptions(screen.getByRole('combobox', { name: '材料通道' }), 'comments');
    await waitFor(() => expect(lastList().search.get('lane')).toBe('comments'));
    await screen.findByRole('button', { name: '合成作品 1' });
    await waitFor(() => expect(view.location()).toBe(withWork(1, `domain=${DOMAIN}&lane=comments`)));
    await act(() => view.router.navigate(-1));
    await waitFor(() => expect(lastList().search.has('lane')).toBe(false));
    expect(view.location()).toBe(withWork(1));
  });

  it('筛选浮层：Esc 关闭并把焦点还给筛选按钮；点外部也关闭', async () => {
    pageRoutes(kit);
    const view = await opened();
    const toggle = screen.getByRole('button', { name: /^筛选/ });
    await view.user.click(toggle);
    expect(screen.getByRole('group', { name: '筛选条件' })).toBeInTheDocument();
    await view.user.keyboard('{Escape}');
    expect(screen.queryByRole('group', { name: '筛选条件' })).not.toBeInTheDocument();
    expect(toggle).toHaveFocus();
    await view.user.click(toggle);
    await view.user.click(document.body);
    expect(screen.queryByRole('group', { name: '筛选条件' })).not.toBeInTheDocument();
  });

  it('系统视图展开成接口条件并覆盖同名的手选筛选；地址里只写 view', async () => {
    pageRoutes(kit);
    const view = await opened(pageUrl(`domain=${DOMAIN}&laneState=OBSERVED`));
    expect(lastList().search.get('laneState')).toBe('OBSERVED');
    await view.user.click(screen.getByRole('button', { name: '部分取得' }));
    await waitFor(() => expect(lastList().search.get('laneState')).toBe('PARTIAL'));
    expect(view.location()).toContain('view=partial');
    expect(view.location()).not.toContain('laneState=PARTIAL');

    await view.user.click(screen.getByRole('button', { name: '媒体已清理' }));
    await waitFor(() => expect(lastList().search.get('lane')).toBe('media_bytes'));
    expect(lastList().search.get('laneState')).toBe('BYTES_CLEANED');
    await view.user.click(screen.getByRole('button', { name: '撤回或受限' }));
    await waitFor(() => expect(lastList().search.get('restriction')).toBe('WITHDRAWN_OR_RESTRICTED'));
  });

  it('排版切换只重排已读到的作品：不重新读取、保持选中、新增历史', async () => {
    pageRoutes(kit);
    const view = await opened();
    const before = listCalls().length;
    await view.user.click(screen.getByRole('button', { name: '表格' }));
    expect(await screen.findByRole('table')).toBeInTheDocument();
    await waitFor(() => expect(view.location()).toBe(withWork(1, `domain=${DOMAIN}&layout=table`)));
    expect(listCalls()).toHaveLength(before);
    expect(screen.getByRole('button', { name: '合成作品 1' })).toHaveAttribute('aria-current', 'true');
    await view.user.click(screen.getByRole('button', { name: '封面' }));
    expect(view.location()).toContain('layout=cover');
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
    // 新增历史：后退一步回到表格，再一步回到研读。
    await act(() => view.router.navigate(-1));
    expect(await screen.findByRole('table')).toBeInTheDocument();
    await act(() => view.router.navigate(-1));
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
    expect(view.location()).toBe(withWork(1));
  });
});

describe('继续读取', () => {
  const twoPages: (call: Call) => Reply = call =>
    call.search.get('cursor') === 'c1'
      ? { status: 200, body: listJson(items(2, () => ({}), 4)) }
      : { status: 200, body: listJson(items(3), { cursor: 'c1' }) };

  it('有游标才有「继续读取作品」；追加在末尾、保持当前选中，游标用完按钮消失', async () => {
    pageRoutes(kit, { count: 5, list: twoPages });
    const view = await opened();
    await view.user.click(rowButton(2));
    await view.user.click(screen.getByRole('button', { name: '继续读取作品' }));
    await screen.findByRole('button', { name: '合成作品 5' });
    expect(lastList().search.get('cursor')).toBe('c1');
    const titles = within(screen.getByRole('list', { name: '作品材料集合' })).getAllByRole('heading', { level: 2 });
    expect(titles.map(heading => heading.textContent)).toEqual(['合成作品 1', '合成作品 2', '合成作品 3', '合成作品 4', '合成作品 5']);
    expect(view.location()).toBe(withWork(2));
    expect(screen.queryByRole('button', { name: '继续读取作品' })).not.toBeInTheDocument();
    expect(document.getElementById('ev-rail-count')).toHaveTextContent('5');
  });

  it('续读的游标不进地址', async () => {
    pageRoutes(kit, { count: 5, list: twoPages });
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '继续读取作品' }));
    await screen.findByRole('button', { name: '合成作品 4' });
    expect(view.location()).not.toContain('cursor');
  });

  it('追加失败：已读到的作品保留，列表下方给内联说明，同一个按钮可以再试', async () => {
    let second: Reply = { status: 503, body: { operation: 'local_read', outcome: 'unavailable', code: 'invalid_material_query_cursor_or_sort' } };
    pageRoutes(kit, { count: 5, list: call => (call.search.get('cursor') === 'c1' ? second : { status: 200, body: listJson(items(3), { cursor: 'c1' }) }) });
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '继续读取作品' }));
    expect(await screen.findByText(/继续读取没有成功，已读到的作品仍保留/)).toBeInTheDocument();
    expect(screen.getAllByText('invalid_material_query_cursor_or_sort').length).toBeGreaterThanOrEqual(1);
    expect(rowButton(3)).toBeInTheDocument();
    expect(screen.queryByText('本机材料读取暂时不可用')).not.toBeInTheDocument();

    second = { status: 200, body: listJson(items(2, () => ({}), 4)) };
    await view.user.click(screen.getByRole('button', { name: '继续读取作品' }));
    await screen.findByRole('button', { name: '合成作品 5' });
    expect(screen.queryByText(/继续读取没有成功/)).not.toBeInTheDocument();
  });

  it('续读读到已经有的作品时去重，保留先到的', async () => {
    pageRoutes(kit, {
      count: 5,
      list: call => (call.search.get('cursor') === 'c1' ? { status: 200, body: listJson(items(3, () => ({}), 3)) } : { status: 200, body: listJson(items(3), { cursor: 'c1' }) }),
    });
    const view = await opened();
    await view.user.click(screen.getByRole('button', { name: '继续读取作品' }));
    await screen.findByRole('button', { name: '合成作品 5' });
    expect(screen.getAllByRole('button', { name: '合成作品 3' })).toHaveLength(1);
    expect(document.getElementById('ev-rail-count')).toHaveTextContent('5');
  });

  it('换检索条件是另一份结果：旧游标不会带到新条件上', async () => {
    pageRoutes(kit, {
      count: 5,
      list: call => (call.search.get('q') === 'abc' ? { status: 200, body: listJson(items(2, () => ({}), 4)) } : { status: 200, body: listJson(items(3), { cursor: 'c1' }) }),
    });
    const view = await opened();
    await view.user.type(screen.getByRole('searchbox', { name: '检索作品与证据' }), 'abc{Enter}');
    await screen.findByRole('button', { name: '合成作品 4' });
    expect(lastList().search.has('cursor')).toBe(false);
    expect(screen.queryByRole('button', { name: '继续读取作品' })).not.toBeInTheDocument();
  });
});

describe('入站链接与返回横幅', () => {
  const creatorsReturn = `/corpus/creators?domain=${DOMAIN}&page=2`;
  const filter = JSON.stringify({ authorName: '合成作者' });

  it('创作者作品集合：条件随列表请求上送（returnTo 不上送）；「返回创作者」回到原来的位置', async () => {
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&creatorFilter=${encodeURIComponent(filter)}&creatorKey=xhs%3A61&returnTo=${encodeURIComponent(creatorsReturn)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(lastList().search.get('creatorFilter')).toBe(filter);
    expect(lastList().search.get('creatorKey')).toBe('xhs:61');
    expect(lastList().search.has('returnTo')).toBe(false);
    expect(screen.getByText(/当前为创作者作品筛选集合/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '返回创作者' })).toHaveAttribute('href', creatorsReturn);
  });

  it('返回目标不是创作者页的同源路径就回创作者页本身', async () => {
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&creatorKey=xhs%3A61&returnTo=${encodeURIComponent('https://evil.example/corpus/creators?x=1')}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(screen.getByRole('link', { name: '返回创作者' })).toHaveAttribute('href', `/corpus/creators?domain=${DOMAIN}`);
  });

  it('本次采集的固定作品集合：横幅与「返回采集任务」；条件随列表请求上送', async () => {
    const run = '30000000-0000-4000-8000-000000000001';
    const back = `/collection/tasks?domain=${DOMAIN}&run=${run}`;
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&collectionRunRef=${run}&returnTo=${encodeURIComponent(back)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(lastList().search.get('collectionRunRef')).toBe(run);
    expect(screen.getByText(/当前为本次采集的固定作品；详情与评论按本次取得／明确复用版本回查；媒体为共享当前资源。/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '返回采集任务' })).toHaveAttribute('href', back);
  });

  it('采集任务的返回目标不合规时回到本次采集的任务页', async () => {
    const run = '30000000-0000-4000-8000-000000000001';
    pageRoutes(kit);
    renderEvidencePage(pageUrl(`domain=${DOMAIN}&collectionRunRef=${run}&returnTo=${encodeURIComponent('/somewhere/else')}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    expect(screen.getByRole('link', { name: '返回采集任务' })).toHaveAttribute('href', `/collection/tasks?domain=${DOMAIN}&run=${run}`);
  });

  it('筛选条件与返回目标在页内每次改动之后都原样写回地址', async () => {
    const run = '30000000-0000-4000-8000-000000000001';
    const back = `/collection/tasks?domain=${DOMAIN}&run=${run}`;
    pageRoutes(kit);
    const view = renderEvidencePage(pageUrl(`domain=${DOMAIN}&collectionRunRef=${run}&returnTo=${encodeURIComponent(back)}`));
    await screen.findByRole('button', { name: '合成作品 1' });
    await view.user.click(screen.getByRole('button', { name: '表格' }));
    const url = new URL(view.location(), 'http://local.invalid');
    expect(url.searchParams.get('domain')).toBe(DOMAIN);
    expect(url.searchParams.get('collectionRunRef')).toBe(run);
    expect(url.searchParams.get('returnTo')).toBe(back);
  });
});

describe('失败的写入口不存在', () => {
  it('页面从不在读取路径上发写请求，即使连续切换作品与排版', async () => {
    pageRoutes(kit);
    const view = await opened();
    await view.user.click(rowButton(2));
    await view.user.click(screen.getByRole('button', { name: '表格' }));
    await view.user.click(screen.getByRole('tab', { name: '证据' }));
    expect(kit.calls.filter(call => call.method !== 'GET')).toEqual([]);
  });

  it('详情读取失败不影响列表继续切换', async () => {
    pageRoutes(kit);
    kit.route('GET', detailPathOf(2), fail(503, 'material_detail_unavailable'));
    const view = await opened();
    await view.user.click(rowButton(2));
    expect(await screen.findByText('当前作品详情读取失败')).toBeInTheDocument();
    await view.user.click(rowButton(3));
    await waitFor(() => expect(screen.queryByText('当前作品详情读取失败')).not.toBeInTheDocument());
    expect(kit.of('GET', detailPathOf(3))).toHaveLength(1);
  });

  it('晚到的旧详情不会盖住新选中的作品', async () => {
    pageRoutes(kit);
    let release!: () => void;
    const gate = new Promise<void>(resolve => {
      release = resolve;
    });
    const detail2 = (await import('./evidenceFixtures')).detailJson(2);
    kit.route('GET', detailPathOf(2), async () => {
      await gate;
      return { status: 200, body: detail2 };
    });
    const view = await opened();
    await view.user.click(rowButton(2));
    await view.user.click(rowButton(3));
    await waitFor(() => expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 3' })).toBeInTheDocument());
    release();
    await new Promise(resolve => setTimeout(resolve, 20));
    expect(within(aside()).getByRole('heading', { level: 2, hidden: true, name: '合成作品 3' })).toBeInTheDocument();
    expect(within(aside()).queryByRole('heading', { level: 2, hidden: true, name: '合成作品 2' })).not.toBeInTheDocument();
  });
});
