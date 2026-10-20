import React,{useState} from 'react';
import {spec} from './data.js';
import './problem-cards.css';
// Source JSX: real ProblemCards.tsx. The fixtures and local helpers below have no business I/O.
const recipe=spec.visual.compositionApplication.preservedCards;
const c=(...names)=>names.join(' '),num=n=>n==null?'未知':n.toLocaleString('zh-CN'),nonBlank=s=>s?.trim(),resultDate=s=>s||'时间未知';
const changeKinds={created:'首次建档',definition_revision:'定义更新'},candidateStates={pending:'等待归并',context:'上下文待核对'},signalKindLabel={problem:'问题表达',experience:'个人经验'};
const problemWarning=p=>!p.definitionReadable?'定义依据当前不可读取':p.supportWorkCount===1?'依据集中在1篇作品':'';
const problems=[
 {problemRef:'DEMO-P01',changeKind:'created',title:'如何让任务更容易开始？',definition:'合成研究定义：在任务开始前，需要把下一步说清楚，并保留孩子表达困难的机会。',preview:{sourceState:'known',commentText:'合成原声：我们先找一件能开始的小事，再一起看真正卡住的位置。'},definitionReadable:true,supportCommentCount:12,supportAuthorCount:7,supportWorkCount:3,recentAddedSupportCommentCount:5,updatedAt:'10.16',recent:true},
 {problemRef:'DEMO-P02',changeKind:'definition_revision',title:'提醒为什么越来越困难？',definition:'合成研究定义：重复提醒与任务顺序、环境和表达方式共同有关，不能只归为不愿意。',preview:{sourceState:'known',commentText:'合成原声：把顺序写下来以后，我们终于能讨论下一步，而不是一直催促。'},definitionReadable:true,supportCommentCount:8,supportAuthorCount:5,supportWorkCount:2,recentAddedSupportCommentCount:2,updatedAt:'10.15',recent:false}
];
const clues=[
 {signalRef:'DEMO-C01',state:'pending',kind:'problem',proposition:'开始前需要一个看得见的步骤',sourceState:'known',commentText:'合成原声：我需要先看到第一步，才知道从哪里开始。',createdAt:'10.16'},
 {signalRef:'DEMO-C02',state:'context',kind:'experience',proposition:'一次调整带来的个人经验',sourceState:'known',commentText:'合成原声：这次我们换了一个顺序，但还需要继续观察。',createdAt:'10.15'}
];
const g=recipe.geometry,m=recipe.material;
const style={'--lgi-space-1':'4px','--lgi-space-2':'8px','--lgi-space-3':g.quoteCutPx+'px','--lgi-space-4':g.rowGapPx+'px','--lgi-space-5':g.paddingPx+'px','--lgi-space-6':'24px','--lgi-space-8':g.openGlyphPx+'px','--lgi-ink':m.ink,'--lgi-canvas':m.canvas,'--lgi-canvas-low':m.quote,'--lgi-body':'var(--ds-ink)','--lgi-muted':'var(--ds-sub)','--lgi-on-dark':'#fff','--lgi-border-solid':m.ink,'--lgi-hairline':m.hairline,'--lgi-signal-ink':'var(--ds-orange-text)','--lgi-radius-none':g.radiusPx+'px','--lgi-shadow-hard':`${g.shadowOffsetPx}px ${g.shadowOffsetPx}px 0 rgb(17 19 21 / ${g.shadowOpacity})`,'--lgi-shadow-brutal':`${g.hoverShadowOffsetPx}px ${g.hoverShadowOffsetPx}px 0 ${m.ink}`,'--lgi-weight-regular':'var(--ty-body-weight)','--lgi-weight-medium':'var(--ty-label-weight)','--lgi-weight-bold':'var(--ty-object-weight)','--lgi-text-data':'var(--ty-label-size)','--lgi-lh-data':'var(--ty-label-line)','--lgi-text-body':'var(--ty-body-size)','--lgi-lh-body':'var(--ty-body-line)','--lgi-text-section':'var(--ty-section-size)','--lgi-text-longform':'var(--ty-reading-size)','--lgi-lh-heading':'var(--ty-object-line)','--lgi-lh-longform':'var(--ty-reading-line)','--lgi-font-sans':'var(--ty-ui)','--lgi-font-evidence':'var(--ty-evidence)','--lgi-font-mono':'var(--ty-data)','--lgi-ls-normal':'normal','--lgi-duration-state':'0ms','--lgi-z-stage':1};

export function ProblemMetric({ value, label }) {
  return <span className={c('sr3-metric')}><b title={num(value)}>{value != null && value >= 10000 ? `${(value / 10000).toFixed(1)}万` : num(value)}</b><span>{label}</span></span>;
}
export function ProblemResultCard({ problem: p, onOpen }) {
  const canDefine = p.definitionReadable, preview = p.preview?.sourceState === 'known' && p.preview.commentText?.trim() ? p.preview : null;
  const title = canDefine ? nonBlank(p.title) || '问题标题未记录' : '问题定义当前不可读取';
  return <article className={c('sr3-card', p.changeKind === 'definition_revision' ? 'sr3-revised' : '')}>
    <div className={c('sr3-card-top')}><span className={c('sr3-id')}>{p.problemRef.slice(0, 8)}</span><span className={c('sr3-kind')}>{changeKinds[p.changeKind || ''] || '更新状态未记录'}</span><span className={c('sr3-register')} aria-hidden="true">+</span></div>
    <h3 className={c('sr3-card-title')}>{title}</h3><div className={c('sr3-note')}><span>已保存的研究定义</span><p>{canDefine ? nonBlank(p.definition) || '定义正文未记录' : '不从受限材料补写结论。'}</p></div>
    <div className={c('sr3-quote')}><span className={c('sr3-quote-label')}>关联原声 <i aria-hidden="true">↘</i></span><blockquote>{preview ? preview.commentText : '当前没有可展示的原声预览。'}</blockquote><span className={c('sr3-quote-caption')}>{preview ? '预览 1 条 · 不作为整段定义的验证' : '原声可读性以最新读取为准'}</span></div>
    <div className={c('sr3-card-bottom')}><div className={c('sr3-metrics')}><ProblemMetric value={p.supportCommentCount} label="原声" /><ProblemMetric value={p.supportAuthorCount} label="账号" /><ProblemMetric value={p.supportWorkCount} label="作品" /></div><span className={c('sr3-open-glyph')} aria-hidden="true">↗</span></div>
    <div className={c('sr3-card-foot')}><span>{problemWarning(p) || `近 28 天首次关联 ${num(p.recentAddedSupportCommentCount)} 条原声`}</span><time>{resultDate(p.updatedAt)}</time></div><button type="button" className={c('sr3-card-hit')} data-sr3="open" data-ref={p.problemRef} aria-label={`查看问题：${title}`} onClick={onOpen}><span className="ds-sr-only">查看定义、原声与形成过程</span></button>
  </article>;
}
export function ProblemClueCard({ clue: p, onOpen }) {
  const canRead = p.sourceState === 'known' && Boolean(p.commentText?.trim()), other = ['solution', 'experience'].includes(p.kind);
  return <article className={c('sr3-card sr3-clue')}><div className={c('sr3-card-top')}><span className={c('sr3-id')}>线索</span><span className={c('sr3-kind')}>{candidateStates[p.state] || '处理状态未记录'}</span><span aria-hidden="true">+</span></div><h3 className={c('sr3-card-title')}>{canRead ? nonBlank(p.proposition) || '尚未记录归一表达' : '来源当前不可读取'}</h3><div className={c('sr3-note')}><span>{signalKindLabel[p.kind] || p.kind}</span><p>{other ? '建议或个人经验，不作为已归纳问题。' : '有效表达尚未建立长期问题，处理状态不代表用户观点。'}</p></div><div className={c('sr3-quote')}><span className={c('sr3-quote-label')}>原始表达</span><blockquote>{canRead ? p.commentText : '来源受限，原声及衍生内容不显示。'}</blockquote></div><div className={c('sr3-card-bottom')}><span className={c('sr3-caption')}>查看原声与处理依据</span><span className={c('sr3-open-glyph')} aria-hidden="true">↗</span></div><div className={c('sr3-card-foot')}><span>1 条表达 · 不计入问题数量</span><time>{resultDate(p.createdAt)}</time></div><button type="button" className={c('sr3-card-hit')} data-sr3="clue" data-ref={p.signalRef} aria-label="查看这条未归并表达" onClick={onOpen}><span className="ds-sr-only">查看线索和处理依据</span></button></article>;
}
export function ProblemCards({query='',onOpen}){
 const [view,setView]=useState('all'),[density,setDensity]=useState('standard');const isClue=view==='clues';
 const rows=(isClue?clues:problems).filter(p=>(view!=='changes'||p.recent)&&[p.title||p.proposition,p.definition||p.commentText].join(' ').includes(query));
 return <div className="q08-problems" style={style} data-card-source="ALIGN-COMPOSITION-009"><div className="pc-local-views" data-region="Z04-V"><div role="group" aria-label="问题阅读范围">{[['all','全部问题'],['changes','最近变化'],['clues','未归并线索']].map(([id,label])=><button type="button" key={id} aria-pressed={view===id} onClick={()=>setView(id)}>{label}</button>)}</div><div role="group" aria-label="卡片密度">{[['standard','阅读卡'],['compact','紧凑卡']].map(([id,label])=><button type="button" key={id} aria-pressed={density===id} onClick={()=>setDensity(id)}>{label}</button>)}</div></div><p className="pc-synthetic">合成样例 · {rows.length}{isClue?'条线索 · 不计入问题数量':'个已建档问题'} · 卡片沿用真实用户问题页</p><div data-density={density}>{rows.length?<div className="sr3-grid">{rows.map(p=>isClue?<ProblemClueCard key={p.signalRef} clue={p} onOpen={()=>onOpen(p.proposition)}/>:<ProblemResultCard key={p.problemRef} problem={p} onOpen={()=>onOpen(p.title)}/>)}</div>:<p className="ds-empty">当前合成样例没有匹配的问题。</p>}</div></div>;
}
