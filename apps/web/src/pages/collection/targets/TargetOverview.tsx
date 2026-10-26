import { useState } from 'react';
import { Link } from 'react-router';
import type { Drawer } from './api';
import { returnFields, pendingTargetFields, listHref, type Commands } from './useWorkspace';
import { Action } from './TargetControls';
import { archiveLabels,rankings } from './targetLabels';
import styles from './inspection/TargetResearch.module.css';

export function TargetMaintenance({data,params,commands}:{data:Drawer;params:URLSearchParams;commands:Commands}){
  const i=data.inspector,t=data.target,creator=t.kind==='creator';
  const [retire,setRetire]=useState<string[]|null>(()=>{
    const value=pendingTargetFields('retire-materials',{row_target_ref:t.targetRef})?.content_public_ref;
    return Array.isArray(value)?value:null;
  });
  const selection=retire??data.blocked?.map(item=>item.publicRef)??[];
  if(!i)return <section className="c-dw-section c-dw-now">
    <h2>最近观察暂不可读</h2>
    <p className={styles.meta}>无法确认本轮命中、详情覆盖和巡查状态。读取失败不等于没有作品或无新增。</p>
    {!creator&&data.decision.action&&<Action target={t} action={data.decision.action} params={params} commands={commands}/>}
  </section>;
  const isStopped=t.lifecycle==='dismissed';
  const paused=!t.monitoring&&!isStopped;
  const rules=data.rules;
  const hasRules=rules!==null&&rules.length>0;
  const gapHref=listHref(params,{dtab:'works',wview:'list',inspect_keyword_view:'catalog',catalog_filter:'pending'});
  const count=(n:number|null)=>n===null?'未取得':n;
  return <div className={styles.root}>
    {paused&&hasRules&&<details className={styles.resumeDetails}><summary>恢复自动巡查</summary>
      <ul>{rules.map(rule=><li key={rule.ruleRef}>{rankings[rule.slot]??(creator?'主页目录':rule.slot)} · {rule.interval===null?'间隔未取得':'每 '+Math.round(rule.interval/3600)+' 小时'}</li>)}</ul>
      <p className={styles.meta}>恢复现有规则的后续排期。</p>
      <button className="c-btn-secondary" disabled={commands.busy||isStopped} onClick={()=>{void commands.run('patrol-toggle',{...returnFields(params),row_target_ref:t.targetRef,enable:true});}}>确认恢复排期</button>
    </details>}
    {paused&&rules!==null&&rules.length===0&&<Link className="c-btn-secondary" to={listHref(params,{rule:t.targetRef})}>设置巡查规则</Link>}
    <section className={styles.maintenance} id="archive-problems">
      <div><strong>资料覆盖</strong>
        <p>作品目录 <b>{count(i.works)}</b> · 详情已取得 <b>{count(i.details)}</b> · 待取得详情 <b>{count(i.missing)}</b></p>
      </div>
      {i.missing!==null&&i.missing>0&&<Link className="c-btn-secondary" to={gapHref}>查看并处理缺口</Link>}
    </section>
    {i.blocked!==null&&i.blocked>0&&<p className={styles.notice}>有 {i.blocked} 条已知详情受阻；请核对对应作品及执行记录，不自动判定为失效。</p>}
    {i.quarantined!==null&&i.quarantined>0&&<p className={styles.notice}>有 {i.quarantined} 条材料待核对；不计入成功取得。</p>}
    {data.decision.action&&!((creator&&data.decision.action.kind==='gaps')||(paused&&hasRules&&data.decision.action.kind==='rule'))&&
      <section className="c-dw-section c-dw-decision">
        <div className="c-dw-decision-copy"><strong>{data.decision.title}</strong><p>{data.decision.note}</p></div>
        <Action target={t} action={data.decision.action} params={params} commands={commands}/>
      </section>}
    {creator&&data.blocked!==null&&data.blocked.length>0&&
      <details className={styles.conditions}>
        <summary>确认已失效作品 · {data.blocked.length} 篇</summary>
        <form className="c-dw-retire" onSubmit={event=>{
          event.preventDefault();
          void commands.run('retire-materials',{
            ...returnFields(params),row_target_ref:t.targetRef,
            content_public_ref:selection,return_drawer:t.targetRef,return_dtab:'patrol',
          });
        }}>
          <p>只有人工确认失效的作品才应登记。记录仍保留在作品目录中，不会删除历史证据。</p>
          <ul>{data.blocked.map(item=><li key={item.publicRef}><label>
            <input type="checkbox" checked={selection.includes(item.publicRef)}
              onChange={event=>setRetire(event.target.checked?[...selection,item.publicRef]:selection.filter(v=>v!==item.publicRef))}/>
            {item.title??'标题尚未取得'}</label></li>)}</ul>
          <button className="c-btn-secondary" disabled={!selection.length||commands.busy}>确认这些作品已失效</button>
        </form>
      </details>}
    {creator&&i.archive!=='complete'&&i.missing===0&&<p className={styles.meta}>建档：{archiveLabels[i.archive]??i.archive}。建档完整性与持续巡查分别判断。</p>}
  </div>;
}
