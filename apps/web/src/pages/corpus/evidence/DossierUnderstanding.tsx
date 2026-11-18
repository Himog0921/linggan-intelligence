import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { materialReadinessCopy } from './evidencePresent';
import { deepResearchWatchKey, useContentUnderstanding, type DeepResearchWatch } from './contentUnderstandingApi';
import styles from './DossierDocument.module.css';

const GROUPS = [
  ['discussion', '主要议题'], ['audience_context', '适用对象与场景'], ['author_position', '作者立场'],
  ['provision', '具体内容与方法'], ['reader_value', '读者收获'], ['answer_form', '内容形式'],
  ['expression', '表达方式'], ['participation', '参与方式'],
] as const;
export function DossierUnderstanding({ domain, work, revision, asOf, active }: { domain: string; work: string; revision: string; asOf: string; active: boolean }) {
  // The header owns the L2 launch action. This shared cache signal lets the dossier
  // poll for the published finding even when the launch happened from another tab.
  const watchQuery = useQuery<DeepResearchWatch | null>({
    queryKey: deepResearchWatchKey(domain, work),
    queryFn: async () => null,
    enabled: false,
    initialData: null,
    gcTime: 10 * 60 * 1000,
  });
  const waiting = watchQuery.data;
  const [expiredAt, setExpiredAt] = useState<number | null>(null);
  const submittedAt = waiting?.submittedAt;
  useEffect(() => {
    if (submittedAt === undefined) return;
    const remaining = Math.max(0, 10 * 60 * 1000 - (Date.now() - submittedAt));
    const timer = window.setTimeout(() => setExpiredAt(submittedAt), remaining);
    return () => window.clearTimeout(timer);
  }, [submittedAt]);
  const watchExpired = waiting !== null && expiredAt === waiting.submittedAt;
  const watching = waiting !== null && !watchExpired;
  const query = useContentUnderstanding(domain, work, revision, asOf, active);
  const analysis = query.isError ? null : query.data?.analysis;
  const currentRequest = query.data?.currentRequest;
  const result = analysis?.result;
  const deepAnalyses = query.isError ? undefined : query.data?.deepAnalyses;
  const deepEvidence = query.isError ? undefined : query.data?.deepEvidence;
  const materialReadiness = query.isError ? null : query.data?.materialReadiness;
  const readiness = materialReadinessCopy(materialReadiness);
  const waitingExpired = waiting !== null && watchExpired && (deepAnalyses?.length ?? 0) <= waiting.count;
  return <section className={`${styles.section} ${styles.understanding}`} aria-label="整篇内容理解">
    <header className={styles.sectionIndex}><h2>作品概览</h2></header>
    <div className={styles.content}>
      {query.isPlaceholderData ? <p role="status">正在更新理解结果，暂时显示上一份底稿的结果。</p> : null}
      {query.isPending ? <p role="status">正在读取理解结果…</p> : null}
      {query.isError ? <p role="status">理解结果暂时无法读取，原始材料仍可阅读。<ActionButton className={`${styles.control} ${styles.quiet}`} onClick={() => void query.refetch()}>重试读取</ActionButton></p> : null}
      {!query.isPending && readiness && materialReadiness && (!analysis || materialReadiness.gaps.length > 0) ? <>
        <p role="status">{readiness.label}</p>
        {materialReadiness.gaps.length > 0 ? <ul className={styles.list}>{materialReadiness.gaps.map(gap => <li key={`${gap.code}:${gap.slotKey ?? ''}`}>{gap.message}</li>)}</ul> : null}
      </> : null}
      {!query.isPending && !query.isError && !analysis ? <p>这篇尚无当前材料的理解结果，以下保留原始材料。</p> : null}
      {analysis && !result ? <p role="status">{analysis.state === 'pending' ? '等待理解处理。' : analysis.state === 'running' ? '正在理解这篇内容。' : analysis.failureCode === 'ai_dispatch_unknown' ? '本次处理结果尚未确认，以下仍可阅读原始材料。' : analysis.state === 'cancelled' ? '本次理解已取消，原始材料不受影响。' : '本次理解未完成，原始材料不受影响。'}</p> : null}
      {result && currentRequest ? <p role="status">{currentRequest.state === 'pending' ? '新的理解任务正在等待处理。' : currentRequest.state === 'running' ? '正在基于新材料理解。' : '新的理解任务未完成。'}</p> : null}
      {result ? <>
        {analysis?.sourceChanged ? <p role="status">材料已有更新，以下是基于先前材料生成的结果。</p> : null}
        <p className={`${styles.paragraph} ${styles.summary}`}>{result.card.core.summary.text}</p>
        <div className={styles.understandingGroups}>
          {GROUPS.map(([type, title]) => {
            const units = result.card.core.knowledgeUnits.filter(unit => unit.unitType === type);
            return units.length ? <section className={styles.understandingGroup} key={type}>
              <h3>{title}</h3>
              <ul className={styles.list}>{units.map(unit => <li key={unit.id}>{unit.statement}</li>)}</ul>
            </section> : null;
          })}
        </div>
        {watching && (deepAnalyses?.length ?? 0) <= (waiting?.count ?? 0) ? <p role="status">正在等待研究结果…</p> : null}
        {waitingExpired ? <p role="status">提交后一段时间内没有新的研究发现出现。可能仍在处理，也可能没有通过校验；稍后刷新页面再看。</p> : null}
        {deepAnalyses?.length ? <section className={styles.researchFindings}>
          <h3 className={styles.researchTitle}>研究发现</h3>
          {deepAnalyses.map(analysis => <article className={styles.researchAnalysis} key={analysis.analysisRef}>
            <p className={styles.researchQuestion}>{analysis.question}</p>
            <ul className={styles.list}>{analysis.findings.map(finding => <li key={`${analysis.analysisRef}:${finding.id}`}>
              {finding.statement}
              <details className={styles.researchEvidence}><summary>引用依据</summary><ul className={styles.list}>
                {finding.support
                  ? finding.support.map((item, index) => (
                    <li key={`${analysis.analysisRef}:${finding.id}:support:${index}`}>{item.excerpt ?? item.observation}</li>
                  ))
                  : (finding.evidenceRefs ?? []).flatMap(ref => ((deepEvidence ?? {})[ref] ?? []).map(excerpt => (
                    <li key={JSON.stringify([analysis.analysisRef, ref, excerpt])}>{excerpt}</li>
                  )))}
              </ul></details>
            </li>)}</ul>
          </article>)}
        </section> : null}
      </> : null}
    </div>
  </section>;
}
