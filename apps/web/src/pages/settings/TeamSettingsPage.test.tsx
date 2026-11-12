import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TeamSettingsPage } from './TeamSettingsPage';
import { savePendingStop } from './teamSettingsApi';

const roster = { actorMemberRef: 'admin-2', members: [
  { memberRef: 'admin-1', email: 'admin@example.com', displayName: 'Admin', role: 'admin', state: 'active', version: 2, currentAccountCount: 1 },
  { memberRef: 'member-1', email: 'member@example.com', displayName: 'Member', role: 'operator', state: 'invited', invitationState: 'pending', version: 1, currentAccountCount: 0 },
] };
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><TeamSettingsPage /></QueryClientProvider>);
}

beforeEach(() => { sessionStorage.clear(); vi.stubGlobal('fetch', vi.fn()); });
afterEach(() => { vi.unstubAllGlobals(); });

describe('团队成员设置页', () => {
  it('用完整服务器名册展示生命周期和账号责任，而非把未读到的值补零', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(response(roster));
    mount();
    await screen.findByText('admin@example.com');
    expect(within(screen.getByRole('table')).getByText('可登录')).toBeVisible();
    expect(screen.getByText('待签收')).toBeVisible();
    expect(screen.getByRole('cell', { name: '1 个当前负责账号' })).toBeVisible();
    expect(screen.getByRole('heading', { name: '团队与权限' })).toBeVisible();
  });

  it('明确把邀请定义为受控交接链接，不把创建结果描述为已发邮件', async () => {
    vi.mocked(fetch)
      .mockResolvedValueOnce(response(roster))
      .mockResolvedValueOnce(response({ invitation: { ref: 'invite-1', memberRef: 'member-2', token: 'one-time-token', expiresInSeconds: 604800 } }))
      .mockResolvedValueOnce(response(roster));
    mount(); await screen.findByText('Admin');
    fireEvent.click(screen.getByRole('button', { name: '邀请成员' }));
    fireEvent.input(screen.getByLabelText('姓名'), { target: { value: 'New member' } });
    fireEvent.input(screen.getByLabelText('工作邮箱'), { target: { value: 'new@example.com' } });
    fireEvent.click(screen.getByRole('button', { name: '生成邀请链接' }));
    await screen.findByText('邀请链接已生成');
    expect(screen.getByText(/尚未发送邮件/)).toBeVisible();
    expect(String(screen.getByLabelText('一次性设密链接').getAttribute('value'))).toContain('#token=one-time-token');
    expect(fetch).toHaveBeenNthCalledWith(2, '/api/local/auth/invitations', expect.objectContaining({ method: 'POST', body: JSON.stringify({ email: 'new@example.com', displayName: 'New member', role: 'operator' }) }));
  });

  it('邀请提交期间保持当前表单，回执标出对应受邀人', async () => {
    let complete!: (result: Response) => void;
    const pending = new Promise<Response>(resolve => { complete = resolve; });
    vi.mocked(fetch).mockResolvedValueOnce(response(roster)).mockReturnValueOnce(pending).mockResolvedValueOnce(response(roster));
    mount();
    await screen.findByText('Admin');
    fireEvent.click(screen.getByRole('button', { name: '邀请成员' }));
    fireEvent.input(screen.getByLabelText('姓名'), { target: { value: '受邀运营' } });
    fireEvent.input(screen.getByLabelText('工作邮箱'), { target: { value: 'new@example.com' } });
    fireEvent.click(screen.getByRole('button', { name: '生成邀请链接' }));
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByRole('button', { name: '关闭邀请' })).toBeDisabled();
    dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
    expect(within(dialog).getByLabelText('姓名')).toHaveValue('受邀运营');
    complete(response({ invitation: { ref: 'invite-1', memberRef: 'member-2', token: 'one-time-token', expiresInSeconds: 604800 } }));
    expect(await within(dialog).findByText(/通过团队渠道交给受邀运营/)).toBeVisible();
  });

  it('停用提交失去响应后，刷新按原请求 ID 找回回执', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(response(roster)).mockRejectedValueOnce(new Error('timeout'));
    const first = mount();
    await screen.findByText('Admin');
    fireEvent.click(screen.getByRole('button', { name: '办理离职' }));
    expect(screen.queryByRole('button', { name: '立即停用' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: /紧急停权/ }));
    fireEvent.input(screen.getByLabelText('停用原因'), { target: { value: '运营交接' } });
    fireEvent.click(screen.getByRole('button', { name: '立即停用' }));
    await screen.findByRole('button', { name: '重试同一停用请求' });
    const [url, request] = vi.mocked(fetch).mock.calls[1]!;
    expect(url).toBe('/api/local/auth/members/admin-1/deactivate');
    const body = JSON.parse(String((request as RequestInit).body));
    expect(body).toMatchObject({ expectedVersion: 2, reason: '运营交接' });
    first.unmount();

    vi.mocked(fetch).mockResolvedValueOnce(response(roster)).mockResolvedValueOnce(response({
      requestId: body.requestId, memberRef: 'admin-1', outcome: 'accepted', code: 'ops_member_deactivated',
      unassignedAccountCount: null, resultingVersion: 3, replayed: false,
    }));
    mount();
    await screen.findByText('Admin已停用');
    expect(screen.getByRole('link', { name: '去接管' })).toHaveAttribute('href', '/ops/accounts?pending_from=admin-1');
    expect(fetch).toHaveBeenNthCalledWith(4, `/api/local/auth/deactivations/${body.requestId}`, expect.objectContaining({ method: 'GET' }));
    await waitFor(() => expect(sessionStorage.length).toBe(0));
  });

  it('找回冲突回执后保留计划交接内容并重新读取账号版本', async () => {
    const activeRoster = { ...roster, members: [
      roster.members[0],
      { memberRef: 'operator', email: 'operator@example.com', displayName: '小周', role: 'operator', state: 'active', version: 1, currentAccountCount: 0 },
    ] };
    savePendingStop('admin-2', { memberRef: 'admin-1', body: {
      requestId: 'stale-request', expectedVersion: 2, reason: '已确认交接',
      assignments: [{ accountRef: 'account-1', responsibilityVersion: 1, nextMemberRef: 'operator', groupRef: null }],
    } });
    vi.mocked(fetch)
      .mockResolvedValueOnce(response(activeRoster))
      .mockResolvedValueOnce(response({ requestId: 'stale-request', outcome: 'stale', code: 'ops_member_stale', replayed: true }))
      .mockResolvedValueOnce(response({ memberVersion: 3, groupVacancyRef: null, accounts: [
        { accountRef: 'account-1', accountName: '育儿日记', platform: 'xhs', responsibilityVersion: 2 },
      ] }));
    mount();
    expect(await screen.findByText(/原请求未执行/)).toBeVisible();
    expect(screen.getByRole('radio', { name: /计划交接/ })).toBeChecked();
    expect(screen.getByLabelText('停用原因')).toHaveValue('已确认交接');
    expect(await screen.findByRole('combobox', { name: /育儿日记/ })).toHaveValue('operator');
  });
});
