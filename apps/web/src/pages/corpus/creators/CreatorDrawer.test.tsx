import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DOMAIN,
  createFetchMock,
  creatorKeyFor,
  defaultRoutes,
  itemJson,
  listJson,
  observationRecordJson,
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
  const dialog = await screen.findByRole('complementary', { name });
  return { ...view, dialog };
}

const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;

describe('依据抽屉 · 结构与结论在先', () => {
  it('头部：名字、小红书 · 用途；三项数字同排；四个分区，默认概览', async () => {
    defaultRoutes(kit);
    const { dialog } = await openDrawer();
    expect(dialog).toHaveTextContent('小红书 · 主研究材料');
    const header = dialog.querySelector('header') as HTMLElement;
    expect(header).toHaveTextContent('粉丝2,001');
    expect(header).toHaveTextContent('相关作品3');
    expect(header).toHaveTextContent('领域最高赞1,500');
    expect(header).toHaveTextContent('主页资料 09-20');
    expect(header).not.toHaveTextContent('当前材料范围内已取得');
    const tabs = within(dialog).getAllByRole('tab');
    expect(tabs.map(tab => tab.textContent)).toEqual(['概览', '主页作品', '研究线索', '观察记录']);
    expect(tabs.map(tab => tab.getAttribute('aria-selected'))).toEqual(['true', 'false', 'false', 'false']);
    expect(within(dialog).getByRole('tabpanel', { name: '概览' })).toBeVisible();
  });

  it('粉丝没有记录时写「—」，不写 0；有作品条件时最高赞改叫「命中最高赞」', async () => {
    kit = createFetchMock();
    kit.install();
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { followers: null })] })));
    const first = await openDrawer();
    const firstHead = first.dialog.querySelector('header') as HTMLElement;
    expect(firstHead).toHaveTextContent('粉丝—');
    expect(firstHead).not.toHaveTextContent('粉丝0');
    first.unmount();

    kit = createFetchMock();
    kit.install();
    defaultRoutes(kit);
    const filtered = await openDrawer('合成作者 1', `/corpus/creators?domain=${DOMAIN}&traits=personal_experience`);
    expect(filtered.dialog).toHaveTextContent('命中最高赞');
    expect(filtered.dialog).not.toHaveTextContent('当前命中作品，含待判断');
  });

  it('概览先给结论：样本内倾向在最前面，其后是公开简介、身份线索、发现来源', async () => {
    defaultRoutes(kit);
    const { dialog } = await openDrawer();
    const panel = within(dialog).getByRole('tabpanel', { name: '概览' });
    const headings = within(panel).getAllByRole('heading', { level: 3 }).map(heading => heading.textContent);
    expect(headings.slice(0, 4)).toEqual(['样本内倾向', '公开简介', '公开身份线索', '发现来源']);
    const conclusion = within(panel).getByRole('region', { name: '样本内倾向' });
    expect(conclusion).toHaveTextContent('本领域持续创作');
    expect(conclusion).toHaveTextContent('简介与已有作品呈现相近的内容方向');
    expect(conclusion).toHaveTextContent('结果时间 2026-10-14 10:00');
    expect(within(conclusion).getByRole('button', { name: '调整作者倾向' })).toBeEnabled();
  });

  it('材料范围里的爆款数：未知保持未知，0 篇就是 0', async () => {
    defaultRoutes(kit, ok(listJson({ threshold: 100, items: [itemJson(1, { viralWorkCount: null })] })));
    const first = await openDrawer();
    await first.user.click(within(first.dialog).getByText('判断依据与材料范围'));
    expect(within(first.dialog).getByText(/当前范围相关爆款 未知/)).toBeInTheDocument();
    expect(within(first.dialog).queryByText(/当前范围相关爆款 0 篇/)).toBeNull();
    first.unmount();

    kit = createFetchMock();
    kit.install();
    defaultRoutes(kit, ok(listJson({ threshold: 100, items: [itemJson(1, { viralWorkCount: 0 })] })));
    const second = await openDrawer();
    await second.user.click(within(second.dialog).getByText('判断依据与材料范围'));
    expect(within(second.dialog).getByText(/当前范围相关爆款 0 篇/)).toBeInTheDocument();
    expect(within(second.dialog).queryByText(/当前范围相关爆款 未知/)).toBeNull();
  });

  it('发现来源：发现词、首次发现时间与当时看到的那篇作品，作品标题可回查', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { keywords: ['注意力训练', '写作业'] })] })));
    kit.route(
      'GET',
      '/api/local/work-resources',
      ok({
        items: [
          {
            identity: { publicRef: workRef(11) },
            display: { title: '最早见到的作品' },
            media: { avatar: { localAssetUrl: '/api/local/media/avatar-1', blob: { deliveryState: 'INLINE_SAFE' } }, cover: { localAssetUrl: '/api/local/media/cover-1' } },
          },
        ],
      }),
    );
    const { dialog } = await openDrawer();
    const card = within(dialog).getByRole('region', { name: '发现来源' });
    expect(card).toHaveTextContent('注意力训练、写作业');
    expect(card).toHaveTextContent('2026-09-25 09:00 · 本领域首次');
    const link = await within(card).findByRole('link', { name: '最早见到的作品' });
    expect(link.getAttribute('href')).toContain('/corpus/evidence?');
    expect(link.getAttribute('href')).toContain(workRef(11));
    expect(card).not.toHaveTextContent('更早的发现不可知');
  });

  it('发现来源没有记录时如实说明，不写 0 也不猜一篇作品', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { keywords: [], firstSeenDate: null, firstSeenWorkRef: null })] })));
    const { dialog } = await openDrawer();
    const card = within(dialog).getByRole('region', { name: '发现来源' });
    expect(card).toHaveTextContent('未记录发现词');
    expect(card).toHaveTextContent('首次发现时间未记录');
    expect(card).toHaveTextContent('未记录最早收录的作品');
  });

  it('作者倾向与作品表达是两个层次：身份线索在概览单独成块，作品门槛只在研究线索保留一份', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    const conclusion = within(dialog).getByRole('region', { name: '样本内倾向' });
    expect(conclusion).toHaveTextContent('本领域持续创作');
    const identity = within(dialog).getByRole('region', { name: '公开身份线索' });
    expect(identity).toHaveTextContent('机构／品牌自述');
    expect(identity).toHaveTextContent('不凭昵称、头像或粉丝数推断');
    const basis = within(dialog).getByText('判断依据与材料范围').closest('details') as HTMLElement;
    expect(basis).toHaveTextContent('已判相关');
    expect(basis).not.toHaveTextContent('持续创作倾向须至少两篇独立相关作品');
    // 作品层的判定标准只保留一份，放在研究线索底部，不逐项重复。
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    const criteria = within(dialog).getByText('判断标准').closest('details') as HTMLElement;
    expect(criteria).toHaveTextContent('具体的本人或家庭经历、行动、过程或感受');
    expect(criteria).toHaveTextContent('知识与方法讲解');
    const evidencePanel = within(dialog).getByRole('tabpanel', { name: '研究线索' });
    expect(within(evidencePanel).queryAllByText(/判定门槛：/)).toHaveLength(0);
  });

  it('实际取样篇数：主线 worker 没写这两个字段时如实说「未记录」，不显示成 0', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    expect(within(dialog).getByRole('region', { name: '样本内倾向' })).toHaveTextContent('本次实际取样篇数未记录');
    await user.click(within(dialog).getByText('判断依据与材料范围'));
    expect(dialog).toHaveTextContent('本次实际取样篇数未记录。');
    expect(dialog).not.toHaveTextContent(/实际使用 0 篇/);
  });

  it('取样篇数字段存在就显示实际数，分析当时已有数只出现一次', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { sampleWorkCount: 12, availableWorkCount: 24 })] })));
    const { dialog, user } = await openDrawer();
    expect(within(dialog).getByRole('region', { name: '样本内倾向' })).toHaveTextContent('已分析 12 篇 · 分析时可用 24 篇');
    await user.click(within(dialog).getByText('材料范围与局限'));
    expect(dialog).toHaveTextContent('本次实际使用 12 篇作品摘要；分析当时已有 24 篇。');
  });

  it('代表作品预览最多三篇，带相关性与赞数；「查看全部」去证据库', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { works: [workJson(11), workJson(12), workJson(13)], unknown: 1 })] })));
    const { dialog } = await openDrawer();
    const preview = within(dialog).getByRole('region', { name: '代表作品' });
    expect(within(preview).getAllByRole('link', { name: /合成作品/ })).toHaveLength(3);
    expect(preview).toHaveTextContent('已判相关 · 赞 1,211');
    expect(preview).not.toHaveTextContent('藏 30 · 评 8');
    const all = within(preview).getByRole('link', { name: '查看全部 3 篇 →' });
    expect(all.getAttribute('href')).toContain('/corpus/evidence?');
  });

  it('普通层不露机器语言与 UUID：整个抽屉里看不到作品编号、片段编号、观察目标编号、作者键', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { inDomain: true })] })));
    const { dialog, user } = await openDrawer();
    for (const tab of ['概览', '研究线索', '观察记录']) {
      await user.click(within(dialog).getByRole('tab', { name: tab }));
      expect(dialog.textContent).not.toMatch(UUID);
      expect(dialog.textContent).not.toMatch(/xhs:[0-9a-f]{6,}/);
    }
  });
});

describe('依据抽屉 · 研究线索（按作品分组）', () => {
  it('每篇作品一个可折叠的块；展开后是这篇作品的四项判断、来源与引文', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '研究线索' });
    expect(panel).toHaveTextContent('当前命中 2 篇');
    expect(panel).not.toHaveTextContent('范围内 5 篇');
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
    expect(within(first).queryAllByText(/判定门槛：/)).toHaveLength(0);
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
                workJson(11, { state: 'pending', withResult: false }),
                workJson(12, { state: 'failed', lastErrorCode: 'model_provider_unavailable', withResult: false }),
                workJson(13, { state: 'idle', withResult: true, relevance: 'unknown' }),
              ],
            }),
          ],
        }),
      ),
    );
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    const [queued, failed, insufficient] = Array.from(dialog.querySelectorAll('details[data-work]')) as HTMLElement[];
    expect(queued).toHaveTextContent('研究已排队，等待执行');
    expect(within(queued!).queryByRole('button', { name: /重试/ })).toBeNull();
    expect(failed).toHaveTextContent('读取失败 · 模型服务暂不可用');
    expect(failed).toHaveTextContent('研究未完成 · 模型服务暂不可用');
    expect(within(failed!).queryByRole('button', { name: /重试/ })).toBeNull();
    expect(failed!).not.toHaveTextContent('处理信息');
    expect(failed!).not.toHaveTextContent('model_provider_unavailable');
    expect(insufficient).toHaveTextContent('已研究，依据不足');
  });

  it('分析没开启时失败的作品也不出现重试', async () => {
    defaultRoutes(kit, ok(listJson({ analysisEnabled: false, items: [itemJson(1, { works: [workJson(11, { state: 'failed', lastErrorCode: 'model_disabled', withResult: false })] })] })));
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    expect(within(dialog).queryByRole('button', { name: /重试/ })).toBeNull();
  });

  it('人工调整的判断写明「人工调整」与时间；原依据已变化时提示复核，原记录仍保留', async () => {
    const manual = {
      personal_experience: { value: { value: 'unknown', reason: '当前依据已变化或不可读，请复核；原人工记录仍保留', supportUnavailable: true }, supportFragmentIds: [], supportUnavailable: true, at: '2026-10-15 12:00:00+08' },
    };
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { works: [workJson(11, { manual })] })] })));
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    const group = dialog.querySelector('details[data-work]') as HTMLElement;
    expect(group).toHaveTextContent('人工调整 · 2026-10-15 12:00');
    expect(group).toHaveTextContent('支持材料已变化，请复核');
    expect(group).toHaveTextContent('原人工记录仍保留');
    expect(within(group).getAllByText(/自动依据更新于 2026-10-14 10:00/)).toHaveLength(1);
    expect(within(group).queryByText('单篇作品研究')).toBeNull();
    expect(within(group).getByRole('link', { name: '打开原作品 →' })).toBeInTheDocument();
    expect(within(group).getByRole('button', { name: '检查研究底稿' })).toBeInTheDocument();
    expect(within(group).getAllByRole('button', { name: /补充|调整/ }).length).toBeGreaterThan(0);
  });
});

describe('依据抽屉 · 观察记录与提交栏', () => {
  it('公开简介与身份线索归概览；观察记录只承担观察与巡查，不重复资料', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    const overview = within(dialog).getByRole('tabpanel', { name: '概览' });
    expect(overview).toHaveTextContent('合成公开简介：记录学习方法与日常实践。');
    const identity = within(overview).getByRole('region', { name: '公开身份线索' });
    expect(identity).toHaveTextContent('机构／品牌自述');
    expect(identity).toHaveTextContent('不凭昵称、头像或粉丝数推断');
    expect(overview).not.toHaveTextContent('author-1');
    await user.click(within(dialog).getByRole('tab', { name: '观察记录' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '观察记录' });
    expect(panel).not.toHaveTextContent('合成公开简介');
    expect(panel).not.toHaveTextContent('机构／品牌自述');
    expect(panel).not.toHaveTextContent('author-1');
  });

  it('提交栏在每个分区都在：观察关系 + 动作 + 回执', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    for (const tab of ['概览', '研究线索', '观察记录']) {
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
    expect(within(dialog).getByRole('button', { name: '调整作者倾向' })).toBeDisabled();
    expect(within(dialog).getByRole('button', { name: '加入本领域观察' })).toBeDisabled();
    await user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
    expect(within(dialog).queryByRole('button', { name: /重试/ })).toBeNull();
    for (const button of within(dialog).getAllByRole('button', { name: /调整|补充/ })) expect(button).toBeDisabled();
  });
});

describe('依据抽屉 · 观察记录分区', () => {
  it('已加入：加入时间与用途、档案状态、上次巡查结果与最近变化都写明来源', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { inDomain: true })] })));
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '观察记录' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '观察记录' });
    const record = await within(panel).findByRole('region', { name: '观察记录' });
    expect(record).toHaveTextContent('加入本领域');
    expect(record).toHaveTextContent('2026-09-24 12:23 · 主研究');
    expect(record).toHaveTextContent('首次保存');
    expect(record).toHaveTextContent('2026-09-22 03:12');
    expect(record).toHaveTextContent('已暂停（2026-10-15 16:26 起）');
    expect(record).toHaveTextContent('成功于 2026-10-14 14:12');
    expect(record).toHaveTextContent('12 篇，其中 3 篇新见');
    expect(record).toHaveTextContent('自动巡查未开启');
    expect(record).toHaveTextContent('2026-10-15 16:26 从「巡查中」到「已暂停」：人工暂停巡查');
  });

  it('巡查开着且还没有成功结果：下次时间照实显示，结果不写成 0', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1, { inDomain: true })] })));
    kit.route(
      'GET',
      `/api/local/creators/${encodeURIComponent(creatorKeyFor('author-1'))}/observation-record`,
      ok(
        observationRecordJson({
          lifecycleState: 'monitoring',
          lifecycleChangedAt: null,
          monitoringState: 'monitoring',
          patrol: {
            automaticEnabled: true,
            lastDispatchedAt: null,
            lastSucceededAt: null,
            nextRunAt: '2026-11-01 09:00',
            missedRuns: 2,
            latestHits: null,
            latestNew: null,
            state: 'waiting',
          },
          lastTransition: null,
        }),
      ),
    );
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '观察记录' }));
    const record = await within(dialog).findByRole('region', { name: '观察记录' });
    expect(record).toHaveTextContent('巡查中 · 自动巡查已开启');
    expect(record).toHaveTextContent('下次巡查2026-11-01 09:00');
    expect(record).toHaveTextContent('没有巡查记录');
    expect(record).toHaveTextContent('还没有成功巡查的结果');
    expect(record).toHaveTextContent('错过计划2 次');
    expect(record).toHaveTextContent('没有变化记录');
  });

  it('未加入：说明没有巡查记录，且不请求观察记录接口', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    await user.click(within(dialog).getByRole('tab', { name: '观察记录' }));
    const panel = within(dialog).getByRole('tabpanel', { name: '观察记录' });
    expect(await within(panel).findByRole('region', { name: '观察记录' })).toHaveTextContent(
      '未加入本领域观察，所以没有巡查记录',
    );
    expect(kit.of('GET', `/api/local/creators/${encodeURIComponent(creatorKeyFor('author-1'))}/observation-record`)).toHaveLength(0);
  });
});

describe('依据抽屉 · 键盘、焦点与头像', () => {
  it('方向键 / Home / End 在分区之间移动，焦点跟着走', async () => {
    defaultRoutes(kit);
    const { dialog, user } = await openDrawer();
    const [overview, works, evidence, profile] = within(dialog).getAllByRole('tab');
    overview!.focus();
    await user.keyboard('{ArrowRight}');
    expect(works).toHaveAttribute('aria-selected', 'true');
    expect(works).toHaveFocus();
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

  it('Esc 关闭检查器，焦点回到表格里这位作者的名字按钮；行高亮随之取消', async () => {
    defaultRoutes(kit);
    const { user } = await openDrawer();
    expect(screen.getByRole('button', { name: '合成作者 1' }).closest('tr')).toHaveAttribute('data-open', 'true');
    fireEvent.keyDown(screen.getByRole('complementary', { name: '合成作者 1' }), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('complementary', { name: '合成作者 1' })).toBeNull());
    await waitFor(() => expect(screen.getByRole('button', { name: '合成作者 1' })).toHaveFocus());
    expect(screen.getByRole('button', { name: '合成作者 1' }).closest('tr')).toHaveAttribute('data-open', 'false');
    await user.click(screen.getByRole('button', { name: '合成作者 2' }));
    await user.click(await screen.findByRole('button', { name: '关闭作者检查器' }));
    await waitFor(() => expect(screen.queryByRole('complementary', { name: '合成作者 2' })).toBeNull());
    await waitFor(() => expect(screen.getByRole('button', { name: '合成作者 2' })).toHaveFocus());
  });

  it('观察格未加入时只有一个入口：点「加入观察」打开目标设置，不打开检查器', async () => {
    defaultRoutes(kit, ok(listJson({ items: [itemJson(1), itemJson(2, { hasOtherPrimaryDomain: true, existsInOtherDomains: true })] })));
    const { user } = renderCreatorsPage();
    await screen.findByRole('button', { name: '合成作者 1' });
    const row = (await screen.findByRole('button', { name: '合成作者 2' })).closest('tr') as HTMLElement;
    await user.click(within(row).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialog).toHaveTextContent('只能按参照用途加入');
    expect(screen.queryByRole('complementary', { name: '合成作者 2' })).toBeNull();
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
    const dialog = await screen.findByRole('complementary', { name: '合成作者 1' });
    await first.user.click(within(dialog).getByRole('tab', { name: '研究线索' }));
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
    const restored = await screen.findByRole('complementary', { name: '合成作者 1' });
    expect(within(restored).getByRole('tab', { name: '研究线索' })).toHaveAttribute('aria-selected', 'true');
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
    expect(screen.queryByRole('complementary', { name: '合成作者 1' })).toBeNull();
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
    expect(screen.queryByRole('complementary', { name: '合成作者 1' })).toBeNull();
  });
});
