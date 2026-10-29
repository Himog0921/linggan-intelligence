import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { PersonaFact } from './assetsModel';
import styles from './Assets.module.css';

type Operation = 'save' | 'confirm' | 'withdraw';
function PersonaForm({ accountRef, personRef, current: observed, checking, onSaved, onDirty, onPending, onReload }: {
  accountRef: string; personRef: string | null; current: PersonaFact | null; checking: boolean;
  onSaved: (fact: PersonaFact) => void; onDirty: (dirty: boolean) => void; onPending: (pending: boolean) => void;
  onReload: () => void;
}) {
  const [initial] = useState(observed);
  const [capturePerson] = useState(personRef);
  const [key, setKey] = useState(initial?.key ?? '');
  const [text, setText] = useState(initial?.text ?? '');
  const [source, setSource] = useState(initial?.source ?? 'manual');
  const [decision, setDecision] = useState<'keep_old' | 'use_new' | ''>('');
  const [confirmed, setConfirmed] = useState(false);
  const [withdraw, setWithdraw] = useState(false);
  const [method, setMethod] = useState('subject_present');
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState<Operation | null>(null);
  const [canAbandon, setCanAbandon] = useState(false);
  const [notice, setNotice] = useState('');
  const [ids] = useState(() => new RequestIdentities());
  const pendingId = useRef<string | null>(null);
  const paused = !checking && (!!capturePerson || !!initial) && !personRef && observed?.state !== 'withdrawn';
  const unavailable = !checking && ((!!initial && !!observed && (observed.state === 'withdrawn' || (!!personRef && !observed.currentPerson))) ||
    (!initial && !!capturePerson && !!personRef && personRef !== capturePerson));
  const locked = busy || uncertain !== null;
  const changed = !!initial && text.trim() !== (initial.text ?? '');
  const dirty = key.trim() !== (initial?.key ?? '') || text.trim() !== (initial?.text ?? '') || source !== (initial?.source ?? 'manual');
  const history = useQuery({ queryKey: ['creation-assets', 'persona-history', initial?.id],
    queryFn: ({ signal }) => api.personaHistory(initial!.id, signal), enabled: !!initial && !unavailable && !paused, gcTime: 0 });
  useEffect(() => { onDirty(dirty && !unavailable); }, [dirty, unavailable, onDirty]);
  useEffect(() => { if (unavailable) {
    // Erase the form's local copy on withdrawal or subject replacement.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setKey(''); setText(''); setSource('manual');
  } }, [unavailable]);

  async function run(kind: Operation) {
    if (busy || (uncertain && uncertain !== kind)) return;
    if (uncertain && (unavailable || paused)) {
      if (!pendingId.current) return;
      setBusy(true); setNotice('');
      try {
        const result = await api.personaRequestStatus(pendingId.current);
        if (!result) { setCanAbandon(true); setNotice('暂未查到结果，可以稍后核对或放弃原请求。'); return; }
        if (result.kind !== (kind === 'save' ? 'put' : kind) || (initial && result.factRef !== initial.id)) {
          setNotice('请求记录与当前操作不一致，请稍后重新核对。'); return;
        }
        setCanAbandon(false);
        if (paused) {
          setNotice('原请求已登记；确认真人后，请继续核对原请求以读取最新结果。'); return;
        }
        ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false);
        setNotice('已核对原请求；该事实当前不可用。');
      } catch { setNotice('暂时无法核对原请求。'); }
      finally { setBusy(false); }
      return;
    }
    const payload = kind === 'save' ? { accountRef, expectedPersonRef: capturePerson!, key: key.trim(), expectedVersion: initial?.version ?? 0,
      text: text.trim(), source, conflictDecision: changed ? decision || null : null }
      : kind === 'confirm' ? { expectedVersion: initial!.version, subjectConfirmed: true as const, confirmationMethod: method } : {};
    const requestId = ids.keyFor(kind, JSON.stringify([initial?.id, payload]));
    pendingId.current = requestId; setBusy(true); onPending(true); setNotice('');
    try {
      const result = kind === 'save' ? await api.putPersonaFact({ ...payload as Parameters<typeof api.putPersonaFact>[0], requestId })
        : kind === 'confirm' ? await api.confirmPersonaFact(initial!.id, { ...payload as Omit<Parameters<typeof api.confirmPersonaFact>[1], 'requestId'>, requestId })
          : await api.withdrawPersonaFact(initial!.id, { requestId });
      ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false); onSaved(result.fact);
    } catch (error) {
      if (error instanceof ApiError && error.answered) { ids.settle(kind); pendingId.current = null; setUncertain(null); onPending(false); }
      else { setUncertain(kind); onPending(true); }
      setNotice(api.assetsErrorText(error));
    } finally { setBusy(false); }
  }
  async function abandon() {
    if (!uncertain || !pendingId.current || busy) return;
    setBusy(true);
    try {
      const outcome = await api.cancelPersonaRequest(pendingId.current);
      if (outcome === 'committed' && paused) {
        setCanAbandon(false);
        setNotice('原请求已登记；确认真人后，请继续核对原请求以读取最新结果。');
        return;
      }
      ids.settle(uncertain); pendingId.current = null; setUncertain(null); setCanAbandon(false); onPending(false);
      setNotice(outcome === 'committed' ? '原请求已登记，当前事实不可用。' : '已放弃原请求，迟到请求不会执行。');
    } catch { setNotice('暂时无法放弃原请求。'); }
    finally { setBusy(false); }
  }
  if (unavailable || paused) return <div><h3>人设事实暂不可用</h3><p>{paused ? '账号真人暂未确认，草稿暂时隐藏；确认仍是同一位真人后可继续编辑。' : observed?.state === 'withdrawn' ? '本人已撤回，事实内容已移除。' : initial ? '这是上一位真人的事实，不能用于当前真人。' : '账号真人已变化，原真人的未保存事实已清除。'}</p>
    {!paused && initial && observed?.state !== 'withdrawn' ? <><label><input type="checkbox" disabled={locked} checked={withdraw} onChange={event => setWithdraw(event.target.checked)} /> 本人已要求撤回这条事实</label>
      <ActionButton variant="danger" disabled={!withdraw || locked} onClick={() => void run('withdraw')}>撤回事实</ActionButton></> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {uncertain ? <ActionButton disabled={busy} onClick={() => void run(uncertain)}>核对原请求</ActionButton> : null}
    {canAbandon ? <ActionButton disabled={busy} onClick={() => void abandon()}>放弃并停止原请求</ActionButton> : null}
  </div>;
  if (initial && !initial.currentPerson && observed?.currentPerson) return <div><p>账号真人已确认，请重新读取当前事实。</p><ActionButton disabled={locked} onClick={onReload}>读取当前事实</ActionButton></div>;
  return <form className={styles.form} onSubmit={event => { event.preventDefault(); void run('save'); }}>
    <h3>{initial ? '编辑人设事实' : '补充人设事实'}</h3>
    <p className={styles.hint}>只记录写稿需要的事实，不写真实姓名、学校或就诊细节。录入不等于本人确认。</p>
    <label>事实名称<input value={key} disabled={!!initial || locked} maxLength={40} onChange={event => setKey(event.target.value)} placeholder="例如：年级" /></label>
    <label>事实内容<input value={text} disabled={locked} maxLength={500} onChange={event => setText(event.target.value)} /></label>
    <label>来源<select value={source} disabled={locked} onChange={event => setSource(event.target.value)}><option value="manual">人工记录</option><option value="oral">本人口述</option><option value="manuscript">稿件</option><option value="inferred">系统推断</option></select></label>
    {changed ? <fieldset className={styles.choice} disabled={locked}><legend>新旧事实不一致</legend><p>旧内容：{initial?.text}</p>
      <label><input type="radio" name="persona-conflict" checked={decision === 'keep_old'} onChange={() => setDecision('keep_old')} /> 保留原来的</label>
      <label><input type="radio" name="persona-conflict" checked={decision === 'use_new'} onChange={() => setDecision('use_new')} /> 以新的为准</label>
      <p className={styles.hint}>两条都会保留在这条事实的历史中。</p></fieldset> : null}
    {!initial && !capturePerson ? <p className={styles.hint}>请先确认账号背后的真人。</p> : null}
    <ActionButton type="submit" variant="primary" disabled={locked || (!!initial && !dirty) || (!initial && !capturePerson) || !key.trim() || !text.trim() || (changed && !decision)}>保存事实</ActionButton>
    {initial?.status !== 'confirmed' && initial ? <fieldset className={styles.choice} disabled={locked}><legend>本人确认</legend>
      <label><input type="checkbox" checked={confirmed} onChange={event => setConfirmed(event.target.checked)} /> 本人已确认当前事实</label>
      <label>确认方式<select value={method} onChange={event => setMethod(event.target.value)}><option value="subject_present">本人当面确认</option><option value="subject_statement">本人明确表示同意</option></select></label>
      <ActionButton disabled={!confirmed || dirty} onClick={() => void run('confirm')}>登记本人确认</ActionButton>
      {dirty ? <p className={styles.hint}>请先保存更改，再确认保存后的事实。</p> : null}</fieldset> : initial ? <p>本人已确认 · 修改后需要再次确认。</p> : null}
    {initial ? <><details><summary>查看事实历史</summary>
      {history.isPending ? <p>正在读取历史…</p> : history.isError ? <p role="alert">历史读取失败。</p> : <ul>{history.data?.map(item => <li key={item.revision}>版本 {item.revision} · {item.applied ? '采用' : '保留为冲突记录'} · {item.status === 'confirmed' ? '本人已确认' : '待确认'}：{item.text}</li>)}</ul>}</details>
      <fieldset className={styles.choice} disabled={locked}><legend>本人撤回</legend><p className={styles.hint}>撤回会移除该事实及其历史内容。</p>
      <label><input type="checkbox" checked={withdraw} onChange={event => setWithdraw(event.target.checked)} /> 本人已要求撤回这条事实</label>
      <ActionButton variant="danger" disabled={!withdraw} onClick={() => void run('withdraw')}>撤回事实</ActionButton></fieldset></> : null}
    {notice ? <p role="alert" className={styles.problem}>{notice}</p> : null}
    {uncertain ? <ActionButton disabled={busy} onClick={() => void run(uncertain)}>核对并重试原请求</ActionButton> : null}
  </form>;
}

export function PersonaFactsLibrary({ domainRef, onDirtyChange = () => {}, onPendingChange = () => {} }: {
  domainRef: string; onDirtyChange?: (dirty: boolean) => void; onPendingChange?: (pending: boolean) => void;
}) {
  const client = useQueryClient();
  const [account, setAccount] = useState(''); const [selected, setSelected] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false); const [pending, setPending] = useState(false); const [formVersion, setFormVersion] = useState(0);
  const [rechecking, setRechecking] = useState(false);
  const [recheckFailed, setRecheckFailed] = useState(false);
  const retryRecheck = useRef<(() => void) | null>(null);
  const accounts = useQuery({ queryKey: ['creation-assets','accounts',domainRef], queryFn: ({ signal }) => api.listAccounts(domainRef,signal), gcTime: 0, refetchOnWindowFocus: false });
  const facts = useQuery({ queryKey: ['creation-assets','persona-facts',account], queryFn: ({ signal }) => api.listPersonaFacts(account,signal), enabled: !!account, gcTime: 0, refetchOnWindowFocus: false });
  const refetchAccounts = accounts.refetch; const refetchFacts = facts.refetch;
  const current = facts.data?.find(item => item.id === selected) ?? null;
  useEffect(() => {
    if (!account) return;
    let generation = 0; let disposed = false;
    const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('linggan-creation-experience-events');
    async function recheck() {
      const thisGeneration = ++generation;
      setRechecking(true); setRecheckFailed(false);
      client.removeQueries({ queryKey: ['creation-assets', 'persona-history'] });
      const [updatedAccounts, updatedFacts] = await Promise.all([refetchAccounts(), refetchFacts()]);
      if (!disposed && thisGeneration === generation) {
        if (updatedAccounts.isError || updatedFacts.isError) setRecheckFailed(true);
        else setRechecking(false);
      }
    }
    retryRecheck.current = () => void recheck();
    if (channel) channel.onmessage = event => { if (event.data === 'persona-changed' || event.data === 'identity-changed') void recheck(); };
    const onFocus = () => void recheck();
    window.addEventListener('focus', onFocus);
    return () => { disposed = true; retryRecheck.current = null; channel?.close(); window.removeEventListener('focus', onFocus); };
  }, [account, refetchAccounts, refetchFacts, client]);
  useEffect(() => {
    for (const item of facts.data ?? []) if (item.state === 'withdrawn' || !item.currentPerson) client.removeQueries({ queryKey: ['creation-assets','persona-history',item.id] });
  }, [facts.data, client]);
  function leave() { if (dirty && !window.confirm('这条事实有未保存内容，切换会丢弃本页编辑。确定切换吗？')) return false; setDirty(false); onDirtyChange(false); return true; }
  return <><div role="status" hidden={!rechecking}>{recheckFailed ? <>重新核对失败，已暂时隐藏事实。<ActionButton onClick={() => retryRecheck.current?.()}>重新核对</ActionButton></> : '正在重新核对事实，请稍候。'}</div><section style={rechecking ? { display: 'none' } : undefined} className={styles.assetBody} aria-label="人设事实"><div>
    <div className={styles.toolbar}><h2>人设事实</h2>{account ? <ActionButton variant="outline" disabled={pending} onClick={() => { if (leave()) { setSelected(null); setFormVersion(value => value + 1); } }}>补充事实</ActionButton> : null}</div>
    <label>账号<select value={account} disabled={pending} onChange={event => { if (leave()) { setAccount(event.target.value); setSelected(null); } }}><option value="">选择账号</option>
      {(accounts.data ?? []).map(item => <option key={item.accountRef} value={item.accountRef}>{item.accountName}</option>)}</select></label>
    {accounts.isError ? <p role="alert">账号读取失败。</p> : null}{account && facts.isPending ? <p role="status">正在读取事实…</p> : facts.isError ? <p role="alert">事实读取失败。</p> : null}
    <ul className={styles.list}>{facts.data?.map(item => <li key={item.id}><button type="button" disabled={pending} aria-current={item.id === selected ? 'true' : undefined} onClick={() => { if (item.id !== selected && leave()) setSelected(item.id); }}>
      <b>{item.key ?? '历史事实'}</b><small>{item.state === 'withdrawn' ? '已撤回' : !item.currentPerson ? accounts.data?.find(row => row.accountRef === account)?.identity?.hasRealPerson === true ? '原真人事实' : '真人待确认' : item.status === 'confirmed' ? '本人已确认' : item.status === 'inferred' ? '推断待确认' : '待确认'}</small></button></li>)}</ul>
  </div><div className={styles.detail}>{account && (!selected || current) ? <PersonaForm key={`${account}:${selected ?? 'new'}:${formVersion}`} accountRef={account}
    personRef={accounts.data?.find(row => row.accountRef === account)?.identity?.hasRealPerson ? accounts.data?.find(row => row.accountRef === account)?.identity?.personRef ?? null : null}
    current={current} checking={rechecking} onDirty={value => { setDirty(value); onDirtyChange(value); }} onPending={value => { setPending(value); onPendingChange(value); }} onReload={() => setFormVersion(value => value + 1)}
    onSaved={item => { if (typeof BroadcastChannel !== 'undefined') { const channel = new BroadcastChannel('linggan-creation-experience-events'); channel.postMessage('persona-changed'); channel.close(); } setDirty(false); onDirtyChange(false); client.setQueryData<PersonaFact[]>(['creation-assets','persona-facts',account], old => { const observed = old?.find(row => row.id === item.id); const safe = observed && (observed.state === 'withdrawn' || !observed.currentPerson) ? observed : item; return [safe,...(old ?? []).filter(row => row.id !== item.id)]; });
      if (item.state === 'withdrawn') client.removeQueries({ queryKey: ['creation-assets','persona-history',item.id] });
      setSelected(item.id); setFormVersion(value => value + 1); void client.invalidateQueries({ queryKey: ['creation-assets','persona-facts',account] }); }} /> : <p className={styles.hint}>选择账号后补充和确认事实。</p>}</div></section></>;
}
