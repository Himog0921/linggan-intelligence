import { useEffect, useState, type FormEvent } from 'react';
import { useBlocker } from 'react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError, isRecord, requestJson } from '../../shared/api';
import styles from './AiSettings.module.css';

type Budget = { capCents: number; revision: number; month: string; knownPicoUsd: string; reservedPicoUsd: string; unknownCalls: number };
type Command = { requestId: string; expectedRevision: number; capCents: number };
const KEY = ['creation-ai-month-budget'] as const;
function decode(value: unknown): Budget {
  if (!isRecord(value) || !Number.isSafeInteger(value.capCents) || !Number.isSafeInteger(value.revision) || typeof value.month !== 'string' || typeof value.knownPicoUsd !== 'string' || !/^\d+$/.test(value.knownPicoUsd) || typeof value.reservedPicoUsd !== 'string' || !/^\d+$/.test(value.reservedPicoUsd) || !Number.isSafeInteger(value.unknownCalls)) throw new Error('额度数据格式不正确');
  return value as Budget;
}
function usd(pico: string): string {
  const amount = BigInt(pico);
  const cents = amount / 10_000_000_000n;
  if (amount > 0n && cents === 0n) return '<$0.01';
  return `$${cents / 100n}.${(cents % 100n).toString().padStart(2, '0')}`;
}
function parseCents(value: string): number | null {
  const m = /^(\d{1,7})(?:\.(\d{1,2}))?$/.exec(value.trim());
  if (!m) return null;
  const cents = Number(m[1]) * 100 + Number((m[2] ?? '').padEnd(2, '0'));
  return Number.isSafeInteger(cents) && cents <= 1_000_000_000 ? cents : null;
}
export function CreativeBudget() {
  const client = useQueryClient();
  const budget = useQuery({ queryKey: KEY, queryFn: async ({ signal }) => decode(await requestJson('/ops/api/creation/ai-budget', { signal })), retry: false });
  const [draft, setDraft] = useState<{ value: string; revision: number } | null>(null);
  const [pending, setPending] = useState<Command | null>(null);
  const blocker = useBlocker(pending !== null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const visibleAmount = draft?.value ?? (budget.data ? (budget.data.capCents / 100).toFixed(2) : '');
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => { if (pending) { event.preventDefault(); event.returnValue = ''; } };
    window.addEventListener('beforeunload', beforeUnload);
    return () => window.removeEventListener('beforeunload', beforeUnload);
  }, [pending]);
  async function send(command: Command) {
    setPending(command); setBusy(true); setNotice('');
    try {
      const receipt = await requestJson('/ops/api/creation/ai-budget', { method: 'PUT', body: command });
      if (!isRecord(receipt) || receipt.capCents !== command.capCents || !Number.isSafeInteger(receipt.revision) || typeof receipt.replayed !== 'boolean') throw new Error('回执无法核实');
      setPending(null); setDraft(null); setNotice('月度额度已更新。');
      await client.invalidateQueries({ queryKey: KEY });
    } catch (error) {
      if (error instanceof ApiError && error.answered) { setPending(null); setNotice('设置未保存，已保留输入，请核对当前额度后再试。'); void budget.refetch(); }
      else setNotice('结果未确认，请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  function submit(event: FormEvent) {
    event.preventDefault();
    if (pending) { void send(pending); return; }
    const cents = parseCents(visibleAmount);
    if (!budget.data || cents === null) { setNotice('请输入不超过两位小数的非负美元金额。'); return; }
    if (draft && draft.revision !== budget.data.revision) { setNotice('额度已在别处更改，请核对后选择基于当前额度继续。'); return; }
    void send({ requestId: crypto.randomUUID(), expectedRevision: draft?.revision ?? budget.data.revision, capCents: cents });
  }
  return <section className={styles.group} aria-labelledby="creative-budget-title">
    {blocker.state === 'blocked' ? <p role="alert">额度设置结果尚未确认，请先留在本页并沿用原请求重试。 <button type="button" onClick={() => blocker.reset()}>返回额度设置</button></p> : null}
    <div><h2 id="creative-budget-title">创作 AI 月度额度</h2><p>按北京时间自然月计算，费用未核实时保留预留额度。写作与标题共用此上限；样板复刻接入后也计入。</p></div>
    {budget.isPending ? <p role="status">正在读取额度…</p> : budget.isError ? <p role="alert">额度读取失败。<button type="button" onClick={() => void budget.refetch()}>重新读取</button></p> : budget.data ? <>
      <p>{budget.data.month} · 已知费用 {usd(budget.data.knownPicoUsd)} · 待结算预留 {usd(budget.data.reservedPicoUsd)}{budget.data.unknownCalls ? ` · ${budget.data.unknownCalls} 次历史费用未知，已暂停新调用` : ''}</p>
      <form className={styles.group} onSubmit={submit}><label>每月上限（美元） <input inputMode="decimal" value={visibleAmount} onChange={event => { const value = event.target.value; const revision = budget.data?.revision; if (revision !== undefined) setDraft(previous => ({ value, revision: previous?.revision ?? revision })); }} disabled={busy || !!pending}/></label><ActionButton type="submit" disabled={busy}>{pending ? '沿用原请求重试' : '保存月度上限'}</ActionButton></form>
      {draft && !pending && draft.revision !== budget.data.revision ? <p>当前上限已变为 ${(budget.data.capCents / 100).toFixed(2)}。<button type="button" onClick={() => setDraft({ ...draft, revision: budget.data.revision })}>基于当前额度继续编辑</button></p> : null}
      {notice ? <p role="status">{notice}</p> : null}
    </> : null}
  </section>;
}
