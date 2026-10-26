import { fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AccountSecurityPage } from './AccountSecurityPage';
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
beforeEach(() => { vi.stubGlobal('fetch', vi.fn(async (path: string) => path === '/api/local/auth/session' ? response({ member: { ref: 'member-1', role: 'operator' } }) : response({ member: { ref: 'member-1' } }))); }); afterEach(() => { vi.unstubAllGlobals(); });
function show() { return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><AccountSecurityPage/></QueryClientProvider>); }
describe('账户安全页', () => {
  it('只在服务器确认后清除密码字段，且不承诺邮件找回', async () => {
    show();
    fireEvent.input(screen.getByLabelText('当前密码'), { target: { value: 'old' } }); fireEvent.input(screen.getByLabelText('新密码'), { target: { value: 'new' } }); fireEvent.input(screen.getByLabelText('确认新密码'), { target: { value: 'new' } }); fireEvent.click(screen.getByRole('button', { name: '修改密码' }));
    await screen.findByText(/密码已修改/); expect(screen.getByLabelText('当前密码')).toHaveValue(''); expect(screen.getByText(/重设邮件与真实投递尚未配置/)).toBeVisible(); expect(fetch).toHaveBeenCalledWith('/api/local/auth/password/change', expect.objectContaining({ method: 'POST', body: JSON.stringify({ currentPassword: 'old', newPassword: 'new' }) }));
  });
  it('密码不一致不会发送请求', () => { show(); fireEvent.input(screen.getByLabelText('当前密码'), { target: { value: 'old' } }); fireEvent.input(screen.getByLabelText('新密码'), { target: { value: 'new' } }); fireEvent.input(screen.getByLabelText('确认新密码'), { target: { value: 'different' } }); fireEvent.click(screen.getByRole('button', { name: '修改密码' })); expect(screen.getByText('两次输入的新密码不一致。')).toBeVisible(); expect(vi.mocked(fetch).mock.calls.every(([path, options]) => path === '/api/local/auth/session' && options?.method !== 'POST')).toBe(true); });
});
