import { Link } from 'react-router';
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './structuralTemplatesApi';
import { stepNames, type Step } from '../workbenchModel';
import { assetsErrorText } from './assetsApi';
import { TemplateLabelFields } from './TemplateLabelFields';
import styles from './Assets.module.css';
import { SelectField } from '../../../shared/SelectField';

type ReviewAction = 'approve' | 'disable' | 'enable';
type Pending = { kind: 'review'; code: string; requestId: string; expectedRevision: number; action: ReviewAction }
  | { kind: 'revise'; code: string; requestId: string; expectedRevision: number; steps: Step[]; labels: api.TemplateLabels };

export function StructuralTemplateLibrary({ domainRef, onPendingChange, onDirtyChange = () => {} }: { domainRef: string; onPendingChange: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const client = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [selection, setSelection] = useState<api.Template | null>(null);
  const selected = selection?.code ?? null;
  const [search, setSearch] = useState('');
  const [submittedSearch, setSubmittedSearch] = useState('');
  const searchText = search.trim();
  const validSearch = Array.from(searchText).length <= 60;
  const [name, setName] = useState('');
  const [tags, setTags] = useState('');
  const labels = api.readTemplateLabels(name, tags);
  const validLabels = api.validTemplateLabels(labels);
  const [status, setStatus] = useState('all');
  const [editing, setEditing] = useState<{ baseRevision: number; steps: Step[] } | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [stale, setStale] = useState(false);
  const query = useQuery({ queryKey: ['creation-assets', 'templates', domainRef, ...(submittedSearch ? [submittedSearch] : [])], queryFn: ({ signal }) => api.listTemplates(domainRef, signal, submittedSearch), retry: false, gcTime: 0 });
  const items = query.data ?? [];
  const history = useQuery({ queryKey: ['creation-assets', 'template-history', selected], queryFn: ({ signal }) => api.templateHistory(selected!, signal), enabled: !!selected, retry: false, gcTime: 0 });
  const listed = items.find(item => item.code === selected);
  const historical = history.data?.[0];
  const preferHistory = historical && (!listed || historical.revision > listed.revision
    || (historical.revision === listed.revision && history.dataUpdatedAt > query.dataUpdatedAt));
  const latest = preferHistory ? historical : listed;
  const current = latest ?? selection;
  const canWrite = !!listed || history.isSuccess;
  const filtered = items.filter(item => status === 'all' || (status === 'unavailable' ? !item.sourceAvailable : item.status === status));
  async function run(command: Pending) {
    if (busy) return;
    setPending(command); onPendingChange(true); setBusy(true); setNotice('');
    try {
      if (command.kind === 'review') await api.reviewTemplate(command.code, { requestId: command.requestId, expectedRevision: command.expectedRevision, action: command.action });
      else await api.reviseTemplate(command.code, { requestId: command.requestId, expectedRevision: command.expectedRevision, steps: command.steps, labels: command.labels });
      identities.settle('template-command'); setPending(null); onPendingChange(false); setEditing(null); onDirtyChange(false);
      await Promise.all([client.invalidateQueries({ queryKey: ['creation-assets', 'templates', domainRef] }), client.invalidateQueries({ queryKey: ['creation-assets', 'template-history', command.code] })]);
      if (command.kind === 'revise') setNotice('新版本已保存，等待审核。');
    } catch (error) {
      if (api.rejectedTemplateCommand(error)) { identities.settle('template-command'); setPending(null); onPendingChange(false); setStale(error.failure.kind === 'http' && error.failure.code === 'creation_assets_stale'); setNotice(assetsErrorText(error)); }
      else setNotice('结果尚未确认，请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  function review(action: ReviewAction) {
    if (!current || !canWrite) return;
    void run({ kind: 'review', code: current.code, requestId: identities.keyFor('template-command', `${current.code}:${current.revision}:${action}`), expectedRevision: current.revision, action });
  }
  function revise() {
    if (!current || !canWrite || !editing?.steps.length || stale || !validLabels) return;
    const { baseRevision, steps } = editing;
    void run({ kind: 'revise', code: current.code, requestId: identities.keyFor('template-command', `${current.code}:${baseRevision}:revise`), expectedRevision: baseRevision, steps, labels });
  }
  function editTemplate(template: api.Template) {
    setEditing({ baseRevision: template.revision, steps: [...template.steps] });
    setName(template.labels?.name ?? '');
    setTags(template.labels?.tags.join('，') ?? '');
    onDirtyChange(true);
  }
  async function refreshBase() {
    setBusy(true);
    try {
      const [result, refreshedHistory] = await Promise.all([query.refetch(), history.refetch()]);
      if (!result.isSuccess || !refreshedHistory.isSuccess || !refreshedHistory.data[0]) {
        setNotice('读取失败，请重试。');
        return;
      }
      const latestListRevision = result.data.find(item => item.code === selected)?.revision ?? 0;
      const baseRevision = Math.max(latestListRevision, refreshedHistory.data[0].revision);
      setEditing(value => value && { ...value, baseRevision });
      setStale(false);
      setNotice('已读取最新版本，请核对后保存。');
    } finally { setBusy(false); }
  }
  const state = (item: api.Template) => !item.sourceAvailable ? '来源不可用' : item.status === 'review' ? '待审核' : item.status === 'ok' ? '已通过' : '已停用';
  return <section aria-label="样板库">
    <header className="as-head">
      <div className="grow">
        <h1 className="h-obj">样板库</h1>
        <p>从稿件提炼的结构；样板不保存正文。来源不可用时会自动标记。</p>
      </div>
    </header>
    <div className="lib-tools">
      <form className={styles.templateSearch} onSubmit={event => { event.preventDefault(); if (validSearch && !busy && !pending) setSubmittedSearch(searchText); }}>
        <div className="find"><input aria-label="搜索样板" placeholder="编码、名称或标签" maxLength={120} value={search} onChange={event => setSearch(event.target.value)} /></div>
        <ActionButton variant="outline" type="submit" disabled={busy || !!pending || !validSearch}>搜索</ActionButton>
      </form>
      <label className="lbl">状态<SelectField label="样板状态" value={status} options={[{ value: 'all', label: '全部' }, { value: 'review', label: '待审核' }, { value: 'ok', label: '已通过' }, { value: 'off', label: '已停用' }, { value: 'unavailable', label: '来源不可用' }]} onChange={setStatus} /></label>
    </div>
    {!validSearch ? <p role="status">搜索词最多 60 字。</p> : null}
    {query.isPending ? <p role="status">正在读取…</p> : null}{query.isError ? <p role="alert">读取失败。<ActionButton variant="quiet" onClick={() => void query.refetch()}>重试</ActionButton></p> : null}
    {query.isSuccess && !items.length && !submittedSearch ? <p className="lib-empty">还没有样板，可在写稿页保存结构。</p> : query.isSuccess && !filtered.length ? <p className="lib-empty">没有符合条件的样板。</p> : null}
    {filtered.length || current ? <div className="lib">
      <div className="lib-table">
        <table className="ledger">
          <thead><tr><th scope="col">编码和名称</th><th scope="col">版本</th><th scope="col">状态</th></tr></thead>
          <tbody>{filtered.map(item => {
            const on = item.code === selected;
            return <tr key={item.code} className={on ? 'lib-row on' : 'lib-row'}>
              <td><button type="button" disabled={busy || !!pending} aria-current={on ? 'true' : undefined} onClick={() => { if (item.code === selected) return; if (editing && !window.confirm('样板结构尚未保存，切换会丢弃编辑。确定切换吗？')) return; setSelection(item); setEditing(null); onDirtyChange(false); setStale(false); setNotice(''); }}><b>{item.code}</b>{item.labels?.name ? <> · {item.labels.name}</> : null}</button></td>
              <td>第 {item.revision} 版</td>
              <td>{state(item)}</td>
            </tr>;
          })}</tbody>
        </table>
      </div>
      <aside className="lib-insp" aria-label="样板详情">{current ? <><h3>{current.labels?.name || current.code}</h3><p>{current.labels?.name ? `${current.code} · ` : null}第 {current.revision} 版</p><p>{state(current)}</p>{current.labels?.tags.length ? <p>标签：{current.labels.tags.join('、')}</p> : null}<p>结构：{current.steps.map(step => stepNames[step]).join(' → ')}</p><p className={styles.hint}>来源：创建样板时保存的稿件版本。 <Link to={`/ops/creation?domain=${encodeURIComponent(domainRef)}&work=${encodeURIComponent(current.sourceWorkId)}&view=body`}>查看来源稿件当前版本 ↗</Link></p>
        {editing ? <section aria-label="编辑样板结构"><TemplateLabelFields name={name} tags={tags} onName={setName} onTags={setTags} disabled={busy || !!pending} />{!validLabels ? <p role="status">名称或标签超过长度或数量上限。</p> : null}<p>新版本：{editing.steps.map(step => stepNames[step]).join(' → ') || '尚未选择'}</p><div className={styles.actions}>{Object.entries(stepNames).map(([key, name]) => <ActionButton key={key} variant="quiet" disabled={busy || !!pending || editing.steps.length >= 12} onClick={() => setEditing(value => value && { ...value, steps: [...value.steps, key as Step] })}>＋ {name}</ActionButton>)}</div><div className={styles.actions}><ActionButton variant="quiet" disabled={busy || !!pending || !editing.steps.length} onClick={() => setEditing(value => value && { ...value, steps: value.steps.slice(0, -1) })}>撤销上一步</ActionButton><ActionButton variant="quiet" disabled={busy || !!pending} onClick={() => { setEditing(null); onDirtyChange(false); }}>取消编辑</ActionButton><ActionButton variant="primary" disabled={busy || !!pending || stale || !canWrite || !editing.steps.length || !validLabels || !current.sourceAvailable || current.status === 'off'} onClick={revise}>保存新版本</ActionButton></div></section> : null}
        <div className={styles.actions}>{pending ? <ActionButton variant="outline" busy={busy} onClick={() => void run(pending)}>重试上次操作</ActionButton> : <>{current.status === 'review' ? <ActionButton variant="outline" disabled={!canWrite || !current.sourceAvailable || busy || !!editing} onClick={() => review('approve')}>审核通过</ActionButton> : null}{current.status !== 'off' ? <><ActionButton variant="quiet" disabled={!canWrite || busy || !!editing} onClick={() => review('disable')}>停用</ActionButton><ActionButton variant="quiet" disabled={!canWrite || busy || !current.sourceAvailable || !!editing} onClick={() => editTemplate(current)}>编辑结构</ActionButton></> : <ActionButton variant="quiet" disabled={!canWrite || busy || !!editing} onClick={() => review('enable')}>恢复待审核</ActionButton>}</>}</div>{notice ? <p role="status">{notice}</p> : null}
        {stale ? <p>样板已有新版本，当前编辑仍保留。<ActionButton variant="outline" busy={busy} onClick={() => void refreshBase()}>读取最新版本</ActionButton></p> : null}
        {!canWrite && history.isError ? <p role="alert">读取样板当前版本失败。<ActionButton variant="quiet" onClick={() => void history.refetch()}>重新读取样板</ActionButton></p> : null}
        <details><summary>版本历史</summary>{history.isPending ? <p>正在读取…</p> : history.isError ? <p>读取失败。<ActionButton variant="quiet" onClick={() => void history.refetch()}>重试</ActionButton></p> : <ol>{history.data?.map(item => <li key={item.revision}>第 {item.revision} 版 · {state(item)} · {item.steps.map(step => stepNames[step]).join(' → ')}{item.labels?.name ? <> · {item.labels.name}</> : null}{item.labels?.tags.length ? <> · {item.labels.tags.join('、')}</> : null}</li>)}</ol>}</details>
        </> : <p className={styles.hint}>选择左侧样板查看结构和来源。</p>}</aside>
    </div> : null}
  </section>;
}
