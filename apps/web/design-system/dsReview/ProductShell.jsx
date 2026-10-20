import React,{useEffect,useRef,useState} from 'react';
import {Select} from './Select.jsx';
import {TertiaryTabs} from './TertiaryTabs.jsx';
import {PersonalMenu} from './PersonalMenu.jsx';
import {ShellFrame} from '../shellReview/ShellFrame.jsx';
import {spec} from './data.js';
import '../shellReview/shell.css';

export function ContextRail({state='ready',module='语料',page='创作者',business=false,onNotice=()=>{},children}){
 const [domain,setDomain]=useState(state==='unselected'?'':'adhd');
 const unavailable=['loading','empty','unavailable'].includes(state);
 const options=business?[{value:'adhd',label:'家庭协作',meta:'演示业务'},{value:'learning',label:'学习支持',meta:'演示业务'}]:[{value:'all',label:'全部领域',meta:'全部领域'},{value:'adhd',label:'ADHD',meta:'演示范围'},{value:'learning',label:'考研自习',meta:'演示范围'},{value:'autism',label:'自闭症干预',meta:'演示范围'},{value:'math',label:'卢叔数学',meta:'演示范围'}];
 return <div className="ds-context-example" data-region="Z02"><nav className="ds-context-rail" aria-label="演示当前位置与领域"><span>{module}</span><span aria-hidden="true">/</span><Select label={business?'切换当前演示业务':'切换当前演示领域'} appearance="domain" value={unavailable?'':domain} onChange={v=>{setDomain(v);onNotice('已切换演示范围；真实业务未改变。')}} options={options} disabled={unavailable} placeholder={state==='loading'?'正在读取领域':state==='empty'?'暂无可选领域':state==='unavailable'?'领域暂不可用':business?'选择业务':'选择领域'} footer={{label:business?'管理业务':'管理领域',meta:'配置与目标用途',onClick:()=>onNotice('管理入口示例；本次只审阅组件，不跳转业务配置。')}}/><span aria-hidden="true">/</span><strong aria-current="page">{page}</strong>{children&&<div className="ds-context-tabs">{children}</div>}</nav></div>
}

export {PersonalMenu} from './PersonalMenu.jsx';

export function ProductShell({state='normal',variant='workspace',onNotice=()=>{}}){
 const [active,setActive]=useState('语料'),[section,setSection]=useState('创作者'),[partition,setPartition]=useState('overview'),[pageId,setPageId]=useState('P05'),[domain,setDomain]=useState('adhd'),[width,setWidth]=useState(900),frame=useRef(null);
 useEffect(()=>{const ro=new ResizeObserver(([e])=>setWidth(e.contentRect.width));if(frame.current)ro.observe(frame.current);return()=>ro.disconnect()},[]);
 const page=spec.pages.find(p=>p.id===pageId),mode=page.modes.find(m=>m.id===partition)||page.modes[0];
 if(variant!=='entry')return <div ref={frame} style={{minWidth:0,'--q06-preview-width':width+'px'}}><ShellFrame width={state==='narrow'?Math.min(width,390):width} page={page} mode={mode} domain={domain} onDomain={setDomain} onNavigate={setPageId} onMode={setPartition} onNotice={onNotice}><section className="ds-real-workspace" id={'preview-surface-'+page.id} role={page.modes.length>1?'tabpanel':undefined}><span className="ds-caption">Z04 · 工作区插槽</span><p>品牌、领域与三级导航由共享壳承载。</p><p className="ds-muted">当前 {page.name} · 合成内容</p></section></ShellFrame></div>;
 const sections=active==='采集'?['观察目标','采集任务']:active==='运营'?['运营概览','发布记录','账号管理','每日数据','数据交付']:active==='主题图谱'?['主题图谱']:['证据库','评论研究','创作者'];
 const nav=[['雷达','RADAR'],['主题图谱','TOPIC MAP'],['语料','CORPUS'],['洞察','INSIGHTS'],['采集','COLLECTION'],['运营','']];
 return <div className={`ds-real-shell ${state==='narrow'?'compact':''}`}>
  <header className="ds-real-header"><div className="ds-real-brand"><b className="ds-real-li">LI</b><strong>Linggan Intelligence</strong></div>{variant!=='entry'&&<nav className="ds-real-primary" aria-label="应用外壳演示一级导航">{nav.map(([name,key])=><button type="button" key={name} disabled={['雷达','洞察'].includes(name)||state==='permission_limited'&&name==='运营'} aria-current={active===name?'page':undefined} onClick={()=>{setActive(name);setSection(name==='采集'?'观察目标':name==='运营'?'运营概览':name==='主题图谱'?'主题图谱':'创作者');onNotice('仅切换导航示例；业务内容未接入。')}}><b>{name}</b><small><span>{key}</span> 演示</small></button>)}</nav>}<span className="ds-real-boundary"><i aria-hidden="true"/>合成预览</span><PersonalMenu onNotice={onNotice}/></header>
  {variant!=='entry'&&<><ContextRail module={active} page={section} onNotice={onNotice}>{section==='评论研究'&&<TertiaryTabs items={[{id:'overview',name:'评论概览'},{id:'comments',name:'用户评论'},{id:'runs',name:'研究批次'},{id:'problems',name:'用户问题'}]} value={partition} onChange={setPartition}/>}</ContextRail><div className="ds-real-layout"><nav className="ds-real-side" aria-label="应用外壳演示二级导航">{sections.map((n,i)=><button type="button" aria-current={n===section?'page':undefined} key={n} onClick={()=>setSection(n)}><small>0{i+1}</small><span>{n}</span></button>)}</nav><section className="ds-real-workspace"><span className="ds-caption">Z04 · 工作区插槽</span><p>品牌与一级导航共用页头，领域留在路径带。</p><p className="ds-muted">当前 {section} · 合成内容</p></section></div></>}
  {variant==='entry'&&<div className="ds-real-entry">入口页只继承品牌与个人入口，不展示未建立的业务上下文。</div>}
 </div>
}
