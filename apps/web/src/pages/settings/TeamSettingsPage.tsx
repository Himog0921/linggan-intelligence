import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState, type FormEvent } from 'react';
import { ModalDialog } from '../../corpus/ModalDialog';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { ActionButton } from '../../shared/ActionButton';
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

function InviteForm({ onCreated, busy, onBusy }: {
  onCreated: (receipt: api.InvitationReceipt, name: string) => void;
  busy: boolean;
  onBusy: (busy: boolean) => void;
}) {
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [notice, setNotice] = useState('');
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    onBusy(true); setNotice('');
    try {
      const receipt = await api.createInvitation({ email, displayName, role: 'operator' });
      setEmail(''); setDisplayName(''); onCreated(receipt, displayName.trim());
    } catch (error) { setNotice(errorText(error, 'invite')); }
    finally { onBusy(false); }
  }
  return <form className={styles.form} onSubmit={submit}>
    <label>姓名<input data-initial-focus required value={displayName} disabled={busy} onChange={event => setDisplayName(event.target.value)} /></label>
    <label>工作邮箱<input type="email" required autoComplete="email" value={email} disabled={busy} onChange={event => setEmail(event.target.value)} /></label>
    {notice ? <p className={styles.error} role="status">{notice}</p> : null}
    <footer className={styles.formActions}><ActionButton type="submit" variant="primary" busy={busy}>生成邀请链接</ActionButton></footer>
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
  const [copyNotice, setCopyNotice] = useState('');
  const [invitedName, setInvitedName] = useState('');
  const [filter, setFilter] = useState<api.TeamMember['state'] | 'all'>('all');
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
  const crumb = <ContextBreadcrumb items={['运营', '团队与权限']} />;
  const waiting = members.filter(member => member.state === 'invited').length;
  const visibleMembers = members.filter(member => filter === 'all' || member.state === filter);
  function closeInvite() {
    if (inviteBusy) return;
    setShowInvite(false);
    setReceipt(null);
    setCopyNotice('');
  }
  async function copyInvitation(link: string) {
    try {
      await navigator.clipboard.writeText(link);
      setCopyNotice('已复制链接');
    } catch {
      const input = document.querySelector<HTMLInputElement>('#invite-link');
      input?.focus();
      input?.select();
      setCopyNotice('链接已选中，请手动复制。');
    }
  }
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
        setCopyNotice('');
        const updated = await api.createInvitation({ email: member.email, displayName: member.displayName, role: member.role });
        setInvitedName(member.displayName);
        setReceipt(updated);
      }
    } catch (error) {
      setInviteNotice(errorText(error, 'invite'));
    } finally {
      setInviteBusy(false);
      void refresh();
    }
  }
  return <AppShell active="ops" boundaryLabel="团队管理" contextBar={{ breadcrumb: crumb }}>
    <main className={styles.page} aria-labelledby="team-settings-title">
      <div className={styles.workspace}>
        <header className={styles.header}>
          <div><h1 id="team-settings-title" tabIndex={-1}>团队与权限</h1><p>邀请成员、分配运营账号，或办理离职交接。</p></div>
          <ActionButton variant="primary" disabled={inviteBusy} onClick={() => { setReceipt(null); setCopyNotice(''); setShowInvite(true); }}>邀请成员</ActionButton>
        </header>
        {inviteNotice ? <p className={styles.notice} role="status">{inviteNotice}</p> : null}
        <section className={styles.roster} aria-labelledby="roster-title">
          <div className={styles.rosterHead}>
            <h2 id="roster-title">成员名册</h2>
            <ActionButton onClick={() => void roster.refetch()} busy={roster.isFetching}>刷新</ActionButton>
          </div>
          {roster.data ? <nav className={styles.filters} aria-label="成员状态筛选">
            {([
              ['all', '全部', members.length],
              ['active', '可登录', current],
              ['invited', '待激活', waiting],
              ['disabled', '已停用', members.length - current - waiting],
            ] as const).map(([key, label, count]) => <button key={key} type="button" aria-pressed={filter === key} onClick={() => setFilter(key)}>
              {label}<span>{count}</span>
            </button>)}
          </nav> : null}
          {roster.isPending ? <p className={styles.empty} role="status">正在读取成员名册…</p> : null}
          {roster.isError ? <p className={styles.empty} role="alert">成员名册无法读取。{errorText(roster.error, 'invite')}</p> : null}
          {roster.data && !visibleMembers.length ? <p className={styles.empty}>此状态下暂无成员。</p> : null}
          {visibleMembers.length ? <table className={styles.members}>
            <thead><tr><th scope="col">成员</th><th scope="col">角色</th><th scope="col">状态</th><th scope="col">运营账号</th><th scope="col">操作</th></tr></thead>
            <tbody>{visibleMembers.map(member => <tr key={member.memberRef}>
              <th scope="row">
                <div className={styles.memberIdentity}>
                  <b>{member.displayName}</b><small>{member.email}</small>
                  {member.groupName ? <small>{member.groupName}{member.groupHasActiveLead === false ? ' · 管理员代管' : ''}</small> : null}
                </div>
              </th>
              <td>{roleLabel[member.role]}</td>
              <td><span className={styles.memberStatus} data-state={member.state}>{memberStatus(member)}</span></td>
              <td><span className={styles.accountCount}>{member.currentAccountCount}</span> 个{member.state === 'disabled' ? '待接管' : '当前负责'}账号</td>
              <td><div className={styles.actions}>
                {member.state === 'active' ? <a href={`/ops/accounts?assign_to=${encodeURIComponent(member.memberRef)}`}>分配账号</a> : null}
                {member.state === 'active' && member.memberRef !== actor ? <ActionButton onClick={() => openStop(member)}>办理离职</ActionButton> : null}
                {member.state === 'invited' ? <ActionButton busy={inviteBusy} onClick={() => void updateInvitation(member, false)}>重新生成邀请</ActionButton> : null}
                {member.state === 'invited' && member.invitationState === 'pending' && member.invitationRef
                  ? <ActionButton disabled={inviteBusy} onClick={() => void updateInvitation(member, true)}>撤销邀请</ActionButton> : null}
                {member.state === 'disabled' && member.currentAccountCount > 0 ? <a href={pendingAccountsUrl(member.memberRef)}>去接管</a> : null}
                {member.latestDeactivationRef ? <ActionButton onClick={() => setViewingReceipt(member.latestDeactivationRef)}>查看停用回执</ActionButton> : null}
              </div></td>
            </tr>)}</tbody>
          </table> : null}
        </section>
      </div>
    </main>
    {showInvite || receipt ? <ModalDialog labelledBy="invite-title" className={styles.modal} onClose={closeInvite}>
      <header className={styles.modalHeader}>
        <div><h2 id="invite-title">{receipt ? '邀请链接已生成' : '邀请运营成员'}</h2>
          <p>{receipt ? `尚未发送邮件。复制链接并通过团队渠道交给${invitedName}。` : '受邀人设密后将成为运营成员，随后可为其分配运营账号。'}</p></div>
        <ActionButton onClick={closeInvite} disabled={inviteBusy} aria-label="关闭邀请">关闭</ActionButton>
      </header>
      {receipt ? <div className={styles.form}>
        <label>一次性设密链接<input id="invite-link" readOnly value={invitationUrl(receipt)} onFocus={event => event.currentTarget.select()} /></label>
        <p>链接有效期为 {Math.floor(receipt.expiresInSeconds / 3600)} 小时。成员激活后，从名册点「分配账号」。</p>
        <footer className={styles.formActions}><span role="status">{copyNotice}</span><ActionButton variant="primary" onClick={() => void copyInvitation(invitationUrl(receipt))}>复制邀请链接</ActionButton></footer>
      </div> : <InviteForm busy={inviteBusy} onBusy={setInviteBusy} onCreated={(created, name) => { setInvitedName(name); setReceipt(created); void refresh(); }} />}
    </ModalDialog> : null}
    {deactivating || resume ? <ModalDialog labelledBy="deactivate-title" className={styles.modal} restoreFocus={() => document.getElementById('team-settings-title')} onClose={() => {
      setDeactivating(null);
      if (actor) setResumeOverride({ actor, stop: null });
    }}>
      <header className={styles.modalHeader}>
        <div><h2 id="deactivate-title">{deactivating ? `办理离职 · ${deactivating.displayName}` : '继续办理离职'}</h2></div>
        <ActionButton onClick={() => { setDeactivating(null); if (actor) setResumeOverride({ actor, stop: null }); }} aria-label="关闭离职">关闭</ActionButton>
      </header>
      <OffboardingPanel key={deactivating?.memberRef ?? resume?.memberRef} actor={actor}
        member={deactivating ?? members.find(item => item.memberRef === resume?.memberRef) ?? null}
        members={members} resume={deactivating ? null : resume} onDone={() => void refresh()} />
    </ModalDialog> : null}
    {viewingReceipt ? <ModalDialog labelledBy="stop-receipt-title" className={styles.modal} onClose={() => setViewingReceipt(null)}>
      <header className={styles.modalHeader}><h2 id="stop-receipt-title">离职回执</h2><ActionButton onClick={() => setViewingReceipt(null)} data-initial-focus>关闭回执</ActionButton></header>
      {pastReceipt.data ? <StopReceipt receipt={pastReceipt.data} members={members} /> : null}
      {pastReceipt.isPending ? <p className={styles.empty} role="status">正在读取停用回执…</p> : null}
      {pastReceipt.isError ? <p className={styles.empty} role="alert">停用回执暂不可读取。</p> : null}
    </ModalDialog> : null}
  </AppShell>;
}
