import { useState, type FormEvent } from 'react';
import { ApiError, requestJson } from '../../shared/api';
import styles from './Login.module.css';

function invitationToken(hash: string) {
  return new URLSearchParams(hash.startsWith('#') ? hash.slice(1) : hash).get('token') || '';
}

function accepted(value: unknown) {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const member = (value as { member?: unknown }).member;
  return typeof member === 'object' && member !== null && !Array.isArray(member)
    && typeof (member as { ref?: unknown }).ref === 'string';
}

function errorText(error: unknown) {
  if (!(error instanceof ApiError)) return '暂时无法设置密码，请重试。';
  if (error.failure.kind === 'network') return '提交结果未确认。请保持此页面打开并使用同一邀请链接重试。';
  if (error.failure.kind === 'invalid_response') return '服务器返回了无法验证的响应，请稍后重试。';
  if (error.failure.status === 409) return '邀请链接已失效、被撤销或已使用。请联系管理员重新邀请。';
  if (error.failure.status === 422) return '邀请链接或密码不符合要求，请检查后重试。';
  return '服务器暂时无法完成设密，请稍后重试。';
}

/** Invitation material is deliberately held in the fragment: it never reaches page routing logs. */
export function AcceptInvitationPage({ onEnter = () => window.location.assign('/ops/publications') }: { onEnter?: () => void } = {}) {
  const token = invitationToken(window.location.hash);
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [notice, setNotice] = useState(token ? '' : '邀请链接缺少安全令牌。请联系管理员重新发送邀请。');
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (submitting || !token) return;
    if (!password) { setNotice('请设置密码。'); return; }
    if (password !== confirm) { setNotice('两次输入的密码不一致。'); return; }
    setSubmitting(true); setNotice('');
    try {
      const value = await requestJson('/api/local/auth/invitations/accept', { method: 'POST', body: { token, password } });
      if (!accepted(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
      // Remove the bearer material before any authenticated navigation or further UI activity.
      window.history.replaceState({}, '', '/invite/accept');
      try { onEnter(); } catch { setNotice('账号已启用，但暂时无法进入工作台。请从登录页重新登录。'); }
    } catch (error) { setNotice(errorText(error)); }
    finally { setSubmitting(false); }
  }
  return <div className={`login-page ${styles.page}`}>
    <header className="topbar"><div className="brand" aria-label="Linggan Intelligence 领域情报工作台"><div className="brand-mark" aria-hidden="true">LI</div><div><div className="brand-name">Linggan Intelligence</div><div className="brand-subtitle">领域情报工作台</div></div></div><div className="topbar-right"><span className="orange-square" aria-hidden="true"/><span>团队邀请</span></div></header>
    <main className="layout"><section className="hero" aria-hidden="true"><div className="hero-copy"><h1><span>加入团队，</span><span>再开始工作<span className="accent">。</span></span></h1><p className="hero-description">邀请只为指定团队成员启用一次。</p></div></section>
      <section className="access" aria-labelledby="invite-title"><div className="auth"><p className="auth-eyebrow" aria-hidden="true">TEAM / INVITATION</p><h1 className="auth-title" id="invite-title">设置团队密码<span className="title-square" aria-hidden="true"/></h1><p className="auth-description">设密会启用受邀账号并建立当前会话。邀请链接仅可使用一次。</p>
        <form onSubmit={submit} noValidate><div className="field"><label className="field-label" htmlFor="invite-password">密码</label><input className="text-input" id="invite-password" type="password" autoComplete="new-password" required value={password} disabled={!token || submitting} onChange={event => setPassword(event.target.value)} /></div>
          <div className="field"><label className="field-label" htmlFor="invite-confirm">确认密码</label><input className="text-input" id="invite-confirm" type="password" autoComplete="new-password" required value={confirm} disabled={!token || submitting} onChange={event => setConfirm(event.target.value)} /></div>
          <button className="submit-button" type="submit" disabled={!token || submitting} aria-busy={submitting}><span>{submitting ? '正在启用账号' : '启用团队账号'}</span></button>
          <p className="notice" role="status" aria-live="polite">{notice}</p>
        </form>
      </div></section>
    </main>
  </div>;
}
