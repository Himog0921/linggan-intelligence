import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useRef, useState, type FormEvent } from 'react';
import { AppShell } from '../../app/AppShell';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError } from '../../shared/api';
import * as api from './teamSettingsApi';
import styles from './TeamSettingsPage.module.css';

const ROSTER_KEY = ['ops-auth', 'members'] as const;

const roleLabel: Record<api.TeamMember['role'], string> = { admin: '管理员', lead: '负责人', operator: '成员' };
const stateLabel: Record<api.TeamMember['state'], string> = { active: '可登录', invited: '待设密码', disabled: '已停用' };

function invitationUrl(receipt: api.InvitationReceipt) {
  return `${window.location.origin}/invite/accept#token=${encodeURIComponent(receipt.token)}`;
}

function errorText(error: unknown, action: 'invite' | 'deactivate') {
  if (!(error instanceof ApiError)) return action === 'invite' ? '暂时无法创建邀请，请重试。' : '暂时无法停用成员，请重试。';
  if (error.failure.kind === 'network') return action === 'invite'
    ? '邀请创建结果未确认。为避免生成多条有效链接，请先重新读取成员列表并确认。'
    : '停用结果未确认。请保持本次原因不变，重试同一请求。';
  if (error.failure.kind === 'invalid_response') return '服务器返回内容无法核对，请稍后重试。';
  if (error.failure.status === 403) return '当前会话没有团队管理权限。';
  if (error.failure.status === 409) return action === 'invite' ? '此邮箱当前不可邀请。请先刷新成员列表核对状态。' : '成员状态已变化或受最后一位管理员保护。请刷新后核对。';
  if (error.failure.status === 422) return action === 'invite' ? '请检查姓名、邮箱和角色。' : '停用请求内容无效，请检查原因。';
  return '服务器暂时无法完成此操作，请稍后重试。';
}

function InviteForm({ onCreated }: { onCreated: (receipt: api.InvitationReceipt) => void }) {
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [role, setRole] = useState<api.TeamMember['role']>('operator');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true); setNotice('');
    try {
      const receipt = await api.createInvitation({ email, displayName, role });
      setEmail(''); setDisplayName(''); setRole('operator'); onCreated(receipt);
    } catch (error) { setNotice(errorText(error, 'invite')); }
    finally { setBusy(false); }
  }
  return <form className={styles.form} onSubmit={submit} noValidate>
    <label>姓名<input required value={displayName} disabled={busy} onChange={event => setDisplayName(event.target.value)} /></label>
    <label>工作邮箱<input type="email" required autoComplete="email" value={email} disabled={busy} onChange={event => setEmail(event.target.value)} /></label>
    <label>角色<select value={role} disabled={busy} onChange={event => setRole(event.target.value as api.TeamMember['role'])}>
      <option value="operator">成员</option><option value="lead">负责人</option><option value="admin">管理员</option>
    </select></label>
    {notice ? <p className={styles.error} role="status">{notice}</p> : null}
    <ActionButton type="submit" variant="primary" busy={busy}>生成邀请链接</ActionButton>
  </form>;
}

function DeactivatePanel({ member, onDone }: { member: api.TeamMember; onDone: () => Promise<void> }) {
  const requestId = useRef<string | null>(null);
  const [reason, setReason] = useState('');
  const [busy, setBusy] = useState(false);
  const [retrySameRequest, setRetrySameRequest] = useState(false);
  const [notice, setNotice] = useState('');
  const locked = busy || retrySameRequest;
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy || !reason.trim()) { if (!reason.trim()) setNotice('请填写停用原因。'); return; }
    const id = requestId.current || crypto.randomUUID();
    requestId.current = id;
    setBusy(true); setNotice('');
    try {
      const receipt = await api.deactivateMember(member, id, reason.trim());
      if (receipt.outcome === 'accepted' || receipt.outcome === 'unchanged') {
        requestId.current = null; setRetrySameRequest(false);
        await onDone();
        setNotice(receipt.unassignedAccountCount > 0
          ? `已停用；${receipt.unassignedAccountCount} 个当前负责账号已改为未分配。`
          : '成员已停用。');
      } else {
        requestId.current = null; setRetrySameRequest(false);
        setNotice(`服务器未执行停用：${receipt.code}。请刷新成员列表后再判断。`);
      }
    } catch (error) {
      if (error instanceof ApiError && !error.answered) {
        setRetrySameRequest(true); setNotice(errorText(error, 'deactivate'));
      } else {
        requestId.current = null; setRetrySameRequest(false); setNotice(errorText(error, 'deactivate'));
      }
    } finally { setBusy(false); }
  }
  return <form className={styles.deactivate} onSubmit={submit}>
    <p><b>停用 {member.displayName}</b> 会撤销其当前浏览器会话和未接受的邀请。{member.currentAccountCount > 0 ? `其负责的 ${member.currentAccountCount} 个账号会改为“未分配”，不会自动转给其他人。` : '当前没有账号会被改动。'}</p>
    <label>停用原因<textarea required value={reason} disabled={locked} onChange={event => setReason(event.target.value)} /></label>
    {notice ? <p role="status" className={styles.error}>{notice}</p> : null}
    <ActionButton type="submit" variant="danger" busy={busy}>{retrySameRequest ? '重试同一停用请求' : '确认停用成员'}</ActionButton>
  </form>;
}

function TeamMemberRow({ member, onDeactivate }: { member: api.TeamMember; onDeactivate: (member: api.TeamMember) => void }) {
  return <li className={styles.member} data-state={member.state}>
    <div className={styles.memberIdentity}><b>{member.displayName}</b><small>{member.email}</small></div>
    <span>{roleLabel[member.role]}</span><span>{stateLabel[member.state]}</span>
    <span>{member.currentAccountCount} 个当前负责账号</span>
    <div>{member.state === 'disabled' ? <small>停用版本 {member.version}</small> : <ActionButton onClick={() => onDeactivate(member)} variant="danger">停用</ActionButton>}</div>
  </li>;
}

export function TeamSettingsPage() {
  const client = useQueryClient();
  const roster = useQuery({ queryKey: ROSTER_KEY, queryFn: ({ signal }) => api.fetchRoster(signal), retry: false, refetchOnWindowFocus: false });
  const [showInvite, setShowInvite] = useState(false);
  const [receipt, setReceipt] = useState<api.InvitationReceipt | null>(null);
  const [deactivating, setDeactivating] = useState<api.TeamMember | null>(null);
  const members = roster.data ?? [];
  const current = members.filter(member => member.state === 'active').length;
  const crumb = <span className={styles.crumb}><span>设置</span><span className="v7-slash">/</span><b>团队成员</b></span>;
  async function refresh() { await client.invalidateQueries({ queryKey: ROSTER_KEY }); }
  return <AppShell active={null} boundaryLabel="本机团队控制" crumb={crumb} meta={<small>成员状态和账号责任以当前服务端读数为准</small>}>
    <main className={styles.page} aria-labelledby="team-settings-title">
      <header className={styles.header}><div><p className={styles.eyebrow}>TEAM / ACCESS</p><h1 id="team-settings-title">团队成员</h1><p>只有管理员可以读取、邀请或停用成员。此页面不显示密码、会话令牌或邀请令牌的存储值。</p></div><ActionButton variant="primary" onClick={() => { setShowInvite(value => !value); setReceipt(null); }}>{showInvite ? '收起邀请表单' : '邀请成员'}</ActionButton></header>
      <section className={styles.metrics} aria-label="团队概况"><div><b>{members.length}</b><span>全部成员</span></div><div><b>{current}</b><span>当前可登录</span></div><div><b>{members.filter(member => member.state === 'invited').length}</b><span>待设密码</span></div></section>
      {showInvite ? <section className={styles.panel} aria-labelledby="invite-title"><h2 id="invite-title">生成成员邀请</h2><p>系统只生成一次性设密链接，当前没有邮件投递配置。请通过受控渠道交付链接，并在交付后关闭此处。</p><InviteForm onCreated={created => { setReceipt(created); void refresh(); }}/></section> : null}
      {receipt ? <section className={styles.receipt} aria-labelledby="invite-receipt-title"><div><h2 id="invite-receipt-title">邀请链接已生成</h2><p>尚未发送邮件。链接在本页内存中仅显示一次；请使用受控渠道交给受邀人，完成后关闭或刷新页面。</p></div><label>一次性设密链接<input readOnly value={invitationUrl(receipt)} aria-label="一次性设密链接" onFocus={event => event.currentTarget.select()} /></label><small>有效期：{Math.floor(receipt.expiresInSeconds / 3600)} 小时。邀请记录：{receipt.ref}</small></section> : null}
      {deactivating ? <section className={styles.panel} aria-labelledby="deactivate-title"><div className={styles.panelHeading}><h2 id="deactivate-title">停用成员</h2><ActionButton onClick={() => setDeactivating(null)}>取消</ActionButton></div><DeactivatePanel member={deactivating} onDone={async () => { await refresh(); }}/></section> : null}
      <section className={styles.roster} aria-labelledby="roster-title"><div className={styles.rosterHead}><div><h2 id="roster-title">成员名册</h2><p>读取失败时不以空列表替代；停用会保留成员历史，并显式解除其当前账号责任。</p></div><ActionButton onClick={() => void roster.refetch()} busy={roster.isFetching}>刷新</ActionButton></div>
        {roster.isPending ? <p role="status">正在读取成员名册…</p> : null}
        {roster.isError ? <p className={styles.error} role="alert">成员名册无法读取。{errorText(roster.error, 'invite')}</p> : null}
        {roster.data ? <ul className={styles.members}><li className={styles.memberHead}><span>成员</span><span>角色</span><span>状态</span><span>当前账号责任</span><span>操作</span></li>{members.map(member => <TeamMemberRow key={member.memberRef} member={member} onDeactivate={setDeactivating} />)}</ul> : null}
      </section>
    </main>
  </AppShell>;
}
