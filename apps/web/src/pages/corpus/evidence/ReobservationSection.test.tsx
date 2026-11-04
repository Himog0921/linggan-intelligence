import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ObservationRecordSection } from './ObservationRecordSection';
import { parseWorkDetail } from './evidenceDecode';
import { DOMAIN, detailJson, operationJson, taskJson, workRef } from './evidenceFixtures';
import type { Reobservation as Channel } from './evidenceTypes';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';
import { useToast } from './toastContext';
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
  const toast = useToast();
  const state = useReobservation({
    actionPath: channel.url,
    onTerminal,
    onReceipt: result => {
      if (result.tone === 'success') toast.notify({ key: 'work:reobserve', tone: 'success', text: result.text, context: '合成作品' });
      else if (result.tone === 'unknown') toast.notify({ key: 'work:reobserve', tone: 'unknown', text: '提交结果未确认，请核对原请求', context: '合成作品' });
    },
  });
  const { item, inspector } = parseWorkDetail(detailJson(1));
  const work = platform === 'xhs' ? item : { ...item, platform };
  return <ObservationRecordSection item={work} inspector={inspector} channel={channel} domain={DOMAIN} state={state} onRecheck={onRecheck} />;
}

function setup(props: Parameters<typeof Harness>[0] = {}) {
  return renderWithProviders(<Harness {...props} />);
}

describe('可用性（M01）', () => {
  // Mog 2026-10-23：普通层只说人话，原因码不在这里出现（留在「技术信息」的技术事实里）。
  it('没有关联授权：用人话写原因，不显示原因码，不提供按钮', () => {
    setup({ channel: { ...eligible, eligible: false, url: null, reason: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED' } });
    expect(screen.getByText(/没有有效的观察授权。/)).toBeInTheDocument();
    expect(screen.queryByText('TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED')).toBeNull();
    expect(screen.queryByRole('button', { name: /复观测/ })).toBeNull();
  });

  it('资格理由未知：保守写待确认或暂不可用，不推断授权过期或目标停用', () => {
    const unknown = setup({ channel: { ...eligible, eligible: false, url: null, reason: 'REOBSERVATION_ELIGIBILITY_UNKNOWN' } });
    expect(screen.getByText(/资格待确认。/)).toBeInTheDocument();
    expect(screen.queryByText(/过期|停用/)).toBeNull();
    unknown.unmount();
    setup({ channel: { ...eligible, eligible: false, url: null, reason: 'SOME_FUTURE_REASON' } });
    expect(screen.getByText(/暂不可用。/)).toBeInTheDocument();
  });

  it('平台不支持、入口没通过同源校验、非小红书：各写各的原因', () => {
    const { unmount } = setup({ channel: { ...eligible, supported: false, eligible: false, url: null } });
    expect(screen.getByText(/暂不支持这个平台。/)).toBeInTheDocument();
    unmount();
    const second = setup({ channel: { ...eligible, url: 'https://evil.example/reobserve' } });
    expect(screen.getByText(/入口地址异常，页面没有提供按钮。/)).toBeInTheDocument();
    second.unmount();
    setup({ platform: 'douyin' });
    expect(screen.getByText(/暂不支持这个平台。/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '复观测' })).toBeNull();
  });

  it('可用：标题行给次级按钮；范围留在确认层，不单列媒体策略码', async () => {
    const { user } = setup();
    expect(screen.getByRole('button', { name: '复观测' })).toBeEnabled();
    expect(screen.queryByText(/EXISTING_ASSETS_REUSED|媒体策略/)).toBeNull();
    await user.click(screen.getByRole('button', { name: '复观测' }));
    const dialog = screen.getByRole('dialog', { name: '确认复观测' });
    expect(within(dialog).getByText(/读取这篇作品的详情、评论（最多 30 条）与回复/)).toBeInTheDocument();
  });
});

describe('确认步骤（DR-3）', () => {
  it('点击先弹紧凑确认，说清真实后果与不可撤销；取消不发请求', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: '复观测' }));
    const dialog = screen.getByRole('dialog', { name: '确认复观测' });
    expect(within(dialog).getByText(/重新访问平台，读取这篇作品的详情、评论（最多 30 条）与回复/)).toBeInTheDocument();
    expect(within(dialog).getByText(/提交后不能撤销/)).toBeInTheDocument();
    expect(within(dialog).getByText(/提交后等待执行，不下载新媒体/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/再提交会新建一张工单/)).toBeNull();
    await user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(kit.calls).toHaveLength(0);
  });

  it('「取消」排在提交之前并持有初始焦点：真实世界动作不靠一次 Enter 误触发', async () => {
    const { user } = setup();
    await user.click(screen.getByRole('button', { name: '复观测' }));
    const buttons = within(screen.getByRole('dialog')).getAllByRole('button');
    expect(buttons.map(button => button.textContent)).toEqual(['取消', '提交复观测']);
    expect(buttons[0]).toHaveFocus();
  });
});

describe('四种结局', () => {
  async function confirm(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole('button', { name: '复观测' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^(提交复观测|仍然提交)$/ }));
  }

  it('已提交：回执进轻通知，持续状态只留一条等待行；不再渲染技术大框', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson(), statusUrl: null }));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('复观测已提交，等待执行')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '复观测已入队' })).toBeDisabled();
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    expect(screen.getByText('等待执行')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看采集任务' })).toHaveAttribute('href', `/collection/tasks?domain=${DOMAIN}`);
    expect(screen.queryByText('本次复观测执行状态')).toBeNull();
    expect(screen.queryByText('已按关联授权准入')).toBeNull();
    expect(screen.queryByText(operationJson().workOrderRef as string)).toBeNull();
  });

  it('复用已有材料：回执写「已使用已有材料」，不留一条假的等待执行', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson({ admission: 'REUSE', workOrderRef: null, execution: 'NOT_STARTED' }), statusUrl: null }));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('已使用已有材料')).toBeInTheDocument();
    expect(screen.queryByText('等待执行')).toBeNull();
    expect(screen.queryByText('查看采集任务')).toBeNull();
  });

  it('未受理：200 却被拒——就地写原因，不写成已排队，也不发成功通知', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson({ admission: 'DEFERRED', workOrderRef: null, execution: 'NOT_STARTED', admissionReason: '当前没有可用资源' }), statusUrl: null }));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('未受理')).toBeInTheDocument();
    expect(screen.getByText('本次没有形成复观测任务。原因：当前没有可用资源')).toBeInTheDocument();
    expect(screen.getAllByText(/当前没有可用资源/).length).toBe(1);
    expect(screen.queryByText('复观测已提交，等待执行')).toBeNull();
    expect(screen.queryByText('等待执行')).toBeNull();
    expect(screen.getByRole('button', { name: '复观测' })).toBeEnabled();
  });

  it('未完成：明确的非 2xx 答复，说人话且不直出后台错误码', async () => {
    kit.route('POST', ACTION, fail(409, 'reobservation_authorization_not_linked'));
    const { user } = setup();
    await confirm(user);
    expect(await screen.findByText('未完成')).toBeInTheDocument();
    expect(screen.getByText(/本次没有提交。该作品当前尚未具备监控目标关联的有效深度归档授权/)).toBeInTheDocument();
    expect(screen.queryByText('reobservation_authorization_not_linked')).toBeNull();
  });

  it('未确认：通知与就地核对入口同时保留；再次提交要再确认并写明可能重复执行', async () => {
    kit.route('POST', ACTION, () => 'network', ok({ operation: operationJson(), statusUrl: null }));
    const onRecheck = vi.fn();
    const { user } = setup({ onRecheck });
    await confirm(user);
    expect(screen.getByText('提交结果未确认，请核对原请求')).toBeInTheDocument();
    expect(screen.getByText('提交结果未确认')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看采集任务' })).toHaveAttribute('href', `/collection/tasks?domain=${DOMAIN}`);
    await user.click(screen.getByRole('button', { name: '重新核对' }));
    expect(onRecheck).toHaveBeenCalledTimes(1);
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: '复观测' }));
    const dialog = screen.getByRole('dialog', { name: '确认复观测' });
    expect(within(dialog).getByText(/上次提交结果未确认，再次提交可能重复执行。建议先核对。/)).toBeInTheDocument();
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    await user.click(within(dialog).getByRole('button', { name: '仍然提交' }));
    await waitFor(() => expect(kit.of('POST', ACTION)).toHaveLength(2));
    expect(await screen.findByText('复观测已提交，等待执行')).toBeInTheDocument();
  });
});

describe('终态与任务行', () => {
  it('多通道终态只收成一条轻状态，不再铺通道名、编号与技术详情', async () => {
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
    await user.click(screen.getByRole('button', { name: '复观测' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '提交复观测' }));
    expect(await screen.findByText('等待执行')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '查看采集任务' })).toBeInTheDocument();
    expect(screen.queryByText('平台页面已不存在（采集端确认）')).toBeNull();
    expect(screen.queryByText('技术详情')).toBeNull();
    expect(screen.queryByText('本次复观测执行状态')).toBeNull();
  });
});
