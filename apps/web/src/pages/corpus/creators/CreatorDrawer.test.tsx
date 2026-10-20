import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DOMAIN,
  createFetchMock,
  defaultRoutes,
  itemJson,
  listJson,
  ok,
  renderCreatorsPage,
  workJson,
  workRef,
  type FetchKit,
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

async function openDrawer(name = '合成作者 1', url?: string) {
  const view = renderCreatorsPage(url);
  await view.user.click(await screen.findByRole('button', { name }));
  const dialog = await screen.findByRole('dialog', { name });
  return { ...view, dialog };
}

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;

describe('依据抽屉 · 结构与结论在先', () => {
  it('头部：名字、领域 · 用途 · 小红书 · 粉丝，三项数字；三个分区，默认概览', async () => {
    defaultRoutes(kit);
    const { dialog } = await openDrawer();
    expect(dialog).toHaveTextContent('合成领域一 · 主研究材料 · 小红书 · 粉丝 2,001');
    expect(dialog).toHaveTextContent('相关作品3');
    expect(dialog).toHaveTextContent('待判断2');
    expect(dialog).toHaveTextContent('相关最高赞1,500');
    const tabs = within(dialog).getAllByRole('tab');
    expect(tabs.map(tab => tab.textContent)).toEqual(['概览', '作品与依据', '作者资料']);
    expect(tabs.map(tab => tab.getAttribute('aria-selected'))).toEqual(['true', 'false', 'false']);
    expect(within(dialog).getByRole('tabpanel', { name: '概览' })).toBeVisible();
  });

  it('概览先给结论：作者内容倾向在最前面，其后才是方向与表达、作品预览', async () => {
    defaultRoutes(kit);
    const { dialog } = await openDrawer();
    const panel = within(dialog).getByRole('tabpanel', { name: '概览' });
    const headings = within(panel).getAllByRole('heading', { level: 3 }).map(heading => heading.textContent);
    expect(headings.slice(0, 3)).toEqual(['作者内容倾向', '内容方向与作品表达', '当前条件的作品预览']);
    const conclusion = within(panel).getByRole('region', { name: '作者内容倾向' });
    expect(conclusion).toHaveTextContent('本领域持续创作');
    expect(conclusion).toHaveTextContent('简介与已有作品呈现相近的内容方向');
    expect(conclusion).toHaveTextContent('自动依据更新于 2026-10-14 10:00');
    expect(within(conclusion).getByRole('button', { name: '调整判断' })).toBeEnabled();
  });

  it('作者倾向与作品表达是两个层次：各有各的门槛，并存关系写明在抽屉里（#384 发现 1）', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    const conclusion = within(dialog).getByRole('region', { name: '作者内容倾向' });
    expect(conclusion).toHaveTextContent('作者在本领域的内容倾向：持续创作倾向须至少两篇独立相关作品');
    const expression = within(dialog).getByRole('region', { name: '内容方向与作品表达' });
    expect(expression).toHaveTextContent('表达属于作品层面，可以同时出现，也与上面的作者倾向并存、互不推出');
    expect(expression).toHaveTextContent('亲历自述');
    await user.click(within(dialog).getByText('判断标准与材料范围'));
    expect(dialog).toHaveTextContent('「作者」层面的倾向');
    // 作品层的门槛在「作品与依据」里，随每一项判断出现。
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    expect(within(dialog).getAllByText(/判定门槛：/).length).toBeGreaterThanOrEqual(4);
    expect(dialog).toHaveTextContent('判定门槛：具体的本人或家庭经历、行动、过程或感受');
  });

  it('实际取样篇数：主线 worker 没写这两个字段时如实说「未记录」，不显示成 0', async () => {
    defaultRoutes(kit);
    const { dialog } = await openDrawer();
    expect(within(dialog).getByRole('region', { name: '作者内容倾向' })).toHaveTextContent('本次实际取样篇数未记录。');
    expect(dialog).not.toHaveTextContent(/实际使用 0 篇/);
  });

  it('取样篇数字段存在就显示实际数与当时已有数', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { sampleWorkCount: 12, availableWorkCount: 24 })] })));
    const { dialog } = await openDrawer();
    expect(within(dialog).getByRole('region', { name: '作者内容倾向' })).toHaveTextContent('本次实际使用 12 篇作品摘要；分析当时已有 24 篇。');
  });

  it('作品预览最多三篇，带相关性与赞藏评；「查看全部命中作品」去证据库', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { works: [workJson(11), workJson(12), workJson(13)], unknown: 1 })] })));
    const { dialog } = await openDrawer();
    const preview = within(dialog).getByRole('region', { name: '当前条件的作品预览' });
    expect(within(preview).getAllByRole('link', { name: /合成作品/ })).toHaveLength(3);
    expect(preview).toHaveTextContent('已判相关 · 赞 1,211 · 藏 30 · 评 8');
    const all = within(preview).getByRole('link', { name: '查看全部 3 篇命中作品 →' });
    expect(all.getAttribute('href')).toContain('/corpus/evidence?');
  });

  it('普通层不露机器语言与 UUID：整个抽屉里看不到作品编号、片段编号、观察目标编号、作者键', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { inDomain: true })] })));
    const { dialog, user } = await openDrawer();
    for (const tab of ['概览', '作品与依据', '作者资料']) {
      await user.click(within(dialog).getByRole('tab', { name: tab }));
      expect(dialog.textContent).not.toMatch(UUID);
      expect(dialog.textContent).not.toMatch(/xhs:[0-9a-f]{6,}/);
    }
  });
});

describe('依据抽屉 · 作品与依据（按作品分组）', () => {
  it('每篇作品一个可折叠的块；展开后是这篇作品的四项判断，各带门槛、理由、来源与引文', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '作品与依据' });
    expect(panel).toHaveTextContent('范围内 5 篇 · 当前条件命中 2 篇');
    const groups = panel.querySelectorAll('details[data-work]');
    expect(groups).toHaveLength(2);
    const first = groups[0] as HTMLDetailsElement;
    expect(first.open).toBe(false);
    await user.click(within(first).getByText(/合成作品 11/));
    await waitFor(() => expect(first.open).toBe(true));
    expect(first).toHaveTextContent('主页发现作品 · 收录于 2026-10-10 08:00 · 指标观察 2026-10-12 09:30');
    for (const name of ['与当前领域的关系', '亲历自述', '知识与方法讲解', '推广表达']) {
      expect(first).toHaveTextContent(name);
    }
    expect(within(first).getAllByText(/判定门槛：/)).toHaveLength(4);
    expect(first).toHaveTextContent('记录本人具体行动与变化');
    expect(within(first).getByRole('link', { name: '打开原作品 →' }).getAttribute('href')).toContain('work=');
    // 引文是来源里的原话，标明取自哪个字段，并能回查原作品。
    const quote = within(first).getAllByText(/合成正文片段 11/)[0] as HTMLElement;
    expect(quote.closest('blockquote')).toHaveTextContent('正文');
    expect(within(quote.closest('blockquote') as HTMLElement).getByRole('link', { name: '回查原作品' }).getAttribute('href')).toContain('/corpus/evidence?');
  });

  it('历史分析状态与错误依据保持可读；旧开关已开启也不出现重试', async () => {
    defaultRoutes(
      kit,
      ok(
        listJson({
          analysisEnabled: true,
          items: [
            itemJson(1, {
              works: [
                workJson(11, { state: 'queued', withResult: false }),
                workJson(12, { state: 'failed', lastErrorCode: 'model_provider_unavailable', withResult: false }),
                workJson(13, { state: 'idle', withResult: true, relevance: 'unknown' }),
              ],
            }),
          ],
        }),
      ),
    );
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    const [queued, failed, insufficient] = Array.from(dialog.querySelectorAll('details[data-work]')) as HTMLElement[];
    expect(queued).toHaveTextContent('排队待分析');
    expect(within(queued!).queryByRole('button', { name: /重试/ })).toBeNull();
    expect(failed).toHaveTextContent('分析失败 · 模型服务暂不可用');
    expect(within(failed!).queryByRole('button', { name: /重试/ })).toBeNull();
    const processing = within(failed!).getByText('处理信息').closest('details') as HTMLElement;
    expect(processing).toHaveTextContent('model_provider_unavailable');
    expect(failed!.textContent?.replace(processing.textContent ?? '', '')).not.toContain('model_provider_unavailable');
    expect(insufficient).toHaveTextContent('已分析，依据不足');
  });

  it('分析没开启时失败的作品也不出现重试', async () => {
    defaultRoutes(kit, ok(listJson({ analysisEnabled: false, items: [itemJson(1, { works: [workJson(11, { state: 'failed', lastErrorCode: 'model_disabled', withResult: false })] })] })));
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    expect(within(dialog).queryByRole('button', { name: /重试/ })).toBeNull();
  });

  it('人工调整的判断写明「人工调整」与时间；原依据已变化时提示复核，原记录仍保留', async () => {
    const manual = {
      personal_experience: { value: { value: 'unknown', reason: '当前依据已变化或不可读，请复核；原人工记录仍保留', supportUnavailable: true }, supportFragmentIds: [], supportUnavailable: true, at: '2026-10-15 12:00:00+08' },
    };
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { works: [workJson(11, { manual })] })] })));
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    const group = dialog.querySelector('details[data-work]') as HTMLElement;
    expect(group).toHaveTextContent('人工调整 · 2026-10-15 12:00');
    expect(group).toHaveTextContent('当时支持这项判断的材料已变化或不可读，请复核；原人工记录仍保留');
    expect(within(group).getAllByRole('button', { name: '补充判断' }).length).toBeGreaterThan(0);
  });
});

describe('依据抽屉 · 作者资料与提交栏', () => {
  it('作者资料：公开简介、机构／品牌自述（不凭昵称头像推断）、材料与来源；平台账号标识只在折叠的处理信息里', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '作者资料' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '作者资料' });
    expect(panel).toHaveTextContent('合成公开简介：记录学习方法与日常实践。');
    expect(panel).toHaveTextContent('仅依据公开资料，不根据昵称、头像或粉丝量推断身份');
    const sources = within(panel).getByRole('region', { name: '材料与来源' });
    expect(sources).toHaveTextContent('当前领域合成领域一');
    expect(sources).toHaveTextContent('观察关系未加入观察');
    expect(sources).not.toHaveTextContent('author-1');
    const diagnostics = within(panel).getByText('处理信息').closest('details') as HTMLElement;
    expect(diagnostics).toHaveTextContent('平台账号标识：author-1');
    expect(diagnostics).toHaveTextContent('来源采样：profile_discovery_sample');
  });

  it('提交栏在每个分区都在：观察关系 + 动作 + 回执', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    for (const tab of ['概览', '作品与依据', '作者资料']) {
      await user.click(within(dialog).getByRole('tab', { name: tab }));
      const footer = dialog.querySelector('footer') as HTMLElement;
      expect(footer).toHaveTextContent('未加入观察');
      expect(within(footer).getByRole('button', { name: '加入本领域观察' })).toBeEnabled();
    }
  });

  it('提交栏的各种观察状态', async () => {
    for (const [options, expectation] of [
      [{ inDomain: true }, { link: '查看观察目标 →' }],
      [{ inDomain: null }, { disabled: '观察关系暂不可用' }],
      [{ hasOtherPrimaryDomain: true, existsInOtherDomains: true }, { button: '按参照用途加入', note: '其他领域已有主研究关系；此操作仅加入参照用途，不改变原巡查规则。' }],
      [{ lifecycleState: 'dismissed', monitoringState: 'dismissed', targetRef: '30000000-0000-4000-8000-0000000000aa' }, { button: '加入并保持已忽略', link: '查看原目标' }],
    ] as const) {
      kit = createFetchMock();
      kit.install();
      defaultRoutes(kit, ok(listJson({ outside: options.inDomain === null ? null : 20, items: [itemJson(1, options)] })));
      const { dialog, unmount } = await openDrawer();
      const footer = dialog.querySelector('footer') as HTMLElement;
      if ('link' in expectation) expect(within(footer).getByRole('link', { name: expectation.link })).toBeInTheDocument();
      if ('disabled' in expectation) expect(within(footer).getByRole('button', { name: expectation.disabled })).toBeDisabled();
      if ('button' in expectation) expect(within(footer).getByRole('button', { name: expectation.button })).toBeEnabled();
      if ('note' in expectation) expect(footer).toHaveTextContent(expectation.note);
      unmount();
    }
  });

  it('领域已暂停：人工调整与加入不可用，旧分析重试已移除', async () => {
    defaultRoutes(kit, ok(listJson({ domainStatus: 'paused', analysisEnabled: true, items: [itemJson(1, { works: [workJson(11, { state: 'failed', lastErrorCode: 'model_disabled', withResult: false })] })] })));
    const { dialog, user } = await openDrawer();
    expect(within(dialog).getByRole('button', { name: '调整判断' })).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: '加入本领域观察' })).toBeDisabled();
    await user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    expect(within(dialog).queryByRole('button', { name: /重试/ })).toBeNull();
    for (const button of within(dialog).getAllByRole('button', { name: /调整判断|补充判断/ })) expect(button).toBeDisabled();
  });
});

describe('依据抽屉 · 键盘、焦点与头像', () => {
  it('方向键 / Home / End 在分区之间移动，焦点跟着走', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    const [overview, evidence, profile] = within(dialog).getAllByRole('tab');
    overview!.focus();
    await user.keyboard('{ArrowRight}');
    expect(evidence).toHaveAttribute('aria-selected', 'true');
    expect(evidence).toHaveFocus();
    await user.keyboard('{End}');
    expect(profile).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{ArrowRight}');
    expect(overview).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{ArrowLeft}');
    expect(profile).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{Home}');
    expect(overview).toHaveAttribute('aria-selected', 'true');
  });

  it('Esc 关闭抽屉，焦点回到表格里这位作者的名字按钮；行高亮随之取消', async () => {
    defaultRoutes(kit);
    const { user } = await openDrawer();
    expect(screen.getByRole('button', { name: '合成作者 1' }).closest('tr')).toHaveAttribute('data-selected', 'true');
    fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(screen.getByRole('button', { name: '合成作者 1' })).toHaveFocus();
    expect(screen.getByRole('button', { name: '合成作者 1' }).closest('tr')).toHaveAttribute('data-selected', 'false');
    await user.click(screen.getByRole('button', { name: '合成作者 2' }));
    await user.click(await screen.findByRole('button', { name: '关闭发现依据' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(screen.getByRole('button', { name: '合成作者 2' })).toHaveFocus();
  });

  it('点「作品表达」直接打开「作品与依据」分区；点「查看加入方式」打开作者资料分区', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2, { hasOtherPrimaryDomain: true, existsInOtherDomains: true })] })));
    const { user } = renderCreatorsPage();
    const row = (await screen.findByRole('button', { name: '合成作者 1' })).closest('tr') as HTMLElement;
    await user.click(within(row).getByRole('button', { name: '亲历自述' }));
    expect(within(await screen.findByRole('dialog')).getByRole('tab', { name: '作品与依据' })).toHaveAttribute('aria-selected', 'true');
    fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await user.click(within((await screen.findByRole('button', { name: '合成作者 2' })).closest('tr') as HTMLElement).getByRole('button', { name: '查看加入方式' }));
    expect(within(await screen.findByRole('dialog')).getByRole('tab', { name: '作者资料' })).toHaveAttribute('aria-selected', 'true');
  });

  it('抽屉里的头像名称随加载状态变化：读取中 → 作者头像；加载失败 → 头像未取得', async () => {
    defaultRoutes(kit);
    kit.route(
      'GET',
      '/api/local/work-resources',
      ok({ items: [{ identity: { publicRef: workRef(11) }, display: { title: '展示标题 11' }, media: { avatar: { localAssetUrl: '/api/local/media/avatar-1', blob: { deliveryState: 'INLINE_SAFE' } }, cover: { localAssetUrl: '/api/local/media/cover-1' } } }] }),
    );
    const { dialog } = await openDrawer();
    const header = dialog.querySelector('header') as HTMLElement;
    const image = await waitFor(() => {
      const found = header.querySelector('img');
      expect(found).not.toBeNull();
      return found as HTMLImageElement;
    });
    expect(within(header).getByRole('img', { name: '头像读取中' })).toBeInTheDocument();
    fireEvent.load(image);
    expect(within(header).getByRole('img', { name: '作者头像' })).toBeInTheDocument();
    // 作品资源给的展示标题优先于作品自带标题。
    expect(within(dialog).getAllByText('展示标题 11').length).toBeGreaterThan(0);
  });
});

describe('从证据库返回：恢复位置', () => {
  const url = `/corpus/creators?domain=${DOMAIN}`;

  it('去证据库之前记下位置（表格里的链接）：地址、作者，不含任何内容', async () => {
    defaultRoutes(kit);
    const { user } = renderCreatorsPage(url);
    const row = (await screen.findByRole('button', { name: '合成作者 2' })).closest('tr') as HTMLElement;
    const link = within(row).getAllByRole('link').find(candidate => candidate.getAttribute('href')?.startsWith('/corpus/evidence?')) as HTMLAnchorElement;
    link.addEventListener('click', event => event.preventDefault());
    await user.click(link);
    const saved = JSON.parse(sessionStorage.getItem('linggan.creatorDiscovery.return') as string);
    expect(saved).toMatchObject({ url, creatorKey: expect.stringMatching(/^xhs:/), scrollY: 0, scrollX: 0 });
    expect(JSON.stringify(saved)).not.toContain('合成作者');
    expect(JSON.stringify(saved)).not.toContain('合成正文');
  });

  it('抽屉里的链接额外记下分区、滚动与展开的作品；回来时抽屉原样打开', async () => {
    defaultRoutes(kit);
    const first = renderCreatorsPage(url);
    await first.user.click(await screen.findByRole('button', { name: '合成作者 1' }));
    const dialog = await screen.findByRole('dialog', { name: '合成作者 1' });
    await first.user.click(within(dialog).getByRole('tab', { name: '作品与依据' }));
    const group = dialog.querySelector('details[data-work]') as HTMLDetailsElement;
    await first.user.click(within(group).getByText(/合成作品 11/));
    await waitFor(() => expect(group.open).toBe(true));
    const link = within(group).getByRole('link', { name: '打开原作品 →' }) as HTMLAnchorElement;
    link.addEventListener('click', event => event.preventDefault());
    await first.user.click(link);
    const saved = JSON.parse(sessionStorage.getItem('linggan.creatorDiscovery.return') as string);
    expect(saved).toMatchObject({ url, drawer: true, tab: 'evidence', expanded: [workRef(11)] });
    first.unmount();

    const second = renderCreatorsPage(url);
    const restored = await screen.findByRole('dialog', { name: '合成作者 1' });
    expect(within(restored).getByRole('tab', { name: '作品与依据' })).toHaveAttribute('aria-selected', 'true');
    expect((restored.querySelector(`details[data-work="${workRef(11)}"]`) as HTMLDetailsElement).open).toBe(true);
    expect((restored.querySelector(`details[data-work="${workRef(12)}"]`) as HTMLDetailsElement).open).toBe(false);
    await waitFor(() => expect(sessionStorage.getItem('linggan.creatorDiscovery.return')).toBeNull());
    expect(second.location()).toBe(url);
  });

  it('地址不同就不恢复（不会把别的范围的位置套到这一页）', async () => {
    sessionStorage.setItem('linggan.creatorDiscovery.return', JSON.stringify({ url: `${url}&page=3`, scrollY: 80, scrollX: 0, creatorKey: 'xhs:x', drawer: true, tab: 'profile' }));
    defaultRoutes(kit);
    renderCreatorsPage(url);
    await screen.findByRole('table');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('只回到表格（没有开抽屉）时，焦点回到那位作者', async () => {
    defaultRoutes(kit);
    const first = renderCreatorsPage(url);
    const row = (await screen.findByRole('button', { name: '合成作者 2' })).closest('tr') as HTMLElement;
    const link = within(row).getAllByRole('link').find(candidate => candidate.getAttribute('href')?.startsWith('/corpus/evidence?')) as HTMLAnchorElement;
    link.addEventListener('click', event => event.preventDefault());
    await first.user.click(link);
    first.unmount();
    renderCreatorsPage(url);
    await waitFor(() => expect(screen.getByRole('button', { name: '合成作者 2' })).toHaveFocus());
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
