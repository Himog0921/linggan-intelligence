import type { Dossier } from './dossierTypes';
import type { AnalysisEvidence, AnalysisRead, AnalysisResult, AnalysisStatement } from './noteAnalysisTypes';
import { momentText } from './evidencePresent';
import page from './EvidencePage.module.css';
import styles from './NoteAnalysis.module.css';
function Evidence({ evidence, dossier }: { evidence: AnalysisEvidence; dossier: Dossier | undefined }) {
  const unit = evidence.kind === 'material' ? dossier?.materials.find(material => material.sourceRef === evidence.sourceRef && material.sourceVersion === evidence.sourceVersion) : undefined;
  const block = unit?.blocks.find(candidate => candidate.ref === evidence.citationId);
  function locate() {
    const element = [...document.querySelectorAll<HTMLElement>('[data-block-ref]')].find(candidate => candidate.dataset.blockRef === block?.ref);
    element?.scrollIntoView({ block: 'center' });
    if (element) { element.tabIndex = -1; element.focus({ preventScroll: true }); }
  }
  function locateComments() {
    const element = document.querySelector<HTMLElement>('[data-note-analysis-comments]');
    element?.scrollIntoView({ block: 'start' }); element?.focus({ preventScroll: true });
  }
  return <div><blockquote>{evidence.quote}</blockquote><p className={styles.meta}>{evidence.kind === 'comment' ? '评论原声' : '笔记材料'} · 位置 {evidence.start}–{evidence.end}</p>
    {block ? <button type="button" className={page.textButton} onClick={locate}>回到原文</button> : <p className={styles.meta}>冻结引句；尚未核对到当前阅读中的同版本原文。</p>}
    {evidence.kind === 'comment' ? <button type="button" className={page.textButton} onClick={locateComments}>查看当前评论区</button> : null}
    <details><summary>来源位置</summary><p className={styles.meta}>{evidence.sourceRef} · 版本 {evidence.sourceVersion} · {evidence.citationId}</p></details></div>;
}
function Basis({ refs, result, dossier }: { refs: string[]; result: AnalysisResult; dossier: Dossier | undefined }) {
  if (!refs.length) return null;
  return <details><summary>依据</summary>{refs.map(ref => { const evidence = result.evidence.find(item => item.id === ref); return evidence ? <Evidence key={ref} evidence={evidence} dossier={dossier} /> : null; })}</details>;
}
function Statement({ value, result, dossier }: { value: AnalysisStatement | null; result: AnalysisResult; dossier: Dossier | undefined }) {
  if (!value) return null;
  return <div><p>{value.kind === 'author_claim' ? <span className={styles.meta}>作者主张 · </span> : value.kind === 'interpretation' ? <span className={styles.meta}>分析推测 · </span> : null}{value.text}</p><Basis refs={value.evidenceRefs} result={result} dossier={dossier} /></div>;
}
export function NoteAnalysisResult({ read, dossier }: { read: AnalysisRead; dossier?: Dossier }) {
  const result = read.result;
  if (!result) return null;
  const statements = [...Object.values(result.understanding).filter((value): value is AnalysisStatement => value !== null), ...result.features, ...result.feedback, ...result.explanations];
  const columnLabels: Record<string, string> = { title: '标题', body: '正文', ocr: '图片文字', asr: '音视频文字' };
  const basisText = (ids: string[]) => ids.map(id => statements.find(item => item.id === id)?.text).filter(Boolean).join('；');
  return <div className={styles.panel}>
    <p className={styles.meta}>{momentText(read.versions.createdAt)} · {read.scope === 'note' ? '笔记文字' : '笔记与已有评论'} · {read.freshness === 'source_changed' ? '材料已更新' : read.freshness === 'not_checked' ? '材料更新情况暂未核对' : '与当前材料一致'}</p>
    <p>{result.summary}</p><Basis refs={[...new Set(statements.filter(item => result.summaryBasisIds.includes(item.id)).flatMap(item => item.evidenceRefs))]} result={result} dossier={dossier} />
    <div className={styles.group}><h4>讲什么、给什么</h4>{[result.understanding.discussion, result.understanding.audience, result.understanding.situation, result.understanding.answer, result.understanding.offeredValue].map((value, index) => <Statement key={value?.id ?? index} value={value} result={result} dossier={dossier} />)}</div>
    {result.features.length + result.explanations.length > 0 ? <div className={styles.group}><h4>关键做法与可能作用</h4>{result.features.map(feature => <div key={feature.id}><strong>{feature.label}</strong><Statement value={feature} result={result} dossier={dossier} /></div>)}{result.explanations.map(explanation => <div key={explanation.id}><Statement value={explanation} result={result} dossier={dossier} /><p className={styles.meta}>相关做法：{basisText(explanation.featureIds)}</p><p className={styles.meta}>{explanation.support}</p>{explanation.uncertainty ? <p className={styles.meta}>{explanation.uncertainty}</p> : null}</div>)}</div> : null}
    {result.feedback.length > 0 ? <div className={styles.group}><h4>已有反馈</h4>{result.feedback.map(value => <Statement key={value.id} value={value} result={result} dossier={dossier} />)}</div> : null}
    {result.tags.length > 0 ? <details><summary>候选标签与检索用途 · {result.tags.length}</summary><div className={styles.group}>{result.tags.map((tag, index) => <div key={index}><strong>{tag.label}</strong><p>{tag.retrievalUse}</p><p className={styles.meta}>判断依据：{basisText(tag.judgmentIds)}</p></div>)}</div></details> : null}
    {result.methodCandidates.length > 0 ? <details><summary>方法候选与条件</summary><div className={styles.group}>{result.methodCandidates.map(method => <div key={method.id}><p>{method.text}</p><p>适用条件：{method.conditions}</p>{method.openQuestion ? <p>待验证：{method.openQuestion}</p> : null}<p className={styles.meta}>判断依据：{basisText(method.basisIds)}</p></div>)}</div></details> : null}
    <details><summary>原文依据与本次材料范围</summary><div className={styles.group}><p>笔记材料：{read.coverage.materialColumns.map(column => columnLabels[column] ?? column).join('、')}。</p><p>{read.scope === 'note' ? '本次未选评论' : `纳入 ${read.coverage.includedCommentCount} 条评论，其中作者评论 ${read.coverage.authorCommentCount} 条`}；平台评论总数 {read.coverage.platformCommentCount ?? '未知'}。</p><ul>{[...read.coverage.issues, ...result.limitations].map((issue, index) => <li key={index}>{issue}</li>)}</ul>{result.evidence.map(evidence => <Evidence key={evidence.id} evidence={evidence} dossier={dossier} />)}</div></details>
  </div>;
}
