import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DOMAIN,
  OTHER_DOMAIN,
  createFetchMock,
  defaultRoutes,
  deferred,
  itemJson,
  listJson,
  ok,
  renderCreatorsPage,
  shellJson,
  workJson,
  type FetchKit,
  type Reply,
} from './testKit';

let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.install();
  sessionStorage.clear();
});
afterEach(() => {
  vi.unstubAllGlobals();
});

const creatorsCalls = () => kit.of('GET', '/api/local/creators');
const lastCreatorsCall = () => creatorsCalls().at(-1);
const table = () => screen.getByRole('table');
const rowFor = (name: string) => screen.getByRole('button', { name }).closest('tr') as HTMLTableRowElement;

async function loaded(url?: string) {
  const view = renderCreatorsPage(url);
  await screen.findByRole('table');
  return view;
}

describe('创作者页 · 列表', () => {
  it('八列表格：列名、命中数、读取时间；数字与说明都在', async () => {
    defaultRoutes(kit);
    await loaded();
    expect(within(table()).getAllByRole('columnheader').map(header => header.textContent)).toEqual([
      '创作者', '内容方向', '作者倾向', '作品表达', '相关作品', '相关最高赞', '作品预览', '观察状态',
    ]);
    expect(screen.getByText('2 位创作者')).toBeInTheDocument();
    expect(screen.getByText('读取于 2026-10-15 14:30')).toBeInTheDocument();
    expect(document.title).toBe('创作者 · 语料');
  });

  it('一行里每一格：名字、粉丝、方向、倾向与分析状态、表达、相关作品、最高赞、作品预览、观察状态', async () => {
    defaultRoutes(kit);
    await loaded();
    const row = rowFor('合成作者 1');
    const cells = within(row).getAllByRole('cell');
    expect(cells).toHaveLength(8);
    expect(cells[0]).toHaveTextContent('合成作者 1');
    expect(cells[0]).toHaveTextContent('粉丝 2,001');
    expect(cells[1]).toHaveTextContent('任务启动 · 晚间练习');
    expect(cells[2]).toHaveTextContent('本领域持续创作');
    expect(cells[2]).toHaveTextContent('已有分析结果');
    expect(cells[3]).toHaveTextContent('亲历自述');
    expect(cells[4]).toHaveTextContent('3');
    expect(cells[4]).toHaveTextContent('待判断 2');
    expect(cells[5]).toHaveTextContent('1,500');
    expect(cells[5]).toHaveTextContent('已判相关作品');
    expect(cells[6]).toHaveTextContent('合成作品 11');
    expect(cells[6]).toHaveTextContent('已判相关 · 赞 1,211');
    expect(cells[7]).toHaveTextContent('未加入观察');
    expect(within(cells[7] as HTMLElement).getByRole('button', { name: '加入观察' })).toBeEnabled();
  });

  it('未知不显示成 0：粉丝、最高赞、点赞缺失时是「—」；≥1 万缩写并在悬停里保留精确值', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { followers: null, maxLikes: null, works: [workJson(11, { likes: null })] }), itemJson(2, { followers: 123456, maxLikes: 20000 })] })));
    await loaded();
    const unknown = rowFor('合成作者 1');
    expect(within(unknown).getAllByTitle('未知').length).toBeGreaterThanOrEqual(3);
    expect(unknown).not.toHaveTextContent('粉丝 0');
    const big = rowFor('合成作者 2');
    expect(within(big).getByTitle('123,456')).toHaveTextContent('12.3万');
    expect(within(big).getByTitle('20,000')).toHaveTextContent('2万');
  });

  it('作者名缺失写「作者名称未取得」；名字取自观察目标档案时标明来源', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { name: null }), itemJson(2, { displayNameKind: 'observation_target' })] })));
    await loaded();
    expect(screen.getByRole('button', { name: '作者名称未取得' })).toBeInTheDocument();
    expect(rowFor('合成作者 2')).toHaveTextContent('观察目标名称 · 粉丝');
  });

  it('分析状态分别显示：排队 / 失败（带中文原因）/ 已分析但依据不足 / 未取得', async () => {
    defaultRoutes(
      kit,
      ok(
        listJson({
          analysisEnabled: true,
          items: [
            itemJson(1, { authorState: 'queued', authorResult: false, focusValue: 'unknown' }),
            itemJson(2, { authorState: 'failed', authorErrorCode: 'model_budget_exhausted', authorResult: false, focusValue: 'unknown' }),
            itemJson(3, { authorState: 'idle', authorResult: true, focusValue: 'unknown' }),
            itemJson(4, { authorState: 'idle', authorResult: false, focusValue: 'unknown' }),
          ],
        }),
      ),
    );
    await loaded();
    expect(rowFor('合成作者 1')).toHaveTextContent('排队待分析');
    expect(rowFor('合成作者 2')).toHaveTextContent('分析失败 · 本日额度已用尽');
    expect(rowFor('合成作者 3')).toHaveTextContent('已分析，依据不足');
    expect(rowFor('合成作者 4')).toHaveTextContent('尚未取得当前分析结果');
  });

  it('分析未开启时不把「没有结果」说成「尚未分析」', async () => {
    defaultRoutes(kit, ok(listJson({ analysisEnabled: false, items: [itemJson(1, { authorState: null, authorResult: false, focusValue: 'unknown' })] })));
    await loaded();
    expect(rowFor('合成作者 1')).toHaveTextContent('自动分析未开启');
    expect(screen.getByRole('button', { name: '发现设置' })).toHaveTextContent('自动分析未开启');
  });

  it('观察关系读不到：不列入「未纳入」，行内显示「关系待读取」，不可按观察关系筛选', async () => {
    defaultRoutes(kit, ok(listJson({ outside: null, items: [itemJson(1, { inDomain: null })] })));
    await loaded();
    const row = rowFor('合成作者 1');
    expect(row).toHaveTextContent('观察关系暂不可用');
    expect(row).toHaveTextContent('关系待读取');
    expect(within(row).queryByRole('button', { name: '加入观察' })).toBeNull();
    expect(screen.getByRole('button', { name: /^未加入观察/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^未观察的高赞作者/ })).toBeDisabled();
  });

  it('已加入的作者给出「查看目标」；其他领域已有主研究关系或目标处于暂停时不直接加入', async () => {
    defaultRoutes(
      kit,
      ok(listJson({ items: [itemJson(1, { inDomain: true }), itemJson(2, { hasOtherPrimaryDomain: true, existsInOtherDomains: true }), itemJson(3, { lifecycleState: 'paused', monitoringState: 'paused' })] })),
    );
    await loaded();
    const joined = within(rowFor('合成作者 1')).getByRole('link', { name: '查看目标' });
    expect(joined).toHaveAttribute('href', expect.stringContaining('/collection/targets?domain='));
    expect(joined.getAttribute('href')).toContain('drawer=30000000');
    expect(within(rowFor('合成作者 2')).getByRole('button', { name: '查看加入方式' })).toBeInTheDocument();
    expect(rowFor('合成作者 3')).toHaveTextContent('观察已暂停 · 未加入本领域');
    expect(within(rowFor('合成作者 3')).getByRole('button', { name: '查看加入方式' })).toBeInTheDocument();
  });

  it('数量链接带着创作者的精确作品集合去证据库，并带回本页地址', async () => {
    defaultRoutes(kit);
    await loaded(`/corpus/creators?domain=${DOMAIN}&sort=high_likes`);
    const link = within(rowFor('合成作者 1')).getAllByRole('link').find(candidate => candidate.getAttribute('href')?.startsWith('/corpus/evidence?')) as HTMLAnchorElement;
    const url = new URL(link.getAttribute('href') as string, 'http://x');
    expect(url.searchParams.get('domain')).toBe(DOMAIN);
    expect(url.searchParams.get('returnTo')).toBe(`/corpus/creators?domain=${DOMAIN}&sort=high_likes`);
    expect(JSON.parse(url.searchParams.get('creatorFilter') as string)).toMatchObject({ domain: DOMAIN, relevance: 'related' });
    expect(url.searchParams.get('creatorKey')).toMatch(/^xhs:/);
  });
});

describe('创作者页 · 领域与读取失败', () => {
  it('没选领域：不替用户挑，不发列表请求，说「请选择领域」；选择器可用', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson({ selectedDomain: null })));
    renderCreatorsPage('/corpus/creators');
    expect(await screen.findByText('请选择领域。')).toBeInTheDocument();
    expect(screen.getByText('请选择领域')).toBeInTheDocument();
    expect(creatorsCalls()).toHaveLength(0);
    expect(screen.getByLabelText('切换当前观察领域')).toHaveTextContent('选择领域');
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled();
  });

  it('地址里的领域无效（不是已有领域）也是未选，不报错', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson({ selectedDomain: null })));
    renderCreatorsPage('/corpus/creators?domain=not-a-domain');
    expect(await screen.findByText('请选择领域。')).toBeInTheDocument();
    expect(kit.of('GET', '/api/local/corpus/shell')[0]?.search.get('domain')).toBe('not-a-domain');
    expect(creatorsCalls()).toHaveLength(0);
  });

  it('一个领域也没有：说明并给出前往领域管理的出口，不是死路', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson({ domains: [], selectedDomain: null })));
    renderCreatorsPage('/corpus/creators');
    expect(await screen.findByText(/还没有可选择的研究领域/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '前往领域管理 →' })).toHaveAttribute('href', '/collection/domains');
    expect(screen.queryByLabelText('切换当前观察领域')).toBeNull();
  });

  it('领域外壳读取失败：如实说明并可重试，不发列表请求，不把失败变成空', async () => {
    kit.route('GET', '/api/local/corpus/shell', () => ({ status: 503, body: { operation: 'local_read', outcome: 'unavailable', code: 'domain_read_unavailable' } }), ok(shellJson()));
    kit.route('GET', '/api/local/creators', ok(listJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    const { user } = renderCreatorsPage();
    expect(await screen.findByText(/领域范围当前不可读/)).toBeInTheDocument();
    expect(screen.getByText('读取失败')).toBeInTheDocument();
    expect(creatorsCalls()).toHaveLength(0);
    await user.click(screen.getByRole('button', { name: '重试读取' }));
    await screen.findByRole('table');
  });

  it('列表读取失败：读取失败 + 重试读取；没有旧表格', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', () => ({ status: 503, body: { operation: 'local_read', outcome: 'unavailable', code: 'creator_read_unavailable' } }), ok(listJson()));
    const { user } = renderCreatorsPage();
    expect(await screen.findByText('读取暂不可用，请稍后重试。')).toBeInTheDocument();
    expect(screen.getByText('读取失败')).toBeInTheDocument();
    expect(screen.queryByRole('table')).toBeNull();
    await user.click(screen.getByRole('button', { name: '重试读取' }));
    await screen.findByRole('table');
    expect(creatorsCalls()).toHaveLength(2);
  });

  it('未设爆款标准时选了爆款：说明并给「改看高赞线索」', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', call => (call.search.get('viral') === 'true' ? { status: 400, body: { code: 'viral_threshold_unset' } } : { status: 200, body: listJson() }));
    const { user, location } = renderCreatorsPage(`/corpus/creators?domain=${DOMAIN}&viral=true&sort=viral_works`);
    expect(await screen.findByText(/当前未设爆款标准/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '改看高赞线索' }));
    await screen.findByRole('table');
    expect(location()).toContain('highLikes=true');
    expect(location()).toContain('sort=high_likes');
    expect(location()).not.toContain('viral=true');
  });

  it('观察关系读不到又按观察关系筛选：说明并给「清除观察条件后重试」', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', call => (call.search.get('observation') ? { status: 400, body: { code: 'observation_lookup_unavailable' } } : { status: 200, body: listJson() }));
    const { user, location } = renderCreatorsPage(`/corpus/creators?domain=${DOMAIN}&observation=outside`);
    expect(await screen.findByText('观察关系暂时无法读取。')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '清除观察条件后重试' }));
    await screen.findByRole('table');
    expect(location()).not.toContain('observation');
  });

  it('刷新失败但页面上仍是上一次读到的内容：明说不是最新，并可重试', async () => {
    defaultRoutes(kit, () => ({ status: 200, body: listJson() }));
    const { user } = await loaded();
    kit.route('GET', '/api/local/creators', () => ({ status: 503, body: { code: 'creator_read_unavailable' } }), ok(listJson()));
    await user.click(screen.getByRole('button', { name: '刷新结果' }));
    expect(await screen.findByText(/最新读取没有成功，下方仍是上一次读取到的内容/)).toBeInTheDocument();
    expect(screen.getByRole('table')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '重试读取' }));
    await waitFor(() => expect(screen.queryByText(/最新读取没有成功/)).toBeNull());
  });

  it('媒体读取失败只影响图片：文字、链接与查询仍可用', async () => {
    defaultRoutes(kit);
    kit.route('GET', '/api/local/work-resources', () => ({ status: 503, body: { code: 'x' } }));
    await loaded();
    expect(await screen.findByText(/媒体暂不可用；文字、作品链接与查询仍可使用/)).toBeInTheDocument();
    expect(screen.getAllByRole('img', { name: '头像未取得' }).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '合成作者 1' })).toBeInTheDocument();
  });

  it('领域已暂停：历史材料仍可读，写入操作不可用', async () => {
    defaultRoutes(kit, ok(listJson({ domainStatus: 'paused' })));
    await loaded();
    expect(screen.getByText(/当前领域已暂停，历史材料仍可读，写入操作不可用/)).toBeInTheDocument();
    expect(within(rowFor('合成作者 1')).getByRole('button', { name: '加入观察' })).toBeDisabled();
  });

  it('零命中：只说当前条件下没有，并给「清除额外条件」；翻页控件不可用', async () => {
    defaultRoutes(kit, ok(listJson({ items: [], total: 0 })));
    const { user } = await loaded(`/corpus/creators?domain=${DOMAIN}&query=没有的词`);
    expect(screen.getByText(/当前条件下没有找到创作者/)).toBeInTheDocument();
    expect(screen.getByText('0 位创作者')).toBeInTheDocument();
    expect(screen.getByText('0 页')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled();
    await user.click(within(screen.getByText(/当前条件下没有找到创作者/).parentElement as HTMLElement).getByRole('button', { name: '清除额外条件' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('query')).toBeNull());
  });

  it('切换领域停留在创作者页并只带 domain；晚到的上一个领域的响应不会覆盖新领域', async () => {
    const first = deferred<Reply>();
    kit.route('GET', '/api/local/corpus/shell', call => ({
      status: 200,
      body: shellJson({ selectedDomain: call.search.get('domain') }),
    }));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', call =>
      call.search.get('domain') === DOMAIN
        ? first.promise
        : { status: 200, body: { ...listJson({ items: [itemJson(9, { name: '第二领域作者' })] }), domain: { ...listJson().domain, domainRef: OTHER_DOMAIN, name: '合成领域二' } } },
    );
    const { user, location } = renderCreatorsPage(`/corpus/creators?domain=${DOMAIN}&sort=high_likes&query=旧词`);
    await waitFor(() => expect(creatorsCalls()).toHaveLength(1));
    await user.click(screen.getByLabelText('切换当前观察领域'));
    await user.click(await screen.findByRole('link', { name: /合成领域二/ }));
    await screen.findByRole('button', { name: '第二领域作者' });
    expect(location()).toBe(`/corpus/creators?domain=${OTHER_DOMAIN}`);
    first.resolve({ status: 200, body: listJson({ items: [itemJson(1, { name: '第一领域作者' })] }) });
    await new Promise(resolve => setTimeout(resolve, 30));
    expect(screen.queryByRole('button', { name: '第一领域作者' })).toBeNull();
    expect(screen.getByRole('button', { name: '第二领域作者' })).toBeInTheDocument();
    // 侧栏的链接带上新领域；停留在同一个页面。
    expect(screen.getAllByRole('link', { name: /证据库/ })[0]).toHaveAttribute('href', `/corpus/evidence?domain=${OTHER_DOMAIN}`);
  });
});

describe('创作者页 · 筛选、快捷视图与重置', () => {
  it('快捷视图：数字是范围统计；点击后地址与请求都套用，并清掉其他额外条件', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&query=旧词&traits=personal_experience`);
    const outside = screen.getByRole('button', { name: '未加入观察 20' });
    expect(screen.getByRole('button', { name: '全部创作者 27' })).toHaveAttribute('aria-pressed', 'false');
    await user.click(outside);
    await waitFor(() => expect(lastCreatorsCall()?.search.get('observation')).toBe('outside'));
    expect(lastCreatorsCall()?.search.get('query')).toBeNull();
    expect(lastCreatorsCall()?.search.get('traits')).toBeNull();
    expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&observation=outside`);
    await waitFor(() => expect(screen.getByRole('button', { name: '未加入观察 20' })).toHaveAttribute('aria-pressed', 'true'));
  });

  it('「高赞」快捷视图：没有爆款标准时用高赞线索，有标准时用爆款并按爆款排序', async () => {
    defaultRoutes(kit, ok(listJson({ threshold: null })));
    const { user, location, unmount } = await loaded();
    await user.click(screen.getByRole('button', { name: '未观察的高赞作者' }));
    await waitFor(() => expect(location()).toContain('highLikes=true'));
    expect(location()).toContain('sort=high_likes');
    unmount();
    kit.calls.length = 0;
    defaultRoutes(kit, ok(listJson({ threshold: 100 })));
    const second = await loaded();
    await second.user.click(screen.getByRole('button', { name: '未观察的爆款作者 6' }));
    await waitFor(() => expect(second.location()).toContain('viral=true'));
    expect(second.location()).toContain('sort=viral_works');
  });

  it('输入后立刻打开筛选，提交待发词后仍保留筛选窗口，应用不丢查询', async () => {
    defaultRoutes(kit); const { user, location } = await loaded();
    await user.type(screen.getByRole('searchbox'), '待发词');
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('traits')).toBe('personal_experience'));
    expect(lastCreatorsCall()?.search.get('query')).toBe('待发词'); expect(location()).toContain('query=');
  });

  it('菜单Escape不关闭筛选，草稿关闭先确认，继续保留输入，明确放弃才关闭并返回焦点', async () => {
    defaultRoutes(kit); const { user } = await loaded();
    const opener = screen.getByRole('button', { name: /^筛选/ }); await user.click(opener);
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    const select = within(dialog).getByRole('combobox', { name: '材料用途' });
    await user.click(select); await user.keyboard('{Escape}');
    expect(dialog).toBeInTheDocument(); expect(select).toHaveAttribute('aria-expanded', 'false');
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    fireEvent(dialog, new Event('cancel', { cancelable: true }));
    expect(within(dialog).getByText('筛选尚未应用，放弃本次修改？')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '继续筛选' }));
    expect(within(dialog).getByRole('checkbox', { name: '亲历自述' })).toBeChecked();
    await user.click(within(dialog).getByRole('button', { name: '关闭筛选' }));
    await user.click(within(dialog).getByRole('button', { name: '放弃修改' }));
    expect(screen.queryByRole('dialog')).toBeNull(); expect(opener).toHaveFocus();
    expect(creatorsCalls()).toHaveLength(1);
  });

  it('筛选脏稿保护浏览器后退，继续保留当前URL，明确放弃才返回上一条查询', async () => {
    defaultRoutes(kit); const { user, router, location } = await loaded();
    await user.click(screen.getByRole('button', { name: /^未加入观察/ }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('observation')).toBe('outside'));
    const current = location();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    await act(async () => { await router.navigate(-1); });
    expect(location()).toBe(current);
    await user.click(within(dialog).getByRole('button', { name: '继续筛选' }));
    expect(within(dialog).getByRole('checkbox', { name: '亲历自述' })).toBeChecked();
    await act(async () => { await router.navigate(-1); });
    await user.click(within(dialog).getByRole('button', { name: '放弃修改' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}`));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(creatorsCalls().some(call => call.search.get('traits') === 'personal_experience')).toBe(false);
  });

  it('范围统计与筛选结果分开：筛选后结果条变，快捷视图上的范围数字不变', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', call => ({ status: 200, body: listJson({ items: call.search.get('traits') ? [itemJson(1)] : [itemJson(1), itemJson(2)] }) }));
    const { user } = await loaded();
    expect(screen.getByText('2 位创作者')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '全部创作者 27' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    expect(await screen.findByText('1 位创作者')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '全部创作者 27' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '材料范围' }));
    const scope = await screen.findByRole('dialog', { name: '材料范围与判断标准' });
    expect(scope).toHaveTextContent('不随搜索与筛选改变');
    expect(within(scope).getByRole('button', { name: /范围内作者/ })).toHaveTextContent('27');
  });

  it('筛选对话框是草稿：取消不改任何东西；应用才生效，条件可组合并以芯片显示', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    let dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    await user.click(within(dialog).getByRole('combobox', { name: /作者倾向/ }));
    await user.click(screen.getByRole('option', { name: '本领域持续创作' }));
    await user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(within(dialog).getByText('筛选尚未应用，放弃本次修改？')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '放弃修改' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}`);
    expect(creatorsCalls()).toHaveLength(1);

    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    expect(within(dialog).getByRole('checkbox', { name: '亲历自述' })).not.toBeChecked();
    await user.click(within(dialog).getByRole('checkbox', { name: '亲历自述' }));
    await user.click(within(dialog).getByRole('checkbox', { name: '推广表达' }));
    await user.click(within(dialog).getByRole('combobox', { name: /作者倾向/ }));
    await user.click(screen.getByRole('option', { name: '本领域持续创作' }));
    await user.type(within(dialog).getByRole('spinbutton', { name: '最低点赞' }), '100');
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('traits')).toBe('personal_experience,explicit_promotion'));
    expect(lastCreatorsCall()?.search.get('focus')).toBe('vertical_tendency');
    expect(lastCreatorsCall()?.search.get('minLikes')).toBe('100');
    const chips = within(screen.getByRole('group', { name: '当前筛选条件' }));
    for (const text of ['亲历自述', '推广表达', '本领域持续创作', '最低点赞：100']) {
      expect(chips.getByRole('button', { name: `移除条件：${text}` })).toBeInTheDocument();
    }
    expect(screen.getByRole('button', { name: /^筛选 4/ })).toBeInTheDocument();
  });

  it('已选条件可单独移除（表达逐项）；「清除筛选」只清列表条件，领域与材料范围不动', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(
      `/corpus/creators?domain=${DOMAIN}&usageRole=reference&publishedFrom=2026-09-16&publishedTo=2026-10-16&traits=personal_experience,explicit_promotion&focus=unknown&query=词&sort=high_likes&page=0&pageSize=100`,
    );
    await user.click(screen.getByRole('button', { name: '移除条件：亲历自述' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('traits')).toBe('explicit_promotion'));
    expect(lastCreatorsCall()?.search.get('focus')).toBe('unknown');
    await user.click(screen.getByRole('button', { name: '清除筛选' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&usageRole=reference&publishedFrom=2026-09-16&publishedTo=2026-10-16&pageSize=100`));
    expect(lastCreatorsCall()?.search.get('usageRole')).toBe('reference');
    expect(lastCreatorsCall()?.search.get('publishedFrom')).toBe('2026-09-16');
    expect(screen.queryByRole('group', { name: '当前筛选条件' })).toBeNull();
  });

  it('自定义日期：结束日期不晚于开始日期时就地提示，不发请求', async () => {
    defaultRoutes(kit);
    const { user } = await loaded();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('combobox', { name: '作品发表日期' }));
    await user.click(screen.getByRole('option', { name: '自定义日期' }));
    await user.type(within(dialog).getByLabelText('开始日期'), '2026-10-16');
    await user.type(within(dialog).getByLabelText('结束日期（不含）'), '2026-10-16');
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('结束日期必须晚于开始日期');
    expect(creatorsCalls()).toHaveLength(1);
  });

  it('近 30 天换算成北京时间的日期范围写进地址', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('combobox', { name: '作品发表日期' }));
    await user.click(screen.getByRole('option', { name: '近30天' }));
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    await waitFor(() => expect(location()).toMatch(/publishedFrom=\d{4}-\d{2}-\d{2}&publishedTo=\d{4}-\d{2}-\d{2}/));
    const params = new URLSearchParams(location().split('?')[1]);
    const days = (new Date(params.get('publishedTo') as string).getTime() - new Date(params.get('publishedFrom') as string).getTime()) / 86_400_000;
    expect(days).toBe(30);
    // 再打开时显示为自定义日期，不假装还是「近 30 天」。
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    expect(within(await screen.findByRole('dialog', { name: '筛选材料与作者' })).getByRole('combobox', { name: '作品发表日期' })).toHaveTextContent('自定义日期');
  });

  it('观察关系读不到时，对话框里的「观察关系」与「有相关爆款」不可选', async () => {
    defaultRoutes(kit, ok(listJson({ outside: null, threshold: null })));
    const { user } = await loaded();
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    expect(within(dialog).getByRole('combobox', { name: '观察关系' })).toBeDisabled();
    expect(within(dialog).getByRole('checkbox', { name: '有相关爆款' })).toBeDisabled();
    expect(within(dialog).getByText(/尚未设置爆款标准；仍可筛选高赞线索/)).toBeInTheDocument();
  });

  it('对话框内「清除额外条件」只重置草稿（含搜索词与排序），应用后才生效', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&usageRole=reference&query=词&traits=personal_experience&sort=high_likes`);
    await user.click(screen.getByRole('button', { name: /^筛选/ }));
    const dialog = await screen.findByRole('dialog', { name: '筛选材料与作者' });
    await user.click(within(dialog).getByRole('button', { name: '清除额外条件' }));
    expect(within(dialog).getByRole('checkbox', { name: '亲历自述' })).not.toBeChecked();
    expect(location()).toContain('query=');
    await user.click(within(dialog).getByRole('button', { name: '应用筛选' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&usageRole=reference`));
  });
});

describe('创作者页 · 搜索、排序、分页', () => {
  it('输入停止后才查询；回到第一页；地址与输入框同步', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&page=2`);
    await user.type(screen.getByRole('searchbox', { name: '搜索创作者或作品' }), '开始');
    expect(creatorsCalls().filter(call => call.search.get('query'))).toHaveLength(0);
    await waitFor(() => expect(lastCreatorsCall()?.search.get('query')).toBe('开始'), { timeout: 2000 });
    expect(lastCreatorsCall()?.search.get('page')).toBe('0');
    expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&query=${encodeURIComponent('开始')}`);
    await user.click(screen.getByRole('button', { name: '清空搜索' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}`));
    expect(screen.getByRole('searchbox', { name: '搜索创作者或作品' })).toHaveValue('');
  });

  it('切换搜索范围带着当前输入一起提交', async () => {
    defaultRoutes(kit);
    const { user } = await loaded();
    await user.type(screen.getByRole('searchbox', { name: '搜索创作者或作品' }), '正文词');
    await user.click(screen.getByRole('combobox', { name: '搜索范围' }));
    await user.click(screen.getByRole('option', { name: '作品内容' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('searchMode')).toBe('work'));
    expect(lastCreatorsCall()?.search.get('query')).toBe('正文词');
  });

  it('「/」把焦点放进搜索框（输入框里或对话框开着时不抢）', async () => {
    defaultRoutes(kit);
    const { user } = await loaded();
    await user.keyboard('/');
    expect(screen.getByRole('searchbox', { name: '搜索创作者或作品' })).toHaveFocus();
    expect(screen.getByRole('searchbox', { name: '搜索创作者或作品' })).toHaveValue('');
    await user.keyboard('{Escape}');
  });

  it('排序：下拉与表头都改 sort 并回到第一页；表头标出当前排序列；爆款排序在没有标准时不可选', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&page=1`);
    await user.click(screen.getByRole('combobox', { name: '排序' }));
    expect(screen.getByRole('option', { name: '命中相关爆款数' })).toHaveAttribute('aria-disabled', 'true');
    await user.keyboard('{Escape}');
    await user.click(screen.getByRole('button', { name: '相关作品' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&sort=related_works`));
    expect(screen.getByRole('columnheader', { name: '相关作品' })).toHaveAttribute('aria-sort', 'descending');
    await user.click(screen.getByRole('combobox', { name: '排序' }));
    await user.click(screen.getByRole('option', { name: '命中最高点赞' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('sort')).toBe('high_likes'));
    // 按最高赞排序时表头改称「命中最高赞」，仍标出当前排序列。
    expect(screen.getByRole('columnheader', { name: '命中最高赞' })).toHaveAttribute('aria-sort', 'descending');
    expect(screen.getByRole('columnheader', { name: '相关作品' })).not.toHaveAttribute('aria-sort');
  });

  it('筛选或高赞模式下，表头改称「命中相关」「命中最高赞」', async () => {
    defaultRoutes(kit);
    await loaded(`/corpus/creators?domain=${DOMAIN}&relevance=related&sort=high_likes`);
    expect(screen.getByRole('button', { name: '命中相关' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '命中最高赞' })).toBeInTheDocument();
    expect(rowFor('合成作者 1')).toHaveTextContent('范围内 3');
    expect(rowFor('合成作者 1')).toHaveTextContent('当前命中作品');
  });

  it('分页：翻页写进地址，请求的页码从 0 起；改每页数量回到第一页', async () => {
    defaultRoutes(kit, ok(listJson({ total: 60, pageSize: 25 })));
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&pageSize=25`);
    expect(screen.getByText('1 / 3 页')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '下一页' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('page')).toBe('1'));
    expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&page=1&pageSize=25`);
    await screen.findByText('2 / 3 页');
    await user.click(screen.getByRole('combobox', { name: /每页/ }));
    await user.click(screen.getByRole('option', { name: '50' }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('pageSize')).toBe('50'));
    expect(lastCreatorsCall()?.search.get('page')).toBe('0');
  });

  it('地址里的页码比现有总数还大：回到最后一页并替换历史，不循环请求', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', call => ({ status: 200, body: listJson({ total: 60, pageSize: 25, page: Number(call.search.get('page')) }) }));
    const { location } = renderCreatorsPage(`/corpus/creators?domain=${DOMAIN}&page=9&pageSize=25`);
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&page=2&pageSize=25`));
    await screen.findByText('3 / 3 页');
    expect(creatorsCalls().map(call => call.search.get('page'))).toEqual(['9', '2']);
  });

  it('总数为 0 而页码不为 0：回到第 0 页，不循环请求', async () => {
    kit.route('GET', '/api/local/corpus/shell', ok(shellJson()));
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.route('GET', '/api/local/creators', ok(listJson({ items: [], total: 0 })));
    const { location } = renderCreatorsPage(`/corpus/creators?domain=${DOMAIN}&page=4`);
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}`));
    await screen.findByText('0 页');
    expect(creatorsCalls()).toHaveLength(2);
  });

  it('「刷新结果」在第一页就地重读；不在第一页先回到第一页', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&page=1&pageSize=25`);
    kit.calls.length = 0;
    kit.route('GET', '/api/local/creators', ok(listJson({ total: 60, pageSize: 25 })));
    await user.click(screen.getByRole('button', { name: '刷新结果' }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&pageSize=25`));
    const before = creatorsCalls().length;
    await user.click(screen.getByRole('button', { name: '刷新结果' }));
    await waitFor(() => expect(creatorsCalls().length).toBe(before + 1));
  });
});

describe('创作者页 · 材料范围与判断标准', () => {
  it('材料范围对话框：统计、爆款标准说明、作者身份待补链接、发布时间未知；领域说明与目标缺失时写「尚未填写」', async () => {
    defaultRoutes(kit, ok(listJson({ threshold: 100, description: null, researchGoal: null })));
    const { user } = await loaded(`/corpus/creators?domain=${DOMAIN}&publishedFrom=2026-09-16&publishedTo=2026-10-16`);
    await user.click(screen.getByRole('button', { name: '材料范围' }));
    const dialog = await screen.findByRole('dialog', { name: '材料范围与判断标准' });
    expect(dialog).toHaveTextContent('相关爆款标准：点赞不少于 100');
    expect(dialog).toHaveTextContent('当前范围 80 篇作品 · 已确认相关 20 位作者');
    expect(dialog).toHaveTextContent('领域说明：尚未填写');
    expect(dialog).toHaveTextContent('研究目标：尚未填写');
    expect(dialog).toHaveTextContent('本领域未开启');
    const unknownAuthors = within(dialog).getByRole('link', { name: '作者身份待补 4 篇' });
    expect(unknownAuthors.getAttribute('href')).toContain('/corpus/evidence?');
    expect(JSON.parse(new URL(unknownAuthors.getAttribute('href') as string, 'http://x').searchParams.get('creatorFilter') as string)).toMatchObject({ unknownAuthor: true });
    await user.click(within(dialog).getByRole('button', { name: /另有 3 篇发布时间未知/ }));
    await waitFor(() => expect(lastCreatorsCall()?.search.get('publishedFrom')).toBeNull());
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('没有爆款标准：不拿 0 或默认值冒充，高赞入口写「查看 →」', async () => {
    defaultRoutes(kit, ok(listJson({ threshold: null })));
    const { user } = await loaded();
    await user.click(screen.getByRole('button', { name: '材料范围' }));
    const dialog = await screen.findByRole('dialog', { name: '材料范围与判断标准' });
    expect(dialog).toHaveTextContent('尚未设置爆款标准，高赞线索仅按已有点赞排序');
    expect(within(dialog).getByRole('button', { name: /未观察的高赞作者/ })).toHaveTextContent('查看 →');
  });

  it('判断标准常驻可见：六项门槛、「作品表达」与「作者倾向」分层且可并存、分析状态的含义（#384 发现 1、2）', async () => {
    defaultRoutes(kit);
    const { user } = await loaded();
    await user.click(screen.getByRole('button', { name: '材料范围' }));
    const dialog = await screen.findByRole('dialog', { name: '材料范围与判断标准' });
    const criteria = within(dialog).getByRole('region', { name: '判断标准' });
    for (const term of ['亲历自述', '知识与方法讲解', '推广表达', '机构／品牌自述', '作者在本领域的内容倾向', '与当前领域的关系']) {
      expect(within(criteria).getByText(term)).toBeInTheDocument();
    }
    expect(criteria).toHaveTextContent('至少两篇独立相关作品');
    expect(criteria).toHaveTextContent('「作品」层面的表达');
    expect(criteria).toHaveTextContent('「作者」层面的倾向');
    const states = within(dialog).getByRole('region', { name: '分析状态说明' });
    expect(states).toHaveTextContent('已分析，依据不足');
    expect(states).toHaveTextContent('不是失败');
  });

  it('统计按钮点击 = 只看此项并清掉其他条件，对话框随之收起', async () => {
    defaultRoutes(kit);
    const { user, location } = await loaded(`/corpus/creators?domain=${DOMAIN}&query=词`);
    await user.click(screen.getByRole('button', { name: '材料范围' }));
    await user.click(within(await screen.findByRole('dialog', { name: '材料范围与判断标准' })).getByRole('button', { name: /有亲历自述作品/ }));
    await waitFor(() => expect(location()).toBe(`/corpus/creators?domain=${DOMAIN}&traits=personal_experience`));
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('创作者页 · 外壳', () => {
  it('采集项显示服务器给出的状态词（与证据库一致）；语料是当前入口；页头读数在面包屑一侧', async () => {
    defaultRoutes(kit);
    await loaded();
    expect(screen.getByRole('link', { name: /采集/ }).querySelector('.v7-nav-state')).toHaveTextContent('观察中');
    expect(screen.getByRole('link', { name: /语料/ })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByLabelText('切换当前观察领域')).toHaveTextContent('合成领域一');
    expect(screen.getByRole('complementary', { name: '语料导航' })).toHaveTextContent('作者来自当前领域已有作品');
  });

  it('材料范围摘要：用途与日期', async () => {
    defaultRoutes(kit);
    await loaded(`/corpus/creators?domain=${DOMAIN}&usageRole=reference&publishedFrom=2026-09-16`);
    expect(screen.getByText('参照 · 2026-09-16 至 不限（不含）')).toBeInTheDocument();
  });
});
