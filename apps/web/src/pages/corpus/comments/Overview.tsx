import { useQuery } from '@tanstack/react-query';
import type { Dispatch, SetStateAction } from 'react';
import { readResearchOverview } from './commentResearchApi';
import { ObservationSeries } from './ObservationSeries';
import { OverviewMetrics } from './OverviewMetrics';
import { DomainWordCloud, type Category } from './DomainWordCloud';
import { type VoicePreview } from './overviewDtos';
import type { StudyNavigation } from './studyApi';
import { errorText, num } from './format';
import { c } from './styles';
export interface OverviewPreferences {seriesDays:number;seriesView:'chart'|'table';wordCategory:Category;method:'solution'|'experience'}
export function Overview({domain,nav,preferences,onPreferences}:{domain:string;nav:StudyNavigation;preferences:OverviewPreferences;onPreferences:Dispatch<SetStateAction<OverviewPreferences>>}){
 const query=useQuery({queryKey:['comment-research',domain,'overview'],queryFn:({signal})=>readResearchOverview(domain,signal),retry:false});
 const overview=query.data,knowledge=!query.isError && !query.isFetching ? overview?.knowledgeSummary : undefined;
 const method=preferences.method;
 const voices=knowledge?.[method==='solution'?'solutionPreview':'experiencePreview']??[];
 function voice(item:VoicePreview){return <article key={item.requestRef+item.quote} className={c('study-voice')}><blockquote>“{item.quote}”</blockquote><p>{item.proposition}</p><div className={c('study-voice-footer')}><span>{item.workTitle||'来源作品'}</span><button className={c('study-link')} type="button" onClick={()=>nav.comment(item.commentKey)}>查看评论及父语境</button></div></article>;}
 return <div className={c('study-overview')} data-compact-wordcloud="v23">
 {query.isError?<p className={c('study-restricted')} role="alert">概览读数刷新失败：{errorText(query.error)} <button className={c('study-link')} onClick={()=>void query.refetch()}>重试读取</button></p>:null}
 <section className={c('study-overview-hero')} aria-label="评论研究概览">
 {overview?<OverviewMetrics overview={overview} candidates={()=>nav.view('problems')}/>:<p className={c('study-empty')}>{query.isPending?'正在读取评论研究读数…':'概览读数暂不可用；用户评论仍可单独查看。'}</p>}
 <div className={c('study-overview-right')}>
 <ObservationSeries series={overview?.observationSeries??null} days={preferences.seriesDays} view={preferences.seriesView} onDays={seriesDays=>onPreferences(old=>({...old,seriesDays}))} onView={seriesView=>onPreferences(old=>({...old,seriesView}))}/>
 <DomainWordCloud domain={domain} days={preferences.seriesDays} nav={nav} category={preferences.wordCategory} onCategory={wordCategory=>onPreferences(old=>({...old,wordCategory}))}/>
 </div></section>
 {knowledge?<><section className={c('study-overview-section')} aria-labelledby="study-problem-title"><div className={c('study-section-head')}><h2 id="study-problem-title">已有依据的用户问题</h2><button type="button" className={c('study-link')} onClick={()=>nav.view('problems')}>查看用户问题</button></div>
 <p className={c('study-section-note')}>需要独立作者原声支持并通过核验；当前 {num(knowledge.problemCount)} 个问题，{num(knowledge.pendingCandidateCount)} 条表达等待独立支持。</p>
 {knowledge.problemPreview.length?<ol className={c('study-problem-list')}>{knowledge.problemPreview.map((problem,index)=>{const evidence=problem.evidence[0];return <li key={problem.problemRef} className={c('study-problem-item')}><div className={c('study-problem-top')}><span className={c('study-problem-index')}>0{index+1}</span><h3 className={c('study-problem-title')}>{problem.label}</h3></div>{evidence?<><blockquote className={c('study-problem-quote')}>“{evidence.quote}”</blockquote><div className={c('study-link-actions')}><button type="button" className={c('study-link')} onClick={()=>nav.research(evidence.requestRef,evidence)}>查看研究与依据</button><button type="button" className={c('study-link')} onClick={()=>nav.comment(evidence)}>查看原声</button></div></>:null}</li>;})}</ol>:<p className={c('study-empty')}>当前尚无通过核验的用户问题。已有原始评论仍可在“用户评论”中阅读；研究结果按新流程生成。</p>}
 </section><section className={c('study-overview-section')} aria-labelledby="study-method-title"><div className={c('study-section-head')}><h2 id="study-method-title">用户提到的办法与经历</h2><button type="button" className={c('study-link')} onClick={()=>nav.view('comments',{signalKind:method})}>查看相关原声</button></div><div className={c('study-method-switcher')} role="tablist" aria-label="办法与经历">{([['solution','解决办法'],['experience','使用经历']] as const).map(([kind,label])=><button type="button" key={kind} role="tab" aria-selected={method===kind} onClick={()=>onPreferences(old=>({...old,method:kind}))}>{label}</button>)}</div><p className={c('study-section-note')}>当前 {num(knowledge[method==='solution'?'solutionSignalCount':'experienceSignalCount'])} 条信号，覆盖 {num(knowledge[method==='solution'?'solutionWorkCount':'experienceWorkCount'])} 篇作品。来自用户自述，不代表相关做法已经验证有效或安全；此处最多预览 3 条。</p>{voices.length?<div className={c('study-voice-list')}>{voices.map(voice)}</div>:<p className={c('study-empty')}>当前没有可读的此类研究结果。</p>}</section></>:null}
 </div>;
}
