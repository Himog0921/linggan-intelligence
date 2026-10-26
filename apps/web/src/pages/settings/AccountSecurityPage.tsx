import { useState, type FormEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { AppShell } from '../../app/AppShell';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError, requestJson } from '../../shared/api';
import styles from './AccountSecurityPage.module.css';

function changed(value: unknown) {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    && typeof (value as { member?: unknown }).member === 'object';
}

function errorText(error: unknown) {
  if (!(error instanceof ApiError)) return '暂时无法修改密码，请重试。';
  if (error.failure.kind === 'network') return '修改结果未确认。系统不会自动重复提交；请先用新密码尝试登录，或联系管理员核对会话。';
  if (error.failure.kind === 'invalid_response') return '服务器返回内容无法核对，请稍后重试。';
  if (error.failure.status === 401) return '当前密码不正确，或当前会话已失效。';
  if (error.failure.status === 422) return '新密码不能为空且长度不能超过系统限制。';
  return '服务器暂时无法修改密码，请稍后重试。';
}

/** The password only exists in controlled form state for this submit; it is never stored locally. */
export function AccountSecurityPage() {
  const session = useQuery({ queryKey: ['auth', 'session'], queryFn: ({ signal }) => requestJson('/api/local/auth/session', { signal }) });
  const admin = (session.data as { member?: { role?: string } } | undefined)?.member?.role === 'admin';
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    if (!currentPassword || !newPassword) { setNotice('请填写当前密码和新密码。'); return; }
    if (newPassword !== confirm) { setNotice('两次输入的新密码不一致。'); return; }
    setBusy(true); setNotice('');
    try {
      const value = await requestJson('/api/local/auth/password/change', { method: 'POST', body: { currentPassword, newPassword } });
      if (!changed(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
      setCurrentPassword(''); setNewPassword(''); setConfirm('');
      setNotice('密码已修改；此前的其他浏览器会话已失效，当前会话已更新。');
    } catch (error) { setNotice(errorText(error)); }
    finally { setBusy(false); }
  }
  const crumb = <span className={styles.crumb}><span>设置</span><span className="v7-slash">/</span><b>账户安全</b></span>;
  return <AppShell active={null} restrictedOps={!admin} boundaryLabel="会话安全" crumb={crumb} meta={<small>密码不会保存在浏览器本地存储</small>}>
    <main className={styles.page} aria-labelledby="account-security-title"><section className={styles.card}><p className={styles.eyebrow}>ACCOUNT / SECURITY</p><h1 id="account-security-title">修改密码</h1><p>提交前会校验当前密码。成功后服务器撤销此前的其他会话，并为当前浏览器更新会话。</p>
      <form onSubmit={submit} className={styles.form} noValidate><label>当前密码<input type="password" autoComplete="current-password" value={currentPassword} disabled={busy} onChange={event => setCurrentPassword(event.target.value)} /></label><label>新密码<input type="password" autoComplete="new-password" value={newPassword} disabled={busy} onChange={event => setNewPassword(event.target.value)} /></label><label>确认新密码<input type="password" autoComplete="new-password" value={confirm} disabled={busy} onChange={event => setConfirm(event.target.value)} /></label>{notice ? <p role="status" className={styles.notice}>{notice}</p> : null}<ActionButton type="submit" variant="primary" busy={busy}>修改密码</ActionButton></form>
      <p className={styles.boundary}>忘记密码的重设邮件与真实投递尚未配置；请联系管理员，不要把本页当作邮件找回功能。</p>
    </section></main>
  </AppShell>;
}
