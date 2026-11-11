import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { isRecord, requestJson } from '../../../shared/api';
import { parseMaterialReadiness } from './evidenceDecode';
import { materialReadinessCopy } from './evidencePresent';
import type { MaterialReadiness } from './evidenceTypes';
import styles from './DossierDocument.module.css';

type Unit = { id: string; unitType: string; statement: string };
type DeepFinding = { id: string; statement: string; evidenceRefs: string[] };
type Deep = { question: string; findings: DeepFinding[]; evidence: Record<string, string[]> };
type Understanding = {
  materialReadiness: MaterialReadiness;
  deep?: Deep;
  analysis: null | { state: string; failureCode: string | null; result: null | {
    card: { core: { summary: { text: string }; knowledgeUnits: Unit[] } };
  } };
};
const GROUPS = [
  ['discussion', '主要议题'], ['audience_context', '适用对象与场景'], ['author_position', '作者立场'],
  ['provision', '具体内容与方法'], ['reader_value', '读者收获'], ['answer_form', '内容形式'],
  ['expression', '表达方式'], ['participation', '参与方式'],
] as const;
function readDeep(card: Record<string, unknown>, question: unknown): Deep | undefined {
  if (typeof question !== 'string' || !isRecord(card.deep) || !Array.isArray(card.deep.findings)
      || !Array.isArray(card.evidence)) return;
  const findings: DeepFinding[] = [];
  for (const finding of card.deep.findings) {
    if (!isRecord(finding) || typeof finding.id !== 'string' || typeof finding.statement !== 'string'
        || !Array.isArray(finding.evidenceRefs) || !finding.evidenceRefs.every(ref => typeof ref === 'string')) return;
    findings.push({ id: finding.id, statement: finding.statement, evidenceRefs: finding.evidenceRefs });
  }
  const evidence = Object.fromEntries(card.evidence.flatMap(item => {
    if (!isRecord(item) || typeof item.id !== 'string') return [];
    const excerpts = [...new Set([item.quote, item.modelQuote, item.observation]
      .filter((text): text is string => typeof text === 'string' && text.trim() !== ''))];
    return excerpts.length ? [[item.id, excerpts]] : [];
  }));
  return { question, findings, evidence };
}
async function read(domain: string, work: string, asOf: string, signal: AbortSignal): Promise<Understanding> {
  const value = await requestJson(`/api/local/content-understanding/latest?${new URLSearchParams({ domainRef: domain, workRef: work, asOf })}`, { signal });
  if (!isRecord(value) || value.contract !== 'content-understanding.read.v1' || value.workRef !== work) throw new Error('理解结果无法读取');
  const materialReadiness = parseMaterialReadiness(value.materialReadiness);
  const analysis = value.analysis;
  if (analysis === null) return { analysis: null, materialReadiness };
  if (!isRecord(analysis) || !['pending', 'running', 'succeeded', 'failed', 'skipped', 'cancelled'].includes(String(analysis.state))) throw new Error('理解状态无法读取');
  let deep: Deep | undefined;
  if (analysis.result !== null) {
    const result = analysis.result;
    if (analysis.state !== 'succeeded' || !isRecord(result) || !isRecord(result.card) || result.card.workRef !== work || result.card.schemaVersion !== 'content-card.v0.2' || !isRecord(result.card.core) || !isRecord(result.card.core.summary) || typeof result.card.core.summary.text !== 'string' || !Array.isArray(result.card.core.knowledgeUnits) || !result.card.core.knowledgeUnits.every(u => isRecord(u) && typeof u.id === 'string' && typeof u.unitType === 'string' && typeof u.statement === 'string')) throw new Error('理解内容无法读取');
    deep = readDeep(result.card, analysis.deepQuestion);
  } else if (analysis.state === 'succeeded') throw new Error('理解内容缺失');
  return { analysis: analysis as Understanding['analysis'], materialReadiness, deep };
}
export function DossierUnderstanding({ domain, work, revision, asOf, active }: { domain: string; work: string; revision: string; asOf: string; active: boolean }) {
  const query = useQuery({
    queryKey: ['content-understanding', domain, work, revision, asOf],
    queryFn: ({ signal }) => read(domain, work, asOf, signal), enabled: active, retry: false,
    gcTime: 0, refetchOnWindowFocus: false,
    refetchInterval: q => ['pending', 'running'].includes(q.state.data?.analysis?.state ?? '') ? 1500 : false,
  });
  const analysis = query.isError ? null : query.data?.analysis;
  const result = analysis?.result;
  const deep = query.isError ? undefined : query.data?.deep;
  const materialReadiness = query.isError ? null : query.data?.materialReadiness;
  const readiness = materialReadinessCopy(materialReadiness);
  return <section className={styles.section} aria-label="整篇内容理解">
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
      {result ? <>
        <p className={styles.paragraph}>{result.card.core.summary.text}</p>
        {GROUPS.map(([type, title]) => {
          const units = result.card.core.knowledgeUnits.filter(u => u.unitType === type);
          return units.length ? <div key={type}><h3 className={styles.heading}>{title}</h3><ul className={styles.list}>{units.map(unit => <li key={unit.id}>{unit.statement}</li>)}</ul></div> : null;
        })}
        {deep ? <div>
          <h3 className={styles.heading}>研究发现</h3>
          <p className={styles.paragraph}>{deep.question}</p>
          <ul className={styles.list}>{deep.findings.map(finding => <li key={finding.id}>
            {finding.statement}
            <details><summary>引用依据</summary><ul className={styles.list}>
              {finding.evidenceRefs.flatMap(ref => (deep.evidence[ref] ?? []).map(excerpt => (
                <li key={JSON.stringify([ref, excerpt])}>{excerpt}</li>
              )))}
            </ul></details>
          </li>)}</ul>
        </div> : null}
      </> : null}
    </div>
  </section>;
}
