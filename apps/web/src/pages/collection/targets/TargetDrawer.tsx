import { createPortal } from 'react-dom';
import { useEffect,useRef } from 'react';
import { Link } from 'react-router';
import { decodeDrawer,type Drawer } from './api';
import { useRead,listHref,type Commands } from './useWorkspace';
import { Avatar } from './TargetControls';
import { LifecyclePanel } from './TargetLifecycle';
import { Overview } from './TargetOverview';
import { Works } from './TargetWorks';
import { Patrol } from './TargetPatrol';
import { TargetResearchProvider,ResearchSelectionBar,useTargetResearch } from './inspection/TargetResearchContext';
import { WorkComparisonPanel } from './inspection/WorkComparisonPanel';
import { ObservationTracePanel } from './inspection/ObservationTracePanel';
import { RelatedWorksPanel } from './inspection/RelatedWorksPanel';
import { KeywordRounds } from './inspection/KeywordRounds';
import { ActionButton } from '../../../shared/ActionButton';
import styles from './inspection/TargetResearch.module.css';

export function TargetDrawer({targetRef,params,commands,revision}:{targetRef:string;params:URLSearchParams;commands:Commands;revision:number}){
  const legacyLife=[...params.keys()].some(key=>key.startsWith('life_'));
  const rawTab=params.get('dtab');
  const tab=rawTab===null?(legacyLife?'works':'overview'):['works','baseline','archive'].includes(rawTab)?'works':rawTab==='patrol'?'patrol':'overview';
  const worksView=params.get('wview')==='performance'||(params.get('wview')===null&&legacyLife)?'performance':'list';
  const query=new URLSearchParams(params);
  if(tab==='works'&&worksView==='performance'){
    if(!query.has('life_window'))query.set('life_window','all');
    if(!query.has('life_metric'))query.set('life_metric','likes');
  }
  const {data,error}=useRead(`/api/collection/workspace/target/${encodeURIComponent(targetRef)}?${query}`,decodeDrawer,revision);
  const heading=useRef<HTMLHeadingElement>(null);
  useEffect(()=>{heading.current?.focus();return()=>{document.getElementById(`target-${targetRef}`)?.focus();};},[targetRef]);
  const t=data?.target;
  const cleanup:Record<string,string|null>={drawer:null,dtab:null,wview:null,catalog_query:null,catalog_filter:null};
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
      <div className="c-dw-actions">
        {t&&t.lifecycle!=='dismissed'&&<Link className="c-btn-secondary" to={`/collection/tasks?once=1&target=${targetRef}&domain=${encodeURIComponent(params.get('domain')??'all')}&returnTo=${encodeURIComponent(listHref(params))}`}>立即采一次</Link>}
        {source&&<a className="c-btn-secondary c-dw-source" href={source} target="_blank" rel="noreferrer"><span>打开小红书主页</span><svg className="c-dw-action-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-9 9M19 14v5H5V5h5"/></svg></a>}
        <Link id="target-drawer-close" className="c-btn-secondary c-dw-close" to={close}><svg className="c-dw-action-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg><span>关闭</span></Link>
      </div>
    </div></div>
    {t&&<nav className="c-dw-tabs" aria-label="目标检查器页签">{[['overview','概览'],['works','作品'],['patrol','巡查']].map(([key,label])=><Link key={key} className={`c-dw-tab${tab===key?' c-dw-tab-on':''}`} aria-current={tab===key?'page':undefined} to={listHref(params,{dtab:key??'overview',inspect_panel:null,inspect_work:null})}>{label}</Link>)}</nav>}
    <div className="c-dw-body">{!data?<p className="c-dw-empty">{error??'正在读取目标事实…'}</p>:<TargetResearchProvider key={`${targetRef}:${params.get('domain')??'all'}`} data={data} params={params}>
      <DrawerContent data={data} params={params} commands={commands} tab={tab} worksView={worksView}/>
    </TargetResearchProvider>}</div>
  </aside>,document.body);
}

function DrawerContent({data,params,commands,tab,worksView}:{data:Drawer;params:URLSearchParams;commands:Commands;tab:string;worksView:string}){
  const research=useTargetResearch();
  if(research.panel==='compare')return <WorkComparisonPanel/>;
  if(research.panel==='trace'&&research.focusedWork)return <ObservationTracePanel key={research.focusedWork} work={research.focusedWork}/>;
  if(research.panel==='similar'&&research.focusedWork&&data.target.kind==='creator')return <RelatedWorksPanel key={research.focusedWork} work={research.focusedWork}/>;
  if(research.panel)return <div className={styles.notice}><p>当前展开条件已失效；已有作品和分析仍可查看。</p><ActionButton onClick={()=>research.patch({inspect_panel:null,inspect_work:null})}>返回作品</ActionButton></div>;
  if(tab==='patrol')return <Patrol data={data} params={params} commands={commands}/>;
  if(tab!=='works')return <Overview data={data} params={params} commands={commands}/>;
  const creator=data.target.kind==='creator';
  const keywordRounds=params.get('inspect_keyword_view')!=='catalog'&&params.get('catalog_filter')!=='pending';
  return <>
    <nav className="c-dw-view-tabs" aria-label="作品视图">{(creator?[['list','列表'],['performance','表现']]:[['rounds','本轮观察'],['catalog','全部作品']]).map(([key,label])=><Link key={key} className="c-dw-view-tab" aria-current={(creator?worksView===key:keywordRounds?key==='rounds':key==='catalog')?'page':undefined}
      to={listHref(params,creator?{wview:key??'list'}:{inspect_keyword_view:key??'rounds',catalog_filter:key==='rounds'?null:params.get('catalog_filter')})}>{label}</Link>)}</nav>
    {creator?(worksView==='performance'?<LifecyclePanel lifecycle={data.lifecycle} params={params}/>:<Works data={data} params={params}/>):keywordRounds?<KeywordRounds/>:<Works data={data} params={params}/>}
    <ResearchSelectionBar/>
  </>;
}
