import React,{useEffect,useMemo,useRef,useState} from 'react';
import {spec} from '../dsReview/data.js';
import {Button,Find,Mark,Modal} from '../dsReview/ui.jsx';
import {Select} from '../dsReview/Select.jsx';
import {INITIAL_RUNS,FIELDS,qty,stage,tone,sourceShort,stamp,matchFilter,commentLabel} from '../taskLedger/model.js';
import {filters} from '../tasksReview/data.js';
const rule=spec.visual.ledgerApplication;
const presets=rule;
import '../dsReview/review.css';
import '../shellReview/shell.css';
import './ledger.css';

// Q07 compares column structure in the accepted A shell. No production data or commands.
const p0=presets.priority.P0,all=['name','id','stage',...FIELDS.map(f=>f[0]).filter(x=>!['name','id','stage','action'].includes(x)),'action'],labels=Object.fromEntries(FIELDS.map(f=>[f[0],f[1]]));
const numeric=['scope','details','comments','reused','added','cumulative','created','updated','id'];
function value(r,id){switch(id){case'name':return r.name;case'id':return r.id;case'domain':return r.domain;case'source':return sourceShort(r);case'scope':return `${qty(r.fixed)} / ${qty(r.limit)}`;case'mode':return commentLabel(r.mode);case'rank':return r.rank??(r.status==='legacy'?'未知':'不适用');case'stage':return stage(r);case'next':return r.next;case'created':return stamp(r);case'updated':return stamp(r,true);case'action':return '查看 ↗';default:return qty(r[id])}}
function priority(id){return Object.entries(presets.priority).find(([,fields])=>fields.includes(id))?.[0]}

export function FieldsMenu({fields,onChange}){
 const ref=useRef(null);
 useEffect(()=>{const close=e=>{if(!ref.current?.contains(e.target)&&ref.current)ref.current.open=false};document.addEventListener('pointerdown',close);return()=>document.removeEventListener('pointerdown',close)},[]);
 return <details className="q07-fields" ref={ref} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))e.currentTarget.open=false}} onKeyDown={e=>{if(e.key==='Escape'){e.stopPropagation();e.currentTarget.open=false;e.currentTarget.querySelector('summary').focus()}}}><summary>显示字段 <b>{fields.length}</b><span aria-hidden="true">⌄</span></summary><div><p>未选字段始终可从任务详情读取。P0不可关闭；缩窄不会自动减列。</p>{Object.entries(presets.priority).map(([key,ids])=><fieldset key={key}><legend>{key} · {key==='P0'?'固定身份与行动':key==='P1'?'常用核对':'按需核对'}</legend>{ids.map(id=><label key={id}><input type="checkbox" checked={fields.includes(id)} disabled={p0.includes(id)} onChange={e=>onChange(e.target.checked?[...fields,id]:fields.filter(f=>f!==id))}/>{labels[id]}</label>)}</fieldset>)}<Button tone="quiet" onClick={()=>onChange(all)}>全部17列</Button></div></details>
}

export function Ledger({runs,fields,colWidths,row,onOpen,selected,metrics,onMetrics}){
 const ref=useRef(null),cols=fields.map(id=>[id,labels[id],colWidths[id]]),total=cols.reduce((n,c)=>n+c[2],0);
 const [scroll,setScroll]=useState({left:0,max:0});
 useEffect(()=>{const el=ref.current;function measure(){const next={width:Math.round(el.clientWidth),table:Math.round(el.querySelector('table').getBoundingClientRect().width),row:Math.round(el.querySelector('tbody tr')?.getBoundingClientRect().height||0),count:runs.length,shellCollapsed:el.closest('.q06-frame')?.dataset.collapsed};onMetrics(next);setScroll({left:Math.round(el.scrollLeft),max:Math.max(0,Math.round(el.scrollWidth-el.clientWidth))})}const o=new ResizeObserver(measure);o.observe(el);measure();return()=>o.disconnect()},[fields,colWidths,row,runs,onMetrics]);
 return <><div className="q07-scroll-tools"><span>列宽合计 {total}px · <span aria-live="polite">{scroll.max?`横向位置 ${scroll.left} / ${scroll.max}px`:'当前宽度可完整容纳'}</span></span><div><Button tone="quiet" disabled={scroll.left<=0} onClick={()=>ref.current.scrollBy({left:-Math.max(180,ref.current.clientWidth*.75)})}>← 向左</Button><Button tone="quiet" disabled={scroll.left>=scroll.max} onClick={()=>ref.current.scrollBy({left:Math.max(180,ref.current.clientWidth*.75)})}>向右 →</Button></div></div><div ref={ref} className="q07-scroll" role="region" aria-label="单行任务账本，可横向滚动，左右方向键移动" tabIndex={0} data-sticky-name={metrics.width>=colWidths.name+96} data-sticky-action={metrics.width>=presets.narrow.stickyActionMinContentWidth} onScroll={e=>setScroll({left:Math.round(e.currentTarget.scrollLeft),max:Math.max(0,Math.round(e.currentTarget.scrollWidth-e.currentTarget.clientWidth))})} onKeyDown={e=>{if(e.target!==e.currentTarget)return;if(e.key==='ArrowRight'||e.key==='ArrowLeft'){e.preventDefault();e.stopPropagation();e.currentTarget.scrollBy({left:e.key==='ArrowRight'?180:-180})}}} style={{'--q07-row':row+'px','--q07-name-width':colWidths.name+'px'}}><table className="q07-table" style={{width:total,minWidth:total}}><caption className="ds-sr-only">合成采集任务，一次运行一行；未知与0区分</caption><colgroup>{cols.map(([id,,w])=><col key={id} style={{width:w}}/>)}</colgroup><thead><tr>{cols.map(([id,label])=><th scope="col" key={id} className={'q07-col-'+id} title={id==='scope'?'已固定作品数 / 本次发现上限，单位篇':label}>{label}</th>)}</tr></thead><tbody>{runs.map(r=><tr key={r.id} data-run-id={r.id} data-selected={selected===r.id}>{cols.map(([id])=><td key={id} className={'q07-col-'+id+(numeric.includes(id)?' q07-number':'')}><button type="button" tabIndex={id==='name'||id==='action'?0:-1} className={'q07-cell '+(id==='name'?'q07-name':'')} title={value(r,id)} aria-label={id==='action'?`查看详情：${r.name}`:`${labels[id]}：${value(r,id)}；查看完整任务`} onClick={e=>onOpen(r,e.currentTarget)}>{id==='stage'?<Mark tone={tone(r)}>{value(r,id)}</Mark>:value(r,id)}</button></td>)}</tr>)}</tbody></table></div>{!runs.length&&<div className="q07-empty"><strong>当前条件没有匹配任务</strong><p>没有删除任何材料，清空查询可恢复本地样例。</p></div>}</>;
}

export function TaskDetail({run,onClose,onAction}){
 const [tab,setTab]=useState('fields');
 return <Modal title={run.name} onClose={onClose} footer={<><span>合成任务 · 只读检视</span>{onAction&&<Button tone="quiet" onClick={()=>{onClose();onAction('A-P09-control',run.name)}}>任务控制</Button>}<Button onClick={onClose}>返回原位置</Button></>}><div className="q07-detail"><p className="q07-detail-identity">{run.id} · {run.domain} · {stage(run)}</p><div className="q07-detail-tabs" role="group" aria-label="任务详情查看内容">{[['fields','全部字段'],['progress','进展与缺口'],['results','成果口径']].map(([id,label])=><Button key={id} tone="quiet" aria-pressed={tab===id} onClick={()=>setTab(id)}>{label}</Button>)}</div>{tab==='fields'?<dl>{all.filter(x=>x!=='action').map(id=><div key={id}><dt>{labels[id]}</dt><dd>{value(run,id)}</dd></div>)}</dl>:tab==='progress'?<><h3>{stage(run)}</h3><p>{run.next}</p><p>{run.gap||'当前样例未报告缺口，不据此承诺完整。'}</p><p>停止取消未认领工作；已产生的材料仍可能继续交付。固定集合与上限分别解释，不计算伪完成百分比。</p></>:<><dl>{[['本次可读详情 · 篇',run.details],['其中复用 · 篇',run.reused],['本次采回评论 · 条',run.received],['本次可检索评论 · 条',run.comments],['本次新增评论 · 条',run.added],['共享累计评论 · 条',run.cumulative]].map(([k,v])=><div key={k}><dt>{k}</dt><dd>{qty(v)}</dd></div>)}</dl><p>未知保持未知；0是本次已知读数，不代表平台没有评论。本次、新增与共享累计分别核对。</p></>}</div></Modal>;
}


export function TaskLedgerSurface({domain='all',onNotice=()=>{},onAction,onInspect,runsOverride,labels:showLabels=false,onZone=()=>{},onMetrics,readState='normal'}){
 const [fields,setFields]=useState(rule.fields),[query,setQuery]=useState(''),[view,setView]=useState('all'),[selected,setSelected]=useState(null),[detail,setDetail]=useState(null),[metrics,setMetrics]=useState({width:0});const opener=useRef(null);
 useEffect(()=>setQuery(''),[domain]);
 const runs=useMemo(()=>readState==='empty'?[]:(runsOverride||INITIAL_RUNS).filter(r=>(domain==='all'||r.domain===(domain==='learning'?'考研自习':'ADHD'))&&matchFilter(r,view)&&(r.name+r.id+r.source).includes(query)),[query,view,domain,readState,runsOverride]);
 useEffect(()=>onMetrics?.({...metrics,fields}),[metrics,fields,onMetrics]);
 function open(r,el){opener.current=el;setSelected(r.id);if(onInspect)onInspect(r,el);else setDetail(r)}
 function close(){setDetail(null);requestAnimationFrame(()=>opener.current?.isConnected&&opener.current.focus({preventScroll:true}))}
 function zone(id,children){return <section className="q08-region" data-region={id} data-labeled={showLabels} onClick={()=>onZone(id)}>{showLabels&&<button type="button" className="ds-zone-label" onClick={e=>{e.stopPropagation();onZone(id)}} aria-label={'定位 '+id}>{id}</button>}{children}</section>}
 return <section className="q07-workspace" data-ledger-standard={rule.id}><h1 className="ds-sr-only">采集任务</h1>
 {zone('Z04-T',<div className="q07-toolbar"><Find value={query} onChange={setQuery} trigger="submit" withScope={false} label="搜索采集任务" placeholder="任务名称、来源或运行编号" submitLabel="搜索"/><Button tone="quiet" onClick={()=>onNotice('已重新展示同一组合成快照，没有读取业务接口。')}>刷新</Button><Button tone="primary" onClick={()=>onAction?onAction('A-P09-create','当前任务集合'):onNotice('新建采集完整流程仍在连续账本交互稿，本页为已采用行规范。')}>＋ 新建采集</Button></div>)}
 {zone('Z04-V',<div className="q07-views"><div>{filters.map(([id,label])=><button type="button" key={id} aria-pressed={view===id} onClick={()=>setView(id)}>{label}</button>)}</div></div>)}
 {zone('Z04-R',<div className="q07-result"><span><b>{runs.length}</b>条匹配任务 · 合成截至10.16 / 10:45</span><FieldsMenu fields={fields} onChange={next=>setFields([...next.filter(x=>x!=='action'),'action'])}/></div>)}
 {readState==='stale'&&zone('Z04-F',<div className="ds-notice" role="alert">读取失败，保留上次任务与数据截至，不能据此认定没有任务。</div>)}
 {zone('Z04-B',<Ledger runs={runs} fields={fields} colWidths={rule.widths} row={rule.decision.rowHeight} onOpen={open} selected={selected} metrics={metrics} onMetrics={setMetrics}/>)}
 {zone('Z04-P',<div className="q07-foot"><span>第1页 · 本地样例共{runs.length}条</span><Button tone="quiet" disabled>上一页</Button><Button tone="quiet" disabled>下一页</Button></div>)}
 {detail&&<TaskDetail run={detail} onClose={close} onAction={onAction}/>}
 </section>;
}
