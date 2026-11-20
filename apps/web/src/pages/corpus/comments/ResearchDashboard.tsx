import { useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { StudyDialog } from './StudyDialog';
import { CommentResearchSelection } from './CommentResearchSelection';
import { studyTime } from './format';
import { STATES } from './researchModel';
import { cancelRun, commandError, readProblem, readProblems, readRun, readRuns, revokeRedirect, startConsolidation, uncertain, why } from './understandingApi';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';

const state = (value: string) => ({ open: '等待或执行中', completed: '已结束', ...STATES }[value] ?? '状态待核对');
const problemState = (value: string) => value === 'formal' ? '正式' : '候选';

export function ResearchDashboard({ domain, view, nav, runRef, problemRef }: { domain: string; view: 'runs' | 'problems'; nav: StudyNavigation; runRef?: string | null; problemRef?: string | null }) {
  return view === 'runs'
    ? <Runs domain={domain} nav={nav} runRef={runRef} />
    : <Problems domain={domain} nav={nav} problemRef={problemRef} />;
}

function Runs({ domain, nav, runRef }: { domain: string; nav: StudyNavigation; runRef?: string | null }) {
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const command = useRef<{ revision: number; id: string } | null>(null);
  const runs = useQuery({
    queryKey: ['comment-understanding', domain, 'runs'],
    queryFn: ({ signal }) => readRuns(domain, signal),
    retry: false,
    refetchInterval: query => query.state.data?.runs.some(item => item.state === 'open') ||
      query.state.data?.consolidations.some(item => ['pending', 'running'].includes(item.state)) ? 2500 : false,
  });
  async function consolidate() {
    if (!runs.data) return;
    setBusy(true);
    command.current ??= { revision: runs.data.consolidationTaskRevision, id: crypto.randomUUID() };
    try {
      await startConsolidation(domain, command.current.revision, command.current.id);
      setNotice('已提交候选整理，等待执行。'); command.current = null; void runs.refetch();
    } catch (error) {
      setNotice(commandError(error));
      if (!uncertain(error)) command.current = null;
    } finally { setBusy(false); }
  }
  return <section className={c('study-detail-section')} aria-label="评论研究进度">
    <h2>研究记录</h2>
    <div className={c('study-link-actions')}>
      <ActionButton type="button" disabled={runs.isFetching} onClick={() => void runs.refetch()}>刷新研究记录</ActionButton>
      {runs.data && !runs.isError ? <ActionButton type="button" disabled={busy} onClick={() => void consolidate()}>整理候选</ActionButton> : null}
    </div>
    {runs.isPending ? <p>正在读取研究记录…</p> : runs.isError ? <p role="alert">研究记录读取失败，请重试刷新。</p> : <>
      {!runs.data.runs.length ? <p>尚无评论理解运行。</p> : <ul>{runs.data.runs.map(run => <li key={run.batchRef}>
        <button className={c('study-link')} type="button" aria-current={runRef === run.batchRef || undefined} onClick={() => nav.run?.(run.batchRef)}>
          {studyTime(run.createdAt)} · {run.cancelRequested && run.state === 'open' ? '取消处理中' : state(run.state)} · {run.unitCount} 个单元
        </button>
      </li>)}</ul>}
      {runs.data.consolidations.length ? <section aria-label="候选整理记录"><h3>候选整理</h3><ul>{runs.data.consolidations.map(item => <li key={item.batchRef}>
        {studyTime(item.createdAt)} · {item.researchBatchRef ? '自动整理' : '手动整理'} · {state(item.state)}
        {item.redirects !== null ? ` · 合并 ${item.redirects} 项` : ''}{item.failureCode ? ` · ${why(item.failureCode)}` : ''}
      </li>)}</ul></section> : null}
    </>}
    {notice ? <p role="status">{notice}</p> : null}
    {runRef ? <RunDetail key={runRef} domain={domain} batch={runRef} nav={nav} refreshList={() => { void runs.refetch(); }} /> : null}
  </section>;
}

function RunDetail({ domain, batch, nav, refreshList }: { domain: string; batch: string; nav: StudyNavigation; refreshList: () => void }) {
  const [confirm, setConfirm] = useState(false);
  const [retry, setRetry] = useState(false);
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
  if (run.isPending) return <p>正在读取运行回执…</p>;
  if (run.isError) return <p role="alert">运行回执读取失败。 <button type="button" onClick={() => void run.refetch()}>重试读取</button></p>;
  const data = run.data;
  const active = data.units.some(unit => ['pending', 'running'].includes(unit.state));
  return <section className={c('study-problem-card')} aria-label="运行回执">
    <h3>运行回执</h3>
    <p>{data.units.length} 个研究单元 · 已记账 {data.usage.calls} 次调用 · 输入 {data.usage.input} Token · 输出 {data.usage.output} Token</p>
    {data.cancelRequested ? <p>已请求取消。</p> : active ? <ActionButton variant="danger" type="button" onClick={() => setConfirm(true)}>取消运行</ActionButton> : null}
    {notice && !confirm ? <p role="status">{notice}</p> : null}
    <h4>单元执行状态</h4><ol>{data.units.map(unit => <li key={unit.unitRef}>{state(unit.state)} · {unit.targetCount} 条评论{unit.state !== 'cancelled' && unit.failureCode ? ` · ${why(unit.failureCode)}` : ''}</li>)}</ol>
    <h4>评论处理结果</h4><ul>{data.targets.map(target => <li key={`${target.state}:${target.failureCode}`}>{state(target.state)} · {target.count} 条{target.state !== 'cancelled' && target.failureCode ? ` · ${why(target.failureCode)}` : ''}</li>)}</ul>
    {!active && data.retrySelections.length ? <ActionButton type="button" onClick={() => setRetry(!retry)}>重试失败评论</ActionButton> : null}
    {retry && !active && data.retrySelections.length ? <CommentResearchSelection domain={domain} selected={data.retrySelections} nav={nav} /> : null}
    {confirm ? <StudyDialog titleId="comment-run-cancel-title" onClose={() => { if (!busy) setConfirm(false); }} className={c('study-detail-dialog')}>
      <section className={c('study-detail-section')}><h3 id="comment-run-cancel-title">取消这次评论研究？</h3>
        <p>停止尚未开始的单元。已保存的结果保留，在途调用以回执为准。</p>
        <ActionButton type="button" disabled={busy} onClick={() => setConfirm(false)}>继续等待</ActionButton>
        <ActionButton variant="danger" type="button" disabled={busy} onClick={() => void cancel()}>确认取消</ActionButton>
        {notice ? <p role="status">{notice}</p> : null}
      </section>
    </StudyDialog> : null}
  </section>;
}

function Problems({ domain, nav, problemRef }: { domain: string; nav: StudyNavigation; problemRef?: string | null }) {
  const problems = useQuery({ queryKey: ['comment-understanding', domain, 'problems'],
    queryFn: ({ signal }) => readProblems(domain, signal), retry: false });
  return <section className={c('study-detail-section')} aria-label="用户问题">
    <h2>用户问题</h2>
    {problems.isPending ? <p>正在读取用户问题…</p> : problems.isError ? <p role="alert">问题读取失败。 <button type="button" onClick={() => void problems.refetch()}>重试读取</button></p> :
      !problems.data.length ? <p>尚无来源支持的 V2 需求。</p> : problems.data.map(problem => <article className={c('study-problem-card')} key={problem.problemRef}>
        <h3><button className={c('study-link')} type="button" onClick={() => nav.problem?.(problem.problemRef)}>{problem.label}</button></h3>
        <p>{problemState(problem.state)} · {problem.authors} 位作者 · {problem.comments} 条评论 · {problem.works} 个作品</p>
        {problem.lastResearchedAt ? <p>最近研究 {studyTime(problem.lastResearchedAt)}</p> : null}
      </article>)}
    {problemRef ? <ProblemDetail key={problemRef} domain={domain} problem={problemRef} nav={nav}
      refreshed={() => { void problems.refetch(); }} /> : null}
  </section>;
}

function ProblemDetail({ domain, problem, nav, refreshed }: { domain: string; problem: string; nav: StudyNavigation; refreshed: () => void }) {
  const detail = useQuery({ queryKey: ['comment-understanding', domain, 'problem', problem],
    queryFn: ({ signal }) => readProblem(domain, problem, signal), retry: false, gcTime: 0 });
  const [busy, setBusy] = useState(false);
  const [target, setTarget] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const commands = useRef(new Map<string, string>());
  async function revoke(redirect: string) {
    const command = commands.current.get(redirect) ?? crypto.randomUUID();
    commands.current.set(redirect, command); setBusy(true);
    try {
      await revokeRedirect(domain, redirect, command);
      commands.current.delete(redirect); setTarget(null); setNotice('已撤销合并。');
      void detail.refetch(); refreshed();
    } catch (error) {
      if (!uncertain(error)) commands.current.delete(redirect);
      setNotice(commandError(error));
    } finally { setBusy(false); }
  }
  const close = () => { if (!busy) nav.problem?.(null); };
  const data = detail.isError ? null : detail.data;
  return <StudyDialog titleId="comment-problem-title" className={c('study-detail-dialog')} onClose={close}>
    <header className={c('study-dialog-header')}><h2 id="comment-problem-title">{data?.label ?? '需求详情'}</h2>
      <ActionButton type="button" disabled={busy} onClick={close}>关闭</ActionButton></header>
    <div className={c('study-detail-body')}>
      {detail.isPending ? <p>正在读取需求…</p> : detail.isError ? <p role="alert">需求读取失败。 <button type="button" onClick={() => void detail.refetch()}>重试读取</button></p> : null}
      {data ? <>
        <section className={c('study-detail-section')}>
          {data.redirectedFrom ? <p>原需求已归并到此需求。</p> : null}
          <p>{problemState(data.state)} · {data.authors} 位作者 · {data.comments} 条评论 · {data.works} 个作品</p>
          {data.state === 'candidate' ? <p>{data.authors === 1 ? '等待第二位独立作者支持。' : '等待可区分的独立作者支持。'}</p> : null}
          <dl><dt>主体</dt><dd>{data.actor ?? '主体未知'}</dd><dt>目标</dt><dd>{data.goal}</dd><dt>障碍</dt><dd>{data.barrier}</dd>
            {data.context ? <><dt>情境</dt><dd>{data.context}</dd></> : null}</dl>
        </section>
        <section className={c('study-detail-section')}><h3>代表原声</h3>
          {data.evidence.length ? data.evidence.map(entry => <div key={entry.requestRef}>
            <blockquote className={c('study-problem-quote')}>{entry.quote}</blockquote><p>{entry.proposition}</p>
            <button className={c('study-link')} type="button" onClick={() => nav.comment(entry)}>查看原声 · {entry.workTitle}</button>
          </div>) : <p>暂无可读取的代表原声。</p>}
        </section>
        <section className={c('study-detail-section')}><h3>合并历史</h3>
          {data.history.length ? data.history.map(event => <div key={event.redirectRef}>
            <p>{event.from} → {event.to} · {studyTime(event.createdAt)}{event.revokedAt ? ` · ${studyTime(event.revokedAt)} 已撤销` : ''}</p>
            {event.revokedAt ? null : target === event.redirectRef ? <>
              <p>撤销后，两项需求将按各自现有支持重新计数。</p>
              <ActionButton variant="danger" type="button" disabled={busy} onClick={() => void revoke(event.redirectRef)}>确认撤销</ActionButton>
              <ActionButton type="button" disabled={busy} onClick={() => setTarget(null)}>返回</ActionButton>
            </> : <button className={c('study-link')} type="button" disabled={busy} onClick={() => setTarget(event.redirectRef)}>撤销合并</button>}
          </div>) : <p>暂无合并记录。</p>}
        </section>
      </> : null}
      {notice ? <p role="status">{notice}</p> : null}
    </div>
  </StudyDialog>;
}
