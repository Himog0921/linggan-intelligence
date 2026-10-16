import { useQuery } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { useSearchParams } from 'react-router';
import { CollectionFrame, CollectionReadout } from '../CollectionFrame';
import { listRuns, moment, runStatus, sourceLabel, discoveryLabel, errorText, type Run } from './tasksApi';
import { RunCreationDialog } from './RunCreationDialog';
import { RunDrawer } from './RunDrawer';
import { LegacyTaskDiagnostics } from './LegacyTaskDiagnostics';
import { ExecutionDiagnostics } from './ExecutionDiagnostics';

const filters = [['all', '全部'], ['active', '进行中'], ['attention', '需我处理'], ['results', '已有成果'], ['ended', '已结束']] as const;
function count(value: number | null) { return value === null ? '—' : String(value); }
function RunRow({ run: r, onOpen }: { run: Run; onOpen: (run: Run) => void }) {
  return <tr><td><button type="button" className="c-btn-quiet c-run-title" data-open-run={r.run_ref} data-run-domain={r.domain_ref} title={r.name} onClick={() => onOpen(r)}>{r.name}</button><small>{`${sourceLabel(r.source_kind)} · ${r.domain_name}`}</small></td><td>{r.legacy ? '按原工单范围' : r.comment_mode === 'all_public' ? '尽量采全，含回复' : '每篇最多 30 条总样本'}<small>{`最多 ${count(r.work_limit)} 篇`}</small></td><td>{`${count(r.available_details)} 篇详情`}<small>{`${count(r.delivered_comments)} 条评论 · 复用 ${count(r.reused_details)} 篇详情`}</small></td><td>{runStatus(r)}<small>已固定 {r.scope_frozen_at || r.legacy ? count(r.selected_works) : '—'} 篇 · {discoveryLabel(r.discovery_reason)}</small></td><td><time dateTime={r.created_at} title={r.created_at}>{moment(r.created_at)}</time><small><time dateTime={r.latest_delivery_at ?? r.updated_at} title={r.latest_delivery_at ?? r.updated_at}>{moment(r.latest_delivery_at ?? r.updated_at)}</time></small></td><td><button className="c-btn-secondary" data-open-run={r.run_ref} data-run-domain={r.domain_ref} type="button" onClick={() => onOpen(r)}>查看进展</button></td></tr>;
}
export function TasksPage() {
  const [params, setParams] = useSearchParams();
  const [search, setSearch] = useState(params.get('q') ?? '');
  const [creating, setCreating] = useState(params.get('once') === '1' || params.get('selected') === '1');
  const [diagnostics, setDiagnostics] = useState(false);
  const query = useQuery({ queryKey: ['collection-runs', params.get('domain'), params.get('filter'), params.get('q'), params.get('page')], queryFn: ({ signal }) => listRuns(params, signal), gcTime: 0 });
  const page = query.data?.page ?? Math.max(1, Number(params.get('page')) || 1);
  const filter = params.get('filter') ?? 'all';
  function update(values: Record<string, string | null>) { const next = new URLSearchParams(params); for (const [key, value] of Object.entries(values)) if (value === null) next.delete(key); else next.set(key, value); setParams(next); }
  function submit(event: FormEvent) { event.preventDefault(); update({ q: search, page: null, run: null }); }
  function closeCreation() { setCreating(false); const returnTo = params.get('returnTo'); if (returnTo && /^\/(collection\/targets|corpus\/evidence)(\?|$)/.test(returnTo) && !/[\\\r\n]/.test(returnTo)) { location.assign(returnTo); return; } update({ once: null, selected: null, target: null }); }
  const domain = params.get('domain');
  const id = params.get('run');
  const drawerDomain = params.get('runDomain') ?? domain;
  return <CollectionFrame section="tasks" meta={<CollectionReadout valueFirst value={query.data?.total ?? null} label="采集记录" scope="同一领域、搜索、状态范围；列表与分页一致" />}>
    <section data-collection-runs><form className="c-run-filters" onSubmit={event => event.preventDefault()}>{filters.map(([key, label]) => <button key={key} className="c-run-filter" type="button" aria-current={filter === key ? 'page' : undefined} onClick={() => update({ filter: key, page: null, run: null })}>{label}</button>)}</form><div className="c-run-toolbar"><form className="c-run-tools" onSubmit={submit}><label><span className="c-run-sr">搜索采集任务</span><input type="search" name="q" value={search} onChange={e => setSearch(e.target.value)} placeholder="搜索关键词、目标或任务编号" /></label><button className="c-btn-secondary" type="submit">搜索</button></form><button data-new-run id="new-collection-run" className="c-btn-primary c-run-primary" type="button" onClick={() => setCreating(true)}>＋ 新建单次采集</button></div>
      <div className="c-run-scroll"><table className="c-run-table"><thead><tr>{['任务名称／来源', '方式／范围', '可用成果', '本轮进展／下一步', '发起／最近进展', '操作'].map(label => <th key={label}>{label}</th>)}</tr></thead><tbody>{query.data?.items.map(run => <RunRow key={run.run_ref} run={run} onOpen={run => update({ run: run.run_ref, runDomain: run.domain_ref })} />)}{!query.data?.items.length && <tr><td colSpan={6} className="c-run-empty">{query.error ? `任务读取失败；不能据此认定没有任务。${errorText(query.error)}` : query.isPending ? '正在读取采集记录…' : '当前筛选没有采集记录。'}</td></tr>}</tbody></table></div>
      <form className="c-run-pagination" onSubmit={event => event.preventDefault()}><span>共 {query.data?.total ?? '—'} 次采集 · 第 {page} 页</span><button className="c-btn-quiet" type="button" disabled={page <= 1 || query.isFetching} onClick={() => update({ page: String(page - 1) })}>上一页</button><button className="c-btn-quiet" type="button" disabled={!query.data || query.data.total <= page * query.data.pageSize || query.isFetching} onClick={() => update({ page: String(page + 1) })}>下一页</button></form>
    </section>
    <details className="c-run-diagnostics" onToggle={event => { if (event.currentTarget.open) setDiagnostics(true); }}><summary>历史执行记录与诊断</summary>{diagnostics && <><ExecutionDiagnostics domain={domain === 'all' ? null : domain} /><LegacyTaskDiagnostics /></>}</details>
    {creating && <RunCreationDialog params={params} onClose={closeCreation} onCreated={receipt => { setCreating(false); setParams({ domain: receipt.domainRef, run: receipt.runRef }); void query.refetch(); }} />}
    {id && drawerDomain && drawerDomain !== 'all' && <RunDrawer key={`${drawerDomain}:${id}`} id={id} domain={drawerDomain} onClose={() => update({ run: null, runDomain: null })} />}
  </CollectionFrame>;
}
