import { Link } from 'react-router';
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './structuralTemplatesApi';
import { assetsErrorText } from './assetsApi';
import styles from './Assets.module.css';

type ReviewAction = 'approve' | 'disable' | 'enable';
type Pending = { kind: 'review'; code: string; requestId: string; expectedRevision: number; action: ReviewAction }
  | { kind: 'revise'; code: string; requestId: string; expectedRevision: number; steps: api.Step[] };

export function StructuralTemplateLibrary({ domainRef, onPendingChange, onDirtyChange = () => {} }: { domainRef: string; onPendingChange: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const client = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [selected, setSelected] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState('all');
  const [editing, setEditing] = useState<api.Step[] | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [stale, setStale] = useState(false);
  const query = useQuery({ queryKey: ['creation-assets', 'templates', domainRef], queryFn: ({ signal }) => api.listTemplates(domainRef, signal), retry: false, gcTime: 0 });
  const items = query.data ?? [];
  const current = items.find(item => item.code === selected);
  const history = useQuery({ queryKey: ['creation-assets', 'template-history', selected], queryFn: ({ signal }) => api.templateHistory(selected!, signal), enabled: !!selected, retry: false, gcTime: 0 });
  const filtered = items.filter(item => item.code.toLowerCase().includes(search.trim().toLowerCase()) && (status === 'all' || (status === 'unavailable' ? !item.sourceAvailable : item.status === status)));
  async function run(command: Pending) {
    if (busy) return;
    setPending(command); onPendingChange(true); setBusy(true); setNotice('');
    try {
      if (command.kind === 'review') await api.reviewTemplate(command.code, { requestId: command.requestId, expectedRevision: command.expectedRevision, action: command.action });
      else await api.reviseTemplate(command.code, { requestId: command.requestId, expectedRevision: command.expectedRevision, steps: command.steps });
      identities.settle('template-command'); setPending(null); onPendingChange(false); setEditing(null); onDirtyChange(false);
      await Promise.all([client.invalidateQueries({ queryKey: ['creation-assets', 'templates', domainRef] }), client.invalidateQueries({ queryKey: ['creation-assets', 'template-history', command.code] })]);
      if (command.kind === 'revise') setNotice('新版本已保存，等待审核。');
    } catch (error) {
      if (api.rejectedTemplateCommand(error)) { identities.settle('template-command'); setPending(null); onPendingChange(false); setStale(error.failure.kind === 'http' && error.failure.code === 'creation_assets_stale'); setNotice(assetsErrorText(error)); }
      else setNotice('结果尚未确认，请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  function review(action: ReviewAction) {
    if (!current) return;
    void run({ kind: 'review', code: current.code, requestId: identities.keyFor('template-command', `${current.code}:${current.revision}:${action}`), expectedRevision: current.revision, action });
  }
  function revise() {
    if (!current || !editing?.length || stale) return;
    void run({ kind: 'revise', code: current.code, requestId: identities.keyFor('template-command', `${current.code}:${current.revision}:revise`), expectedRevision: current.revision, steps: editing });
  }
  const state = (item: api.Template) => !item.sourceAvailable ? '来源不可用' : item.status === 'review' ? '待审核' : item.status === 'ok' ? '已通过' : '已停用';
  return <section className={styles.assetBody} aria-label="样板库"><div><h2>样板库</h2><p className={styles.hint}>从稿件提炼的结构；样板不保存正文。来源不可用时会自动标记。</p>
    <label>搜索编码<input aria-label="搜索样板编码" value={search} onChange={event => setSearch(event.target.value)} /></label>
    <label>状态<select aria-label="样板状态" value={status} onChange={event => setStatus(event.target.value)}><option value="all">全部</option><option value="review">待审核</option><option value="ok">已通过</option><option value="off">已停用</option><option value="unavailable">来源不可用</option></select></label>
    {query.isPending ? <p role="status">正在读取…</p> : null}{query.isError ? <p role="alert">读取失败。<ActionButton variant="quiet" onClick={() => void query.refetch()}>重试</ActionButton></p> : null}
    {query.isSuccess && !items.length ? <p>还没有样板，可在写稿页保存结构。</p> : query.isSuccess && !filtered.length ? <p>没有符合条件的样板。</p> : null}
    <ul className={styles.list}>{filtered.map(item => <li key={item.code}><button type="button" disabled={!!pending} aria-current={item.code === selected ? 'true' : undefined} onClick={() => { if (item.code === selected) return; if (editing && !window.confirm('样板结构尚未保存，切换会丢弃编辑。确定切换吗？')) return; setSelected(item.code); setEditing(null); onDirtyChange(false); setStale(false); setNotice(''); }}><b>{item.code}</b><small>第 {item.revision} 版 · {state(item)}</small></button></li>)}</ul></div>
    <div className={styles.detail}>{current ? <><h3>{current.code} · 第 {current.revision} 版</h3><p>{state(current)}</p><p>结构：{current.steps.map(step => api.stepNames[step]).join(' → ')}</p><p className={styles.hint}>来源：创建样板时保存的稿件版本。 <Link to={`/ops/creation?domain=${encodeURIComponent(domainRef)}&work=${encodeURIComponent(current.sourceWorkId)}&view=body`}>查看来源稿件当前版本 ↗</Link></p><p className={styles.hint}>目前保存结构和审核状态，尚不能用样板开稿。</p>
      {editing ? <section aria-label="编辑样板结构"><p>新版本：{editing.map(step => api.stepNames[step]).join(' → ') || '尚未选择'}</p><div className={styles.actions}>{Object.entries(api.stepNames).map(([key, name]) => <ActionButton key={key} variant="quiet" disabled={busy || !!pending || editing.length >= 12} onClick={() => setEditing(value => [...(value ?? []), key as api.Step])}>＋ {name}</ActionButton>)}</div><div className={styles.actions}><ActionButton variant="quiet" disabled={busy || !!pending || !editing.length} onClick={() => setEditing(value => value?.slice(0, -1) ?? null)}>撤销上一步</ActionButton><ActionButton variant="quiet" disabled={busy || !!pending} onClick={() => { setEditing(null); onDirtyChange(false); }}>取消编辑</ActionButton><ActionButton variant="primary" disabled={busy || !!pending || stale || !editing.length || !current.sourceAvailable || current.status === 'off'} onClick={revise}>保存新版本</ActionButton></div></section> : null}
      <div className={styles.actions}>{pending ? <ActionButton variant="outline" busy={busy} onClick={() => void run(pending)}>重试上次操作</ActionButton> : <>{current.status === 'review' ? <ActionButton variant="outline" disabled={!current.sourceAvailable || busy || !!editing} onClick={() => review('approve')}>审核通过</ActionButton> : null}{current.status !== 'off' ? <><ActionButton variant="quiet" disabled={busy || !!editing} onClick={() => review('disable')}>停用</ActionButton><ActionButton variant="quiet" disabled={busy || !current.sourceAvailable || !!editing} onClick={() => { setEditing([...current.steps]); onDirtyChange(true); }}>编辑结构</ActionButton></> : <ActionButton variant="quiet" disabled={busy || !!editing} onClick={() => review('enable')}>恢复待审核</ActionButton>}</>}</div>{notice ? <p role="status">{notice}</p> : null}
      {stale ? <p>样板已有新版本，当前编辑仍保留。<ActionButton variant="outline" busy={busy} onClick={() => { setBusy(true); void query.refetch().then(result => { if (result.isSuccess) { setStale(false); setNotice('已读取最新版本，请核对后保存。'); } else setNotice('读取失败，请重试。'); }).finally(() => setBusy(false)); }}>读取最新版本</ActionButton></p> : null}
      <details><summary>版本历史</summary>{history.isPending ? <p>正在读取…</p> : history.isError ? <p>读取失败。<ActionButton variant="quiet" onClick={() => void history.refetch()}>重试</ActionButton></p> : <ol>{history.data?.map(item => <li key={item.revision}>第 {item.revision} 版 · {state(item)} · {item.steps.map(step => api.stepNames[step]).join(' → ')}</li>)}</ol>}</details>
      </> : <p className={styles.hint}>选择左侧样板查看结构和来源。</p>}</div></section>;
}
