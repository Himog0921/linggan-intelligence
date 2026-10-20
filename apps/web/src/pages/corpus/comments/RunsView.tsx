import { useLayoutEffect, useRef } from 'react';
import { useLocation } from 'react-router';
import { errorText, num, shortRef, studyTime } from './format';
import { dispatchReasonLabel, dispatchStateLabel, runStateLabel } from './labels';
import { parseRunDetail, type StudyRun } from './runDtos';
import { RunRequestsPane, RunSignalsPane, RunTargetsPane } from './RunPanel';
import { RunMethod, RunSummary } from './RunSummary';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { c } from './styles';
import { useRunDirectory } from './useRunDirectory';

export function RunPicker({ domain, selected, nav }: { domain: string; selected: string | null; nav: StudyNavigation }) {
  const query = useRunDirectory(domain), runs = query.data?.pages.flatMap(page => page.runs) ?? [], current = selected || runs[0]?.runRef || '';
  return <div id="study-run-picker" className={c('study-run-picker')}><label>切换研究运行<select id="study-run-select" value={current} disabled={!runs.length} onChange={event => nav.run(event.target.value)}>{runs.length ? <>{selected && !runs.some(run => run.runRef === selected) ? <option value={selected}>指定研究运行 {shortRef(selected)}</option> : null}{runs.map(run => <option key={run.runRef} value={run.runRef}>{studyTime(run.createdAt)} · 选 {run.requestedWorkCount == null ? '—' : run.requestedWorkCount} 篇 / 入包 {Number(run.coveredWorkCount ?? run.workCount)} 篇 / 目标 {Number(run.targetCount)} 条 · {runStateLabel[run.state] || '状态未知'}</option>)}</> : <option value="">尚无研究运行</option>}</select></label></div>;
}
export function RunsView({ domain, nav, initialFeedback }: { domain: string; nav: StudyNavigation; initialFeedback: string }) {
  const heading = useRef<HTMLHeadingElement>(null), focusRequested = useRef(false);
  const directory = useRunDirectory(domain), runs = directory.data?.pages.flatMap(page => page.runs) ?? [];
  const location = useLocation(), params = new URLSearchParams(location.search), runRef = params.get('runRef') || runs[0]?.runRef || null, selected = runs.find(run => run.runRef === runRef) ?? null;
  const panel = ['targets', 'signals', 'requests', 'method'].includes(params.get('panel') || '') ? params.get('panel')! : 'targets';
  const detail = useStudyRead(domain, `runs/${encodeURIComponent(runRef || '')}`, row => { const result = parseRunDetail(row); if (result.runRef !== runRef) throw new Error('运行详情回执与当前选择不一致。'); return result; }, Boolean(runRef));
  useLayoutEffect(() => {
    if (!focusRequested.current || !detail.data || detail.isFetching) return;
    focusRequested.current = false;
    heading.current?.scrollIntoView({ block: 'start', behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    heading.current?.focus({ preventScroll: true });
  }, [runRef, location.key, detail.data, detail.isFetching]);
  function openResult(ref: string) { focusRequested.current = true; nav.run(ref); }
  const props = { domain, runRef: runRef ?? '', nav, focusedTarget: params.get('targetRef'), focusedSignal: params.get('signalRef') };
  function historyRow(run: StudyRun) {
    const dispatchLabel = run.finishedAt && run.dispatchState === 'enabled' ? '' : dispatchStateLabel[run.dispatchState ?? ''] || '派发状态未知', dispatchReason = dispatchReasonLabel[run.dispatchReason ?? ''];
    const origin = run.recoverySourceRunRef ? '补跑未完成' : ({ manual: '手动发起', scheduled: '定时发起' }[run.origin ?? ''] || '来源未记录');
    return <tr key={run.runRef}><td><strong>{studyTime(run.createdAt)}</strong><p>{run.methodName || '方法未记录'} · {origin}</p></td><td><strong>选 {num(run.requestedWorkCount)} 篇 → 入包 {num(run.coveredWorkCount ?? run.workCount)} 篇</strong><p>目标 {num(run.targetCount)} 条评论</p></td><td>成功 {num(run.succeededCount)} · 无信号 {num(run.noSignalCount)}<p>等待语境 {num(run.needsContextCount)} · 失败 {num(run.failedCount)} · 处理中 {num(run.pendingCount)}</p></td><td>待归并 {num(run.pendingResolutionCount)}<p>待比较 {num(run.pendingPairCount)}</p></td><td>{runStateLabel[run.state] || '运行状态未知'}{dispatchLabel || dispatchReason ? <p>{dispatchLabel}{dispatchReason ? `${dispatchLabel ? ' · ' : ''}${dispatchReason}` : ''}</p> : null}</td><td><div className={c('study-run-actions')}><button type="button" className={c('study-link')} data-run-open={run.runRef} onClick={() => openResult(run.runRef)}>{runRef === run.runRef ? '当前查看' : '查看结果'}</button></div></td></tr>;
  }
  return <div className={c('study-results-runs')}><p id="study-run-control-feedback" className={c('study-run-control-feedback')} role="status" aria-live="polite" tabIndex={-1}>{initialFeedback}</p>
    {directory.isPending ? <p className={c('study-empty')}>正在读取评论研究…</p> : directory.isError && !directory.isFetchNextPageError ? <p className={c('study-empty')}>运行列表读取失败：{errorText(directory.error)}</p> : !runs.length ? <p className={c('study-empty')}>尚未创建过研究运行。</p> : <>
      {runRef ? <section className={c('study-run-detail')}><header><h2 ref={heading} tabIndex={-1}>本次研究 · {detail.isPending ? '正在读取方法…' : detail.isError ? '摘要暂未取得' : detail.data?.method?.name || '方法未记录'}</h2><p>{detail.isPending ? '正在读取运行时间…' : detail.isError ? '运行时间暂未取得' : studyTime(detail.data?.createdAt)} · <span className={c('study-run-id')} title={runRef}>运行编号 {shortRef(runRef)}</span></p></header>{selected ? <div className={c('study-link-actions study-current-run-actions')} aria-label="本次运行操作"></div> : null}
        {detail.data ? <RunSummary run={detail.data} /> : detail.isError ? <p className={c('study-restricted')}>本次运行摘要暂时读取失败，以下分页内容仍可查看。</p> : <p className={c('study-empty')}>正在读取评论研究…</p>}
        <nav className={c('study-run-panels')} role="tablist" aria-label="本次运行内容">{([['targets', '目标评论'], ['signals', '研究信号'], ['requests', '调用记录'], ['method', '本次方法']] as const).map(([key, title]) => <button type="button" role="tab" data-run-panel={key} aria-selected={panel === key} key={key} onClick={() => nav.run(runRef, key)}>{title}</button>)}</nav><h3 className={c('study-run-panel-title')}>{{ targets: '目标评论与上下文', signals: '研究结果', requests: '调用记录', method: '本次方法' }[panel]}</h3>
        {panel === 'method' ? detail.data ? <RunMethod run={detail.data} /> : <p className={c('study-restricted')}>本次方法暂时读取失败。</p> : panel === 'signals' ? <RunSignalsPane key={runRef} {...props} /> : panel === 'requests' ? <RunRequestsPane key={runRef} {...props} /> : <RunTargetsPane key={runRef} {...props} />}
      </section> : <p className={c('study-empty')}>没有可显示的历史研究运行。</p>}
      <details className={c('study-run-history')}><summary>查看全部研究运行（已加载 {runs.length} 次）</summary><div className={c('study-review-table-wrap')}><table className={c('study-review-table study-runs-table')}><thead><tr>{['发起时间与方法', '选择与冻结', '评论提取', '归并待办', '运行状态', '操作'].map(title => <th scope="col" key={title}>{title}</th>)}</tr></thead><tbody>{runs.map(historyRow)}</tbody></table></div><p className={c('study-run-page-status')}>已显示 {runs.length} 次运行{directory.hasNextPage ? <> <button type="button" className={c('study-link')} data-run-list-load-more disabled={directory.isFetchingNextPage} onClick={() => { void directory.fetchNextPage(); }}>{directory.isFetchingNextPage ? '读取中…' : directory.isFetchNextPageError ? `读取更早运行失败：${errorText(directory.error)}` : '加载更早运行'}</button></> : '，已到列表末尾'}</p></details>
    </>}
  </div>;
}
