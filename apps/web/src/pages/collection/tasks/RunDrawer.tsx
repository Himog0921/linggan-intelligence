import { useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { RunDialog } from './RunDialog';
import { ActionButton } from '../../../shared/ActionButton';
import styles from './RunDrawer.module.css';
import { controlRun, definiteRejection, type ControlIntent, type RunAction, type RunRequest } from './runRequests';
import { commentModeLabel, errorText, readRun, readWorks, uuid, type Run } from './tasksApi';

function status(r: Run) { return r.control_intent === 'stopped' ? '已停止，成果保留' : r.control_intent === 'paused' ? '已暂停' : r.ended ? r.needs_attention ? '本轮结束，有缺口' : '本轮已结束' : r.phase === 'blocked' ? '需要处理' : r.domain_status === 'paused' ? '领域已暂停' : r.live_leases !== null && r.live_leases > 0 ? '执行中' : '等待派发'; }
const count = (value: number | null) => value === null ? '—' : value.toLocaleString('zh-CN');
function reason(r: Run) {
  const reasons: Record<string,string> = { surface_ended:'本轮可见作品已查找完毕', maximum_quota:'已达到本次作品上限', accepted_discovery_scope:'本次作品范围已确定', person_selected_exact_set:'按勾选作品采集', discovery_execution_stopped:'作品发现未完成，需要处理' };
  return reasons[r.discovery_reason ?? ''] ?? (r.ended ? '已取得的材料可以继续查看' : r.control_intent === 'stopped' ? '已有材料保留，这次任务不能继续' : '作品范围说明尚未取得');
}
function Progress({ r, control, locked, feedback }: { r: Run; control: (action: RunAction) => void; locked: boolean; feedback: string }) {
  const [confirming, setConfirming] = useState(false);
  const frozen = !!r.scope_frozen_at || r.legacy;
  if (confirming) return <section className={styles.confirm} aria-labelledby="run-stop-confirm-title"><h3 id="run-stop-confirm-title">停止本次采集？</h3><p>停止后不能继续这次任务。已经取得的材料会保留，在途执行仍须核对实际回执。</p><div className={styles.actions}><ActionButton autoFocus onClick={() => setConfirming(false)}>返回进展</ActionButton><ActionButton variant="danger" disabled={locked} onClick={() => { setConfirming(false); control('stop'); }}>确认停止本次采集</ActionButton></div></section>;
  return <section data-run-progress className={styles.progress}>
    <div className={styles.status}><strong>{status(r)}</strong><span>{reason(r)}</span></div>
    <dl className={styles.summary} aria-label="本轮已取得成果">
      <div><dt>范围内作品</dt><dd>{frozen ? count(r.selected_works) : '待确定'}</dd></div>
      <div><dt>可用详情</dt><dd>{count(r.available_details)}</dd></div>
      <div><dt>可检索评论</dt><dd>{count(r.delivered_comments)}</dd></div>
    </dl>
    <dl className={styles.facts}>
      <div><dt>观察领域</dt><dd>{r.domain_name}</dd></div>
      <div><dt>采集方式</dt><dd>{r.legacy ? '历史采集' : commentModeLabel(r, '每篇最多30条评论样本')}</dd></div>
      <div><dt>作品上限</dt><dd>{count(r.work_limit)} 篇</dd></div>
      <div><dt>复用详情</dt><dd>{count(r.reused_details)} 篇</dd></div>
      <div><dt>本轮采回评论</dt><dd>{count(r.received_comments)} 条</dd></div>
      <div><dt>本轮新增评论</dt><dd>{count(r.new_comments)} 条</dd></div>
      <div><dt>累计可检索评论</dt><dd>{count(r.cumulative_comments)} 条</dd></div>
      <div><dt>有缺口的作品</dt><dd>{count(r.gap_works)} 篇</dd></div>
    </dl>
    <section className={styles.phases} aria-labelledby="run-phases-title"><h3 id="run-phases-title">采集进展</h3><ol>
      <li><span>发现作品</span><span>{frozen ? `${count(r.selected_works)} 篇已确定` : '范围待确定'}</span></li>
      <li><span>取得详情</span><span>{r.source_kind === 'creator_preview' ? '浅看只到发现，不取详情' : r.available_details === null ? '尚未取得数量' : `已取得 ${count(r.available_details)} 篇`}</span></li>
      <li><span>评论采集</span><span>{r.comment_mode === 'none' ? '本次不采评论' : r.completed_discussions === null ? '完成范围尚未确认' : `${count(r.completed_discussions)} 篇已结束本轮采集`}</span></li>
    </ol></section>
    {r.stations.length > 0 && <div className={styles.stations}><span>执行工位</span>{r.stations.map(s => <a key={s.stationRef} href={`/collection/runtime?${new URLSearchParams({station:s.stationRef,return_to:`/collection/tasks?domain=${r.domain_ref}&run=${r.run_ref}`})}`}>{s.name}</a>)}</div>}
    {!r.legacy && !r.ended && r.control_intent !== 'stopped' && <div className={styles.actions}><ActionButton disabled={locked} onClick={() => control(r.control_intent === 'paused' ? 'continue' : 'pause')}>{r.control_intent === 'paused' ? '继续' : '暂停'}</ActionButton><ActionButton variant="quiet" disabled={locked} onClick={() => setConfirming(true)}>停止本次</ActionButton></div>}
    {feedback && <p role="alert" className={styles.feedback}>{feedback}</p>}
  </section>;
}
function Results({ r }: { r: Run }) {
  const [page,setPage]=useState(1);
  const query=useQuery({queryKey:['run-works',r.run_ref,r.domain_ref,page],queryFn:({signal})=>readWorks(r.run_ref,r.domain_ref,page,signal),gcTime:0});
  const context=new URLSearchParams({domain:r.domain_ref,collectionRunRef:r.run_ref,returnTo:`/collection/tasks?domain=${r.domain_ref}&run=${r.run_ref}`});
  return <section className={styles.results}><header><h3>作品与缺口</h3><a href={`/corpus/evidence?${context}`}>查看本次作品与评论 ↗</a></header><div data-run-works>{query.error ? <p role="alert">作品读取失败：{errorText(query.error)}</p> : !query.data ? <p role="status">正在读取作品…</p> : <>
    {query.data.items.map(w=>{
      const gap=({access_restricted:'访问受限，已保留可用成果',detail_not_accepted:'详情尚未取得',discussion_partial_or_not_delivered:'评论或回复有缺口',execution_stopped_or_unavailable:'采集中断',no_progress_after_bounded_retries:'多次尝试未取得新评论，待处理'} as Record<string,string>)[w.gapReason??''] ?? (w.gapReason ? '成果情况待核对' : r.legacy ? '已有材料可查看，历史完成情况待核对' : r.source_kind === 'creator_preview' ? '主页卡片，浅看只到发现' : w.commentsComplete ? '本轮范围已采集完成' : '等待采集');
      const link=new URLSearchParams(context);link.set('work',w.publicRef);
      return <article key={w.publicRef} className={styles.work}><a href={`/corpus/evidence?${link}`}>{w.title||'标题未知'}</a><p><span>{w.creator||'作者未知'}</span><span>{gap}</span></p></article>;
    })}
    {!query.data.items.length && <p>作品集合尚未形成。</p>}
    <div className={styles.pagination}><ActionButton disabled={page<=1||query.isFetching} onClick={()=>setPage(page-1)}>上一页作品</ActionButton><span>第 {page} 页 · 共 {query.data.total} 篇</span><ActionButton disabled={query.data.total<=page*20||query.isFetching} onClick={()=>setPage(page+1)}>下一页作品</ActionButton></div>
  </>}</div></section>;
}
interface SavedControl { request: RunRequest; intent: ControlIntent }
function restoredControl(key: string): SavedControl | null {
  try { const v = JSON.parse(sessionStorage.getItem(key) ?? 'null'); if (v && v.request.sent === true && typeof v.request.fingerprint === 'string' && JSON.stringify(v.intent) === v.request.fingerprint) { uuid(v.request.identity); return v; } } catch { /* no pending identity */ } return null;
}
export function RunDrawer({ id, domain, onClose }: { id: string; domain: string; onClose: () => void }) {
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
  return <RunDialog dismissOnBackdrop id="collection-run-drawer" className={`c-run-drawer ${styles.drawer}`} labelledBy="collection-run-title" onClose={onClose}>
    <header className={styles.heading}><h2 id="collection-run-title" title={query.data?.name}>{query.data?.name ?? '采集进展'}</h2><ActionButton variant="quiet" data-close-run-dialog onClick={onClose}>关闭</ActionButton></header>
    <nav className={styles.tabs} aria-label="任务详情">{([['progress','进展'],['results','成果']] as const).map(([value,label])=><button key={value} type="button" data-run-view={value} aria-current={view===value?'page':undefined} onClick={()=>selectView(value)}>{label}</button>)}</nav>
    <div className={`c-run-dialog-body ${styles.body}`} data-run-detail>{query.data ? <><div hidden={view!=='progress'}><Progress r={query.data} control={action=>void command(action)} locked={busy||uncertain} feedback={feedback}/>{uncertain && <ActionButton disabled={busy} onClick={()=>void command(attempt.current?.intent.action??'pause')}>核实原操作结果</ActionButton>}</div>{resultsVisited && <div hidden={view!=='results'}><Results r={query.data}/></div>}</> : <p role={query.error?'alert':'status'}>{query.error?`进展读取失败：${errorText(query.error)}`:'正在读取采集进展…'}</p>}</div>
    <footer className={styles.footer}><span>{query.data?status(query.data):''}</span><ActionButton variant="primary" onClick={()=>selectView('results')}>查看成果</ActionButton></footer>
  </RunDialog>;
}
