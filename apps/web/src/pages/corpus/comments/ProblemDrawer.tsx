import { useInfiniteQuery } from '@tanstack/react-query';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { errorText, nonBlank, num } from './format';
import { signalKindLabel } from './labels';
import { ProblemMetric } from './ProblemCards';
import { ProblemDrawerShell } from './ProblemDrawerShell';
import { parseProblemDetail, parseProblemEvidence, type ProblemEvidence } from './problemDtos';
import { problemWarning, resultDate } from './problemModel';
import { ProblemContext } from './problemContext';
import { commentIdentity, studyKey, studyRead, useStudyRead } from './studyApi';
import { c } from './styles';

export function ProblemEvidenceRow({ domain, row, index, sourceUnavailable }: { domain: string; row: ProblemEvidence; index: number; sourceUnavailable: () => void }) {
  const readable = row.sourceState === 'known' && Boolean(row.commentText?.trim());
  return <article className={c('sr3-evidence')} data-sr3-evidence-row={index}><header><span className={c('sr3-id')}>{String(index + 1).padStart(2, '0')}</span><strong>{readable ? row.authorDisplayName || '账号名称未记录' : '来源状态未确认'}</strong><span>{resultDate(row.addedAt)} 加入依据</span></header>
    {readable ? <><blockquote>{row.commentText}</blockquote>{row.signals.length ? <details className={c('sr3-related')}><summary>查看与这条原声关联的研究表达</summary>{row.signals.map((signal, i) => <p key={i}><strong>{signalKindLabel[signal.kind] || signal.kind}</strong> · {signal.proposition || '表达未记录'}</p>)}</details> : null}</> : <p>原声当前不可读取，关联研究文字不显示。</p>}
    <ProblemContext domain={domain} commentKey={row.commentKey} sourceUnavailable={sourceUnavailable} />
  </article>;
}
function Criteria({ rows }: { rows: unknown[] | null }) { return rows?.length ? <ul>{rows.map((row, index) => <li key={index}>{typeof row === 'string' ? row : JSON.stringify(row)}</li>)}</ul> : <p>尚未记录具体条件。</p>; }
function ManualCopy({ value }: { value: string }) {
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => { ref.current?.focus(); ref.current?.select(); }, []);
  return <textarea className={c('sr3-copy-text')} ref={ref} readOnly aria-label="可手动复制的问题摘要与来源" value={value} />;
}

export function ProblemDrawer({ domain, problemRef, close, refreshList, onAvailability }: { domain: string; problemRef: string; close: () => void; refreshList: () => void; onAvailability: (ref: string, readable: boolean) => void }) {
  const dialog = useRef<HTMLDialogElement>(null), alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const detail = useStudyRead(domain, `problems/${encodeURIComponent(problemRef)}`, row => {
    const data = parseProblemDetail(row); if (data.problem.problemRef !== problemRef) throw new Error('问题回执不一致'); return data;
  });
  const evidence = useInfiniteQuery({ queryKey: studyKey(domain, `problems/${problemRef}/all-evidence`), initialPageParam: null as string | null,
    queryFn: ({ signal, pageParam }) => { const params = new URLSearchParams({ limit: '20' }); if (pageParam) params.set('cursor', pageParam); return studyRead(domain, `problems/${encodeURIComponent(problemRef)}/evidence?${params}`, row => { const data = parseProblemEvidence(row); if (data.problemRef !== problemRef) throw new Error('问题回执不一致'); return data; }, signal); }, getNextPageParam: page => page.page.nextCursor || undefined, retry: false, gcTime: 0, refetchOnWindowFocus: false,
  });
  const [feedback, setFeedback] = useState('复制只包含可读定义与来源定位，不批量导出原声。'), [copying, setCopying] = useState(false), [manualCopy, setManualCopy] = useState(''), [rechecking, setRechecking] = useState(false);
  const rows = [...new Map((evidence.data?.pages.flatMap(page => page.evidence) ?? []).map(row => [commentIdentity(row.commentKey), row])).values()];
  async function recheck() { setRechecking(true); setManualCopy(''); await Promise.allSettled([detail.refetch(), evidence.refetch()]); if (alive.current) { refreshList(); setRechecking(false); } }
  async function copy() {
    if (!detail.data?.problem.definitionReadable || copying) return;
    setCopying(true); setFeedback('正在重新核对定义可读性…'); setManualCopy('');
    try {
      const fresh = await studyRead(domain, `problems/${encodeURIComponent(problemRef)}`, parseProblemDetail);
      if (!alive.current) return;
      if (fresh.problem.problemRef !== problemRef) throw new Error('读取回执与当前范围不一致，请重新读取。');
      if (!fresh.problem.definitionReadable) { await recheck(); return; }
      const p = fresh.problem, url = new URL(window.location.href); url.search = ''; url.hash = ''; url.searchParams.set('domain', domain); url.searchParams.set('view', 'problems'); url.searchParams.set('problemRef', problemRef);
      const content = `用户问题：${nonBlank(p.title) || '标题未记录'}\n研究定义（非用户原话）：${nonBlank(p.definition) || '未记录'}\n\n关联原声 ${num(p.supportCommentCount)} 条，账号 ${num(p.supportAuthorCount)} 个，作品 ${num(p.supportWorkCount)} 篇。\n范围限制：${problemWarning(p) || '库内材料，不代表市场规模；关联不等于整段定义已验证。'}\n\n来源定位：${url}\n定义版本：${p.revisionRef || '未记录'}\n复制时已重新核对可读性；后续仍以来源权限为准。`;
      try { if (!navigator.clipboard?.writeText) throw new Error('Clipboard unavailable'); await navigator.clipboard.writeText(content); if (alive.current) setFeedback('已复制可读定义与来源定位。'); }
      catch { if (alive.current) { setManualCopy(content); setFeedback('未自动复制，请复制已选中的文本。'); } }
    } catch (error) { if (alive.current) setFeedback(`复制未完成：${errorText(error)}`); }
    finally { if (alive.current) setCopying(false); }
  }
  const p = detail.data?.problem;
  useLayoutEffect(() => { if (p && !detail.isFetching) onAvailability(problemRef, p.definitionReadable); }, [p, detail.isFetching, problemRef, onAvailability]);
  const ready = !detail.isPending && !evidence.isPending && !rechecking;
  const jump = (section: string) => dialog.current?.querySelector(`#sr3-${section}`)?.scrollIntoView({ block: 'start', behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
  return <ProblemDrawerShell dialogRef={dialog} close={close} copy={() => { void copy(); }} copying={!p?.definitionReadable || copying || !ready} feedback={ready ? feedback : '正在核对当前定义与原声…'}>
    {!ready ? <h2 id="sr3-drawer-title" tabIndex={-1}>正在读取问题…</h2> : !p ? <><h2 id="sr3-drawer-title">问题暂时不可读取</h2><div className={c('sr3-message')} role="status"><strong>{errorText(detail.error)}</strong><button className={c('sr3-button')} type="button" onClick={() => { void recheck(); }}>重新读取</button></div></> : <>
      <section className={c('sr3-detail-intro')}><span className={c('sr3-eyebrow')}>{p.problemRef} / 问题研究</span><h2 id="sr3-drawer-title" tabIndex={-1}>{p.definitionReadable ? nonBlank(p.title) || '标题未记录' : '问题定义当前不可读取'}</h2><p className={c('sr3-definition')}>{p.definitionReadable ? nonBlank(p.definition) || '定义未记录' : '当前定义依赖的原声不可读取，不显示衍生结论。'}</p><div className={c('sr3-metrics')}><ProblemMetric value={p.supportCommentCount} label="关联原声" /><ProblemMetric value={p.supportAuthorCount} label="评论账号" /><ProblemMetric value={p.supportWorkCount} label="来源作品" /></div>{problemWarning(p) ? <p className={c('sr3-caution')}>{problemWarning(p)}</p> : null}</section>
      <nav className={c('sr3-jumps')} aria-label="问题详情章节">{([['identity', '问题理解'], ['evidence', '原声依据'], ['history', '形成过程']] as const).map(([section, title]) => <button key={section} type="button" className={c('sr3-button')} data-sr3="jump" data-section={section} onClick={() => jump(section)}>{title}</button>)}</nav>
      <section className={c('sr3-detail-section')} id="sr3-identity"><div className={c('sr3-section-head')}><h3>问题具体是什么</h3><span>研究概括，不是用户原话</span></div>{p.definitionReadable ? <dl className={c('sr3-identity')}>{([['actor', '谁遇到'], ['context', '什么情境'], ['goalOrExpectedState', '想达成'], ['barrierOrUnmetNeed', '卡在哪里']] as const).map(([key, label]) => <div key={key}><dt>{label}</dt><dd>{nonBlank(p.stableIdentity?.[key]) || '当前未记录'}</dd></div>)}</dl> : <p>来源恢复可读后，再查看具体定义。</p>}</section>
      <section className={c('sr3-detail-section')} id="sr3-evidence"><div className={c('sr3-section-head')}><h3>原声与关联表达</h3><span>已读取 <b data-sr3-evidence-count>{rows.length}</b> 条</span></div><p className={c('sr3-caption')}>一条评论只展示一次。关联关系不等于对整段定义的独立验证；需要结合父评论与作品核对。</p><div data-sr3-evidence-list>{evidence.isError && !evidence.data ? <div className={c('sr3-message')} role="status"><strong>原声列表暂时不可读取；不代表没有依据。</strong><button type="button" className={c('sr3-button')} onClick={() => { void recheck(); }}>重新读取</button></div> : rows.length ? rows.map((row, i) => <ProblemEvidenceRow key={commentIdentity(row.commentKey)} domain={domain} row={row} index={i} sourceUnavailable={() => { void recheck(); }} />) : <p className={c('sr3-message')}>当前没有可展示的原声，不代表问题已经解决。</p>}</div><div data-sr3-evidence-pager>{evidence.hasNextPage ? <button type="button" className={c('sr3-button')} data-sr3="more-evidence" disabled={evidence.isFetchingNextPage} onClick={() => { void evidence.fetchNextPage().then(result => { if (alive.current && !result.isError) setFeedback(`已读取 ${new Set(result.data?.pages.flatMap(page => page.evidence).map(row => commentIdentity(row.commentKey))).size} 条原声。`); }); }}>{evidence.isFetchNextPageError ? '读取失败，点击重试' : '继续读取原声'}</button> : null}</div></section>
      <section className={c('sr3-detail-section')} id="sr3-history"><div className={c('sr3-section-head')}><h3>定义与形成过程</h3><span>只呈现已保存的记录</span></div><p className={c('sr3-caption')}>版本新增不自动意味着获得新洞察。以下原文按保存版本排列，不由前端生成差异结论。</p>{detail.data!.revisionHistory.length ? detail.data!.revisionHistory.map((revision, i) => <details className={c('sr3-revision')} key={i}><summary>定义版本 {num(revision.identityVersion)} · {resultDate(revision.createdAt)}{revision.definitionReadable ? '' : ' · 依据不可读取'}</summary>{revision.definitionReadable ? <><h4>{revision.title}</h4><p>{revision.definition}</p></> : <p>这一版本的来源当前不可读取，历史文字不显示。</p>}</details>) : <p>历史版本尚未记录。</p>}
        {p.definitionReadable ? <details className={c('sr3-revision')}><summary>当前纳入与排除边界</summary><h4>纳入</h4><Criteria rows={p.includeCriteria} /><h4>排除</h4><Criteria rows={p.excludeCriteria} /></details> : null}
        <details className={c('sr3-revision')}><summary>来源分布</summary>{!detail.data!.sourceDistribution ? <p>来源分布暂不可读取。</p> : detail.data!.sourceDistribution.works.length ? detail.data!.sourceDistribution.works.map((work, i) => <div className={c('sr3-source-row')} key={i}><span>{work.workTitle || '作品标题未记录'}</span><b>{num(work.commentCount)} 条</b></div>) : <p>暂无可读来源。</p>}</details><p className={c('sr3-caption')}>内容对照尚未接入本页。不据此判断我方没讲过、市场没有回答，也不自动生成运营建议。</p>
      </section>{manualCopy ? <ManualCopy value={manualCopy} /> : null}
    </>}
  </ProblemDrawerShell>;
}
