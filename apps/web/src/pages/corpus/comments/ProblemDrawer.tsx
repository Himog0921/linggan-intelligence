import { useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CrIcon } from './CrIcon';
import { Drawer } from './Drawer';
import { errorText, num, studyTime } from './format';
import { archiveId } from './researchModel';
import type { StudyNavigation } from './studyApi';
import { commandError, readProblem, readProblems, revokeRedirect, uncertain } from './understandingApi';
import { c } from './styles';

export function ProblemDrawer({ domain, problemRef, nav, close, back, showComments }: {
  domain: string; problemRef: string; nav: StudyNavigation; close: () => void; back: (() => void) | null; showComments: (problemRef: string) => void;
}) {
  const detail = useQuery({ queryKey: ['comment-understanding', domain, 'problem', problemRef], queryFn: ({ signal }) => readProblem(domain, problemRef, signal), retry: false, gcTime: 0 });
  // The one-line summary lives on the catalog row (CONTRACT §2); the same cached read as the problem cards.
  const catalog = useQuery({ queryKey: ['comment-understanding', domain, 'problems'], queryFn: ({ signal }) => readProblems(domain, signal), retry: false });
  const summary = catalog.data?.find(row => row.problemRef === problemRef)?.summary ?? null;
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
      void detail.refetch(); void catalog.refetch();
    } catch (error) {
      if (!uncertain(error)) commands.current.delete(redirect);
      setNotice(commandError(error));
    } finally { setBusy(false); }
  }
  const data = detail.isError ? null : detail.data;
  const formal = data?.state === 'formal';
  const shut = () => { if (!busy) close(); };
  return <Drawer id="problem-detail" title="用户问题档案" closeLabel="关闭用户问题档案" close={shut} back={back} note="证据可回到原始评论"
    actions={<><ActionButton disabled={busy} onClick={shut}>关闭</ActionButton>
      {data && data.comments > 0 ? <ActionButton variant="ink" className={c('icon-gap')} onClick={() => showComments(problemRef)}>查看全部支持评论 <CrIcon name="ArrowRight" /></ActionButton> : null}</>}>
    {detail.isPending ? <p className={c('view-status')}>正在读取用户问题…</p> : !data ? <p className={c('view-status')} role="alert">用户问题读取失败：{errorText(detail.error)} <button type="button" className={c('text-action')} onClick={() => void detail.refetch()}>重试读取</button></p> : <>
      <section className={c('drawer-section')}>
        <div className={c('result-title')}><span className={c('pa-archive-id')}>ARCHIVE / {archiveId(problemRef)}</span><span className={c('pa-state', data.state)}>{formal ? '正式问题' : '候选问题'}</span></div>
        <h3 className={c('detail-title')}>{data.label}</h3>
        {summary ? <p className={c('detail-desc')}>{summary}</p> : null}
        <div className={c('detail-metrics')}><div><b>{num(data.authors)}</b><span>独立作者</span></div><div><b>{num(data.comments)}</b><span>评论证据</span></div><div><b>{num(data.works)}</b><span>相关作品</span></div></div>
        <p className={c('threshold-note')}>{formal ? `已有 ${data.authors} 位独立作者支持，达到正式门槛（2 位）。`
          : `目前 ${data.authors} 位独立作者支持；再有 ${Math.max(1, 2 - data.authors)} 位不同作者提到，就成为正式问题。`}</p>
        {data.redirectedFrom ? <p className={c('threshold-note')}>原问题已归并到此问题。</p> : null}
      </section>
      <section className={c('drawer-section')}><h3>问题定义</h3>
        <dl className={c('definition')}><dt>谁</dt><dd>{data.actor ?? '主体未知'}</dd><dt>目标</dt><dd>{data.goal}</dd><dt>阻碍</dt><dd>{data.barrier}</dd>
          {data.context ? <><dt>场景</dt><dd>{data.context}</dd></> : null}</dl>
      </section>
      <section className={c('drawer-section')}><h3>支持这一问题的原声 · {data.evidence.length} 条</h3>
        {data.evidence.length ? data.evidence.map(entry => <article key={entry.requestRef + entry.commentExternalId} className={c('evidence')}>
          <p>{entry.proposition}</p><blockquote>{entry.quote}</blockquote>
          <div className={c('evidence-source')}><span>{entry.workTitle || '未命名作品'}</span>
            <button type="button" className={c('text-action')} aria-label={`查看原声：${entry.quote}`} onClick={() => nav.comment({ workRef: entry.workRef, commentExternalId: entry.commentExternalId }, { related: true, requestRef: entry.requestRef })}>原声 <CrIcon name="ArrowUpRight" /></button></div>
        </article>) : <p className={c('view-status')}>暂无可读取的支持原声。</p>}
      </section>
      <section className={c('drawer-section')}><h3>归并历史</h3>
        {data.history.length ? data.history.map(event => <div key={event.redirectRef} className={c('history-row')}>
          <span>{event.from} → {event.to} · {studyTime(event.createdAt)}{event.revokedAt ? ` · ${studyTime(event.revokedAt)} 已撤销` : ''}</span>
          {event.revokedAt ? null : target === event.redirectRef ? <span className={c('records-detail-actions')}>
            <span>撤销后，两项问题将按各自现有支持重新计数。</span>
            <ActionButton variant="danger" className={c('small')} disabled={busy} onClick={() => void revoke(event.redirectRef)}>确认撤销</ActionButton>
            <ActionButton className={c('small')} disabled={busy} onClick={() => setTarget(null)}>返回</ActionButton>
          </span> : <button type="button" className={c('text-action')} disabled={busy} onClick={() => setTarget(event.redirectRef)}>撤销合并</button>}
        </div>) : <p className={c('muted')}>暂无问题归并记录。</p>}
      </section>
      {notice ? <p role="status" className={c('threshold-note')}>{notice}</p> : null}
    </>}
  </Drawer>;
}
