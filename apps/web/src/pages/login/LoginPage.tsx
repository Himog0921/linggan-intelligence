import { useEffect, useRef, useState, type FormEvent } from 'react';
import { FontNotice } from '../../shared/FontNotice';
import { ApiError, requestJson } from '../../shared/api';
import fallbackUrl from './assets/fallback.svg?url';
import { mountScene } from './sceneRuntime';
import styles from './Login.module.css';

const STORAGE = 'linggan.login.remembered-email';
function Scene() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const controller = new AbortController();
    let destroy: (() => void) | undefined;
    void mountScene(host.current!, controller.signal).then(cleanup => {
      if (controller.signal.aborted) cleanup(); else destroy = cleanup;
    }).catch(() => { /* Static artwork stays visible if scene resources fail. */ });
    return () => { controller.abort(); destroy?.(); };
  }, []);
  return <div className="artwork station-art iso" data-theme="light" id="station-scene" ref={host}>
    <div className="station-fallback"><img src={fallbackUrl} alt="" /></div>
  </div>;
}
function safeNext(search: string) {
  const next = new URLSearchParams(search).get('next');
  return next && next.startsWith('/') && !next.startsWith('//') && !/[\\\r\n\t]/.test(next)
    ? next : '/ops/overview';
}

function hasMember(value: unknown) {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const member = (value as { member?: unknown }).member;
  return typeof member === 'object' && member !== null && !Array.isArray(member)
    && typeof (member as { ref?: unknown }).ref === 'string'
    && typeof (member as { displayName?: unknown }).displayName === 'string'
    && typeof (member as { role?: unknown }).role === 'string';
}

function loginError(error: unknown) {
  if (!(error instanceof ApiError)) return '暂时无法完成登录，请重试。';
  if (error.failure.kind === 'network') return '登录结果未确认。请检查网络后使用相同凭据重试。';
  if (error.failure.kind === 'invalid_response') return '登录服务返回了无法验证的响应，请稍后重试。';
  if (error.failure.status === 401) return '邮箱或密码不正确，或账号已停用。';
  if (error.failure.status === 503) return '登录服务暂不可用，请稍后重试。';
  return '服务器拒绝了本次登录，请检查输入后重试。';
}

export function LoginPage({ onEnter = target => window.location.assign(target) }: { onEnter?: (target: string) => void } = {}) {
  const publicEntry = !['localhost', '127.0.0.1'].includes(window.location.hostname);
  const email = useRef<HTMLInputElement>(null), password = useRef<HTMLInputElement>(null);
  const help = useRef<HTMLDetailsElement>(null), mounted = useRef(true);
  const [savedEmail] = useState(() => { try { return localStorage.getItem(STORAGE) ?? ''; } catch { return ''; } });
  const [remember, setRemember] = useState(!!savedEmail), [show, setShow] = useState(false), [caps, setCaps] = useState(false);
  const [errors, setErrors] = useState({ email: '', password: '' });
  const [submitting, setSubmitting] = useState(false), [submitted, setSubmitted] = useState(false), [notice, setNotice] = useState('');
  useEffect(() => {
    const title = document.title; document.title = '登录工作台 · Linggan Intelligence';
    const click = (event: MouseEvent) => { if (help.current?.open && !help.current.contains(event.target as Node)) help.current.open = false; };
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape' && help.current?.open) { help.current.open = false; help.current.querySelector('summary')?.focus(); } };
    document.addEventListener('click', click); document.addEventListener('keydown', key);
    return () => { mounted.current = false; document.title = title; document.removeEventListener('click', click); document.removeEventListener('keydown', key); };
  }, []);
  function validate(field: 'email' | 'password') {
    const input = (field === 'email' ? email : password).current!;
    const error = field === 'email' ? !input.value.trim() ? '请输入邮箱。' : input.validity.typeMismatch ? '请输入有效的邮箱地址。' : '' : !input.value ? '请输入密码。' : '';
    setErrors(current => ({ ...current, [field]: error })); return !error;
  }
  async function submit(event: FormEvent) {
    event.preventDefault(); if (submitting) return; setSubmitted(true); setNotice('');
    email.current!.value = email.current!.value.trim();
    const validEmail = validate('email'), validPassword = validate('password');
    if (!validEmail || !validPassword) { (validEmail ? password : email).current!.focus(); return; }
    setSubmitting(true);
    try {
      const response = await requestJson('/api/local/auth/login', {
        method: 'POST', body: { email: email.current!.value, password: password.current!.value },
      });
      if (!hasMember(response)) throw new ApiError({ kind: 'invalid_response', status: 200 });
      let storageUnavailable = false;
      try { if (remember) localStorage.setItem(STORAGE, email.current!.value); else localStorage.removeItem(STORAGE); } catch { storageUnavailable = remember; }
      password.current!.value = '';
      try { onEnter(safeNext(window.location.search)); }
      catch { if (mounted.current) setNotice('登录已验证，但暂时无法进入工作台，请重试。'); }
      if (storageUnavailable && mounted.current) setNotice('登录已验证；当前浏览器无法记住邮箱。');
    } catch (error) {
      if (mounted.current) setNotice(loginError(error));
    } finally {
      if (mounted.current) setSubmitting(false);
    }
  }
  return <div className={`login-page ${styles.page}`}>
    <header className="topbar"><div className="brand" aria-label="Linggan Intelligence 领域情报工作台"><div className="brand-mark" aria-hidden="true">LI</div><div><div className="brand-name">Linggan Intelligence</div><div className="brand-subtitle">领域情报工作台</div></div></div><div className="topbar-right"><span className="orange-square" aria-hidden="true"/><span>工作台登录</span></div></header>
    <main className="layout"><section className="hero" aria-labelledby="hero-title"><div className="hero-copy"><h1 id="hero-title"><span>让内容决策，</span><span>有据可依<span className="accent">。</span></span></h1><p className="hero-description">持续观察真实内容，发现信号，验证趋势。</p></div><div className="visual-stage" aria-hidden="true"><div className="visual-coordinate coordinate-a"/><div className="visual-coordinate coordinate-b"/><div className="signal-bracket"/><Scene/></div><footer className="hero-foot"><span>© 2026 Linggan Intelligence</span><FontNotice/><span className="material-stamp" aria-hidden="true"/></footer></section>
    <section className="access" aria-labelledby="login-title"><div className="auth"><p className="auth-eyebrow" aria-hidden="true">WORKSPACE / ACCESS</p><h2 className="auth-title" id="login-title">登录工作台<span className="title-square" aria-hidden="true"/></h2><p className="auth-description">{publicEntry ? '首期公网入口仅向管理员开放。' : '使用受邀的团队账号登录。'}账号状态与密码由服务器实时核验。</p>
      <form id="login-form" noValidate onSubmit={submit}>
        <div className="field"><label className="field-label" htmlFor="email">邮箱</label><div className="input-shell"><input ref={email} defaultValue={savedEmail} className="text-input" id="email" type="email" name="email" autoComplete="username" inputMode="email" autoCapitalize="none" spellCheck={false} placeholder="请输入邮箱" required aria-invalid={!!errors.email} aria-describedby="email-error" onBlur={event => { event.currentTarget.value = event.currentTarget.value.trim(); if (event.currentTarget.value || submitted) validate('email'); }} onInput={() => { if (errors.email) validate('email'); if (!submitting) setNotice(''); }}/></div><p className="field-error" id="email-error" aria-live="polite">{errors.email}</p></div>
        <div className="field"><label className="field-label" htmlFor="password">密码</label><div className="input-shell"><input ref={password} className="text-input password-input" id="password" type={show ? 'text' : 'password'} name="password" autoComplete="current-password" placeholder="请输入密码" required aria-invalid={!!errors.password} aria-describedby="password-error caps-lock" onKeyUp={event => setCaps(event.getModifierState('CapsLock'))} onBlur={event => { if (event.currentTarget.value || submitted) validate('password'); setCaps(false); }} onInput={() => { if (errors.password) validate('password'); if (!submitting) setNotice(''); }}/><button className="password-toggle" id="password-toggle" type="button" aria-label={show ? '隐藏密码' : '显示密码'} aria-pressed={show} aria-controls="password" title={show ? '隐藏密码' : '显示密码'} onClick={() => setShow(value => !value)}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s3.4-6 10-6 10 6 10 6-3.4 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2.8"/><path className="eye-slash" d="m3 3 18 18"/></svg></button></div><p className="field-error" id="password-error" aria-live="polite">{errors.password}</p><p className="caps-lock" id="caps-lock" hidden={!caps}>大写锁定已开启</p></div>
        <div className="utilities"><label className="remember" htmlFor="remember-email"><input id="remember-email" type="checkbox" checked={remember} onChange={event => { setRemember(event.target.checked); if (!event.target.checked) { try { localStorage.removeItem(STORAGE); } catch { /* Storage may be unavailable. */ } } }}/><span>记住邮箱</span></label><details className="help" id="login-help" ref={help}><summary>登录遇到问题？</summary><div className="help-content"><strong>检查受邀账号</strong><p>请使用管理员邀请的邮箱和已设置的密码。账号停用、密码错误或会话失效都不会进入工作台。</p></div></details></div>
        <button className="submit-button" id="submit-button" type="submit" aria-busy={submitting} disabled={submitting}><span id="submit-label">{submitting ? '正在进入' : '登录'}</span><svg className="submit-arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12h15M13 6l6 6-6 6"/></svg><span className="spinner" aria-hidden="true"/></button><div className="notice" id="form-notice" role="status" aria-live="polite">{notice}</div>
      </form><noscript><div className="notice">需要启用 JavaScript 才能登录工作台。</div></noscript>
    </div></section></main>
  </div>;
}
