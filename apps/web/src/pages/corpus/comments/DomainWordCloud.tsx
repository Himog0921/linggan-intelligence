import { useEffect, useRef, useState } from 'react';
import { errorText, num } from './format';
import { useStudyRead, type StudyNavigation } from './studyApi';
import { parseWordCloud } from './wordCloudDtos';
import type { WordCloudCategory } from './overviewState';
import { c } from './styles';

type Category = WordCloudCategory;
const TABS: readonly { key: Category; label: string }[] = [
  { key: 'all', label: '全部表达' },
  { key: 'scenario', label: '困难场景' },
  { key: 'emotion', label: '情绪体验' },
  { key: 'approach', label: '办法与经历' },
];

function weight(value: number, maximum: number): number {
  if (!maximum || value <= 0) return 1;
  return Math.max(1, Math.min(5, Math.ceil(Math.sqrt(value / maximum) * 5)));
}

/** Compact V2.3 voice map: phrases are exact source-text matches, not AI-created labels. */
export function DomainWordCloud({ domain, days, nav, category, onCategory }: {
  domain: string; days: number; nav: StudyNavigation; category: Category; onCategory: (category: Category) => void;
}) {
  const query = useStudyRead(domain, `word-cloud?days=${days}`, parseWordCloud);
  const [chosen, setChosen] = useState<string | null>(null);
  const [showSources, setShowSources] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const items = query.data?.items ?? [];
  const visible = category === 'all' ? items : items.filter(item => item.category === category);
  const selected = visible.find(item => item.text === chosen) ?? visible[0] ?? null;
  const maximum = visible[0]?.count ?? 0;

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (showSources && !element.open) {
      if (typeof element.showModal === 'function') element.showModal();
      else element.setAttribute('open', ''); // jsdom has no native dialog methods
    }
    if (!showSources && element.open) {
      if (typeof element.close === 'function') element.close();
      else element.removeAttribute('open');
    }
  }, [showSources]);

  return <section className={c('study-wordcloud')} aria-label="领域高频表达">
    <div className={c('study-word-head')}>
      <div className={c('study-word-title')}><h2>领域高频表达</h2><small>评论原声中的短语 · 非用户问题判定</small></div>
      <div className={c('study-word-tabs')} role="group" aria-label="表达分类">
        {TABS.map(tab => <button key={tab.key} type="button" aria-pressed={category === tab.key}
          onClick={() => { onCategory(tab.key); setChosen(null); }}>{tab.label}</button>)}
      </div>
    </div>
    {query.isPending ? <div className={c('study-word-state')} role="status">正在统计近 {days} 天合格评论的词语…</div>
      : query.isError ? <div className={c('study-word-state')} role="status">词频暂时无法读取：{errorText(query.error)} <button type="button" onClick={() => { void query.refetch(); }}>重试读取</button></div>
      : query.data ? <>
        <div className={c('study-word-layout')}>
          <div className={c('study-word-main')}>
            <div className={c('study-word-stage')} aria-label="可点击的领域高频词语">
              {visible.length ? visible.slice(0, 24).map(item =>
                <button type="button" key={item.text} className={c('study-word-item')}
                  data-weight={weight(item.count, maximum)} data-highlight={selected?.text === item.text}
                  data-category={item.category} aria-pressed={selected?.text === item.text}
                  title={`${item.text}：${num(item.count)} 条不同合格评论，${num(item.workCount)} 篇作品`}
                  onClick={() => setChosen(item.text)}>{item.text}</button>) :
                <p className={c('study-word-empty')}>{query.data.eligibleCommentCount === 0
                  ? '此范围没有符合当前研究资格的评论，不代表用户没有表达。'
                  : category === 'all' ? '当前样本未提取出达到频次门槛的短语。' : '当前分类下没有明确归类的高频短语。'}</p>}
            </div>
            <div className={c('study-word-selected')}>
              {selected ? <><span>当前选中</span><b>{selected.text}</b>
                <strong>{num(selected.count)}</strong><span>条不同评论</span>
                <button type="button" disabled={!selected.examples.length} onClick={() => setShowSources(true)}>查看原声 ↗</button></>
                : <span>选择词语后，可查看对应评论的真实来源。</span>}
            </div>
          </div>
          <aside className={c('study-word-top10')} aria-label="高频短语 Top10">
            <div className={c('study-word-tophead')}><b>高频短语 TOP 10</b><span>不同评论数</span></div>
            {visible.slice(0, 10).map((item, i) => <button type="button" className={c('study-word-rank')}
              data-selected={selected?.text === item.text} key={item.text} onClick={() => setChosen(item.text)}
              aria-pressed={selected?.text === item.text}>
              <span>{String(i + 1).padStart(2, '0')}</span><b title={item.text}>{item.text}</b><strong>{num(item.count)}</strong>
            </button>)}
          </aside>
        </div>
        <div className={c('study-word-foot')}>
          <span>{query.data.truncated ? `仅统计已处理的 ${num(query.data.processedCommentCount)} / ${num(query.data.eligibleCommentCount)} 条评论，不代表完整领域。`
            : `近 ${days} 天 · ${num(query.data.eligibleCommentCount)} 条符合资格评论 · 同一评论每词计一次`}</span>
          <span title={`短语提取 ${query.data.algorithmVersion}；每评论分析前 ${query.data.textPrefixCharacters} 字；仅精确短语匹配，不做同义合并。`}>候选词频 · 可回查来源</span>
        </div>
      </> : null}
    <dialog ref={dialog} className={c('study-word-source-dialog')} aria-label="词语对应的真实评论"
      onClose={() => setShowSources(false)}>
      <div className={c('study-word-dialog-head')}>
        <h2>{selected?.text || '相关评论'}</h2>
        <button type="button" onClick={() => setShowSources(false)} aria-label="关闭来源列表">关闭</button>
      </div>
      <p>近 {days} 天观察到的合格评论；仅提供可回查来源，不把词频当成需求或解决方案有效性。</p>
      <div className={c('study-word-source-list')}>
        {selected?.examples.map((key, i) => <button key={`${key.workRef}:${key.commentExternalId}`} type="button"
          onClick={() => { setShowSources(false); nav.comment(key); }}>
          查看评论 {i + 1} 及父语境 <span aria-hidden="true">↗</span>
        </button>)}
      </div>
    </dialog>
  </section>;
}
