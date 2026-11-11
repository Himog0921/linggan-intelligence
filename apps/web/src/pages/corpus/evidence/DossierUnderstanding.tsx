import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { isRecord, requestJson } from '../../../shared/api';
import styles from './DossierDocument.module.css';

type Unit = { id: string; unitType: string; statement: string };
type Understanding = {
  analysis: null | { state: string; failureCode: string | null; result: null | {
    card: { schemaVersion: string; core: { summary: { text: string }; knowledgeUnits: Unit[] }; sourceSnapshot: { coverageMode: string } };
    imageContributions: { inputCitationId: string; status: string; summary: string }[];
  } };
};
const GROUPS = [
  ['discussion', '主要议题'], ['audience_context', '适用对象与场景'], ['author_position', '作者立场'],
  ['provision', '具体内容与方法'], ['reader_value', '读者收获'], ['answer_form', '内容形式'],
  ['expression', '表达方式'], ['participation', '参与方式'],
] as const;
async function read(domain: string, work: string, asOf: string, signal: AbortSignal): Promise<Understanding> {
  const value = await requestJson(`/api/local/content-understanding/latest?${new URLSearchParams({ domainRef: domain, workRef: work, asOf })}`, { signal });
  if (!isRecord(value) || value.contract !== 'content-understanding.read.v1' || value.workRef !== work) throw new Error('理解结果无法读取');
  const analysis = value.analysis;
  if (analysis === null) return { analysis: null };
  if (!isRecord(analysis) || !['pending', 'running', 'succeeded', 'failed', 'skipped', 'cancelled'].includes(String(analysis.state))) throw new Error('理解状态无法读取');
  if (analysis.result !== null) {
    const result = analysis.result;
    if (analysis.state !== 'succeeded' || !isRecord(result) || !isRecord(result.card) || result.card.workRef !== work || result.card.schemaVersion !== 'content-card.v0.2' || !isRecord(result.card.core) || !isRecord(result.card.core.summary) || typeof result.card.core.summary.text !== 'string' || !Array.isArray(result.card.core.knowledgeUnits) || !result.card.core.knowledgeUnits.every(u => isRecord(u) && typeof u.id === 'string' && typeof u.unitType === 'string' && typeof u.statement === 'string') || !isRecord(result.card.sourceSnapshot) || typeof result.card.sourceSnapshot.coverageMode !== 'string' || !Array.isArray(result.imageContributions) || !result.imageContributions.every(i => isRecord(i) && typeof i.inputCitationId === 'string' && typeof i.status === 'string' && typeof i.summary === 'string')) throw new Error('理解内容无法读取');
  } else if (analysis.state === 'succeeded') throw new Error('理解内容缺失');
  return value as unknown as Understanding;
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
  return <section className={styles.section} aria-label="整篇内容理解">
    <header className={styles.sectionIndex}><h2>整篇理解</h2></header>
    <div className={styles.content}>
      {query.isPending ? <p role="status">正在读取理解结果…</p> : null}
      {query.isError ? <p role="status">理解结果暂时无法读取，原始材料仍可阅读。<ActionButton className={`${styles.control} ${styles.quiet}`} onClick={() => void query.refetch()}>重试读取</ActionButton></p> : null}
      {!query.isPending && !query.isError && !analysis ? <p>这篇尚无当前材料的理解结果，以下保留原始材料。</p> : null}
      {analysis && !result ? <p role="status">{analysis.state === 'pending' ? '等待理解处理。' : analysis.state === 'running' ? '正在理解这篇内容。' : analysis.failureCode === 'ai_dispatch_unknown' ? '本次处理结果尚未确认，以下仍可阅读原始材料。' : analysis.state === 'cancelled' ? '本次理解已取消，原始材料不受影响。' : '本次理解未完成，原始材料不受影响。'}</p> : null}
      {result ? <>
        <p className={styles.paragraph}>{result.card.core.summary.text}</p>
        {GROUPS.map(([type, title]) => {
          const units = result.card.core.knowledgeUnits.filter(u => u.unitType === type);
          return units.length ? <div key={type}><h3 className={styles.heading}>{title}</h3><ul className={styles.list}>{units.map(unit => <li key={unit.id}>{unit.statement}</li>)}</ul></div> : null;
        })}
        {result.card.sourceSnapshot.coverageMode === 'partial' ? <p className={styles.gap}>本次按已取得的材料理解，材料缺口见下方原始材料。</p> : null}
        {result.imageContributions.filter(image => image.status === 'unclear').map(image => <p className={styles.gap} key={image.inputCitationId}>有图片局部未能看清：{image.summary}</p>)}
      </> : null}
    </div>
  </section>;
}
