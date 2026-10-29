import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { Experience } from './assetsModel';
import styles from './Assets.module.css';

function ExperienceForm({ accountRef, personRef, current: initialCurrent, onSaved, onPending, onDirty }: { accountRef: string; personRef: string | null; current: Experience | null; onSaved: (item: Experience) => void; onPending: (pending: boolean) => void; onDirty: (dirty: boolean) => void }) {
  const [current, setCurrent] = useState(initialCurrent);
  const [capturePersonRef] = useState(personRef);
  const [ids] = useState(() => new RequestIdentities());
  const pendingId = useRef<string | null>(null);
  const [title, setTitle] = useState(current?.title ?? '');
  const [body, setBody] = useState(current?.body ?? '');
  const [tags, setTags] = useState(current?.tags?.join(' ') ?? '');
  const [origin, setOrigin] = useState(current?.origin ?? 'oral');
  const [confirmed, setConfirmed] = useState(false);
  const [method, setMethod] = useState('subject_present');
  const [withdraw, setWithdraw] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [uncertain, setUncertain] = useState<'save' | 'confirm' | 'withdraw' | null>(null);
  const [canAbandon, setCanAbandon] = useState(false);
  const noLongerAvailable = !!initialCurrent && (!initialCurrent.currentPerson || initialCurrent.state === 'withdrawn');
  useEffect(() => {
    if (!noLongerAvailable || !initialCurrent) return;
    setTitle(''); setBody(''); setTags(''); setOrigin('oral');
    if (uncertain) ids.settle(uncertain);
    setCurrent(previous => previous ? { ...previous, currentPerson: initialCurrent.currentPerson, state: initialCurrent.state,
      title: null, body: null, tags: null, origin: null } : previous);
  }, [noLongerAvailable, initialCurrent, ids, uncertain]);
  const canEdit = !current || (current.currentPerson && current.state !== 'withdrawn' && current.state !== 'disabled');
  const normalizedTags = tags.split(/\s+/u).filter(Boolean);
  const dirty = title.trim() !== (current?.title ?? '') || body.trim() !== (current?.body ?? '') ||
    JSON.stringify(normalizedTags) !== JSON.stringify(current?.tags ?? []) || origin !== (current?.origin ?? 'oral');
  useEffect(() => { onDirty(dirty); }, [dirty, onDirty]);
  const locked = busy || uncertain !== null;

  async function perform(kind: 'save' | 'confirm' | 'withdraw') {
    if (busy || (uncertain && uncertain !== kind) || (kind === 'confirm' && dirty)) return;
    if (uncertain && noLongerAvailable) {
      if (!pendingId.current) return;
      setBusy(true); setMessage('');
      try {
        const result = await api.experienceRequestStatus(pendingId.current);
        const expectedKind = kind === 'save' ? (current ? 'edit' : 'create') : kind;
        if (!result) { setCanAbandon(true); setMessage('暂未查到该请求结果，可以稍后继续核对，或明确放弃原请求。'); return; }
        if (result.kind !== expectedKind || (current && result.experienceRef !== current.id)) { setMessage('请求结果不匹配，请稍后核对。'); return; }
        ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false);
        setMessage('已核对原请求；经历当前不可用。');
      } catch { setMessage('请求结果暂时无法核对，请稍后重试。'); }
      finally { setBusy(false); }
      return;
    }
    const payload = kind === 'save' ? { title, body, tags: normalizedTags, origin } : kind === 'confirm'
      ? { expectedRevision: current!.revision, subjectConfirmed: true as const, draftUse: true as const, sendToModel: null, confirmationMethod: method }
      : {};
    const requestId = ids.keyFor(kind, JSON.stringify([current?.id, current?.revision, payload]));
    pendingId.current = requestId;
    setBusy(true); onPending(true); setMessage('');
    try {
      const result = kind === 'confirm' ? await api.confirmExperience(current!.id, { ...payload as { expectedRevision: number; subjectConfirmed: true; draftUse: true; sendToModel: null; confirmationMethod: string }, requestId })
        : kind === 'withdraw' ? await api.withdrawExperience(current!.id, { requestId })
          : current ? await api.editExperience(current.id, { ...payload as { title: string; body: string; tags: string[]; origin: string }, expectedRevision: current.revision, requestId })
            : await api.createExperience({ ...payload as { title: string; body: string; tags: string[]; origin: string }, accountRef, expectedPersonRef: capturePersonRef!, requestId });
      ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false); onSaved(result.experience);
    } catch (error) {
      if (error instanceof ApiError && error.answered) { ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false); }
      else { setUncertain(kind); onPending(true); }
      setMessage(api.assetsErrorText(error));
    } finally { setBusy(false); }
  }
  async function abandon() {
    if (busy || !uncertain || !pendingId.current) return;
    setBusy(true);
    try {
      const outcome = await api.cancelExperienceRequest(pendingId.current);
      ids.settle(uncertain); pendingId.current = null; setUncertain(null); setCanAbandon(false); onPending(false);
      setMessage(outcome === 'committed' ? '原请求已经登记，经历当前不可用。' : '已放弃原请求，迟到的请求不会再执行。');
    } catch { setMessage('暂时无法放弃，请稍后重试。'); }
    finally { setBusy(false); }
  }

  if (current && (noLongerAvailable || !current.currentPerson || current.state === 'withdrawn')) return <div>
    <h3>{current.code}</h3><p>{initialCurrent?.state === 'withdrawn' || current.state === 'withdrawn' ? '本人已撤回，经历内容已移除。' : '这是上一位真人的经历，不能用于当前真人。'}</p>
    {initialCurrent?.state !== 'withdrawn' && current.state !== 'withdrawn' ? <><label><input type="checkbox" disabled={locked} checked={withdraw} onChange={e => setWithdraw(e.target.checked)} /> 本人已要求撤回这段经历</label>
      <ActionButton variant="danger" disabled={!withdraw || locked} onClick={() => void perform('withdraw')}>撤回经历</ActionButton></> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    {uncertain ? <ActionButton disabled={busy} onClick={() => void perform(uncertain)}>核对并重试原请求</ActionButton> : null}
    {canAbandon ? <ActionButton disabled={busy} onClick={() => void abandon()}>放弃并停止原请求</ActionButton> : null}
  </div>;
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void perform('save'); }}>
    <h3>{current ? `${current.code} · 编辑经历` : '采集一段经历'}</h3>
    {current && initialCurrent?.usedCount !== undefined ? <p className={styles.hint}>{initialCurrent.usedCount} 篇已保存稿件引用 · 不代表已发布</p> : null}
    <p className={styles.hint}>这里记录账号真人提供的经历，请去标识，不写真实姓名、学校和就诊细节。本人确认后才能以本人经历库记录插入；评论区经历可在材料浏览中作为参考。</p>
    <label>标题<input value={title} disabled={!canEdit || locked} maxLength={200} onChange={e => setTitle(e.target.value)} /></label>
    <label>经历正文<textarea value={body} disabled={!canEdit || locked} maxLength={10000} rows={8} onChange={e => setBody(e.target.value)} /></label>
    <label>标签<input value={tags} disabled={!canEdit || locked} onChange={e => setTags(e.target.value)} placeholder="以空格分隔" /></label>
    <label>来源<select value={origin} disabled={!canEdit || locked} onChange={e => setOrigin(e.target.value)}>
      <option value="oral">本人口述</option><option value="manuscript">稿子</option><option value="anonymized_coaching">去标识的陪跑记录</option>
    </select></label>
    {!current && capturePersonRef === null ? <p className={styles.hint}>请先确认账号背后的真人，再采集经历。</p> : null}
    <div className={styles.actions}><ActionButton type="submit" variant="primary" disabled={!canEdit || locked || (!current && capturePersonRef === null) || !title.trim() || !body.trim()}>保存经历</ActionButton></div>
    {current?.state === 'awaiting_subject' ? <fieldset className={styles.choice} disabled={locked}>
      <legend>本人确认</legend><p className={styles.hint}>请先让提供经历的本人确认当前版本和用于稿件；运营代录不等于本人确认。</p>
      <label><input type="checkbox" checked={confirmed} onChange={e => setConfirmed(e.target.checked)} /> 本人已确认这版内容并同意用于稿件</label>
      <label>确认方式<select value={method} onChange={e => setMethod(e.target.value)}><option value="subject_present">本人当面确认</option><option value="subject_statement">本人明确表示同意</option></select></label>
      {dirty ? <p className={styles.hint}>内容已更改，请先保存，再让本人确认保存后的版本。</p> : null}
      <ActionButton disabled={!confirmed || dirty} onClick={() => void perform('confirm')}>登记本人确认</ActionButton>
    </fieldset> : null}
    {current?.state === 'confirmed' ? <p>本人已确认 · 修改内容后需要再次确认。</p> : null}
    {current ? <fieldset className={styles.choice} disabled={locked}><legend>本人撤回</legend>
      <p className={styles.hint}>撤回会移除已保存内容，本页未保存的编辑也会丢弃。</p>
      <label><input type="checkbox" checked={withdraw} onChange={e => setWithdraw(e.target.checked)} /> 本人已要求撤回这段经历</label>
      <ActionButton variant="danger" disabled={!withdraw} onClick={() => void perform('withdraw')}>撤回经历</ActionButton>
    </fieldset> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    {uncertain ? <ActionButton disabled={busy} onClick={() => void perform(uncertain)}>核对并重试原请求</ActionButton> : null}
  </form>;
}

export function ExperienceLibrary({ domainRef, onPendingChange = () => {}, onDirtyChange = () => {} }: { domainRef: string; onPendingChange?: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const client = useQueryClient();
  const [account, setAccount] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [formVersion, setFormVersion] = useState(0);
  const accounts = useQuery({ queryKey: ['creation-assets', 'accounts', domainRef], queryFn: ({ signal }) => api.listAccounts(domainRef, signal), gcTime: 0 });
  const experiences = useQuery({ queryKey: ['creation-assets', 'experiences', account], queryFn: ({ signal }) => api.listExperiences(account, signal), enabled: !!account, gcTime: 0 });
  const current = experiences.data?.find(item => item.id === selected) ?? null;
  function leaveForm() { if (dirty && !window.confirm('这段经历有未保存内容，切换会丢弃本页编辑。确定切换吗？')) return false; setDirty(false); onDirtyChange(false); return true; }
  return <section className={styles.assetBody} aria-label="经历库"><div>
    <div className={styles.toolbar}><h2>经历库</h2>{account ? <ActionButton variant="outline" disabled={pending} onClick={() => { if (selected && leaveForm()) setSelected(null); }}>采集经历</ActionButton> : null}</div>
    <label>账号<select value={account} disabled={pending} onChange={e => { if (!leaveForm()) return; setAccount(e.target.value); setSelected(null); }}><option value="">选择账号</option>
      {(accounts.data ?? []).map(item => <option key={item.accountRef} value={item.accountRef}>{item.accountName}</option>)}</select></label>
    {accounts.isError || experiences.isError ? <p role="alert" className={styles.problem}>读取失败，请重试。</p> : null}
    {account && experiences.isPending ? <p role="status">正在读取…</p> : null}
    <ul className={styles.list}>{experiences.data?.map(item => <li key={item.id}><button type="button" disabled={pending} aria-current={item.id === selected ? 'true' : undefined} onClick={() => { if (item.id !== selected && leaveForm()) setSelected(item.id); }}>
      <b>{item.title ?? item.code}</b><small>{!item.currentPerson ? '原真人经历' : item.state === 'confirmed' ? '已确认' : item.state === 'withdrawn' ? '已撤回' : '待本人确认'}{item.usedCount === undefined ? '' : ` · ${item.usedCount} 篇稿件引用`}</small>
    </button></li>)}</ul>
  </div><div className={styles.detail}>{account && (!selected || current) ? <ExperienceForm key={`${account}:${selected ?? 'new'}:${formVersion}`} accountRef={account} personRef={accounts.data?.find(row => row.accountRef === account)?.identity?.hasRealPerson ? accounts.data?.find(row => row.accountRef === account)?.identity?.personRef ?? null : null} current={current}
    onDirty={value => { setDirty(value); onDirtyChange(value); }} onPending={value => { setPending(value); onPendingChange(value); }} onSaved={item => { if (item.state === 'withdrawn' && typeof BroadcastChannel !== 'undefined') { const channel = new BroadcastChannel('linggan-creation-experience-events'); channel.postMessage('withdrawn'); channel.close(); } setDirty(false); onDirtyChange(false); client.setQueryData<Experience[]>(['creation-assets', 'experiences', account], old => { const observed = old?.find(row => row.id === item.id); const safe = observed && (observed.state === 'withdrawn' || !observed.currentPerson) ? observed : item; return [safe, ...(old ?? []).filter(row => row.id !== item.id)]; }); setSelected(item.id); setFormVersion(value => value + 1); void client.invalidateQueries({ queryKey: ['creation-assets', 'experiences', account] }); }} />
    : <p className={styles.hint}>选择账号后采集和确认经历。</p>}</div></section>;
}
