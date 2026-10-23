import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ReobservationSection } from './ReobservationSection';
import { DOMAIN, operationJson, taskJson, workRef } from './evidenceFixtures';
import type { Reobservation as Channel } from './evidenceTypes';
import { createFetchMock, fail, ok, type FetchKit } from './testKit';
import { useReobservation } from './useReobservation';

const ACTION = `/api/local/work-resources/${workRef(1)}/reobserve`;
let kit: FetchKit;

beforeEach(() => {
  kit = createFetchMock();
  kit.install();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const eligible: Channel = { supported: true, eligible: true, reason: 'OK', url: `${ACTION}?domain=${DOMAIN}`, mediaPolicy: 'EXISTING_ASSETS_REUSED' };

function Harness({ channel = eligible, platform = 'xhs', onRecheck = () => undefined, onTerminal = () => Promise.resolve(true) }: { channel?: Channel; platform?: string; onRecheck?: () => void; onTerminal?: () => Promise<boolean> }) {
  const state = useReobservation({ actionPath: channel.url, onTerminal });
  return <ReobservationSection item={{ platform }} channel={channel} domain={DOMAIN} state={state} onRecheck={onRecheck} />;
}

function setup(props: Parameters<typeof Harness>[0] = {}) {
  return { user: userEvent.setup(), ...render(<Harness {...props} />) };
}

describe('可用性（M01）', () => {
  // Mog 2026-10-23：普通层只说人话，原因码不在这里出现（留在「技术信息」的技术事实里）。
  it('没有关联授权：用人话写原因，不显示原因码，不提供按钮', () => {
    setup({ channel: { ...eligible, eligible: false, url: null, reason: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED' } });
    expect(screen.getByText(/这篇笔记现在不能复观测：它不在任何仍有效的观察授权里/)).toBeInTheDocument();
    expect(screen.queryByText('TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED')).toBeNull();
    expect(screen.queryByRole('button', { name: /复观测/ })).toBeNull();
  });

  it('平台不支持、入口没通过同源校验、非小红书：各写各的原因', () => {
    const { unmount } = setup({ channel: { ...eligible, supported: false, eligible: false, url: null } });
    expect(screen.getByText('这个平台的作品暂时不能复观测。')).toBeInTheDocument();
    unmount();
    const second = setup({ channel: { ...eligible, url: 'https://evil.example/reobserve' } });
    expect(screen.getByText('复观测入口地址不对，页面没有提供按钮。')).toBeInTheDocument();
    second.unmount();
    setup({ platform: 'douyin' });
    expect(screen.queryByRole('button', { name: '立即复观测' })).toBeNull();
  });

  it('可用：范围用人话常驻（不放 tooltip），按钮可点；不再单列媒体策略码', () => {
    setup();
    expect(screen.getByText('重新打开这篇笔记，更新点赞、评论、收藏、分享，顺带采最多 30 条评论和回复；不重新下载图片视频，也不重新识字。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '立即复观测' })).toBeEnabled();
    expect(screen.queryByText(/EXISTING_ASSETS_REUSED|媒体策略/)).toBeNull();
  });
});

describe('确认步骤（DR-3）', () => {
  it('点击先弹确认，说清会做什么、不会做什么、提交后不能撤销；取消不发请求', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: '立即复观测' }));
    const dialog = screen.getByRole('dialog', { name: '确认立即复观测' });
    expect(within(dialog).getByText(/让合格的工位真实访问平台/)).toBeInTheDocument();
    expect(within(dialog).getByText(/提交后不能撤销/)).toBeInTheDocument();
    expect(within(dialog).getByText('下载新的媒体，或运行图片文字、转录')).toBeInTheDocument();
    expect(within(dialog).queryByText(/再提交会新建一张工单/)).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(kit.calls).toHaveLength(0);
  });

  it('「取消」排在「确认提交」之前，是第一个可聚焦控件：真实世界动作不靠一次 Enter 误触发', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: '立即复观测' }));
    const buttons = within(screen.getByRole('dialog')).getAllByRole('button');
    expect(buttons.map(button => button.textContent)).toEqual(['取消', '确认提交']);
  });
});

describe('四种结局', () => {
  async function confirm(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole('button', { name: '立即复观测' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^(确认提交|仍然提交)$/ }));
  }

  it('已提交：确认后恰好一个 POST，按钮变为「复观测已入队」，执行状态说明这不是已读取', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson(), statusUrl: null }));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('已提交')).toBeInTheDocument();
    expect(screen.getByText(/复观测工单已形成，尚未由工位认领。这不是已读取，也不是已刷新。/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '复观测已入队' })).toBeDisabled();
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    expect(screen.getByText('本次复观测执行状态')).toBeInTheDocument();
    expect(screen.getByText('已按关联授权准入')).toBeInTheDocument();
    expect(screen.getByText('工单已进入立即队列，尚未被合格工位认领；这不是平台已读取、执行或采集成功。')).toBeInTheDocument();
    // 工单编号只在折叠的「技术详情」里，普通层不出现。
    const technical = screen.getByText(operationJson().workOrderRef as string).closest('details') as HTMLDetailsElement;
    expect(technical.open).toBe(false);
    expect(within(technical).getByText('工单')).toBeInTheDocument();
  });

  it('未受理：200 却被拒——显示原因，不写成已排队，按钮仍可用', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson({ admission: 'DEFERRED', workOrderRef: null, execution: 'NOT_STARTED', admissionReason: '当前没有可用资源' }), statusUrl: null }));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('未受理')).toBeInTheDocument();
    expect(screen.getByText(/请求已记录，等待可用资源；没有形成工单/)).toBeInTheDocument();
    expect(screen.getAllByText(/当前没有可用资源/).length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('准入已形成记录，但当前没有可读取的执行 lane；这不是平台已读取或执行成功。')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '立即复观测' })).toBeEnabled();
    expect(screen.queryByText('已提交')).toBeNull();
  });

  it('未完成：明确的非 2xx 答复，本次没有形成工单', async () => {
    kit.route('POST', ACTION, fail(409, 'reobservation_authorization_not_linked'));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('未完成')).toBeInTheDocument();
    expect(screen.getByText(/本次没有形成工单。该作品当前尚未具备监控目标关联的有效深度归档授权/)).toBeInTheDocument();
    expect(screen.getByText('reobservation_authorization_not_linked')).toBeInTheDocument();
  });

  it('未确认：不自动重发；提供“重新核对”和“查看采集任务”；再次提交要再确认并写明后果', async () => {
    kit.route('POST', ACTION, () => 'network', ok({ operation: operationJson(), statusUrl: null }));
    const onRecheck = vi.fn();
    const { user } = setup({ onRecheck });
    await confirm(user);
    expect(await screen.findByText('未确认')).toBeInTheDocument();
    expect(screen.getByText(/可能已经入队/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看采集任务' })).toHaveAttribute('href', `/collection/tasks?domain=${DOMAIN}`);
    await user.click(screen.getByRole('button', { name: '重新核对' }));
    expect(onRecheck).toHaveBeenCalledTimes(1);
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: '立即复观测' }));
    const dialog = screen.getByRole('dialog', { name: '确认立即复观测' });
    expect(within(dialog).getByText(/再提交会新建一张工单，工位会再访问一次平台/)).toBeInTheDocument();
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    await user.click(within(dialog).getByRole('button', { name: '仍然提交' }));
    await waitFor(() => expect(kit.of('POST', ACTION)).toHaveLength(2));
    expect(await screen.findByText('已提交')).toBeInTheDocument();
  });
});

describe('终态与任务行', () => {
  it('任务行：通道名＋状态线；编号下沉到技术详情；新补的三条状态文案', async () => {
    kit.route(
      'POST',
      ACTION,
      ok({
        operation: operationJson({
          leaseRef: 'lease-1',
          tasks: [taskJson('PAGE_UNAVAILABLE'), taskJson('DETAIL_READ_BLOCKED', { capability: 'comments', commentLimit: 30 }), taskJson('QUEUED', { capability: 'replies' })],
        }),
        statusUrl: null,
      }),
    );
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: '立即复观测' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText('平台页面已不存在（采集端确认）')).toBeInTheDocument();
    expect(screen.getByText('详情页已打开，但没有读成')).toBeInTheDocument();
    expect(screen.getByText('详情')).toBeInTheDocument();
    expect(screen.getByText('评论')).toBeInTheDocument();
    expect(screen.getByText('回复')).toBeInTheDocument();
    expect(screen.getByText('30')).toBeInTheDocument();
    expect(screen.getAllByText('本详情 lane 不单独请求评论').length).toBeGreaterThan(0);
  });
});
