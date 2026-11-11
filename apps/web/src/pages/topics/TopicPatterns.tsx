import { c } from './styles';
import { btn } from './topicElements';
import { citationText, locatedCitations, title } from './topicModel';
import type { ContentPattern, Work } from './types';
const contentTypes: Record<string, string> = {
  practical_guide: '方法指南', checklist: '清单', explanation: '解释说明', personal_story: '亲历故事',
  opinion: '观点表达', case_analysis: '案例分析', interactive_challenge: '互动挑战', question: '提问',
  product_intro: '产品介绍', mixed: '混合类型',
};
const kinds = [['content_type', '内容类型'], ['expression', '表达方式'], ['answer_form', '回答形式']] as const;
export function TopicPatterns({ works }: { works: Work[] }) {
  return <>{kinds.map(([kind, name]) => {
    const groups = new Map<string, { work: Work; coreRef: string; item: ContentPattern }[]>();
    const known = new Set<string>();
    for (const work of works) {
      if (!work.research?.readable || work.research.authorSourceState !== 'available') continue;
      const patterns = work.research.contentPatterns;
      if (!patterns) continue;
      for (const item of patterns.items.filter(item => item.kind === kind)) {
        const label = kind === 'content_type' ? contentTypes[item.label] : item.label;
        if (!label || !item.unitIds.length || !citationText(work, item.evidence)) continue;
        known.add(work.workRef);
        const rows = groups.get(label) ?? [];
        rows.push({ work, coreRef: patterns.coreRef, item }); groups.set(label, rows);
      }
    }
    return <section className={c('lgi-tm-section')} aria-label={name} key={kind}>
      <div className={c('lgi-tm-section-head')}><h3>{name}</h3><span>已识别 {known.size} / {works.length} 篇 · 未知 {works.length - known.size} 篇</span></div>
      <div className={c('lgi-tm-pattern-grid')}>{[...groups].map(([label, rows]) => {
        const refs = [...new Set(rows.map(row => row.work.workRef))];
        return <article className={c('lgi-tm-panel')} key={label}>
          <h4>{label}</h4><span>{refs.length} 篇</span>
          <details className={c('lgi-tm-core-details')}><summary>查看依据</summary>{rows.map(({ work, coreRef, item }, index) =>
            <div key={`${work.workRef}:${index}`} data-core-ref={coreRef} data-unit-ids={item.unitIds.join(',')}>
              <h5>{title(work)}</h5>{locatedCitations(work, item.evidence).map(citation => citation.fragment.field === 'core_image_evidence'
                ? <div key={`${citation.fragmentId}:${citation.start}:${citation.end}`}><span className={c('lgi-tm-eyebrow')}>图片理解依据</span><p>{citationText(work, [citation])}</p></div>
                : <blockquote key={`${citation.fragmentId}:${citation.start}:${citation.end}`}>{citationText(work, [citation])}</blockquote>)}
              {btn(locatedCitations(work, item.evidence).some(citation => citation.fragment.field === 'core_image_evidence') ? '查看来源' : '核对原文', 'reader', work.workRef, 'lgi-tm-quiet')}
            </div>)}</details>
          {btn('查看这些作品', 'subset', refs.join(','), 'lgi-tm-quiet')}
        </article>;
      })}</div>
    </section>;
  })}</>;
}
