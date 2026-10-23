import { useState } from 'react';
import { Link } from 'react-router';
import type { Drawer } from './api';
import { TargetSelect } from './TargetSelect';
import { WorkResearchActions,useOptionalTargetResearch } from './inspection/TargetResearchContext';
import { ExactDetailGaps } from './inspection/ExactDetailGaps';
import type { Commands } from './useWorkspace';
import styles from './inspection/TargetResearch.module.css';

export function Works({data,params,commands}:{data:Drawer;params:URLSearchParams;commands:Commands}){
  const creator=data.target.kind==='creator';
  const [query,setQuery]=useState(params.get('catalog_query')??'');
  const [filter,setFilter]=useState(params.get('catalog_filter')??'all');
  const research=useOptionalTargetResearch();
  const works=data.catalog;
  if(!works)return <section className="c-dw-section c-dw-works"><h2>作品目录</h2><p className={styles.empty}>当前读不到作品记录，不能用总数拼成占位作品。原有分析视图仍可单独查看。</p></section>;
  const selected=params.get('catalog_filter')??'all';
  const needle=(params.get('catalog_query')??'').trim().toLowerCase();
  const matching=works.filter(work=>(selected==='pending'?!work.complete:selected==='complete'?work.complete:selected==='patrol_new'?work.source==='patrol':true)&&(!needle||work.externalId.toLowerCase().includes(needle)||work.title?.toLowerCase().includes(needle)));
  const completed=works.filter(work=>work.complete).length;
  const patrol=works.filter(work=>work.source==='patrol').length;
  return <section className="c-dw-section c-dw-works" id="target-works" aria-label={creator?'作品目录':'关键词全部作品'}>
    <div className={styles.inlineSummary}><span>目录 <b>{works.length}</b></span><span>详情已取 <b>{completed}</b></span><span>待取详情 <b>{data.inspector?.missing??'—'}</b></span><span>历次巡查带入 <b>{patrol}</b></span></div>
    <form className="c-dw-catalog-filter" action="/collection/targets" method="get">
      {[...params].filter(([key])=>!['catalog_query','catalog_filter'].includes(key)).map(([key,value])=><input key={key} type="hidden" name={key} value={value}/>)}
      <input type="search" name="catalog_query" value={query} onChange={event=>setQuery(event.target.value)} placeholder="搜索作品标题或作品 ID" aria-label="搜索作品标题或作品 ID"/>
      <TargetSelect className="c-dw-field" name="catalog_filter" label="筛选作品" value={filter} onChange={setFilter} options={[{value:'all',label:'全部'},{value:'pending',label:'待采详情'},{value:'complete',label:'详情已完成'},{value:'patrol_new',label:'历次巡查发现'}]}/>
      <button className="c-tg-act">筛选</button>
    </form>
    {creator&&<details className={styles.conditions}><summary>作品形式：视频 {works.filter(w=>w.kind==='视频').length} · 图文 {works.filter(w=>w.kind==='图文').length} · 未标记 {works.filter(w=>w.kind!=='视频'&&w.kind!=='图文').length}</summary><p>形式来自采集卡标记，历史缺省值不作为内容类型已核验的依据。</p></details>}
    <div className="c-dw-catalog-table-wrap" tabIndex={0} aria-label="作品目录，可横向滚动"><table className="c-dw-catalog-table">
      <thead><tr>{['作品','发布时间',creator?'发现来源':'创作者',creator?'卡片类型':'首次采样位置','详情','媒体处理','已采评论','详情观察时间','操作'].map(label=><th key={label}>{label}</th>)}</tr></thead>
      <tbody>{matching.map(work=><tr key={work.publicRef} data-selected={research?.selected.includes(work.publicRef)}>
        <td><div className="c-dw-catalog-work"><div className={styles.intro}>
          {research&&<input type="checkbox" className={styles.check} checked={research.selected.includes(work.publicRef)} aria-label={`将${work.title??'这篇作品'}加入对照`} onChange={()=>research.toggle(work.publicRef)}/>}
          <div><b title={work.title??undefined}>{work.title??(work.complete?'标题未收录':'标题待取得')}</b><span>{work.externalId}</span></div>
        </div><WorkResearchActions work={work.publicRef}/></div></td>
        <td><time className={styles.moment}>{work.published??'时间待取得'}</time></td>
        <td>{creator?(work.source==='archive'?'初始建档':'巡查发现'):work.creatorId&&data.target.platform==='xhs'?<a className="c-dw-author-link" href={`https://www.xiaohongshu.com/user/profile/${encodeURIComponent(work.creatorId)}`} target="_blank" rel="noopener noreferrer">{work.creatorName??'作者待取得'}</a>:work.creatorName??'作者待取得'}</td>
        <td>{creator?work.kind??'未标记':work.position??'未提供'}</td>
        <td><span className="c-dw-catalog-state" data-state={work.detailState}>{work.detailLabel}</span></td>
        <td>{work.media}</td><td>{work.comments??'—'}</td><td><time className={styles.moment}>{work.last??'—'}</time></td>
        <td><Link className="c-btn-secondary c-dw-work-open" to={`/corpus/evidence?${params.get('domain')&&params.get('domain')!=='all'?`domain=${encodeURIComponent(params.get('domain')??'')}&`:''}work=${work.publicRef}`} target="_blank" rel="noopener noreferrer">查看</Link></td>
      </tr>)}</tbody>
    </table></div>
    {matching.length===0&&<p className={styles.empty}>当前筛选没有匹配作品。</p>}
    {creator&&selected==='pending'&&<ExactDetailGaps data={data} params={params} commands={commands}/>}
  </section>;
}
