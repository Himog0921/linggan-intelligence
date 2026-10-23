import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TeamSettingsPage } from './TeamSettingsPage';

const roster = { members: [
  { memberRef: 'admin-1', email: 'admin@example.com', displayName: 'Admin', role: 'admin', state: 'active', version: 2, currentAccountCount: 1 },
  { memberRef: 'member-1', email: 'member@example.com', displayName: 'Member', role: 'operator', state: 'invited', version: 1, currentAccountCount: 0 },
] };
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><TeamSettingsPage /></QueryClientProvider>);
}

beforeEach(() => { vi.stubGlobal('fetch', vi.fn()); });
afterEach(() => { vi.unstubAllGlobals(); });

describe('团队成员设置页', () => {
  it('用完整服务器名册展示生命周期和账号责任，而非把未读到的值补零', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(response(roster));
    mount();
    await screen.findByText('admin@example.com');
    expect(screen.getByText('可登录')).toBeVisible();
    expect(screen.getAllByText('待设密码')).toHaveLength(2);
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

  it('停用前显示账号责任影响，并由服务端回执决定完成', async () => {
    vi.mocked(fetch)
      .mockResolvedValueOnce(response(roster))
      .mockResolvedValueOnce(response({ requestId: 'request-1', outcome: 'accepted', code: 'ops_member_deactivated', unassignedAccountCount: 1, resultingVersion: 3, replayed: false }))
      .mockResolvedValueOnce(response({ members: [{ ...roster.members[0], state: 'disabled', version: 3, currentAccountCount: 0 }, roster.members[1]] }));
    mount(); await screen.findByText('Admin');
    fireEvent.click(screen.getAllByRole('button', { name: '停用' })[0]!);
    expect(screen.getByText(/会撤销其当前浏览器会话/)).toBeVisible();
    fireEvent.input(screen.getByLabelText('停用原因'), { target: { value: '交接完成' } });
    fireEvent.click(screen.getByRole('button', { name: '确认停用成员' }));
    await screen.findByText('已停用；1 个当前负责账号已改为未分配。');
    const call = vi.mocked(fetch).mock.calls[1];
    expect(call?.[0]).toBe('/api/local/auth/members/admin-1/deactivate');
    expect(JSON.parse(String((call?.[1] as RequestInit).body))).toMatchObject({ expectedVersion: 2, reason: '交接完成' });
    await waitFor(() => expect(screen.getByText('已停用')).toBeVisible());
  });
});
