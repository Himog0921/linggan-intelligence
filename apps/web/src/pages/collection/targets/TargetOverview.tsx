import { useState } from 'react';
import { Link } from 'react-router';
import type { Drawer } from './api';
import { returnFields, pendingTargetFields, listHref, type Commands } from './useWorkspace';
import { Action } from './TargetControls';
import { archiveLabels,executionLabels,patrolLabels,rankings } from './targetLabels';
import styles from './inspection/TargetResearch.module.css';

export function Overview({data,params,commands}:{data:Drawer;params:URLSearchParams;commands:Commands}){
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
  const last=i.last??'尚无合格成功巡查';
  const rules=data.rules;
  const hasRules=rules!==null&&rules.length>0;
  const worksHref=listHref(params,{dtab:'works',wview:'list',inspect_keyword_view:creator?null:'rounds'});
  const gapHref=listHref(params,{dtab:'works',wview:'list',inspect_keyword_view:'catalog',catalog_filter:'pending'});
  const first=(i.latestNew===null?'未取得':i.latestNew);
  const count=(n:number|null)=>n===null?'未取得':n;
  return <div className={styles.root}>
    <section className="c-dw-section c-dw-now" id={creator?'archive-task':'required-action'}>
      <div className={styles.overviewHeading}>
        <div>
          <h2>{paused?'自动巡查已暂停':isStopped?'观察目标已停止':i.patrol==='running'?'巡查正在执行':'最近一次有效观察'}</h2>
          <p>最近成功：<time className={styles.moment}>{last}</time></p>
        </div>
        {paused&&hasRules&&<details className={styles.resumeDetails}>
          <summary>恢复自动巡查</summary>
          <p className={styles.meta}>恢复下列现有规则的后续排期，不代表已经开始采集，也不会把全部历史缺口直接加入补采。</p>
          <ul>{rules.map(rule=><li key={rule.ruleRef}>
            {rankings[rule.slot]??(creator?'主页目录':rule.slot)} · {rule.interval===null?'间隔未取得':'每 '+Math.round(rule.interval/3600)+' 小时'}
          </li>)}</ul>
          <button className="c-btn-primary" disabled={commands.busy||isStopped} onClick={()=>{
            void commands.run('patrol-toggle',{...returnFields(params),row_target_ref:t.targetRef,enable:true});
          }}>确认恢复排期</button>
        </details>}
        {paused&&rules!==null&&rules.length===0&&<Link className="c-btn-secondary" to={listHref(params,{rule:t.targetRef})}>设置巡查规则</Link>}
      </div>
      <div className={styles.inlineSummary}>
        <span>本轮命中 <b>{count(i.hits)}</b></span>
        <span>本目标本轮首次发现 <b>{first}</b></span>
        <span>执行 <b>{executionLabels[i.execution]??i.execution}</b></span>
        <span>巡查 <b>{patrolLabels[i.patrol]??i.patrol}</b></span>
        <Link to={worksHref}>{creator?'查看作品':'查看本轮结果'} ↗</Link>
      </div>
      {paused&&<p className={styles.meta}>本次结果只对应上述成功时点，暂停之后是否出现新作品仍未知。</p>}
      {i.latestNew===0&&!paused&&<p className={styles.meta}>本轮没有首次发现的作品；已有作品是否变化，需查看实际的分轮指标与观察轨迹。</p>}
      {i.patrol==='partial'||i.patrol==='blocked'?<p className={styles.notice}>最近巡查未能完整取得结果，请到巡查页检查实际执行与回执；已有合格作品仍然可查看。</p>:null}
    </section>
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
            content_public_ref:selection,return_drawer:t.targetRef,return_dtab:'overview',
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
