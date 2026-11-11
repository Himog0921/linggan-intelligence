import { useEffect, useState, type FormEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError } from '../../shared/api';
import * as api from './teamSettingsApi';
import { pendingAccountsUrl, savePendingStop, type PendingStop } from './teamSettingsApi';
import styles from './TeamSettingsPage.module.css';

function rejection(error: unknown) {
  if (!(error instanceof ApiError) || error.failure.kind !== 'http') return null;
  switch (error.failure.status) {
    case 401: case 403: return '请用管理员账号重新登录后再操作。';
    case 409: return '成员或账号已变化。请刷新交接清单后再确认。';
    case 422: return '请核对成员和停用原因。最后一位管理员不能停用。';
    default: return null;
  }
}

export function StopReceipt({ receipt, members }: { receipt: api.DeactivationReceipt; members: api.TeamMember[] }) {
  const member = members.find(item => item.memberRef === receipt.memberRef);
  if (receipt.outcome === 'stale' || receipt.outcome === 'rejected') {
    return <p role="status">本次停用没有执行。请核对成员和账号状态。</p>;
  }
  return <div className={styles.receipt} role="status">
    <h2>{member ? `${member.displayName}已停用` : '成员已停用'}</h2>
    {receipt.transfers.length > 0 ? <>
      <p>本次已转交 {receipt.transfers.length} 个账号。</p>
      <ul>{receipt.transfers.map(item => <li key={item.accountRef}>
        {item.accountName ?? '运营账号'} → {item.nextMemberName ?? '接任成员'}
      </li>)}</ul>
    </> : null}
    {receipt.groupVacancyRef || member?.groupHasActiveLead === false
      ? <p>组长空缺，由管理员代管。</p> : null}
    {receipt.memberRef && receipt.unassignedAccountCount !== 0
      ? <a href={pendingAccountsUrl(receipt.memberRef)}>去接管</a>
      : <a href="/ops/accounts">查看账号台账</a>}
  </div>;
}

export function OffboardingPanel({ actor, member, members, resume, onDone }: {
  actor: string | null;
  member: api.TeamMember | null;
  members: api.TeamMember[];
  resume: PendingStop | null;
  onDone: () => void;
}) {
  const [planned, setPlanned] = useState(resume?.body.assignments !== undefined);
  const [reason, setReason] = useState('');
  const [recipients, setRecipients] = useState<Record<string, string>>({});
  const [pending, setPending] = useState(resume);
  const [receipt, setReceipt] = useState<api.DeactivationReceipt | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const preview = useQuery({
    queryKey: ['offboarding-preview', member?.memberRef],
    queryFn: ({ signal }) => api.fetchOffboardingPreview(member?.memberRef ?? '', signal),
    enabled: planned && !!member && !pending,
    retry: false,
    refetchOnWindowFocus: false,
  });

  useEffect(() => {
    if (!resume) return;
    const controller = new AbortController();
    void api.fetchDeactivation(resume.body.requestId, controller.signal).then(found => {
      savePendingStop(actor, null);
      setReceipt(found);
      setPending(null);
    }, () => {
      if (!controller.signal.aborted) setNotice('结果未确认。可以重试同一停用请求。');
    });
    return () => controller.abort();
  }, [actor, resume]);

  const candidates = members.filter(candidate => candidate.state === 'active' && candidate.memberRef !== member?.memberRef);
  const assignments = preview.data?.accounts.flatMap(account => {
    const candidate = candidates.find(item => item.memberRef === recipients[account.accountRef]);
    return candidate ? [{
      accountRef: account.accountRef, responsibilityVersion: account.responsibilityVersion,
      nextMemberRef: candidate.memberRef, groupRef: candidate.groupRef,
    }] : [];
  });
  const planReady = !preview.isFetching && !!preview.data && assignments?.length === preview.data.accounts.length;

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    let next = pending;
    if (!next) {
      if (!member || !reason.trim() || (planned && !planReady)) return;
      next = {
        memberRef: member.memberRef,
        body: {
          requestId: crypto.randomUUID(),
          expectedVersion: planned ? preview.data?.memberVersion ?? member.version : member.version,
          reason: reason.trim(),
          ...(planned ? { assignments } : {}),
        },
      };
    }
    savePendingStop(actor, next);
    setPending(next);
    setBusy(true);
    setNotice('');
    try {
      const result = await api.deactivateMember(next.memberRef, next.body);
      savePendingStop(actor, null);
      setPending(null);
      setReceipt(result);
      onDone();
    } catch (error) {
      const message = rejection(error);
      const sessionUnavailable = error instanceof ApiError && error.failure.kind === 'http'
        && [401, 403].includes(error.failure.status);
      if (message && !sessionUnavailable) {
        savePendingStop(actor, null);
        setPending(null);
        if (next.body.assignments !== undefined) {
          onDone();
          void preview.refetch();
        }
      }
      setNotice(message ?? '结果未确认。可以查询或重试同一停用请求。');
    } finally {
      setBusy(false);
    }
  }

  if (receipt) return <StopReceipt receipt={receipt} members={members} />;
  const groupVacancy = preview.data?.groupVacancyRef;
  return <form className={styles.deactivate} onSubmit={submit}>
    <p>{member ? `停用 ${member.displayName} 会撤销其登录访问。` : '正在恢复上一次停用操作。'}</p>
    {!pending ? <>
      <fieldset disabled={busy}>
        <legend>处理方式</legend>
        <label><input type="radio" name="offboarding-mode" checked={!planned} onChange={() => setPlanned(false)} />立即停用，之后接管</label>
        <label><input type="radio" name="offboarding-mode" checked={planned} onChange={() => setPlanned(true)} />先选接任人，一次完成交接和停用</label>
      </fieldset>
      {planned ? <>
        {preview.isPending ? <p role="status">正在读取账号清单…</p> : null}
        {preview.isError ? <p role="alert">账号清单暂不可用。可刷新清单，或选择立即停用。</p> : null}
        {preview.isError ? <ActionButton type="button" onClick={() => void preview.refetch()}>刷新账号清单</ActionButton> : null}
        {groupVacancy ? <p>停用后该组没有组长，由管理员临时代管。</p> : null}
        {preview.data?.accounts.length === 0 ? <p>没有在营账号需要转交。</p> : null}
        {preview.data?.accounts.map(account => <label key={account.accountRef}>
          {account.accountName}（{account.platform === 'xhs' ? '小红书' : '抖音'}）
          <select required value={candidates.some(candidate => candidate.memberRef === recipients[account.accountRef]) ? recipients[account.accountRef] ?? '' : ''} onChange={event =>
            setRecipients(previous => ({ ...previous, [account.accountRef]: event.target.value }))}>
            <option value="">选择接任人</option>
            {candidates.map(candidate => <option key={candidate.memberRef} value={candidate.memberRef}>
              {candidate.displayName}{candidate.groupName ? ` · ${candidate.groupName}` : ' · 未分组'}
            </option>)}
          </select>
        </label>)}
      </> : <p>仍由该成员负责的在营账号会进入待接管。</p>}
      <label>停用原因<textarea required maxLength={330} value={reason} onChange={event => setReason(event.target.value)} /></label>
    </> : null}
    {notice ? <p role="status" className={styles.error}>{notice}</p> : null}
    <ActionButton type="submit" variant="danger" busy={busy} disabled={!pending && planned && !planReady}>
      {pending ? '重试同一停用请求' : planned ? '确认交接并停用' : '立即停用'}
    </ActionButton>
  </form>;
}
