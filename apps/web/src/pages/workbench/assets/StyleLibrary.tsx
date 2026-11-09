import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './stylesApi';
import { listAccounts, assetsErrorText } from './assetsApi';
import styles from './Assets.module.css';

const lines = (value: string) => value.split('\n').map(line => line.trim()).filter(Boolean);
type Form = { accountRef: string; name: string; sourceNote: string; structure: string; rhythm: string; wording: string; skip: string };
const blank: Form = { accountRef: '', name: '', sourceNote: '', structure: '', rhythm: '', wording: '', skip: '' };
const filled = (style: api.Style): Form => ({
  accountRef: style.accountRef, name: style.name, sourceNote: style.sourceNote,
  structure: style.structure.join('\n'), rhythm: style.rhythm.join('\n'),
  wording: style.wording.join('\n'), skip: style.skip.join('\n'),
});
const problem = (form: Form) => !form.accountRef ? '先选账号'
  : !form.name.trim() ? '风格名称不能为空'
  : [form.structure, form.rhythm, form.wording, form.skip].some(value => lines(value).length > 12) ? '每一类最多 12 条'
  : null;

export function StyleLibrary({ domainRef, onPendingChange, onDirtyChange = () => {} }: { domainRef: string; onPendingChange: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const client = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [selected, setSelected] = useState<string | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [stale, setStale] = useState(false);
  const query = useQuery({ queryKey: ['creation-assets', 'styles', domainRef], queryFn: ({ signal }) => api.listStyles(domainRef, signal), retry: false, gcTime: 0 });
  const accounts = useQuery({ queryKey: ['creation-assets', 'accounts', domainRef], queryFn: ({ signal }) => listAccounts(domainRef, signal), retry: false, gcTime: 0 });
  const items = query.data ?? [];
  const current = items.find(item => item.code === selected);
  const blocked = form ? problem(form) : '先选账号';
  const name = (ref: string) => accounts.data?.find(item => item.accountRef === ref)?.accountName ?? '账号未在列表中';
  function change(next: Partial<Form>) {
    setForm(value => value ? { ...value, ...next } : value);
    onDirtyChange(true);
  }
  async function save() {
    if (!form || busy || blocked) return;
    const command = {
      accountRef: form.accountRef, name: form.name.trim(), sourceNote: form.sourceNote.trim(),
      structure: lines(form.structure), rhythm: lines(form.rhythm), wording: lines(form.wording), skip: lines(form.skip),
    };
    const target = current && form.accountRef === current.accountRef ? current : null;
    const requestId = identities.keyFor('style-command', target ? `${target.code}:${target.revision}` : `new:${form.accountRef}:${form.name.trim()}`);
    setBusy(true); onPendingChange(true); setNotice('');
    try {
      const result = target
        ? await api.reviseStyle(target.code, { ...command, requestId, expectedRevision: target.revision })
        : await api.createStyle({ ...command, requestId, expectedRevision: 0 });
      identities.settle('style-command');
      setNotice(target ? `已保存 ${result.style.code} 第 ${result.style.revision} 版。` : `已新增 ${result.style.code}。`);
      setSelected(result.style.code); setForm(null); onDirtyChange(false);
      await client.invalidateQueries({ queryKey: ['creation-assets', 'styles', domainRef] });
    } catch (error) {
      identities.settle('style-command');
      setStale(api.staleStyleCommand(error));
      setNotice(assetsErrorText(error));
    } finally { setBusy(false); onPendingChange(false); }
  }
  const itemCards = items.length ? <div className="grid-cards">{items.map(item => {
    const traits = ([['结构', item.structure, false], ['节奏', item.rhythm, false], ['用词', item.wording, false], ['不学', item.skip, true]] as const);
    return <article className="card" key={item.code}>
      <div className="card-head"><b className="grow">{item.name}</b><span className="aux data">{item.code}</span></div>
      <div className="card-body">
        <div className="aux">来源：{item.sourceNote || '—'}</div>
        {traits.filter(([, values]) => values.length).map(([label, values, skip]) => <div key={label}>
          <div className="t-label">{label}</div>
          <ul className={skip ? 'trait skip' : 'trait'}>{values.map(value => <li key={value}>{value}</li>)}</ul>
        </div>)}
      </div>
      <div className="card-foot">用在：<span className="chip">{name(item.accountRef)}</span><span className="grow" />
        <button type="button" className="btn quiet sub" disabled={busy} onClick={() => { setSelected(item.code); setStale(false); setNotice(''); setForm(filled(item)); onDirtyChange(false); }}>改成新版本</button>
      </div>
    </article>;
  })}</div> : null;
  return <section aria-label="风格">
    <header className="as-head">
      <div className="grow">
        <h1 className="h-obj">风格</h1>
        <p>一个账号怎么写：结构、节奏、用词和明确不用的写法。写稿页按账号显示正在用的风格。</p>
      </div>
      <div className={styles.actions}><ActionButton variant="outline" disabled={busy || !accounts.data?.length} onClick={() => { setSelected(null); setStale(false); setNotice(''); setForm({ ...blank }); onDirtyChange(false); }}>新增风格</ActionButton></div>
    </header>
    {query.isPending ? <p role="status">正在读取…</p> : null}{query.isError ? <p role="alert">读取失败。<ActionButton variant="quiet" onClick={() => void query.refetch()}>重试</ActionButton></p> : null}
    {accounts.isError ? <p role="alert">账号读取失败，暂时不能新增风格。</p> : null}
    {query.isSuccess && !items.length ? <p className="lib-empty">还没有风格。新增一条，写稿页就会显示这个账号在用的风格。</p> : null}
    {form ? <section aria-label={current ? `修改 ${current.code}` : '新增风格'}>
      <h3>{current ? `${current.code} 第 ${current.revision} 版 → 新版本` : '新增风格'}</h3>
      <label className="lbl">账号<select aria-label="风格所属账号" value={form.accountRef} disabled={!!current || busy} onChange={event => change({ accountRef: event.target.value })}>
        <option value="">选择账号</option>
        {accounts.data?.map(item => <option key={item.accountRef} value={item.accountRef}>{item.accountName}</option>)}
      </select></label>
      <label className="lbl">名称<input className="input" aria-label="风格名称" maxLength={80} placeholder="例如：她本人的原声" value={form.name} onChange={event => change({ name: event.target.value })} /></label>
      <label className="lbl">依据<input className="input" aria-label="风格依据" maxLength={200} placeholder="例如：她自己的稿子 1 篇（越多越准）" value={form.sourceNote} onChange={event => change({ sourceNote: event.target.value })} /></label>
      <label className="lbl">结构<textarea className="input" aria-label="风格结构" rows={3} placeholder="一行一条" value={form.structure} onChange={event => change({ structure: event.target.value })} /></label>
      <label className="lbl">节奏<textarea className="input" aria-label="风格节奏" rows={3} placeholder="一行一条" value={form.rhythm} onChange={event => change({ rhythm: event.target.value })} /></label>
      <label className="lbl">用词<textarea className="input" aria-label="风格用词" rows={3} placeholder="一行一条" value={form.wording} onChange={event => change({ wording: event.target.value })} /></label>
      <label className="lbl">不用<textarea className="input" aria-label="风格不用的写法" rows={3} placeholder="一行一条" value={form.skip} onChange={event => change({ skip: event.target.value })} /></label>
      <div className={styles.actions}>
        <ActionButton variant="primary" busy={busy} disabled={!!blocked || (!!current && stale)} onClick={() => void save()}>{current ? '保存新版本' : '新增风格'}</ActionButton>
        <ActionButton variant="quiet" disabled={busy} onClick={() => { setForm(null); onDirtyChange(false); setNotice(''); }}>取消</ActionButton>
        {blocked ? <span className={styles.hint}>{blocked}</span> : null}
      </div>
      {notice ? <p role="status">{notice}</p> : null}
      {stale && current ? <p>风格已有新版本，当前填写仍保留。<ActionButton variant="quiet" busy={busy} onClick={() => { setBusy(true); void query.refetch().then(result => { if (result.isSuccess) { setStale(false); setNotice('已读取最新版本，请核对后保存。'); } else setNotice('读取失败，请重试。'); }).finally(() => setBusy(false)); }}>读取最新版本</ActionButton></p> : null}
    </section> : itemCards}
  </section>;
}
