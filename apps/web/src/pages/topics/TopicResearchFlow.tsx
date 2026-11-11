import { Link } from 'react-router';
import { TopicAutomation } from './TopicAutomation';
import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiError, isRecord, requestJson } from '../../shared/api';
import { decode, str } from '../../shared/decode';
import { c } from './styles';

import { parseTopicRuns as runs, type Run } from './topicResearchModel';
const PHASES = ['讨论提炼', '讨论核验', '归属判断', '归属核验'];
const STATES: Record<string, string> = { pending: '等待', running: '执行中', succeeded: '已通过', failed: '失败', skipped: '未执行', cancelled: '已取消' };
function message(error: unknown) {
  if (error instanceof ApiError && !error.answered) return '回执未知，请用同一请求重试。';
  if (error instanceof ApiError && isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)) return '暂不能开始：' + error.responseBody.reasons.filter(v => typeof v === 'string').join('、') + '。请核对 AI 设置及底稿。';
  return '操作未完成，请刷新并检查材料状态。';
}
const REASONS: Record<string,string> = {
  topic_assignment_not_required:'没有待归属的讨论，此阶段无需执行', topic_dependency_failed:'前置阶段未通过',
  topic_publication_withheld:'来源、许可或控制状态已变化',ai_output_invalid:'模型结果未通过校验',ai_provider_failed:'模型调用失败',
  ai_dispatch_unknown:'发送结果未知，请先核对调用记录',ai_input_limit_before_dispatch:'完整输入超过模型额度，未发送',ai_source_unavailable:'来源当前不可读取',
};
export type ResearchSubmission = { commandRef: string; workRefs: string[]; inFlight: Promise<string> | null };
export type ResearchRequestState = { get: () => ResearchSubmission | null; set: (value: ResearchSubmission | null) => void };
type Props = { requestState?: ResearchRequestState; onCompleted?: () => void; domain: string; works: { workRef: string; title?: string | null }[] };
export function TopicResearchFlow(props: Props) {
  return <DomainResearchFlow key={props.domain} {...props} />;
}
function DomainResearchFlow({ domain, works, onCompleted, requestState }: Props) {
  const [localRequest] = useState<ResearchRequestState>(() => {
    let current: ResearchSubmission | null = null;
    return { get: () => current, set: value => { current = value; } };
  });
  const request = requestState ?? localRequest;
  const [selected, setSelected] = useState<string[]>(() => request.get()?.workRefs ?? []), [records, setRecords] = useState<Run[]>([]), [notice, setNotice] = useState(''), [busy, setBusy] = useState(Boolean(request.get()?.inFlight)), [uncertain, setUncertain] = useState(Boolean(request.get()));
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(0);
  const filtered = works.filter(work => (work.title || '未命名作品').toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));
  const pages = Math.max(1, Math.ceil(filtered.length / 20));
  const currentPage = Math.min(page, pages - 1);
  const visibleWorks = filtered.slice(currentPage * 20, (currentPage + 1) * 20);
  const mounted = useRef(false);
  const readVersion = useRef(0);
  const pendingRuns = useRef(new Set<string>());
  const observedRuns = useRef(new Set<string>());
  const completedCallback = useRef(onCompleted);
  useEffect(() => { completedCallback.current = onCompleted; }, [onCompleted]);
  const [poll, setPoll] = useState(false);
  const [automaticEnabled, setAutomaticEnabled] = useState(false);
  const read = useCallback(async () => {
    const version = ++readVersion.current;
    try {
      const next = runs(await requestJson(`/api/local/topic-research/runs?${new URLSearchParams({ domainRef: domain })}`));
      if (mounted.current && version === readVersion.current) {
        const completed = next.some(run => run.pending === 0 && (!observedRuns.current.has(run.runRef) || pendingRuns.current.has(run.runRef)));
        for (const run of next) observedRuns.current.add(run.runRef);
        pendingRuns.current = new Set(next.filter(run => run.pending > 0).map(run => run.runRef));
        setRecords(next);
        if (completed) completedCallback.current?.();
        setPoll(next.some(run => run.pending > 0));
      }
    } catch (error) {
      if (mounted.current && version === readVersion.current) setNotice(message(error));
    }
  }, [domain]);
  useEffect(() => {
    mounted.current = true;
    queueMicrotask(() => { if (mounted.current) void read(); });
    return () => { mounted.current = false; };
  }, [read]);
  useEffect(() => {
    if (!poll && !automaticEnabled) return;
    const interval = poll ? 3000 : 5000;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      await read();
      if (active) timer = setTimeout(() => void tick(), interval);
    }
    timer = setTimeout(() => void tick(), interval);
    return () => { active = false; clearTimeout(timer); };
  }, [poll, automaticEnabled, read]);
  async function refresh() { setBusy(true); await read(); if (mounted.current) setBusy(false); }
  const followSubmission = useCallback(async (pending: Promise<string>) => {
    try {
      const runRef = await pending;
      if (!mounted.current) return;
      pendingRuns.current.add(runRef);
      setUncertain(false); setNotice('已提交'); setPoll(true); await read();
    } catch (error) {
      if (mounted.current) { setNotice(message(error)); setUncertain(Boolean(request.get())); }
    } finally { if (mounted.current) setBusy(false); }
  }, [read, request]);
  useEffect(() => {
    const pending = request.get()?.inFlight;
    if (pending) queueMicrotask(() => { if (mounted.current) void followSubmission(pending); });
  }, [followSubmission, request]);
  async function start() {
    if (!selected.length) return;
    setBusy(true);
    const submission = request.get() ?? { commandRef: crypto.randomUUID(), workRefs: [...selected], inFlight: null };
    request.set(submission);
    if (!submission.inFlight) {
      submission.inFlight = requestJson('/api/local/topic-research', { method: 'POST', body: { commandRef: submission.commandRef, domainRef: domain, workRefs: submission.workRefs } })
        .then(value => {
          const runRef = decode(value, root => str(root, 'runRef'));
          if (request.get() === submission) request.set(null);
          return runRef;
        }).catch((error: unknown) => {
          submission.inFlight = null;
          if (error instanceof ApiError && error.answered && request.get() === submission) request.set(null);
          throw error;
        });
    }
    await followSubmission(submission.inFlight);
  }
  async function cancel(runRef: string) { setBusy(true); try { await requestJson('/api/local/topic-research/cancel', { method: 'POST', body: { commandRef: crypto.randomUUID(), domainRef: domain, runRef } }); setNotice('已请求取消'); await read(); } catch { setNotice('取消回执未知，请刷新进度核对。'); } finally { setBusy(false); } }
  return <section className={c('lgi-tm-section')} aria-label="主题研究流程"><h3>主题研究</h3><TopicAutomation domain={domain} onEnabledChange={setAutomaticEnabled} /><Link to="/settings/models?view=advanced&area=work&task=topic_research">去 AI 设置</Link>
    <div className={c('lgi-tm-row')}><input type="search" aria-label="搜索研究作品" placeholder="搜索作品" value={search} onChange={event => { setSearch(event.target.value); setPage(0); }} /><span>已选 {selected.length} / 10</span></div>
    <div className={c('lgi-tm-row')}><button className={c('lgi-tm-button')} type="button" disabled={busy || selected.length === 0} onClick={() => void start()}>{uncertain ? '重试同一请求' : '开始研究'}</button><button className={c('lgi-tm-button')} type="button" disabled={busy} onClick={() => void refresh()}>刷新进度</button></div>
    <fieldset className={c('lgi-tm-research-works')} disabled={busy || uncertain}><legend>本次作品</legend>{visibleWorks.map(work => <label className={c('lgi-tm-check-label')} key={work.workRef}><input type="checkbox" checked={selected.includes(work.workRef)} disabled={selected.length >= 10 && !selected.includes(work.workRef)} onChange={event => setSelected(old => event.target.checked ? [...old, work.workRef] : old.filter(ref => ref !== work.workRef))} /><span>{work.title || '未命名作品'}</span></label>)}{filtered.length === 0 ? <span>没有匹配作品</span> : null}</fieldset>
    {pages > 1 ? <nav className={c('lgi-tm-row')} aria-label="研究作品分页"><button className={c('lgi-tm-button')} type="button" disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)}>上一页</button><span>{currentPage + 1} / {pages}</span><button className={c('lgi-tm-button')} type="button" disabled={currentPage + 1 === pages} onClick={() => setPage(currentPage + 1)}>下一页</button></nav> : null}
    {notice ? <p role="status">{notice}</p> : null}{records.map(run => <article key={run.runRef}><h4>{run.pending ? '研究进行中' : run.failed ? '研究存在未完成阶段' : run.stages.some(stage => stage.state === 'cancelled') ? '研究已取消' : '阶段处理完成'} · {run.windows} 个窗口</h4>{run.pending ? <button className={c('lgi-tm-button')} type="button" disabled={busy} onClick={() => void cancel(run.runRef)}>取消余下阶段</button> : null}<ul>{run.stages.map(stage => <li key={stage.window + stage.phase}>窗口 {Array.from(new Set(run.stages.map(item => item.window))).indexOf(stage.window) + 1} · {PHASES[stage.phase] ?? '阶段未知'}：{STATES[stage.state] ?? stage.state}{stage.failure ? ` · ${REASONS[stage.failure] ?? '请查看 AI 调用记录'}` : stage.wait ? ' · 等待运行条件满足' : ''}</li>)}</ul></article>)}
  </section>;
}
