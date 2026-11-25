import { errorText, num } from './format';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { parseWordCloud } from './wordCloudDtos';
import { c } from './styles';

export type Category = 'all' | 'scenario' | 'emotion' | 'approach';
const TABS: readonly { key: Category; label: string }[] = [
  { key: 'all', label: '全部表达' },
  { key: 'scenario', label: '困难场景' },
  { key: 'emotion', label: '情绪体验' },
  { key: 'approach', label: '办法与经历' },
];

/** B-04 phrase cloud: exact source-text phrases in ink; size follows count, the top three are only heavier (never red). */
export function DomainWordCloud({ domain, days, nav, category, onCategory }: {
  domain: string; days: 7 | 28 | 56; nav: StudyNavigation; category: Category; onCategory: (category: Category) => void;
}) {
  const query = useStudyRead(domain, `word-cloud?days=${days}`, parseWordCloud);
  const items = query.data?.items ?? [];
  const visible = (category === 'all' ? items : items.filter(item => item.category === category)).slice(0, 10);
  const maximum = Math.max(1, ...visible.map(item => item.count));
  const data = query.data;
  return <div className={c('ov-cell ov-row2 chart-col')}><section className={c('expression')} aria-label="领域高频表达">
    <div className={c('expression-head')}>
      <div><strong>领域高频表达</strong> <small>评论原声中的短语 · 非用户问题判定</small></div>
      <div className={c('tabs')} role="group" aria-label="表达类型">
        {TABS.map(tab => <button key={tab.key} type="button" className={c(category === tab.key && 'active')} aria-pressed={category === tab.key} onClick={() => onCategory(tab.key)}>{tab.label}</button>)}
      </div>
    </div>
    <div className={c('wordcloud')} aria-label="可点击的领域高频短语">
      {query.isPending ? <span className={c('muted')} role="status">正在统计近 {days} 天合格评论的短语…</span>
        : query.isError ? <span className={c('muted')} role="status">短语暂时无法读取：{errorText(query.error)} <button type="button" className={c('text-action')} onClick={() => { void query.refetch(); }}>重试读取</button></span>
        : !visible.length ? <span className={c('muted')}>{data?.eligibleCommentCount === 0 ? '此范围没有符合研究资格的评论，不代表用户没有表达。' : category === 'all' ? '当前样本未提取出达到频次门槛的短语。' : '当前分类没有足够的表达。'}</span>
        : visible.map((item, index) => <button type="button" key={item.text} data-rank={index < 3 ? 'top' : 'rest'} style={{ fontSize: `${Math.round(13 + 9 * item.count / maximum)}px` }}
          title={`${num(item.count)} 条评论提到“${item.text}”，${num(item.workCount)} 篇作品`}
          onClick={() => nav.view('comments', { q: item.text, searchScope: null, workRef: null, supportOf: null, page: null, cursor: null })}>
          {item.text}<span className={c('word-count')}>{num(item.count)}</span></button>)}
    </div>
    <div className={c('word-hint')} title={data ? `短语提取 ${data.algorithmVersion}；每条评论分析前 ${data.textPrefixCharacters} 字；仅精确短语匹配，不做同义合并。` : undefined}>
      {data?.truncated ? `仅统计已处理的 ${num(data.processedCommentCount)} / ${num(data.eligibleCommentCount)} 条评论 · ` : ''}选择短语，查看相关评论
    </div>
  </section></div>;
}
