import { useQuery, useQueryClient, type UseQueryResult } from '@tanstack/react-query';
import { Fragment, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { SelectField } from '../../../shared/SelectField';
import { CommentAutomation } from './CommentAutomation';
import { num, shortRef, studyTime } from './format';
import { recordsActivity, runStatus, STATES, STATUS_CLASS, STATUS_LABEL, UNIT_STATUS, type RunStatus as Status } from './researchModel';
import { StudyDialog } from './StudyDialog';
import type { StudyNavigation } from './studyApi';
import { cancelRun, commandError, readRun, readRuns, startConsolidation, uncertain, why, type ResearchRun } from './understandingApi';
import { c } from './styles';

type Runs = Awaited<ReturnType<typeof readRuns>>;
const slashTime = (value: string) => studyTime(value).replaceAll('-', '/');
function Status({ status, label }: { status: Status; label?: string }) {
  return <span className={c('records-status', STATUS_CLASS[status])}>{label ?? STATUS_LABEL[status]}</span>;
}
const consolidationState = (value: string) => ({ open: '等待或执行中', completed: '已结束', ...STATES }[value] ?? '状态待核对');

function RunDetail({ domain, batch, nav, refreshList }: { domain: string; batch: string; nav: StudyNavigation; refreshList: () => void }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const command = useRef<string | null>(null);
  const run = useQuery({
    queryKey: ['comment-understanding', domain, 'run', batch],
    queryFn: ({ signal }) => readRun(domain, batch, signal), retry: false,
    refetchInterval: query => query.state.data?.units.some(unit => ['pending', 'running'].includes(unit.state)) ? 2000 : false,
  });
  async function cancel() {
    setBusy(true); command.current ??= crypto.randomUUID();
    try {
      await cancelRun(domain, batch, command.current);
      setNotice('已收到取消请求，最终状态以运行回执为准。');
      command.current = null; setConfirm(false); void run.refetch(); refreshList();
    } catch (error) {
      if (!uncertain(error)) command.current = null;
      setNotice(commandError(error));
    } finally { setBusy(false); }
  }
  const id = `record-details-${batch}`;
  if (run.isPending) return <section className={c('records-details')} id={id}><p className={c('records-unit-muted')}>正在读取研究单元…</p></section>;
  if (run.isError) return <section className={c('records-details')} id={id}><p className={c('records-unit-error')} role="alert">运行回执读取失败。 <button type="button" className={c('text-action')} onClick={() => void run.refetch()}>重试读取</button></p></section>;
  const data = run.data, active = data.units.some(unit => ['pending', 'running'].includes(unit.state));
  return <section className={c('records-details')} id={id} aria-label="运行回执">
    <div className={c('records-details-heading')}><h3>研究单元</h3><span>{num(data.units.length)} 个单元 · 按所属作品组织</span></div>
    <table className={c('records-units')}><colgroup><col style={{ width: '32%' }} /><col style={{ width: '9%' }} /><col style={{ width: '13%' }} /><col style={{ width: '46%' }} /></colgroup>
      <thead><tr><th scope="col">所属作品 / 单元</th><th scope="col" className={c('records-num')}>评论数</th><th scope="col">状态</th><th scope="col">结果 / 原因</th></tr></thead>
      <tbody>{data.units.map(unit => { const status = UNIT_STATUS[unit.state] ?? 'unknown'; return <tr key={unit.unitRef}>
        <td><span className={c('records-unit-work')}>作品 {shortRef(unit.workRef)}</span><span className={c('records-unit-id')}>{shortRef(unit.unitRef)}</span></td>
        <td className={c('records-num')}>{num(unit.targetCount)}</td><td><Status status={status} label={status === 'cancelled' ? STATES[unit.state] : undefined} /></td>
        <td>{status === 'completed' ? <span className={c('records-unit-results')}>结果已写入评论</span>
          : status === 'failed' ? <span className={c('records-unit-error')}>{unit.failureCode ? why(unit.failureCode) : '该单元未完成，可重试失败项。'}</span>
          : status === 'cancelled' ? <span className={c('records-unit-muted')}>{STATES[unit.state] ?? '未执行'}</span>
          : <span className={c('records-unit-muted')}>{status === 'running' ? '正在研究该作品的评论，结果生成后会更新。' : '等待执行，尚未生成本次研究结果。'}</span>}</td>
      </tr>; })}</tbody></table>
    <div className={c('records-usage')}>
      <span>模型调用 <b>{num(data.usage.calls)}</b> 次</span><span>输入 <b>{num(data.usage.input)}</b> Token</span><span>输出 <b>{num(data.usage.output)}</b> Token</span>
      {active ? <span>执行中的用量将在结束后更新</span> : null}
    </div>
    <div className={c('records-usage')}>评论处理结果：{data.targets.map(target => <span key={`${target.state}:${target.failureCode}`}>{STATES[target.state] ?? '状态待核对'} · {num(target.count)} 条{target.state !== 'cancelled' && target.failureCode ? ` · ${why(target.failureCode)}` : ''}</span>)}</div>
    <div className={c('records-detail-actions')}>
      {data.cancelRequested ? <p>已请求取消。</p> : active ? <ActionButton variant="danger" className={c('small')} onClick={() => setConfirm(true)}>取消运行</ActionButton> : null}
      {!active && data.retrySelections.length ? <ActionButton className={c('small')} onClick={() => nav.preview(data.retrySelections)}>重试失败评论</ActionButton> : null}
      {notice && !confirm ? <p role="status">{notice}</p> : null}
    </div>
    {confirm ? <StudyDialog titleId="comment-run-cancel-title" onClose={() => { if (!busy) setConfirm(false); }} className={c('dialog confirm-dialog')}>
      <header className={c('dialog-head')}><h2 id="comment-run-cancel-title">取消这次评论研究？</h2></header>
      <div className={c('dialog-body')}><p>停止后续调用。已保存的结果保留。已发出的调用仍结算用量，取消后不再发布结果。</p>{notice ? <p role="status" className={c('inline-alert')}>{notice}</p> : null}</div>
      <footer className={c('dialog-footer')}><span /><div className={c('actions')}>
        <ActionButton disabled={busy} onClick={() => setConfirm(false)}>继续等待</ActionButton>
        <ActionButton variant="danger" disabled={busy} onClick={() => void cancel()}>确认取消</ActionButton>
      </div></footer>
    </StudyDialog> : null}
  </section>;
}

function Row({ run, open, toggle, retry, retrying }: { run: ResearchRun; open: boolean; toggle: () => void; retry: () => void; retrying: boolean }) {
  const status = runStatus(run), units = run.unitCounts, outcomes = run.outcomes, total = Math.max(1, run.unitCount), ref = shortRef(run.batchRef);
  return <tr className={c('records-row', open && 'is-expanded')} data-record-row={run.batchRef}>
    <td><span className={c('records-batch')} title={run.batchRef}>{ref}</span><time className={c('records-time')} dateTime={run.createdAt}>{slashTime(run.createdAt)}</time></td>
    <td><Status status={status} label={run.cancelRequested && status === 'running' ? '取消处理中' : undefined} /></td>
    <td><div className={c('records-size-main')}><b>{num(run.targetCount)}</b>{run.targetCount != null ? <span>条评论</span> : null}</div>
      <span className={c('records-subline')}>{run.workCount != null ? `${num(run.workCount)} 篇作品 · ` : ''}{num(run.unitCount)} 个单元</span></td>
    <td>{units ? <>
      <div className={c('records-progress-label')}><b>{num(units.completed)} / {num(run.unitCount)} 完成</b><span>{units.pending ? `${units.pending} 待完成` : units.failed ? `${units.failed} 失败` : '全部完成'}</span></div>
      <div className={c('records-progress')} role="img" aria-label={`共 ${run.unitCount} 个单元，已完成 ${units.completed} 个，失败 ${units.failed} 个，待完成 ${units.pending} 个`}>
        <i className={c('complete')} style={{ width: `${units.completed / total * 100}%` }} /><i className={c('failed')} style={{ width: `${units.failed / total * 100}%` }} /></div>
      <span className={c('records-progress-note')}>{units.pending ? '关闭窗口不影响任务' : units.failed ? '仅重试失败的单元' : '结果已写入评论'}</span>
    </> : <span className={c('records-no-result')}>—</span>}</td>
    <td>{outcomes && units?.completed ? <div className={c('records-results')}>
      <div><b>{num(outcomes.signals)}</b><span>有信号</span></div><div><b>{num(outcomes.needsContext)}</b><span>需要语境</span></div><div><b>{num(outcomes.noSignal)}</b><span>无信号</span></div>
    </div> : <span className={c('records-no-result')}>{units && !units.completed ? (units.failed && !units.pending ? '本次未产生结果' : '等待研究结果') : '—'}</span>}</td>
    <td><div className={c('records-row-actions')}>
      <ActionButton className={c('small icon-gap records-expand')} aria-label={`${open ? '收起' : '展开'}批次 ${ref}`} aria-expanded={open} aria-controls={`record-details-${run.batchRef}`} onClick={toggle}>
        {open ? '收起' : '展开'}<span className={c('records-disclosure')} aria-hidden="true">›</span></ActionButton>
      {units && units.failed > 0 && !units.pending ? <ActionButton className={c('small')} disabled={retrying} aria-label={`重试批次 ${ref} 中失败的单元`} onClick={retry}>重试失败</ActionButton> : null}
    </div></td>
  </tr>;
}

export function ResearchRecords({ domain, runs, runRef, nav, close, expand }: {
  domain: string; runs: UseQueryResult<Runs>; runRef: string | null; nav: StudyNavigation; close: () => void; expand: (runRef: string | null) => void;
}) {
  const queryClient = useQueryClient();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | 'running' | 'completed' | 'errors'>('all');
  const [sort, setSort] = useState<'newest' | 'oldest'>('newest');
  const [opened, setOpened] = useState(() => new Set(runRef ? [runRef] : []));
  const [updated, setUpdated] = useState('');
  const [settings, setSettings] = useState(false);
  const [busy, setBusy] = useState(false);
  const [retrying, setRetrying] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const consolidationCommand = useRef<{ revision: number; id: string } | null>(null);
  const [lastRunRef, setLastRunRef] = useState(runRef);
  if (runRef !== lastRunRef) { setLastRunRef(runRef); if (runRef && !opened.has(runRef)) setOpened(old => new Set(old).add(runRef)); }
  const all = runs.data?.runs ?? [];
  const needle = query.trim().toLocaleLowerCase();
  const rows = all.filter(run => (filter === 'all' || (filter === 'errors' ? (run.unitCounts?.failed ?? 0) > 0 || runStatus(run) === 'failed' : filter === 'running' ? runStatus(run) === 'running' : runStatus(run) === 'completed'))
    && (!needle || run.batchRef.toLocaleLowerCase().includes(needle)))
    .sort((a, b) => (sort === 'oldest' ? 1 : -1) * (new Date(a.createdAt).valueOf() - new Date(b.createdAt).valueOf()));
  const activity = recordsActivity(all);
  function toggle(batch: string) {
    setOpened(old => { const next = new Set(old); if (next.has(batch)) next.delete(batch); else next.add(batch); return next; });
    expand(opened.has(batch) ? null : batch);
  }
  async function retry(batch: string) {
    setRetrying(batch); setNotice('');
    try {
      const run = await queryClient.fetchQuery({ queryKey: ['comment-understanding', domain, 'run', batch], staleTime: 0, queryFn: ({ signal }) => readRun(domain, batch, signal) });
      if (run.retrySelections.length) nav.preview(run.retrySelections); else setNotice('失败评论已在处理中，无需重复提交。');
    } catch { setNotice('运行回执读取失败，请重试。'); }
    finally { setRetrying(null); }
  }
  async function consolidate() {
    if (!runs.data) return;
    setBusy(true);
    consolidationCommand.current ??= { revision: runs.data.consolidationTaskRevision, id: crypto.randomUUID() };
    try {
      await startConsolidation(domain, consolidationCommand.current.revision, consolidationCommand.current.id);
      setNotice('已提交候选整理，等待执行。'); consolidationCommand.current = null; void runs.refetch();
    } catch (error) {
      setNotice(commandError(error));
      if (!uncertain(error)) consolidationCommand.current = null;
    } finally { setBusy(false); }
  }
  async function refresh() {
    const hadActive = activity.active > 0;
    await runs.refetch();
    await queryClient.invalidateQueries({ queryKey: ['comment-understanding', domain, 'run'] });
    setUpdated(`刚刚更新 · ${new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, timeZone: 'Asia/Shanghai' }).format(new Date())}`);
    nav.toast(hadActive ? '研究进度已更新。' : '记录已是最新状态。');
  }
  return <StudyDialog id="research-records" className={c('dialog records-dialog')} titleId="research-records-title" onClose={close} dismissBackdrop>
    <div className={c('records-frame')}>
      <header className={c('records-head')}>
        <div className={c('records-heading')}><span className={c('records-eyebrow')} aria-hidden="true">RESEARCH / LOG</span><h2 id="research-records-title">研究记录</h2></div>
        <div className={c('records-head-actions')}>
          <span className={c('records-activity')}>{activity.active || activity.failed ? <>{activity.active ? <><strong>{num(activity.active)}</strong> 项进行中</> : null}{activity.active && activity.failed ? ' · ' : ''}{activity.failed ? <><strong>{num(activity.failed)}</strong> 项有失败</> : null}</> : all.length ? '所有批次已结束' : '暂无研究批次'}</span>
          <ActionButton className={c('small icon-gap')} aria-label="关闭研究记录" onClick={close}>关闭 <span aria-hidden="true">×</span></ActionButton>
        </div>
      </header>
      <div className={c('records-toolbar')}>
        <div className={c('field records-search-field')}><label htmlFor="records-search">搜索记录</label><input id="records-search" type="search" placeholder="批次编号" autoComplete="off" value={query} onChange={event => setQuery(event.target.value)} /></div>
        <div className={c('field records-filter-field')}><span aria-hidden="true">执行状态</span><SelectField label="执行状态" value={filter} onChange={setFilter}
          options={[{ value: 'all', label: '全部状态' }, { value: 'running', label: '进行中' }, { value: 'completed', label: '已完成' }, { value: 'errors', label: '有失败' }]} /></div>
        <div className={c('field records-filter-field')}><span aria-hidden="true">时间顺序</span><SelectField label="时间顺序" value={sort} onChange={setSort}
          options={[{ value: 'newest', label: '最新在前' }, { value: 'oldest', label: '最早在前' }]} /></div>
        <ActionButton disabled={runs.isFetching} onClick={() => void refresh()}>刷新进度</ActionButton>
        <ActionButton onClick={() => setSettings(true)}>自动化设置</ActionButton>
      </div>
      <div className={c('records-scroll')} tabIndex={0} aria-label="研究批次表格">
        <table className={c('records-table')}>
          <caption className="v7-sr-only">研究批次、执行状态、任务规模与研究结果。展开批次可查看研究单元。</caption>
          <colgroup><col className={c('records-col-batch')} /><col className={c('records-col-state')} /><col className={c('records-col-size')} /><col className={c('records-col-progress')} /><col className={c('records-col-result')} /><col className={c('records-col-action')} /></colgroup>
          <thead><tr><th scope="col">批次 / 发起时间</th><th scope="col">执行状态</th><th scope="col">任务规模</th><th scope="col">处理情况</th><th scope="col">结果摘要</th><th scope="col" className={c('records-action-heading')}>操作</th></tr></thead>
          <tbody>
            {runs.isPending ? <tr className={c('records-empty-row')}><td colSpan={6}><strong>正在读取研究记录…</strong></td></tr>
              : runs.isError ? <tr className={c('records-empty-row')}><td colSpan={6}><strong>研究记录读取失败</strong><p>请重试刷新。</p><ActionButton className={c('small')} onClick={() => void runs.refetch()}>重试读取</ActionButton></td></tr>
              : rows.length ? rows.map(run => <Fragment key={run.batchRef}>
                <Row run={run} open={opened.has(run.batchRef)} toggle={() => toggle(run.batchRef)} retry={() => void retry(run.batchRef)} retrying={retrying === run.batchRef} />
                {opened.has(run.batchRef) ? <tr className={c('records-details-row')}><td colSpan={6}><RunDetail domain={domain} batch={run.batchRef} nav={nav} refreshList={() => { void runs.refetch(); }} /></td></tr> : null}
              </Fragment>)
              : <tr className={c('records-empty-row')}><td colSpan={6}><strong>{all.length ? '没有匹配的研究记录' : '还没有研究记录'}</strong>
                <p>{all.length ? '试试其他批次编号或执行状态。' : '从用户评论中选择评论并发起研究后，记录会出现在这里。'}</p>
                {all.length ? <ActionButton className={c('small')} onClick={() => { setQuery(''); setFilter('all'); }}>清除筛选</ActionButton> : null}</td></tr>}
          </tbody>
        </table>
        <div className={c('records-extra')}>
          <CommentAutomation key={domain} domain={domain} nav={nav} showingSettings={settings} closeSettings={() => setSettings(false)} />
          {runs.data ? <section aria-label="候选整理记录"><h3>候选整理</h3>
            {runs.data.consolidations.length ? <ul>{runs.data.consolidations.map(item => <li key={item.batchRef}>
              {studyTime(item.createdAt)} · {item.researchBatchRef ? '自动整理' : '手动整理'} · {consolidationState(item.state)}
              {item.redirects !== null ? ` · 合并 ${item.redirects} 项` : ''}{item.failureCode ? ` · ${why(item.failureCode)}` : ''}
            </li>)}</ul> : <p>暂无候选整理记录。</p>}
            <div className={c('records-detail-actions')}><ActionButton className={c('small')} disabled={busy} onClick={() => void consolidate()}>整理候选</ActionButton></div>
          </section> : null}
          {notice ? <p role="status">{notice}</p> : null}
        </div>
      </div>
      <footer className={c('records-footer')}><span role="status" aria-live="polite">显示 {rows.length} / {all.length} 个批次</span><span>{updated || '按发起时间排序'}</span></footer>
    </div>
  </StudyDialog>;
}
