import { useEffect,useRef,useState,type ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { ActionButton } from '../../../../shared/ActionButton';
import type { Drawer } from '../api';
import { listHref } from '../useWorkspace';
import styles from './TargetResearch.module.css';

import { Context,useTargetResearch,useOptionalTargetResearch,type ResearchContext,type Panel } from './researchContext';
const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function TargetResearchProvider({data,params,children}:{data:Drawer;params:URLSearchParams;children:ReactNode}){
  const navigate=useNavigate();
  const [message,setMessage]=useState<string|null>(null);
  const returns=useRef<{url:string;scroll:number;focus:string|null}[]>([]);
  const restore=useRef<{url:string;scroll:number;focus:string|null}|null>(null);
  const selected=(params.get('inspect_works')??'').split(',').filter(Boolean);
  const valid=selected.length<=3&&new Set(selected).size===selected.length&&selected.every(id=>uuid.test(id));
  const focus=params.get('inspect_work');
  const patch=(values:Record<string,string|null>,replace=false)=>navigate(listHref(params,values),{replace});
  const toggle=(work:string)=>{
    if(!uuid.test(work))return;
    if(!valid){setMessage('对照选择无效，请清除后重新选择。');return;}
    if(selected.includes(work)){patch({inspect_works:selected.filter(v=>v!==work).join(',')||null,inspect_comparison_request:null});setMessage(null);return;}
    if(selected.length===3){setMessage('最多同时对照 3 篇，请先移除或替换一篇。');return;}
    patch({inspect_works:[...selected,work].join(','),inspect_comparison_request:null});setMessage(null);
  };
  const open=(panel:Panel,work?:string,values:Record<string,string|null>={})=>{
    if(panel==='compare'&&(!valid||selected.length<2)){setMessage('请先选择 2—3 篇作品。');return;}
    if(work&&!uuid.test(work))return;
    const active=document.activeElement;
    returns.current.push({url:listHref(params),scroll:document.querySelector('.c-dw-body')?.scrollTop??0,focus:active instanceof HTMLElement?active.id:null});
    patch({...values,inspect_panel:panel,inspect_work:work??null});
    requestAnimationFrame(()=>{document.querySelector('.c-dw-body')?.scrollTo({top:0});});
  };
  const back=()=>{
    const previous=returns.current.pop();
    if(previous){restore.current=previous;navigate(previous.url);}
    else patch({inspect_panel:null,inspect_work:null,inspect_left_observation:null,inspect_right_observation:null});
  };
  useEffect(()=>{
    const previous=restore.current;if(!previous)return;
    if(listHref(params)!==previous.url){restore.current=null;return;}
    const body=document.querySelector('.c-dw-body');if(!(body instanceof HTMLElement))return;
    const apply=()=>{
      const focus=previous.focus?document.getElementById(previous.focus):null;
      if(previous.focus&&!focus)return false;
      if(previous.scroll>0&&body.scrollHeight<body.clientHeight+previous.scroll)return false;
      body.scrollTop=previous.scroll;focus?.focus({preventScroll:true});restore.current=null;return true;
    };
    if(apply())return;
    // The original bounded read may finish after navigation. Restore only this return.
    const cancel=()=>{observer.disconnect();if(restore.current===previous)restore.current=null;};
    const observer=new MutationObserver(()=>{if(restore.current===previous&&apply())observer.disconnect();});
    observer.observe(body,{childList:true,subtree:true});
    const timeout=setTimeout(cancel,5000);
    const events=['wheel','touchstart','pointerdown','keydown'] as const;
    for(const event of events)document.addEventListener(event,cancel,{capture:true,once:true});
    return()=>{observer.disconnect();clearTimeout(timeout);for(const event of events)document.removeEventListener(event,cancel,true);};
  },[params]);
  const cutoff=params.get('inspect_as_of');
  const value:ResearchContext={data,params,scope:{targetRef:data.target.targetRef,domain:params.get('domain')??'all',cutoff},selected:valid?selected:[],panel:params.get('inspect_panel'),focusedWork:focus&&uuid.test(focus)?focus:null,
    message:message??(valid?null:'对照选择无效，请清除后重新选择。'),patch,toggle,open,back,
    clear:()=>{setMessage(null);patch({inspect_works:null,inspect_comparison_request:null});},
    pinCutoff:(next)=>{if(!params.has('inspect_as_of'))patch({inspect_as_of:next},true);}};
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function WorkResearchActions({work,similar=true}:{work:string;similar?:boolean}){
  const context=useOptionalTargetResearch();if(!context)return null;
  return <span className={styles.rowActions}>
    <ActionButton variant="quiet" aria-pressed={context.selected.includes(work)} onClick={()=>context.toggle(work)}>{context.selected.includes(work)?'移出对照':'加入对照'}</ActionButton>
    {similar&&context.data.target.kind==='creator'&&<ActionButton id={`work-${work}-similar`} variant="quiet" onClick={()=>context.open('similar',work)}>相近旧作</ActionButton>}
    <ActionButton id={`work-${work}-trace`} variant="quiet" onClick={()=>context.open('trace',work)}>观察轨迹</ActionButton>
  </span>;
}
export function ResearchSelectionBar(){
  const context=useTargetResearch();
  if(context.selected.length===0&&context.message===null)return null;
  return <div className={styles.selection} aria-label="作品对照选择">
    <span>已选 {context.selected.length} / 3 篇</span>
    <div className={styles.selectedNames}>{context.selected.map((id,index)=><span key={id} title={context.data.catalog?.find(w=>w.publicRef===id)?.title??`作品 ${index+1}`}>{context.data.catalog?.find(w=>w.publicRef===id)?.title??`作品 ${index+1}`}</span>)}</div>
    {context.message&&<span role="status">{context.message}</span>}
    <ActionButton id="work-comparison-open" variant="outline" disabled={context.selected.length<2} onClick={()=>context.open('compare')}>并排对照</ActionButton>
    <ActionButton variant="quiet" onClick={context.clear}>清除选择</ActionButton>
  </div>;
}
