import { useEffect, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { DeepResearchLauncher } from './DeepResearchLauncher';
import { isRecord, requestJson } from '../../../shared/api';
import { parseMaterialReadiness } from './evidenceDecode';
import { materialReadinessCopy } from './evidencePresent';
import type { MaterialReadiness } from './evidenceTypes';
import styles from './DossierDocument.module.css';

type Unit = { id: string; unitType: string; statement: string };
type DeepSupport = { excerpt?: string; observation?: string };
type DeepFinding = { id: string; statement: string; evidenceRefs?: string[]; support?: DeepSupport[] };
type DeepMaterial = { id: string; materialKind: string };
type DeepAnalysis = {
  analysisRef: string; researchMode: string | null; baseMode: string | null;
  coreBasis: null | { kind?: string }; question: string; methodVersion: string;
  findings: DeepFinding[]; materials: DeepMaterial[];
};
type Understanding = {
  materialReadiness: MaterialReadiness;
  currentRequest?: { state: string; failureCode: string | null };
  deepAnalyses?: DeepAnalysis[];
  deepEvidence?: Record<string, string[]>;
  analysis: null | { state: string; sourceChanged?: boolean; failureCode: string | null; result: null | {
    card: { analysisRef?: string; core: { summary: { text: string }; knowledgeUnits: Unit[] } };
  } };
};
const GROUPS = [
  ['discussion', '主要议题'], ['audience_context', '适用对象与场景'], ['author_position', '作者立场'],
  ['provision', '具体内容与方法'], ['reader_value', '读者收获'], ['answer_form', '内容形式'],
  ['expression', '表达方式'], ['participation', '参与方式'],
] as const;
function readDeep(entries: unknown): DeepAnalysis[] | undefined {
  if (!Array.isArray(entries)) return;
  const analyses: DeepAnalysis[] = [];
  for (const entry of entries) {
    if (!isRecord(entry) || typeof entry.analysisRef !== 'string' || typeof entry.question !== 'string'
        || typeof entry.methodVersion !== 'string' || !Array.isArray(entry.findings)) return;
    const findings: DeepFinding[] = [];
    for (const finding of entry.findings) {
      if (!isRecord(finding) || typeof finding.id !== 'string' || typeof finding.statement !== 'string') return;
      const refs = finding.evidenceRefs;
      const evidenceRefs = Array.isArray(refs) && refs.every((ref): ref is string => typeof ref === 'string')
        ? refs : undefined;
      const items = finding.support;
      const support = Array.isArray(items)
        ? items.filter((item): item is DeepSupport => isRecord(item)
          && (typeof item.excerpt === 'string' || typeof item.observation === 'string'))
        : undefined;
      if (!evidenceRefs && !support) return;
      findings.push({ id: finding.id, statement: finding.statement, ...(evidenceRefs ? { evidenceRefs } : {}), ...(support ? { support } : {}) });
    }
    const materials = Array.isArray(entry.materials)
      ? entry.materials.filter((material): material is DeepMaterial => isRecord(material)
        && typeof material.id === 'string' && typeof material.materialKind === 'string')
      : [];
    const basis = entry.coreBasis;
    const coreBasis = isRecord(basis) && typeof basis.kind === 'string' ? { kind: basis.kind } : null;
    analyses.push({
      analysisRef: entry.analysisRef, question: entry.question, methodVersion: entry.methodVersion,
      researchMode: typeof entry.researchMode === 'string' ? entry.researchMode : null,
      baseMode: typeof entry.baseMode === 'string' ? entry.baseMode : null,
      coreBasis, findings, materials,
    });
  }
  return analyses;
}
async function read(domain: string, work: string, asOf: string, signal: AbortSignal): Promise<Understanding> {
  const value = await requestJson(`/api/local/content-understanding/latest?${new URLSearchParams({ domainRef: domain, workRef: work, asOf })}`, { signal });
  if (!isRecord(value) || value.contract !== 'content-understanding.read.v1' || value.workRef !== work) throw new Error('理解结果无法读取');
  const materialReadiness = parseMaterialReadiness(value.materialReadiness);
  const currentRequest = isRecord(value.currentRequest) && typeof value.currentRequest.state === 'string' ? { state: value.currentRequest.state, failureCode: typeof value.currentRequest.failureCode === 'string' ? value.currentRequest.failureCode : null } : undefined;
  const analysis = value.analysis;
  if (analysis === null) return { analysis: null, materialReadiness, currentRequest };
  if (!isRecord(analysis) || !['pending', 'running', 'succeeded', 'failed', 'skipped', 'cancelled'].includes(String(analysis.state))) throw new Error('理解状态无法读取');
  let deepAnalyses: DeepAnalysis[] | undefined;
  let deepEvidence: Record<string, string[]> | undefined;
  if (analysis.result !== null) {
    const result = analysis.result;
    if (analysis.state !== 'succeeded' || !isRecord(result) || !isRecord(result.card) || result.card.workRef !== work || result.card.schemaVersion !== 'content-card.v0.2' || !isRecord(result.card.core) || !isRecord(result.card.core.summary) || typeof result.card.core.summary.text !== 'string' || !Array.isArray(result.card.core.knowledgeUnits) || !result.card.core.knowledgeUnits.every(u => isRecord(u) && typeof u.id === 'string' && typeof u.unitType === 'string' && typeof u.statement === 'string')) throw new Error('理解内容无法读取');
    deepAnalyses = readDeep(analysis.deepAnalyses);
    const evidenceItems = Array.isArray(result.card.evidence) ? result.card.evidence : [];
    deepEvidence = Object.fromEntries(evidenceItems.flatMap(item => {
      if (!isRecord(item) || typeof item.id !== 'string') return [];
      const excerpts = [...new Set([item.quote, item.modelQuote, item.observation]
        .filter((text): text is string => typeof text === 'string' && text.trim() !== ''))];
      return excerpts.length ? [[item.id, excerpts]] : [];
    }));
  } else if (analysis.state === 'succeeded') throw new Error('理解内容缺失');
  return { analysis: analysis as Understanding['analysis'], materialReadiness, currentRequest, deepAnalyses, deepEvidence };
}
export function DossierUnderstanding({ domain, work, revision, asOf, active }: { domain: string; work: string; revision: string; asOf: string; active: boolean }) {
  // After a deep request is submitted the page watches for one more published result, for a bounded time.
  const [waiting, setWaiting] = useState<null | { count: number; expired: boolean }>(null);
  const watching = waiting !== null && !waiting.expired;
  useEffect(() => {
    if (!watching) return;
    const timer = window.setTimeout(() => setWaiting(current => current && { ...current, expired: true }), 10 * 60 * 1000);
    return () => window.clearTimeout(timer);
  }, [watching]);
  const query = useQuery({
    queryKey: ['content-understanding', domain, work, revision, asOf],
    queryFn: ({ signal }) => read(domain, work, asOf, signal), enabled: active, retry: false,
    gcTime: 0, refetchOnWindowFocus: false,
    refetchInterval: q => ['pending', 'running'].includes(q.state.data?.currentRequest?.state ?? q.state.data?.analysis?.state ?? '') ? 1500
      : watching && (q.state.data?.deepAnalyses?.length ?? 0) <= waiting.count ? 4000 : false,
  });
  const analysis = query.isError ? null : query.data?.analysis;
  const currentRequest = query.data?.currentRequest;
  const result = analysis?.result;
  const deepAnalyses = query.isError ? undefined : query.data?.deepAnalyses;
  const deepEvidence = query.isError ? undefined : query.data?.deepEvidence;
  const materialReadiness = query.isError ? null : query.data?.materialReadiness;
  const readiness = materialReadinessCopy(materialReadiness);
  const coreRef = result?.card.analysisRef;
  const waitingExpired = waiting?.expired === true && (deepAnalyses?.length ?? 0) <= waiting.count;
  return <section className={`${styles.section} ${styles.understanding}`} aria-label="整篇内容理解">
    <header className={styles.sectionIndex}><h2>作品概览</h2></header>
    <div className={styles.content}>
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
        {coreRef ? <DeepResearchLauncher domain={domain} work={work} coreRef={coreRef}
          onSubmitted={() => { setWaiting({ count: deepAnalyses?.length ?? 0, expired: false }); void query.refetch(); }}/> : null}
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
