import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState, type FormEvent } from 'react';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { ActionButton } from '../../shared/ActionButton';
import { SettingsSideNav } from './SettingsSideNav';
import { ApiError } from '../../shared/api';
import * as api from './teamSettingsApi';
import styles from './TeamSettingsPage.module.css';
import { OffboardingPanel, StopReceipt } from './OffboardingPanel';
import { pendingAccountsUrl, readPendingStop } from './teamSettingsApi';

const ROSTER_KEY = ['ops-auth', 'members'] as const;

const roleLabel: Record<api.TeamMember['role'], string> = { admin: '管理员', lead: '组长', operator: '运营' };
const stateLabel: Record<api.TeamMember['state'], string> = { active: '可登录', invited: '待签收', disabled: '已停用' };

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
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true); setNotice('');
    try {
      const receipt = await api.createInvitation({ email, displayName, role: 'operator' });
      setEmail(''); setDisplayName(''); onCreated(receipt);
    } catch (error) { setNotice(errorText(error, 'invite')); }
    finally { setBusy(false); }
  }
  return <form className={styles.form} onSubmit={submit} noValidate>
    <label>姓名<input required value={displayName} disabled={busy} onChange={event => setDisplayName(event.target.value)} /></label>
    <label>工作邮箱<input type="email" required autoComplete="email" value={email} disabled={busy} onChange={event => setEmail(event.target.value)} /></label>
    {notice ? <p className={styles.error} role="status">{notice}</p> : null}
    <ActionButton type="submit" variant="primary" busy={busy}>生成邀请链接</ActionButton>
  </form>;
}

function memberStatus(member: api.TeamMember) {
  if (member.state !== 'invited') return stateLabel[member.state];
  if (member.invitationState === 'expired') return '邀请已过期';
  if (member.invitationState === 'revoked') return '邀请已撤销';
  return member.invitationState === 'pending' ? '待签收' : '尚无有效邀请';
}

export function TeamSettingsPage() {
  const client = useQueryClient();
  const roster = useQuery({ queryKey: ROSTER_KEY, queryFn: ({ signal }) => api.fetchRoster(signal), retry: false, refetchOnWindowFocus: false });
  const [showInvite, setShowInvite] = useState(false);
  const [receipt, setReceipt] = useState<api.InvitationReceipt | null>(null);
  const [deactivating, setDeactivating] = useState<api.TeamMember | null>(null);
  const [resumeOverride, setResumeOverride] = useState<{ actor: string; stop: ReturnType<typeof readPendingStop> } | null>(null);
  const [viewingReceipt, setViewingReceipt] = useState<string | null>(null);
  const [inviteNotice, setInviteNotice] = useState('');
  const [inviteBusy, setInviteBusy] = useState(false);
  const members = roster.data?.members ?? [];
  const actor = roster.data?.actorMemberRef ?? null;
  const storedAtEntry = useMemo(() => actor ? readPendingStop(actor) : null, [actor]);
  const resume = resumeOverride?.actor === actor ? resumeOverride.stop : storedAtEntry;
  function openStop(member: api.TeamMember) {
    const saved = actor ? readPendingStop(actor) : null;
    if (saved && actor) {
      setResumeOverride({ actor, stop: saved });
      setDeactivating(null);
    } else {
      setDeactivating(member);
    }
  }
  const pastReceipt = useQuery({
    queryKey: ['ops-auth', 'deactivation', viewingReceipt],
    queryFn: ({ signal }) => api.fetchDeactivation(viewingReceipt ?? '', signal),
    enabled: viewingReceipt !== null, retry: false,
  });
  const current = members.filter(member => member.state === 'active').length;
  const crumb = <ContextBreadcrumb items={['设置', '团队成员']} />;
  async function refresh() { await client.invalidateQueries({ queryKey: ROSTER_KEY }); }
  async function updateInvitation(member: api.TeamMember, revoke: boolean) {
    if (inviteBusy) return;
    setInviteBusy(true); setInviteNotice('');
    try {
      if (revoke && member.invitationRef) {
        await api.revokeInvitation(member.invitationRef);
        setReceipt(current => current?.ref === member.invitationRef ? null : current);
        setInviteNotice('邀请已撤销。');
      } else {
        setReceipt(await api.createInvitation({ email: member.email, displayName: member.displayName, role: member.role }));
      }
    } catch (error) {
      setInviteNotice(errorText(error, 'invite'));
    } finally {
      setInviteBusy(false);
      void refresh();
    }
  }
  return <AppShell active={null} boundaryLabel="本机团队控制" contextBar={{ breadcrumb: crumb, meta: <small>成员状态和账号责任以当前服务端读数为准</small> }}>
    <div className="v7-shell">
      <SettingsSideNav activeId="team-members" />
      <main className={styles.page} aria-labelledby="team-settings-title">
      <header className={styles.header}><div><h1 id="team-settings-title">团队成员</h1><p>只有管理员可以读取、邀请或停用成员。此页面不显示密码、会话令牌或邀请令牌的存储值。</p></div><ActionButton variant="primary" onClick={() => { setShowInvite(value => !value); setReceipt(null); }}>{showInvite ? '收起邀请表单' : '邀请成员'}</ActionButton></header>
      {roster.data ? <section className={styles.metrics} aria-label="团队概况">
        <div><b>{members.length}</b><span>全部成员</span></div>
        <div><b>{current}</b><span>当前可登录</span></div>
        <div><b>{members.filter(member => member.state === 'invited' && member.invitationState === 'pending').length}</b><span>待签收邀请</span></div>
      </section> : null}
      {showInvite ? <section className={styles.panel} aria-labelledby="invite-title"><h2 id="invite-title">生成成员邀请</h2><p>系统只生成一次性设密链接，当前没有邮件投递配置。请通过受控渠道交付链接，并在交付后关闭此处。</p><InviteForm onCreated={created => { setReceipt(created); void refresh(); }}/></section> : null}
      {receipt ? <section className={styles.receipt} aria-labelledby="invite-receipt-title"><div><h2 id="invite-receipt-title">邀请链接已生成</h2><p>尚未发送邮件。链接在本页内存中仅显示一次；请使用受控渠道交给受邀人，完成后关闭或刷新页面。</p></div><label>一次性设密链接<input readOnly value={invitationUrl(receipt)} aria-label="一次性设密链接" onFocus={event => event.currentTarget.select()} /></label><small>有效期：{Math.floor(receipt.expiresInSeconds / 3600)} 小时。邀请记录：{receipt.ref}</small></section> : null}
      {inviteNotice ? <p role="status">{inviteNotice}</p> : null}
      {deactivating || resume ? <section className={styles.panel} aria-labelledby="deactivate-title">
        <div className={styles.panelHeading}><h2 id="deactivate-title">停用成员</h2>
          <ActionButton onClick={() => { setDeactivating(null); if (actor) setResumeOverride({ actor, stop: null }); }}>关闭</ActionButton>
        </div>
        <OffboardingPanel key={deactivating?.memberRef ?? resume?.memberRef} actor={actor}
          member={deactivating ?? members.find(item => item.memberRef === resume?.memberRef) ?? null}
          members={members} resume={deactivating ? null : resume} onDone={() => void refresh()} />
      </section> : null}
      {viewingReceipt ? <section className={styles.panel}>
        <ActionButton onClick={() => setViewingReceipt(null)}>关闭回执</ActionButton>
        {pastReceipt.data ? <StopReceipt receipt={pastReceipt.data} members={members} /> : null}
        {pastReceipt.isPending ? <p role="status">正在读取停用回执…</p> : null}
        {pastReceipt.isError ? <p role="alert">停用回执暂不可读取。</p> : null}
      </section> : null}
      <section className={styles.roster} aria-labelledby="roster-title"><div className={styles.rosterHead}><div><h2 id="roster-title">成员名册</h2><p>读取失败时不以空列表替代；账号接管后仍可查询原成员的历史记录。</p></div><ActionButton onClick={() => void roster.refetch()} busy={roster.isFetching}>刷新</ActionButton></div>
        {roster.isPending ? <p role="status">正在读取成员名册…</p> : null}
        {roster.isError ? <p className={styles.error} role="alert">成员名册无法读取。{errorText(roster.error, 'invite')}</p> : null}
        {roster.data ? <ul className={styles.members}><li className={styles.memberHead}><span>成员</span><span>角色</span><span>状态</span><span>当前账号责任</span><span>操作</span></li>{members.map(member => <li key={member.memberRef} className={styles.member} data-state={member.state}>
          <div className={styles.memberIdentity}><b>{member.displayName}</b><small>{member.email}</small>
            {member.groupName ? <small>{member.groupName}{member.groupHasActiveLead === false ? ' · 组长空缺，管理员代管' : ''}</small> : null}
          </div>
          <span>{roleLabel[member.role]}</span><span>{memberStatus(member)}</span>
          <span>{member.currentAccountCount} 个{member.state === 'disabled' ? '待接管' : '当前负责'}账号</span>
          <div className={styles.actions}>
            {member.state === 'active' ? <a href={`/ops/accounts?assign_to=${encodeURIComponent(member.memberRef)}`}>分配账号</a> : null}
            {member.state === 'active' && member.memberRef !== actor ? <ActionButton disabled={!!deactivating || !!resume} onClick={() => openStop(member)} variant="danger">停用</ActionButton> : null}
            {member.state === 'invited' ? <ActionButton busy={inviteBusy} onClick={() => void updateInvitation(member, false)}>重新生成邀请</ActionButton> : null}
            {member.state === 'invited' && member.invitationState === 'pending' && member.invitationRef
              ? <ActionButton disabled={inviteBusy} onClick={() => void updateInvitation(member, true)}>撤销邀请</ActionButton> : null}
            {member.state === 'disabled' && member.currentAccountCount > 0 ? <a href={pendingAccountsUrl(member.memberRef)}>去接管</a> : null}
            {member.latestDeactivationRef ? <ActionButton onClick={() => setViewingReceipt(member.latestDeactivationRef)}>查看停用回执</ActionButton> : null}
          </div>
        </li>)}</ul> : null}
      </section>
      </main>
    </div>
  </AppShell>;
}
