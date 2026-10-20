import { useQuery } from '@tanstack/react-query';
import { useRef, useState, type FormEvent } from 'react';
import { useSearchParams } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { RunLedger } from './RunLedger';
import styles from './Tasks.module.css';
import { CollectionFrame } from '../CollectionFrame';
import { listRuns, errorText } from './tasksApi';
import { RunCreationDialog } from './RunCreationDialog';
import { RunDrawer } from './RunDrawer';

const filters = [['all', '全部'], ['active', '进行中'], ['attention', '需我处理'], ['results', '已有成果'], ['ended', '已结束']] as const;
export function TasksPage() {
  const [params, setParams] = useSearchParams();
  const composing = useRef(false);
  const [search, setSearch] = useState(params.get('q') ?? '');
  const queryContext = `${params.get('domain') ?? ''}:${params.get('q') ?? ''}`;
  const [seen, setSeen] = useState(queryContext);
  if (seen !== queryContext) { setSeen(queryContext); setSearch(params.get('q') ?? ''); }
  const [creating, setCreating] = useState(params.get('once') === '1' || params.get('selected') === '1');
  const query = useQuery({ queryKey: ['collection-runs', params.get('domain'), params.get('filter'), params.get('q'), params.get('page')], queryFn: ({ signal }) => listRuns(params, signal), gcTime: 0 });
  const page = query.data?.page ?? Math.max(1, Number(params.get('page')) || 1);
  const filter = params.get('filter') ?? 'all';
  function update(values: Record<string, string | null>) { const next = new URLSearchParams(params); for (const [key, value] of Object.entries(values)) if (value === null) next.delete(key); else next.set(key, value); setParams(next); }
  function submit(event: FormEvent) { event.preventDefault(); if (composing.current) return; if (search.trim() === (params.get('q') ?? '')) void query.refetch(); else update({ q: search.trim() || null, page: null, run: null }); }
  function closeCreation() { setCreating(false); const returnTo = params.get('returnTo'); if (returnTo && /^\/(collection\/targets|corpus\/evidence)(\?|$)/.test(returnTo) && !/[\\\r\n]/.test(returnTo)) { location.assign(returnTo); return; } update({ once: null, selected: null, target: null }); }
  const domain = params.get('domain');
  const id = params.get('run');
  const drawerDomain = params.get('runDomain') ?? domain;
  return <CollectionFrame section="tasks">
    <section data-collection-runs className={styles.workspace}>
      <div className={styles.band}>
        <form className={styles.searchForm} role="search" onSubmit={submit}>
          <div className={styles.find} data-find><span className={styles.mark} data-find-mark aria-hidden="true">FIND</span>
          <input type="search" name="q" aria-label="搜索采集任务" value={search} onChange={e => setSearch(e.target.value)} placeholder="关键词、目标或任务编号"
            onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
            onKeyDown={event => { if (event.key === 'Enter' && (composing.current || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229)) event.preventDefault(); }} />
          {search && <button type="button" aria-label="清空任务搜索" onClick={() => { setSearch(''); update({ q: null, page: null, run: null }); }}>清空</button>}
          </div><ActionButton type="submit" busy={query.isFetching}>搜索</ActionButton>
        </form>
        <nav className={styles.filters} aria-label="采集任务状态">{filters.map(([key, label]) => <button key={key} type="button" aria-current={filter === key ? 'page' : undefined} onClick={() => update({ filter: key, page: null, run: null })}>{label}</button>)}</nav>
        <ActionButton variant="primary" data-new-run id="new-collection-run" onClick={() => setCreating(true)}>＋ 新建单次采集</ActionButton>
      </div>
      {query.error && query.data && <p role="alert">刷新失败，以下保留上次读取结果。{errorText(query.error)}</p>}
      <RunLedger summary={`当前范围 ${query.data?.total ?? '—'} 次采集 · 第 ${page} 页 · 本页 ${query.data?.items.length ?? '—'} 条`} refresh={<ActionButton variant="quiet" busy={query.isFetching} onClick={() => void query.refetch()}>刷新</ActionButton>} runs={query.data?.items ?? []} message={query.error ? `任务读取失败；不能据此认定没有任务。${errorText(query.error)}` : query.isPending ? '正在读取采集记录…' : '当前筛选没有采集记录。'} onOpen={run => update({ run: run.run_ref, runDomain: run.domain_ref })} />
      <div className={styles.pagination}><span>本页 {query.data?.items.length ?? '—'} 条 · 共 {query.data?.total ?? '—'} 次</span><ActionButton disabled={page <= 1 || query.isFetching} onClick={() => update({ page: String(page - 1) })}>上一页</ActionButton><ActionButton disabled={!query.data || query.data.total <= page * query.data.pageSize || query.isFetching} onClick={() => update({ page: String(page + 1) })}>下一页</ActionButton></div>
    </section>
    {creating && <RunCreationDialog params={params} onClose={closeCreation} onCreated={receipt => { setCreating(false); setParams({ domain: receipt.domainRef, run: receipt.runRef }); void query.refetch(); }} />}
    {id && drawerDomain && drawerDomain !== 'all' && <RunDrawer key={`${drawerDomain}:${id}`} id={id} domain={drawerDomain} onClose={() => update({ run: null, runDomain: null })} />}
  </CollectionFrame>;
}
