import type { OperationsCoverage as Coverage } from './types';
import { number } from './topicModel';
import { badge } from './topicElements';
import s from './Overview.module.css';
export function OperationsCoverage({ coverage }: { coverage?: Coverage }) {
  if (!coverage) return null;
  return <section aria-label="运营发布覆盖" className={s.legend}>
    <span>运营发布 <b>{number(coverage.publishedWorkCount)}</b> 篇</span>
    <span>发布记录 <b>{number(coverage.publicationCount)}</b> 条</span>
    <span title={`小红书阅读 ≥ ${number(coverage.hitRule.threshold)}`}>运营爆款 <b>{number(coverage.hitWorkCount)}</b> 篇</span>
    <span>阅读未知 <b>{number(coverage.unknownViewsWorkCount)}</b> 篇</span>
  </section>;
}
export function OperationsWorkBadge({ coverage, workRef }: { coverage?: Coverage; workRef: string }) {
  const fact = coverage?.works.find(work => work.workRef === workRef);
  if (!fact?.publications.length) return null;
  const hit = fact.publications.some(publication => publication.hit);
  return badge(hit ? '运营爆款' : '运营已发布', hit);
}
