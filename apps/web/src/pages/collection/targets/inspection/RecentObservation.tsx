import { ActionButton } from '../../../../shared/ActionButton';
import { InspectorSelect } from './InspectorSelect';
import { METRICS,type MetricKey } from './inspectionApi';
import { metricLabels,metricValue,moment,number,reason } from './inspectionPresent';
import { MetricPicker,ReadFeedback } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { WorkResearchActions } from './TargetResearchContext';
import { useObservationRounds } from './useObservationRounds';
import styles from './TargetResearch.module.css';

export function CoverageSummary(){
  const {data}=useTargetResearch(),i=data.inspector;
  const count=(value:number|null|undefined)=>value==null?'未取得':number(value);
  const equal=i?.works!=null&&i.works>0&&i.details===i.works;
  const summary=i?.works==null&&i?.details==null&&i?.missing==null?'状态未取得':`${equal?`目录 ${count(i.works)} 篇 · 详情均已取得`:`目录 ${count(i?.works)} · 详情已取得 ${count(i?.details)}`} · 待取得详情 ${count(i?.missing)}`;
  return <details className={styles.conditions}>
    <summary>材料：{summary}</summary>
    {i?.blocked!=null&&i.blocked>0&&<p>详情受阻 {i.blocked} 篇</p>}
    {i?.quarantined!=null&&i.quarantined>0&&<p>材料待核对 {i.quarantined} 条</p>}
  </details>;
}

export function RecentObservation(){
  const context=useTargetResearch();
  const {page,result,compared,previous,options,offset,right}=useObservationRounds();
  const rawMetric=context.params.get('inspect_metric');
  const metric:MetricKey=METRICS.includes(rawMetric as MetricKey)?rawMetric as MetricKey:'likes';
  const round=result.data?.round;
  const discoveries=result.data?.works.filter(work=>work.firstSeenInScope===true)??[];
  const changes=(compared.data?.rows??[]).filter(row=>row.metrics[metric].comparable&&row.metrics[metric].delta!==null&&row.metrics[metric].delta!==0)
    .sort((a,b)=>Math.abs(b.metrics[metric].delta??0)-Math.abs(a.metrics[metric].delta??0));
  const hasComparable=compared.data?.rows.some(row=>row.metrics[metric].comparable)??false;
  const discoveryUnknown=result.data?.works.some(work=>work.firstSeenInScope===null)??false;
  const limited=Boolean(compared.data?.reasons.length||result.data?.truncated||(round&&round.coverage!=='complete'));
  return <section className={styles.root} aria-label="最近观察">
    <header className={styles.observationHead}><div><h2>观察结果</h2></div>
      <div className={styles.toolbar}>{options.length>0&&<InspectorSelect label="选择观察轮次" value={right??''} options={options} onChange={value=>context.patch({inspect_right_round:value,inspect_left_round:null})}/>}
        <ActionButton variant="quiet" onClick={()=>context.patch({inspect_as_of:null,inspect_right_round:null,inspect_left_round:null,inspect_round_offset:null})}>刷新</ActionButton></div>
    </header>
    <ReadFeedback read={page}/><ReadFeedback read={result}/><ReadFeedback read={compared}/>
    {round&&<div className={styles.inlineSummary}><span>命中 <b>{number(round.hits)}</b></span>
      {compared.data?<><span>共同 <b>{number(compared.data.commonCount)}</b></span><span>新出现 <b>{number(compared.data.enteredCount)}</b></span><span>未命中 <b>{number(compared.data.absentCount)}</b></span>
        <ActionButton variant="quiet" onClick={()=>context.open('rounds',undefined,{inspect_round_view:'compare',inspect_left_round:previous?.roundRef??null})}>两轮对照</ActionButton></>:<span>参照轮次未取得</span>}
    </div>}
    {limited&&<div className={styles.observationStatus} role="status" aria-label="观察状态">
      {round&&round.coverage!=='complete'&&<span>部分结果</span>}
      {compared.data?.reasons.length? <details><summary>比较受限</summary>{compared.data.reasons.map(code=><p key={code}>{reason(code)}</p>)}</details>:null}
      {result.data?.truncated&&<span>名单未完整取得</span>}
    </div>}
    <div className={styles.recentColumns}>
      <section aria-label="首次发现作品"><div className={styles.sectionHead}><h3>{round?.ruleRef?'本规则首次发现':'本目标首次发现'}</h3><ActionButton variant="quiet" onClick={()=>context.open('rounds',undefined,{inspect_round_view:'latest'})}>本轮作品</ActionButton></div>
        {discoveries.slice(0,2).map(work=><article className={styles.recentWork} key={work.workRef}>
          <button id={`recent-${work.workRef}-trace`} className={styles.recentTitle} onClick={()=>context.open('trace',work.workRef)}>{work.title??'标题未取得'}</button>
          <div className={styles.meta}>{work.libraryPresence==='new'?'新入库':work.libraryPresence==='existing'?'库内已有':'入库待核对'}{work.creator?` · ${work.creator}`:''}</div>
          <WorkResearchActions work={work.workRef} similar={false}/>
        </article>)}
        {!result.pending&&discoveries.length===0&&<p className={styles.recentEmpty}>{!result.data||discoveryUnknown?'首次发现待核对':'当前名单无新发现'}</p>}
      </section>
      <section aria-label="所选两轮的指标变化"><div className={styles.sectionHead}><h3>两轮指标变化</h3><MetricPicker value={metric} onChange={value=>context.patch({inspect_metric:value})}/></div>
        {changes.slice(0,3).map(row=>{const difference=row.metrics[metric];return <article className={styles.changeWork} key={row.workRef}>
          <div><button id={`recent-change-${row.workRef}-trace`} className={styles.recentTitle} onClick={()=>context.open('trace',row.workRef)}>{row.right?.title??row.left?.title??'标题未取得'}</button>
            <p className={styles.meta}>{metricLabels[metric]} {metricValue(difference.left)} → {metricValue(difference.right)}</p>
            <WorkResearchActions work={row.workRef} similar={false}/></div>
          <b className={styles.delta} data-positive={(difference.delta??0)>0}>{(difference.delta??0)>0?'+':''}{number(difference.delta)}</b>
        </article>;})}
        {!compared.pending&&changes.length===0&&<p className={styles.recentEmpty}>{!compared.data?'缺少参照轮次':hasComparable?'可比记录未见变化':'暂无可比指标'}</p>}
      </section>
    </div>
    {compared.data&&<details className={styles.conditions}><summary>比较范围</summary><p>{moment(compared.data.left.observedAt)} → {moment(compared.data.right.observedAt)}</p><p>材料截点 {moment(compared.data.knowledgeCutoff)}。</p></details>}
    {page.data&&(offset>0||page.data.nextOffset!==null)&&<div className={styles.footer}><span className={styles.meta}>{page.data.rounds.length===0?'尚无可还原轮次':''}</span><div className={styles.rowActions}>
      <ActionButton variant="quiet" disabled={offset===0} onClick={()=>context.patch({inspect_round_offset:String(Math.max(0,offset-50)),inspect_right_round:null,inspect_left_round:null})}>较新轮次</ActionButton>
      <ActionButton variant="quiet" disabled={page.data.nextOffset===null} onClick={()=>context.patch({inspect_round_offset:String(page.data!.nextOffset),inspect_right_round:null,inspect_left_round:null})}>更早轮次</ActionButton>
    </div></div>}
    <CoverageSummary/>
  </section>;
}
