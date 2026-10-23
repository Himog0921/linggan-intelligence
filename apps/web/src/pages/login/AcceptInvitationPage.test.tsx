import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { AcceptInvitationPage } from './AcceptInvitationPage';

beforeEach(() => vi.stubGlobal('fetch', vi.fn()));
afterEach(() => { vi.unstubAllGlobals(); window.history.replaceState({}, '', '/'); });
const response = (status = 200, body: unknown = { member: { ref: 'member' } }) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

it('consumes only a fragment token, strips it after activation, then enters the protected workspace', async () => {
  window.history.replaceState({}, '', '/invite/accept#token=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef');
  vi.mocked(fetch).mockResolvedValueOnce(response());
  const enter = vi.fn(); render(<AcceptInvitationPage onEnter={enter}/>);
  fireEvent.change(screen.getByLabelText('密码'), { target: { value: 'synthetic-password' } });
  fireEvent.change(screen.getByLabelText('确认密码'), { target: { value: 'synthetic-password' } });
  fireEvent.click(screen.getByRole('button', { name: '启用团队账号' }));
  await waitFor(() => expect(enter).toHaveBeenCalledOnce());
  expect(fetch).toHaveBeenCalledWith('/api/local/auth/invitations/accept', expect.objectContaining({ method: 'POST', credentials: 'same-origin' }));
  expect(window.location.hash).toBe('');
});

it('does not consume a missing token or navigate on a stale invitation', async () => {
  const enter = vi.fn(); render(<AcceptInvitationPage onEnter={enter}/>);
  expect(screen.getByRole('status')).toHaveTextContent('缺少安全令牌');
  expect(screen.getByRole('button', { name: '启用团队账号' })).toBeDisabled();
  expect(fetch).not.toHaveBeenCalled();
});

it('retains passwords locally and explains a consumed or revoked invitation', async () => {
  window.history.replaceState({}, '', '/invite/accept#token=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef');
  vi.mocked(fetch).mockResolvedValueOnce(response(409, { error: 'ops_invitation_unavailable' }));
  render(<AcceptInvitationPage/>);
  fireEvent.change(screen.getByLabelText('密码'), { target: { value: 'synthetic-password' } });
  fireEvent.change(screen.getByLabelText('确认密码'), { target: { value: 'synthetic-password' } });
  fireEvent.submit(screen.getByRole('button', { name: '启用团队账号' }).closest('form')!);
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('邀请链接已失效'));
  expect(screen.getByLabelText('密码')).toHaveValue('synthetic-password');
});
