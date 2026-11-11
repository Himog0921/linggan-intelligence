import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TeamSettingsPage } from './TeamSettingsPage';

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
    const side = screen.getByRole('complementary', { name: '设置导航' });
    expect(within(side).getByRole('link', { name: '团队成员' })).toHaveAttribute('aria-current', 'page');
    expect(within(side).getByRole('link', { name: '账户安全' })).toHaveAttribute('href', '/settings/account');
    expect(screen.getByRole('navigation', { name: '当前位置' })).toHaveTextContent('设置/团队成员');
    expect(screen.getByText('可登录')).toBeVisible();
    expect(screen.getByText('待签收')).toBeVisible();
    expect(screen.getByText('1 个当前负责账号')).toBeVisible();
    expect(screen.getByText(/只有管理员可以读取、邀请或停用成员/)).toBeVisible();
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

  it('停用提交失去响应后，刷新按原请求 ID 找回回执', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(response(roster)).mockRejectedValueOnce(new Error('timeout'));
    const first = mount();
    await screen.findByText('Admin');
    fireEvent.click(screen.getByRole('button', { name: '停用' }));
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
});
