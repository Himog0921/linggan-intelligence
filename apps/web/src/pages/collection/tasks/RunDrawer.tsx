import { useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { RunDialog } from './RunDialog';
import { ExecutionDiagnostics } from './ExecutionDiagnostics';
import { controlRun, definiteRejection, type ControlIntent, type RunAction, type RunRequest } from './runRequests';
import { errorText, known, readRun, readWorks, uuid, type Run } from './tasksApi';

function status(r: Run) { return r.control_intent === 'stopped' ? '已停止，成果保留' : r.control_intent === 'paused' ? '人工暂停，交付保留' : r.ended ? r.needs_attention ? '本轮结束，有缺口' : '本轮已结束' : r.phase === 'blocked' ? '需要处理，已有材料保留' : r.domain_status === 'paused' ? '领域暂停，任务保留' : r.live_leases !== null && r.live_leases > 0 ? '执行中，已有成果可查看' : '等待派发'; }
function reason(r: Run) { return ({ surface_ended: '本轮可见作品已发现完毕；保留实际数量', maximum_quota: '已达到本次发现上限', accepted_discovery_scope: '已按本次发现结果固定作品', person_selected_exact_set: '按勾选的作品固定集合', discovery_execution_stopped: '发现未完成，需要处理' } as Record<string, string>)[r.discovery_reason ?? ''] ?? (r.discovery_reason ? '发现原因见执行诊断' : '发现尚未结束'); }
function Progress({ r, control, locked, feedback }: { r: Run; control: (action: RunAction) => void; locked: boolean; feedback: string }) {
  const [diagnostics, setDiagnostics] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const frozen = r.scope_frozen_at || r.legacy;
  if (confirming) return <section className="c-run-stop-confirm" aria-labelledby="run-stop-confirm-title"><h3 id="run-stop-confirm-title">停止本次采集？</h3><p>停止后不能继续这次任务。已经取得的材料会保留，在途执行仍须核对实际回执。</p><div className="c-run-controls"><button autoFocus type="button" className="c-btn-secondary" onClick={() => setConfirming(false)}>返回进展</button><button type="button" className="c-btn-quiet" disabled={locked} onClick={() => { setConfirming(false); control('stop'); }}>确认停止本次采集</button></div></section>;
  return <section data-run-progress><p>本次领域：{r.domain_name} · {r.legacy ? '按原工单范围' : r.comment_mode === 'all_public' ? '尽量采全，含回复' : '每篇最多 30 条总样本'}</p>{r.name.length > 40 && <details className="c-run-complete-name"><summary>查看完整任务名称</summary><p>{r.name}</p></details>}<h3>这次采集，已取得什么</h3><div className="c-run-readouts"><div><span>固定作品</span><b>{frozen ? known(r.selected_works) : '待确定'}</b><small>本次上限 {known(r.work_limit)} 篇</small></div><div><span>可用详情</span><b>{known(r.available_details)}</b><small>其中明确复用 {known(r.reused_details)} 篇</small></div><div><span>本轮可检索评论</span><b>{known(r.delivered_comments)}</b><small>本次新增 {known(r.new_comments)} 条</small></div></div><p className="c-run-status-note"><b>{status(r)}</b><br />{reason(r)}</p><dl><dt>评论交付</dt><dd>本轮采回 {known(r.received_comments)} · 可检索 {known(r.delivered_comments)} · 新增 {known(r.new_comments)} · 共享累计 {known(r.cumulative_comments)}</dd><dt>资源进度</dt><dd>{r.legacy ? '按原执行记录核对' : r.domain_status === 'paused' ? '领域暂停，恢复领域后继续' : `待派发 ${known(r.queued_orders)} 单 · 有效租约 ${known(r.live_leases)}`}</dd></dl><h3>执行进展</h3><ol className="c-run-phases"><li><span>发现作品</span><span>{r.scope_frozen_at ? `${known(r.selected_works)} 篇已固定` : r.legacy ? '按原执行范围' : '等待发现回执'}</span></li><li><span>取得详情与作者</span><span>{frozen ? `${known(r.available_details)} / ${known(r.selected_works)} 篇` : `已取得 ${known(r.available_details)} 篇；范围待确定`}</span></li><li><span>{r.comment_mode === 'all_public' ? '评论与回复遍历' : '评论样本交付'}</span><span>{frozen ? `${known(r.completed_discussions)} / ${known(r.selected_works)} 篇范围已结束` : `已交付 ${known(r.completed_discussions)} 篇；范围待确定`}</span></li><li><span>结束本轮</span><span>{r.ended ? '本轮成果与缺口保留' : '材料取得后即可查看'} · 缺口 {known(r.gap_works)} 篇</span></li></ol><p>{r.stations.map((s, i) => <span key={s.stationRef}>{i > 0 && ' · '}<a href={`/collection/runtime?${new URLSearchParams({ station: s.stationRef, return_to: `/collection/tasks?domain=${r.domain_ref}&run=${r.run_ref}` })}`}>工位：{s.name}</a></span>)}</p><p>交付、可用成果与遍历完成分别核对；等待工位不表示采集完成。</p><div className="c-run-controls">{!r.legacy && !r.ended && r.control_intent !== 'stopped' && <><button type="button" className="c-btn-secondary" disabled={locked} onClick={() => control(r.control_intent === 'paused' ? 'continue' : 'pause')}>{r.control_intent === 'paused' ? '继续' : '暂停'}</button><button type="button" className="c-btn-quiet" disabled={locked} onClick={() => setConfirming(true)}>停止本次</button></>}</div><p role="alert">{feedback}</p><details data-run-execution-diagnostics onToggle={e => { if (e.currentTarget.open) setDiagnostics(true); }}><summary>执行记录与诊断</summary><p>任务编号 {r.run_ref} · 版本 {known(r.revision)} · 发现原因 {r.discovery_reason ?? '尚未取得'}</p>{diagnostics && <ExecutionDiagnostics domain={r.domain_ref} run={r.run_ref} />}</details></section>;
}
function Results({ r }: { r: Run }) {
  const [page, setPage] = useState(1);
  const query = useQuery({ queryKey: ['run-works', r.run_ref, r.domain_ref, page], queryFn: ({ signal }) => readWorks(r.run_ref, r.domain_ref, page, signal), gcTime: 0 });
  const context = new URLSearchParams({ domain: r.domain_ref, collectionRunRef: r.run_ref, returnTo: `/collection/tasks?domain=${r.domain_ref}&run=${r.run_ref}` });
  return <section className="c-run-results"><h3>本次作品与缺口</h3><p><a href={`/corpus/evidence?${context}`}>查看本次作品与评论</a></p><div data-run-works>{query.error ? <p>作品范围读取失败：{errorText(query.error)}</p> : !query.data ? '正在读取…' : <>{query.data.items.map(w => {
    const gap = ({ access_restricted: '访问受限，已保留合格成果', detail_not_accepted: '详情尚未被接纳', discussion_partial_or_not_delivered: '评论或回复交付有缺口', execution_stopped_or_unavailable: '执行已停止或不可用', no_progress_after_bounded_retries: '多段未取得新评论，需要处理' } as Record<string, string>)[w.gapReason ?? ''] ?? w.gapReason ?? (r.legacy ? '历史遍历完成证明未知；已取得材料可查看' : w.commentsComplete ? '本轮范围已交付' : '等待执行与交付');
    const link = new URLSearchParams(context); link.set('work', w.publicRef);
    return <p key={w.publicRef}><b>{w.ordinal ?? '顺位未知'} · {w.title || '标题未知'}</b><br /><span>{w.creator || '作者未知'} · {gap}</span><br /><a href={`/corpus/evidence?${link}`}>查看作品／评论</a></p>;
  })}{!query.data.items.length && <p>作品集合尚未形成。</p>}<button type="button" className="c-btn-quiet" disabled={page <= 1 || query.isFetching} onClick={() => setPage(page - 1)}>上一页作品</button><button type="button" className="c-btn-quiet" disabled={query.data.total <= page * 20 || query.isFetching} onClick={() => setPage(page + 1)}>下一页作品</button></>}</div></section>;
}
interface SavedControl { request: RunRequest; intent: ControlIntent }
function restoredControl(key: string): SavedControl | null {
  try { const v = JSON.parse(sessionStorage.getItem(key) ?? 'null'); if (v && v.request.sent === true && typeof v.request.fingerprint === 'string' && JSON.stringify(v.intent) === v.request.fingerprint) { uuid(v.request.identity); return v; } } catch { /* no pending identity */ } return null;
}
export function RunDrawer({ id, domain, onClose }: { id: string; domain: string; onClose: () => void }) {
  const [expanded, setExpanded] = useState(false);
  const [view, setView] = useState('progress');
  const [resultsVisited, setResultsVisited] = useState(false);
  function selectView(value:string) { setView(value); if(value === 'results') setResultsVisited(true); }
  const key = `collection-run-control:${domain}:${id}`;
  const [initialControl] = useState(() => restoredControl(key));
  const attempt = useRef<SavedControl | null>(initialControl);
  const sending = useRef(false);
  const [uncertain, setUncertain] = useState(!!initialControl);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState('');
  const query = useQuery({ queryKey: ['collection-run', id, domain], queryFn: ({ signal }) => readRun(id, domain, signal), gcTime: 0 });
  async function command(action: RunAction) {
    if (sending.current || !query.data || query.data.revision === null) return;
    const intent = attempt.current?.intent ?? { run: id, domainRef: domain, expectedRevision: query.data.revision, action };
    if (intent.run !== id || intent.domainRef !== domain) { setFeedback('原请求不属于当前范围，不能据此执行。'); return; }
    attempt.current ??= { intent, request: { identity: crypto.randomUUID(), fingerprint: JSON.stringify(intent), sent: false } };
    const wasSent = attempt.current.request.sent; sending.current = true; setBusy(true);
    try {
      const pending = controlRun(attempt.current.request, intent); try { sessionStorage.setItem(key, JSON.stringify(attempt.current)); } catch { /* request retained in memory */ }
      await pending; attempt.current = null; sessionStorage.removeItem(key); setUncertain(false); setFeedback(''); await query.refetch();
    } catch (error) {
      if (!wasSent && definiteRejection(error)) { attempt.current = null; sessionStorage.removeItem(key); setUncertain(false); setFeedback(`控制未生效：${errorText(error)}。已有成果保留，请重新读取后核对。`); }
      else { setUncertain(true); setFeedback(`控制结果未确认：${errorText(error)}。已有成果保留，先核对原回执。`); }
    } finally { sending.current = false; setBusy(false); }
  }
  return <RunDialog dismissOnBackdrop id="collection-run-drawer" className={`c-run-drawer${expanded ? ' c-run-expanded' : ''}`} labelledBy="collection-run-title" onClose={onClose}><header><h2 id="collection-run-title" title={query.data?.name}>{query.data?.name ?? '采集进展'}</h2><div className="c-run-controls"><button type="button" className="c-btn-quiet" aria-label="调整详情宽度" onClick={() => setExpanded(!expanded)}>展开</button><button type="button" className="c-btn-quiet" data-close-run-dialog onClick={onClose}>关闭</button></div></header><nav className="c-run-tabs" aria-label="任务详情">{([['progress', '进展'], ['results', '成果']] as const).map(([v, label]) => <button key={v} type="button" data-run-view={v} aria-current={view === v ? 'page' : 'false'} onClick={() => selectView(v)}>{label}</button>)}</nav><div className="c-run-dialog-body" data-run-detail>{query.data ? <><div hidden={view !== 'progress'}><Progress r={query.data} control={a => void command(a)} locked={busy || uncertain} feedback={feedback} />{uncertain && <button type="button" className="c-btn-secondary" disabled={busy} onClick={() => void command(attempt.current?.intent.action ?? 'pause')}>核实原控制回执</button>}</div>{resultsVisited && <div hidden={view !== 'results'}><Results r={query.data} /></div>}</> : <p>{query.error ? `进展读取失败：${errorText(query.error)}。当前不能据此确认成果或完成。` : '正在读取持久进展…'}</p>}</div><footer><span>{query.data ? status(query.data) : ''}</span><button type="button" className="c-btn-primary c-run-primary" onClick={() => selectView('results')}>查看成果</button></footer></RunDialog>;
}
