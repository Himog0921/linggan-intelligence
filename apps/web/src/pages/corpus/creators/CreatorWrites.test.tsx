import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DOMAIN,
  createFetchMock,
  creatorKeyFor,
  defaultRoutes,
  deferred,
  fragmentJson,
  itemJson,
  listJson,
  ok,
  renderCreatorsPage,
  workJson,
  workRef,
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

const registerPath = (n: number) => `/api/local/creators/${encodeURIComponent(creatorKeyFor(`author-${n}`))}/observation-target`;
const receipt = (over: Record<string, unknown> = {}) => ({
  targetRef: '30000000-0000-4000-8000-0000000000f1',
  created: true,
  relationshipAdded: true,
  inCurrentDomain: true,
  usageRole: 'primary',
  lifecycleState: 'pending_decision',
  monitoringEnabled: false,
  collectionRequested: false,
  ...over,
});
const error = (status: number, code: string): Reply => ({ status, body: { operation: 'local_read', outcome: 'unavailable', code } });
const creators = () => kit.of('GET', '/api/local/creators');
const cancel = () => fireEvent(screen.getAllByRole('dialog').at(-1) as HTMLElement, new Event('cancel', { cancelable: true }));
const row = (name: string) => screen.getByRole('button', { name }).closest('tr') as HTMLElement;

async function page(list = listJson({ items: [itemJson(1), itemJson(2)] })) {
  defaultRoutes(kit, ok(list));
  const view = renderCreatorsPage();
  await screen.findByRole('table');
  return view;
}

describe('加入观察 · 先确认，回执后才显示结果', () => {
  it('点「加入观察」先弹确认，说明会 / 不会发生什么；确认之前不发任何请求；取消就什么都没发生', async () => {
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialog).toHaveTextContent('把「合成作者 1」加入「合成领域一」的观察名单，用途：主研究。');
    expect(dialog).toHaveTextContent('这一步会：');
    expect(dialog).toHaveTextContent('登记这位作者为本机观察目标（已登记过则沿用，不重复建立）');
    expect(dialog).toHaveTextContent('这一步不会：');
    for (const text of ['立即采集作品、评论或主页', '开启或关闭巡查，也不改变已有的巡查规则', '调用模型']) expect(dialog).toHaveTextContent(text);
    expect(kit.calls.filter(call => call.method === 'POST')).toHaveLength(0);
    await user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(kit.calls.filter(call => call.method === 'POST')).toHaveLength(0);
  });

  it('确认后才发请求（一次）；进行中显示「加入中…」；回执之后才显示结果，列表随之刷新', async () => {
    const gate = deferred<Reply>();
    kit.route('POST', registerPath(1), () => gate.promise);
    const { user } = await page();
    kit.route('GET', '/api/local/creators', ok(listJson({ items: [itemJson(1, { inDomain: true }), itemJson(2)] })));
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    const pending = within(dialog).getByRole('button', { name: '加入中…' });
    expect(pending).toBeDisabled();
    expect(dialog).not.toHaveTextContent('已完成');
    expect(kit.of('POST', registerPath(1))).toHaveLength(1);
    expect(kit.of('POST', registerPath(1))[0]?.body).toEqual({ domain: DOMAIN, usageRole: 'primary' });
    gate.resolve({ status: 200, body: receipt() });
    await within(dialog).findByText('已完成');
    expect(dialog).toHaveTextContent('已把「合成作者 1」加入「合成领域一」的观察名单。已新登记这位作者。巡查尚未开启。');
    await waitFor(() => expect(creators()).toHaveLength(2));
    await user.click(within(dialog).getByRole('button', { name: '关闭' }));
    expect(await within(row('合成作者 1')).findByRole('link', { name: '查看目标' })).toBeInTheDocument();
  });

  it('连点确认只发一次请求', async () => {
    const gate = deferred<Reply>();
    kit.route('POST', registerPath(1), () => gate.promise);
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    const confirm = within(dialog).getByRole('button', { name: '确认加入' });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => expect(kit.of('POST', registerPath(1))).toHaveLength(1));
    gate.resolve({ status: 200, body: receipt() });
    await within(dialog).findByText('已完成');
    expect(kit.of('POST', registerPath(1))).toHaveLength(1);
  });

  it('服务器拒绝：「未完成」+ 中文原因 + 错误码；对话框保持打开，可以重新提交', async () => {
    kit.route('POST', registerPath(1), () => error(409, 'active_primary_domain_conflict'), ok(receipt({ usageRole: 'reference' })));
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    const status = (await within(dialog).findByText('未完成')).closest('p') as HTMLElement;
    expect(status).toHaveTextContent('其他领域已有主研究关系，本领域只能按参照用途加入');
    expect(within(status).getByText('active_primary_domain_conflict')).toBeInTheDocument();
    expect(dialog).not.toHaveTextContent('已完成');
    await user.click(within(dialog).getByRole('button', { name: '重新提交' }));
    await within(dialog).findByText('已完成');
    expect(dialog).toHaveTextContent('的参照用途');
  });

  it('没有得到答复：「未确认」，说明可能已生效；重试发送与第一次完全相同的请求体，回执是重放也如实说', async () => {
    kit.route('POST', registerPath(1), () => 'network', ok(receipt({ created: false, relationshipAdded: false })));
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    const status = (await within(dialog).findByText('未确认')).closest('p') as HTMLElement;
    expect(status).toHaveTextContent('结果未确认，可能已经生效');
    expect(status).toHaveTextContent('同一位作者在同一个领域只会有一个观察目标');
    expect(within(dialog).queryByText('已完成')).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: '重试（同一请求）' }));
    await within(dialog).findByText('已完成');
    expect(dialog).toHaveTextContent('「合成作者 1」已在「合成领域一」的观察名单中，本次没有新增变化。');
    const posts = kit.of('POST', registerPath(1));
    expect(posts).toHaveLength(2);
    expect(posts[1]?.body).toEqual(posts[0]?.body);
  });

  it('回执形状不对：不报成功', async () => {
    kit.route('POST', registerPath(1), ok({ unexpected: true }));
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    expect(await within(dialog).findByText('未完成')).toBeInTheDocument();
    expect(dialog).toHaveTextContent('服务返回的内容无法识别，页面没有采用它');
    expect(within(dialog).queryByText('已完成')).toBeNull();
  });

  it('已有暂停 / 忽略的目标：对话框说明加入后保持原状，成功回执同样写明', async () => {
    kit.route('POST', registerPath(1), ok(receipt({ created: false, lifecycleState: 'paused', monitoringEnabled: false })));
    const { user } = await page(listJson({ items: [itemJson(1, { lifecycleState: 'paused', monitoringState: 'paused', targetRef: '30000000-0000-4000-8000-0000000000aa' })] }));
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '查看加入方式' }));
    const drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    await user.click(within(drawer).getByRole('button', { name: '加入并保持观察已暂停' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialog).toHaveTextContent('该作者已有观察目标，当前为「观察已暂停」；加入后保持这个状态，不会自动恢复。');
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    await within(dialog).findByText('已完成');
    expect(dialog).toHaveTextContent('目标保持「观察已暂停」。');
    expect(dialog).toHaveTextContent('沿用了已有的观察目标。');
  });

  it('其他领域已有主研究关系：只能按参照用途加入，请求里的用途是 reference', async () => {
    kit.route('POST', registerPath(1), ok(receipt({ usageRole: 'reference' })));
    const { user } = await page(listJson({ items: [itemJson(1, { hasOtherPrimaryDomain: true, existsInOtherDomains: true })] }));
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '查看加入方式' }));
    const drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    await user.click(within(drawer).getByRole('button', { name: '按参照用途加入' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialog).toHaveTextContent('其他领域已有主研究关系；本领域只能按参照用途加入，不改变原巡查规则。');
    expect(dialog).toHaveTextContent('用途：参照。');
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    await within(dialog).findByText('已完成');
    expect(kit.of('POST', registerPath(1))[0]?.body).toEqual({ domain: DOMAIN, usageRole: 'reference' });
  });

  it('从抽屉加入：确认对话框叠在抽屉上，成功后抽屉的提交栏随列表刷新变成「查看观察目标」', async () => {
    kit.route('POST', registerPath(1), ok(receipt()));
    const { user } = await page();
    kit.route('GET', '/api/local/creators', ok(listJson({ items: [itemJson(1, { inDomain: true }), itemJson(2)] })));
    await user.click(screen.getByRole('button', { name: '合成作者 1' }));
    const drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    await user.click(within(drawer).getByRole('button', { name: '加入本领域观察' }));
    const dialog = await screen.findByRole('dialog', { name: '加入观察目标' });
    await user.click(within(dialog).getByRole('button', { name: '确认加入' }));
    await within(dialog).findByText('已完成');
    await user.click(within(dialog).getByRole('button', { name: '关闭' }));
    expect(await within(drawer).findByRole('link', { name: '查看观察目标 →' })).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: '合成作者 1' })).toBeInTheDocument();
  });
});

describe('迟到的回执不改写之后打开的对话框（#384 发现 5）', () => {
  it('先开 A 再开 B：A 的回执晚到，B 的对话框保持原样；列表照常刷新；重复点击不重复提交', async () => {
    const gateA = deferred<Reply>();
    kit.route('POST', registerPath(1), () => gateA.promise);
    kit.route('POST', registerPath(2), ok(receipt({ created: false })));
    const { user } = await page();

    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    const dialogA = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialogA).toHaveTextContent('「合成作者 1」');
    await user.click(within(dialogA).getByRole('button', { name: '确认加入' }));
    await waitFor(() => expect(kit.of('POST', registerPath(1))).toHaveLength(1));
    cancel(); // S1: 明确收起后原请求继续
    await user.click(await screen.findByRole('button', { name: '收起并继续跟踪' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    // 重复点击：A 那一行显示进行中，不能再次提交。
    const pendingButton = within(row('合成作者 1')).getByRole('button', { name: '加入中…' });
    expect(pendingButton).toBeDisabled();
    fireEvent.click(pendingButton);
    expect(kit.of('POST', registerPath(1))).toHaveLength(1);

    await user.click(within(row('合成作者 2')).getByRole('button', { name: '加入观察' }));
    const dialogB = await screen.findByRole('dialog', { name: '加入观察目标' });
    expect(dialogB).toHaveTextContent('「合成作者 2」');
    const listCallsBefore = creators().length;
    kit.route('GET', '/api/local/creators', ok(listJson({ items: [itemJson(1, { inDomain: true }), itemJson(2)] })));

    gateA.resolve({ status: 200, body: receipt() });
    await waitFor(() => expect(creators().length).toBe(listCallsBefore + 1));
    // B 的对话框没有被关闭，也没有出现 A 的回执。
    const stillB = screen.getByRole('dialog', { name: '加入观察目标' });
    expect(stillB).toBe(dialogB);
    expect(stillB).toHaveTextContent('把「合成作者 2」加入');
    expect(stillB).not.toHaveTextContent('已完成');
    expect(stillB).not.toHaveTextContent('合成作者 1');
    expect(within(stillB).getByRole('button', { name: '确认加入' })).toBeEnabled();
    // A 那一行随列表刷新变成已加入。
    expect(await within(row('合成作者 1')).findByRole('link', { name: '查看目标' })).toBeInTheDocument();
    // B 仍然可以正常完成。
    await user.click(within(stillB).getByRole('button', { name: '确认加入' }));
    await within(stillB).findByText('已完成');
    expect(stillB).toHaveTextContent('「合成作者 2」');
  });

  it('A 的失败回执晚到，同样不碰 B 的对话框', async () => {
    const gateA = deferred<Reply>();
    kit.route('POST', registerPath(1), () => gateA.promise);
    const { user } = await page();
    await user.click(within(row('合成作者 1')).getByRole('button', { name: '加入观察' }));
    await user.click(within(await screen.findByRole('dialog', { name: '加入观察目标' })).getByRole('button', { name: '确认加入' }));
    await waitFor(() => expect(kit.of('POST', registerPath(1))).toHaveLength(1));
    cancel();
    await user.click(await screen.findByRole('button', { name: '收起并继续跟踪' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await user.click(within(row('合成作者 2')).getByRole('button', { name: '加入观察' }));
    const dialogB = await screen.findByRole('dialog', { name: '加入观察目标' });
    gateA.resolve(error(409, 'active_primary_domain_conflict'));
    await waitFor(() => expect(within(row('合成作者 1')).queryByRole('button', { name: '加入中…' })).toBeNull());
    expect(dialogB).toHaveTextContent('把「合成作者 2」加入');
    expect(within(dialogB).queryByText('未完成')).toBeNull();
    expect(within(dialogB).getByRole('button', { name: '确认加入' })).toBeEnabled();
  });
});

describe('发现设置', () => {
  async function openPolicy(list = listJson({ revision: 3, items: [itemJson(1)] })) {
    const view = await page(list);
    await view.user.click(screen.getByRole('button', { name: '发现设置' }));
    const dialog = await screen.findByRole('dialog', { name: '发现设置' });
    return { ...view, dialog };
  }

  it('只保存点赞标准与设置版本，AI 状态与统一入口不含旧模型选择', async () => {
    kit.route('PUT', '/api/local/creator-discovery-policy', ok({ revision: 4, saved: true }));
    const { user, dialog } = await openPolicy();
    expect(within(dialog).queryByRole('combobox')).toBeNull();
    expect(within(dialog).getByRole('link', { name: '去 AI 设置' })).toHaveAttribute('href', '/settings/models?section=automation&task=legacy_creator_discovery');
    await user.type(within(dialog).getByRole('spinbutton', { name: /最低点赞/ }), '100');
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(kit.of('PUT', '/api/local/creator-discovery-policy')[0]?.body).toEqual({
      domain: DOMAIN, revision: 3, likeThreshold: 100,
    });
    expect(await screen.findByText(/发现设置已保存。保存本身不启动采集，也不会立刻调用模型/)).toBeInTheDocument();
    await waitFor(() => expect(creators().length).toBeGreaterThanOrEqual(2));
  });

  it('旧分析开关已开启也只读展示退役状态，不能配置或重试旧执行链', async () => {
    const { dialog } = await openPolicy(listJson({ analysisEnabled: true, items: [itemJson(1)] }));
    expect(within(dialog).getByText(/旧分析已停用/)).toBeInTheDocument();
    expect(within(dialog).queryByRole('checkbox')).toBeNull();
    expect(within(dialog).queryByRole('combobox')).toBeNull();
    expect(within(dialog).queryByRole('spinbutton', { name: /词元/ })).toBeNull();
    expect(kit.of('GET', '/api/local/model-settings')).toHaveLength(0);
    expect(kit.of('POST', '/api/local/creator-discovery-analysis/retry')).toHaveLength(0);
  });

  it('设置已被别处更新：「未完成」+ 中文说明 + 错误码，对话框保持打开', async () => {
    kit.route('PUT', '/api/local/creator-discovery-policy', () => error(409, 'policy_revision_conflict'));
    const { user, dialog } = await openPolicy();
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }));
    const status = (await within(dialog).findByText('未完成')).closest('p') as HTMLElement;
    expect(status).toHaveTextContent('设置已被更新');
    expect(within(status).getByText('policy_revision_conflict')).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: '发现设置' })).toBe(dialog);
  });

  it('统一 AI 状态读取失败仍可独立保存点赞标准，不发送旧分析字段', async () => {
    kit.route('PUT', '/api/local/creator-discovery-policy', ok({ revision: 4, saved: true }));
    defaultRoutes(kit, ok(listJson({ revision: 3, analysisEnabled: true, configRef: 'cfg-x', dailyTokenLimit: 5000, items: [itemJson(1)] })));
    kit.route('GET', '/api/local/ai-settings', () => error(503, 'ai_database_unavailable'));
    const { user } = renderCreatorsPage();
    await screen.findByRole('table');
    await user.click(screen.getByRole('button', { name: '发现设置' }));
    const dialog = await screen.findByRole('dialog', { name: '发现设置' });
    expect(await within(dialog).findByText('开关读取失败，当前值待核对。')).toBeInTheDocument();
    await user.type(within(dialog).getByRole('spinbutton', { name: /最低点赞/ }), '50');
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }));
    await waitFor(() => expect(kit.of('PUT', '/api/local/creator-discovery-policy')).toHaveLength(1));
    expect(kit.of('PUT', '/api/local/creator-discovery-policy')[0]?.body).toEqual({ domain: DOMAIN, revision: 3, likeThreshold: 50 });
  });

  it('领域已暂停：设置不可改', async () => {
    const { dialog } = await openPolicy(listJson({ domainStatus: 'paused', items: [itemJson(1)] }));
    expect(within(dialog).getByText('当前领域已暂停，不能修改设置。')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '保存设置' })).toBeDisabled();
    expect(within(dialog).getByRole('spinbutton', { name: /最低点赞/ })).toBeDisabled();
  });

  it('未保存设置的Esc离开先询问，继续保留草稿，放弃不写服务器', async () => {
    const { user, dialog } = await openPolicy();
    await user.type(within(dialog).getByRole('spinbutton', { name: /最低点赞/ }), '100');
    cancel();
    const warning = await screen.findByRole('dialog', { name: '放弃未保存的修改？' });
    expect(within(warning).getByRole('button', { name: '继续编辑' })).toHaveFocus();
    await user.click(within(warning).getByRole('button', { name: '继续编辑' }));
    expect(within(dialog).getByRole('spinbutton', { name: /最低点赞/ })).toHaveValue(100);
    cancel(); await user.click(await screen.findByRole('button', { name: '放弃修改' }));
    expect(screen.queryByRole('dialog')).toBeNull(); expect(kit.of('PUT', '/api/local/creator-discovery-policy')).toHaveLength(0);
  });

  it('保存还在路上时关掉再重新打开：迟到的回执不关闭新对话框，也不清掉新输入', async () => {
    const gate = deferred<Reply>();
    kit.route('PUT', '/api/local/creator-discovery-policy', () => gate.promise);
    const { user, dialog } = await openPolicy();
    await user.type(within(dialog).getByRole('spinbutton', { name: /最低点赞/ }), '100');
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }));
    expect(await within(dialog).findByRole('button', { name: '保存中…' })).toBeDisabled();
    cancel();
    await user.click(await screen.findByRole('button', { name: '收起并继续跟踪' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await user.click(screen.getByRole('button', { name: '发现设置' }));
    const reopened = await screen.findByRole('dialog', { name: '发现设置' });
    await user.type(within(reopened).getByRole('spinbutton', { name: /最低点赞/ }), '500');
    gate.resolve({ status: 200, body: { revision: 4, saved: true } });
    await waitFor(() => expect(creators().length).toBeGreaterThanOrEqual(2));
    expect(screen.getByRole('dialog', { name: '发现设置' })).toBe(reopened);
    expect(within(reopened).getByRole('spinbutton', { name: /最低点赞/ })).toHaveValue(500);
    expect(within(reopened).queryByText('已完成')).toBeNull();
    expect(screen.queryByText(/发现设置已保存/)).toBeNull();
    expect(within(reopened).getByRole('button', { name: '保存设置' })).toBeEnabled();
  });
});

describe('调整判断', () => {
  const overrides = '/api/local/creator-discovery-overrides';

  async function openCorrection(item = itemJson(1)) {
    const view = await page(listJson({ items: [item, itemJson(2)] }));
    await view.user.click(screen.getByRole('button', { name: '合成作者 1' }));
    const drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    return { ...view, drawer };
  }

  it('作者倾向：说明对象与门槛；支持材料按作品分组，缺 workTitle 时用代表作品标题', async () => {
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    expect(dialog).toHaveTextContent('创作者：合成作者 1 · 合成领域一 · 仅修改本项判断');
    expect(dialog).toHaveTextContent('判定依据：持续创作倾向须至少两篇独立相关作品');
    expect(within(dialog).getByRole('combobox', { name: '你的判断' })).toHaveTextContent('本领域持续创作');
    const groups = Array.from(dialog.querySelectorAll('details')).map(details => details.querySelector('summary')?.textContent);
    expect(groups).toEqual(['公开简介 · 1 个片段', '合成作品 11 · 1 个片段', '合成作品 12 · 1 个片段']);
    // 与旧页面一致：全部展开，用户不必先点开分组才能勾选材料。
    expect(Array.from(dialog.querySelectorAll('details')).every(details => details.open)).toBe(true);
  });

  it('片段带 workTitle（#384 后台）时按它分组；认不出作品的片段写「作品材料 N」，不露编号', async () => {
    const base = itemJson(1);
    const withTitles = {
      ...base,
      authorAnalysis: {
        ...base.authorAnalysis,
        supportFragments: [
          fragmentJson('x1', 'body', '带标题的片段', workRef(11), '后台给出的作品标题'),
          fragmentJson('x2', 'body', '未知作品的片段', workRef(99)),
        ],
      },
    };
    const { user, drawer } = await openCorrection(withTitles);
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    const groups = Array.from(dialog.querySelectorAll('details summary')).map(summary => summary.textContent);
    expect(groups).toEqual(['后台给出的作品标题 · 1 个片段', '作品材料 2 · 1 个片段']);
    expect(dialog.textContent).not.toMatch(/[0-9a-f]{8}-[0-9a-f]{4}/);
  });

  it('正向判断必须选当前支持材料，否则不发请求；选了之后保存，请求体带上材料与原因', async () => {
    kit.route('PUT', overrides, ok({ saved: true }));
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    expect(await within(dialog).findByText('请选择至少一项当前材料作为支持依据。')).toBeInTheDocument();
    expect(kit.of('PUT', overrides)).toHaveLength(0);
    await user.click(within(dialog).getByRole('checkbox', { name: /合成公开简介/ }));
    // 勾选之后这条本地提示已过时，不再挂在底部。
    expect(within(dialog).queryByText('请选择至少一项当前材料作为支持依据。')).toBeNull();
    await user.type(within(dialog).getByRole('textbox', { name: /调整原因/ }), '简介自述了领域定位');
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '调整作者倾向' })).toBeNull());
    expect(kit.of('PUT', overrides)[0]?.body).toEqual({
      domain: DOMAIN, creatorKey: creatorKeyFor('author-1'), workRef: null, field: 'focus', value: 'vertical_tendency', reason: '简介自述了领域定位', supportFragmentIds: ['author.1.biography'],
    });
    // 抽屉保持打开，页面给出回执，列表重读。
    expect(screen.getByRole('dialog', { name: '合成作者 1' })).toBeInTheDocument();
    expect(await screen.findByText(/判断已保存；自动分析不会覆盖这一项/)).toBeInTheDocument();
    await waitFor(() => expect(creators().length).toBeGreaterThanOrEqual(2));
  });

  it('改成「尚不能判定」不需要支持材料；原因上限 300 字', async () => {
    kit.route('PUT', overrides, ok({ saved: true }));
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    expect(within(dialog).getByRole('textbox', { name: /调整原因/ })).toHaveAttribute('maxlength', '300');
    await user.click(within(dialog).getByRole('combobox', { name: '你的判断' }));
    await user.click(screen.getByRole('option', { name: '尚不能判定' }));
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    await waitFor(() => expect(kit.of('PUT', overrides)).toHaveLength(1));
    expect(kit.of('PUT', overrides)[0]?.body).toMatchObject({ value: 'unknown', supportFragmentIds: [], reason: null });
  });

  it('恢复自动判断：没有人工调整时不可用；有时发送 value: null', async () => {
    kit.route('PUT', overrides, ok({ saved: true }));
    const first = await openCorrection();
    await first.user.click(within(first.drawer).getByRole('button', { name: '调整判断' }));
    expect(within(await screen.findByRole('dialog', { name: '调整作者倾向' })).getByRole('button', { name: '恢复自动判断' })).toBeDisabled();
    first.unmount();
    kit = createFetchMock();
    kit.install();
    kit.route('PUT', overrides, ok({ saved: true }));
    const manual = { focus: { value: { value: 'multi_topic', reason: '我的判断', evidenceFragmentIds: ['author.1.biography'] }, supportFragmentIds: ['author.1.biography'], at: '2026-10-15 12:00:00+08' } };
    const second = await openCorrection(itemJson(1, { authorManual: manual }));
    await second.user.click(within(second.drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    expect(within(dialog).getByRole('combobox', { name: '你的判断' })).toHaveTextContent('多主题分享');
    expect(within(dialog).getByRole('textbox', { name: /调整原因/ })).toHaveValue('我的判断');
    expect(within(dialog).getByRole('checkbox', { name: /合成公开简介/ })).toBeChecked();
    await second.user.click(within(dialog).getByRole('button', { name: '恢复自动判断' }));
    await waitFor(() => expect(kit.of('PUT', overrides)).toHaveLength(1));
    expect(kit.of('PUT', overrides)[0]?.body).toMatchObject({ value: null, reason: null, supportFragmentIds: [], field: 'focus' });
  });

  it('作品层的判断带 workRef；服务器拒绝时「未完成」+ 中文原因 + 错误码，对话框保持打开', async () => {
    kit.route('PUT', overrides, () => error(400, 'invalid_support_fragment'));
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('tab', { name: '作品与依据' }));
    const group = drawer.querySelector('details[data-work]') as HTMLElement;
    await user.click(within(group).getAllByRole('button', { name: '调整判断' })[1] as HTMLElement); // 亲历自述
    const dialog = await screen.findByRole('dialog', { name: '调整亲历自述' });
    expect(dialog).toHaveTextContent('作品：合成作品 11');
    await user.click(within(dialog).getAllByRole('checkbox')[0] as HTMLElement);
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    const status = (await within(dialog).findByText('未完成')).closest('p') as HTMLElement;
    expect(status).toHaveTextContent('请选择当前仍可读的支持材料');
    expect(within(status).getByText('invalid_support_fragment')).toBeInTheDocument();
    expect(kit.of('PUT', overrides)[0]?.body).toMatchObject({ workRef: workRef(11), field: 'personal_experience', creatorKey: creatorKeyFor('author-1') });
    expect(screen.getByRole('dialog', { name: '调整亲历自述' })).toBe(dialog);
  });

  it('保存没有得到答复：「未确认」，按内容保存所以重试不产生第二条记录', async () => {
    kit.route('PUT', overrides, () => 'network', ok({ saved: true }));
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const dialog = await screen.findByRole('dialog', { name: '调整作者倾向' });
    await user.click(within(dialog).getByRole('checkbox', { name: /合成公开简介/ }));
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    const status = (await within(dialog).findByText('未确认')).closest('p') as HTMLElement;
    expect(status).toHaveTextContent('重新保存同样的内容不会产生第二条记录');
    await user.click(within(dialog).getByRole('button', { name: '保存判断' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '调整作者倾向' })).toBeNull());
    const puts = kit.of('PUT', overrides);
    expect(puts).toHaveLength(2);
    expect(puts[1]?.body).toEqual(puts[0]?.body);
  });

  it('保存还在路上时关掉，再为别的判断项打开：迟到的回执既不关闭新对话框，也不改它的内容', async () => {
    const gate = deferred<Reply>();
    kit.route('PUT', overrides, () => gate.promise);
    const { user, drawer } = await openCorrection();
    await user.click(within(drawer).getByRole('button', { name: '调整判断' }));
    const first = await screen.findByRole('dialog', { name: '调整作者倾向' });
    await user.click(within(first).getByRole('checkbox', { name: /合成公开简介/ }));
    await user.click(within(first).getByRole('button', { name: '保存判断' }));
    expect(await within(first).findByRole('button', { name: '保存中…' })).toBeDisabled();
    cancel();
    await user.click(await screen.findByRole('button', { name: '收起并继续跟踪' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '调整作者倾向' })).toBeNull());
    // 为同一位作者的另一项（机构／品牌自述，在「作者资料」分区）打开新的对话框，写上还没保存的内容。
    await user.click(within(drawer).getByRole('tab', { name: '作者资料' }));
    await user.click(within(drawer.querySelector('[role="tabpanel"]:not([hidden])') as HTMLElement).getByRole('button', { name: /调整判断|补充判断/ }));
    const second = await screen.findByRole('dialog', { name: /机构／品牌自述/ });
    await user.type(within(second).getByRole('textbox', { name: /调整原因/ }), '新的尚未保存的说明');
    const listCalls = creators().length;
    gate.resolve({ status: 200, body: { saved: true } });
    await waitFor(() => expect(creators().length).toBeGreaterThan(listCalls));
    expect(screen.getByRole('dialog', { name: /机构／品牌自述/ })).toBe(second);
    expect(within(second).getByRole('textbox', { name: /调整原因/ })).toHaveValue('新的尚未保存的说明');
    expect(within(second).queryByText('已完成')).toBeNull();
    expect(screen.queryByText(/判断已保存/)).toBeNull();
    expect(screen.getByRole('dialog', { name: '合成作者 1' })).toBeInTheDocument();
  });
});

describe('旧分析退役后的历史读取', () => {
  it('保留作者与作品失败依据；在各分区切换、关闭再打开都不创建重试', async () => {
    const item = itemJson(1, { works: [workJson(11, { state: 'failed', lastErrorCode: 'model_budget_exhausted', withResult: false })], authorState: 'failed', authorErrorCode: 'model_disabled', authorResult: false, focusValue: 'unknown' });
    const { user } = await page(listJson({ analysisEnabled: true, items: [item] }));
    await user.click(screen.getByRole('button', { name: '合成作者 1' }));
    let drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    expect(within(drawer).queryByRole('button', { name: /重试/ })).toBeNull();
    await user.click(within(drawer).getByRole('tab', { name: '作品与依据' }));
    expect(drawer).toHaveTextContent('model_budget_exhausted');
    expect(within(drawer).queryByRole('button', { name: /重试/ })).toBeNull();
    cancel();
    await user.click(screen.getByRole('button', { name: '合成作者 1' }));
    drawer = await screen.findByRole('dialog', { name: '合成作者 1' });
    expect(within(drawer).queryByRole('button', { name: /重试/ })).toBeNull();
    expect(kit.of('POST', '/api/local/creator-discovery-analysis/retry')).toHaveLength(0);
  });
});
