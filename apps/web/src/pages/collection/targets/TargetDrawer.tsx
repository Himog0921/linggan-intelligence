import { createPortal } from 'react-dom';
import { useEffect,useRef } from 'react';
import { Link,useNavigate } from 'react-router';
import { decodeDrawer,type Drawer } from './api';
import { useRead,listHref,type Commands } from './useWorkspace';
import { Avatar } from './TargetControls';
import { LifecyclePanel } from './TargetLifecycle';
import { RecentObservation } from './inspection/RecentObservation';
import { Works } from './TargetWorks';
import { Patrol } from './TargetPatrol';
import { TargetResearchProvider,ResearchSelectionBar } from './inspection/TargetResearchContext';
import { useTargetResearch } from './inspection/researchContext';
import { WorkComparisonPanel } from './inspection/WorkComparisonPanel';
import { ObservationTracePanel } from './inspection/ObservationTracePanel';
import { ResearchPanel } from './inspection/ResearchParts';
import { RelatedWorksPanel } from './inspection/RelatedWorksPanel';
import { KeywordRounds } from './inspection/KeywordRounds';
import { ActionButton } from '../../../shared/ActionButton';
import styles from './inspection/TargetResearch.module.css';
import actionStyles from '../../../shared/ActionButton.module.css';

export function TargetDrawer({targetRef,params,commands,revision}:{targetRef:string;params:URLSearchParams;commands:Commands;revision:number}){
  const navigate=useNavigate();
  const legacyLife=[...params.keys()].some(key=>key.startsWith('life_'));
  const rawTab=params.get('dtab');
  const tab=rawTab===null?(legacyLife?'works':'overview'):['works','baseline','archive'].includes(rawTab)?'works':rawTab==='patrol'?'patrol':'overview';
  const worksView=params.get('wview')==='list'?'list':'performance';
  const query=new URLSearchParams(params);
  // Research reads own their scope; UI navigation must not unload the base target.
  for(const key of [...query.keys()])if(key.startsWith('inspect_')||key.startsWith('catalog_'))query.delete(key);
  if(tab==='works'&&worksView==='performance'){
    if(!query.has('life_window'))query.set('life_window','all');
    if(!query.has('life_metric'))query.set('life_metric','likes');
  }
  const {data,error}=useRead(`/api/collection/workspace/target/${encodeURIComponent(targetRef)}?${query}`,decodeDrawer,revision);
  const heading=useRef<HTMLHeadingElement>(null);
  useEffect(()=>{heading.current?.focus();return()=>{document.getElementById(`target-${targetRef}`)?.focus();};},[targetRef]);
  const t=data?.target;
  const cleanup:Record<string,string|null>={drawer:null,dtab:null,wview:null,catalog_query:null,catalog_filter:null,catalog_page:null};
  for(const key of params.keys())if(key.startsWith('life_')||key.startsWith('inspect_'))cleanup[key]=null;
  const close=listHref(params,cleanup,`target-${targetRef}`);
  const source=t?.platform==='xhs'&&t.kind==='creator'?`https://www.xiaohongshu.com/user/profile/${encodeURIComponent(t.identity)}`:null;
  return createPortal(<aside id="c-drawer" className={`c-dw ${styles.drawer}`} aria-labelledby="c-drawer-title" onKeyDown={event=>{
    if(event.key==='Escape'&&!event.defaultPrevented){event.preventDefault();document.getElementById('target-drawer-close')?.click();}
  }}>
    <div className="c-dw-head"><div className="c-dw-title-row">
      <div className="c-dw-identity">{t&&<Avatar target={t} drawer/>}<div className="c-dw-identity-copy">
        <div className="c-dw-name-row"><h2 id="c-drawer-title" className="c-dw-title" tabIndex={-1} ref={heading}>{t?.name??error??'正在读取观察目标'}</h2>
          {t&&<span className="c-dw-watch-badge" data-state={t.lifecycle==='dismissed'?'stopped':t.lifecycle==='paused'?'paused':t.monitoring?'active':'inactive'}><i aria-hidden="true"/>{t.lifecycle==='dismissed'?'已停止观察':t.lifecycle==='paused'?'已暂停':t.monitoring?'观察中':'未开启观察'}</span>}
        </div>
        {t&&<div className="c-dw-meta"><span>{t.platform==='xhs'?'小红书':t.platform}</span>{t.kind==='creator'?` · ${t.handle}`:''}<i className="c-dw-meta-dot" aria-hidden="true"/><span>{t.group}</span></div>}
      </div></div>
      <div className={`c-dw-actions ${styles.inspectorActions}`}>
        {t&&<ActionButton variant="primary" onClick={()=>navigate(listHref(params,{rule:targetRef}))}>管理巡查</ActionButton>}
        {t&&t.lifecycle!=='dismissed'&&<ActionButton onClick={()=>navigate(`/collection/tasks?once=1&target=${targetRef}&domain=${encodeURIComponent(params.get('domain')??'all')}&returnTo=${encodeURIComponent(listHref(params))}`)}>立即采一次</ActionButton>}
        {source&&<a className={`${actionStyles.action} ${actionStyles.outline} ${styles.sourceAction}`} data-action-button="v8" data-lids-component="C09" data-variant="outline" href={source} target="_blank" rel="noreferrer"><span className={actionStyles.label}>打开小红书<svg className="c-dw-action-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-9 9M19 14v5H5V5h5"/></svg></span></a>}
        <ActionButton id="target-drawer-close" onClick={()=>navigate(close)}>关闭</ActionButton>
      </div>
    </div></div>
    {t&&<nav className="c-dw-tabs" aria-label="目标检查器页签">{[['overview','最近观察'],['works','作品'],['patrol','巡查']].map(([key,label])=><Link key={key} className={`c-dw-tab${tab===key?' c-dw-tab-on':''}`} aria-current={tab===key?'page':undefined} to={listHref(params,{dtab:key??'overview',inspect_panel:null,inspect_work:null})}>{label}</Link>)}</nav>}
    <div className="c-dw-body">{!data?<p className="c-dw-empty">{error??'正在读取目标事实…'}</p>:<TargetResearchProvider key={`${targetRef}:${params.get('domain')??'all'}`} data={data} params={params}>
      <DrawerContent data={data} params={params} commands={commands} tab={tab} worksView={worksView}/>
    </TargetResearchProvider>}</div>
  </aside>,document.body);
}

function DrawerContent({data,params,commands,tab,worksView}:{data:Drawer;params:URLSearchParams;commands:Commands;tab:string;worksView:string}){
  const research=useTargetResearch();
  if(research.panel==='rounds')return <ResearchPanel title="历史轮次"><KeywordRounds/><ResearchSelectionBar/></ResearchPanel>;
  if(research.panel==='compare')return <WorkComparisonPanel/>;
  if(research.panel==='trace'&&research.focusedWork)return <ObservationTracePanel key={research.focusedWork} work={research.focusedWork}/>;
  if(research.panel==='similar'&&research.focusedWork&&data.target.kind==='creator')return <RelatedWorksPanel key={research.focusedWork} work={research.focusedWork}/>;
  if(research.panel)return <div className={styles.notice}><p>当前展开条件已失效；已有作品和分析仍可查看。</p><ActionButton onClick={()=>research.patch({inspect_panel:null,inspect_work:null})}>返回作品</ActionButton></div>;
  if(tab==='patrol')return <Patrol data={data} params={params} commands={commands}/>;
  if(tab!=='works')return <><RecentObservation/><ResearchSelectionBar/></>;
  const creator=data.target.kind==='creator';
  const keywordRounds=params.get('inspect_keyword_view')==='rounds'&&params.get('catalog_filter')!=='pending';
  return <>
    <nav className="c-dw-view-tabs" aria-label="作品视图">{(creator?[['performance','图表'],['list','作品目录']]:[['catalog','作品目录'],['rounds','历史轮次']]).map(([key,label])=><Link key={key} className="c-dw-view-tab" aria-current={(creator?worksView===key:keywordRounds?key==='rounds':key==='catalog')?'page':undefined}
      to={listHref(params,creator?{wview:key??'list'}:{inspect_keyword_view:key??'rounds',catalog_filter:key==='rounds'?null:params.get('catalog_filter')})}>{label}</Link>)}</nav>
    {creator?(worksView==='performance'?<LifecyclePanel lifecycle={data.lifecycle} params={params}/>:<Works key={`${params.get('catalog_query')}:${params.get('catalog_filter')}`} data={data} params={params} commands={commands}/>):keywordRounds?<KeywordRounds/>:<Works key={`${params.get('catalog_query')}:${params.get('catalog_filter')}`} data={data} params={params} commands={commands}/>}
    <ResearchSelectionBar/>
  </>;
}
