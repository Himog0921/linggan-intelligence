import { useEffect,useState } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { decodeObservationComparison,decodeTrace,inspectionUrl,METRICS,type MetricKey } from './inspectionApi';
import { metricLabels,metricValue,moment,ruleLabels,sourceName } from './inspectionPresent';
import { Cutoff,Delta,EvidenceLink,MetricPicker,ReadFeedback,ResearchPanel } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';

export function ObservationTracePanel({work}:{work:string}){
  const context=useTargetResearch();const [message,setMessage]=useState<string|null>(null);
  const rawMetric=context.params.get('inspect_metric');const metric:MetricKey=METRICS.includes(rawMetric as MetricKey)?rawMetric as MetricKey:'likes';
  const offset=Math.max(0,Number(context.params.get('inspect_trace_offset')??'0')||0);
  const read=useInspectionRead(inspectionUrl(context.scope,`works/${work}/observations`,{offset}),decodeTrace);
  const left=context.params.get('inspect_left_observation');const right=context.params.get('inspect_right_observation');
  const compared=useInspectionRead(left&&right?inspectionUrl(context.scope,`works/${work}/observation-comparison`,{left,right}):null,decodeObservationComparison);
  useEffect(()=>{if(read.data)context.pinCutoff(read.data.knowledgeCutoff);},[read.data,context]);
  const selected=[left,right].filter((id):id is string=>id!==null);
  const toggle=(id:string)=>{
    if(selected.includes(id)){const next=selected.filter(value=>value!==id);context.patch({inspect_left_observation:next[0]??null,inspect_right_observation:next[1]??null});setMessage(null);return;}
    if(selected.length===2){setMessage('请选择两次观察；更换时先取消其中一次。');return;}
    const next=[...selected,id];context.patch({inspect_left_observation:next[0]??null,inspect_right_observation:next[1]??null});setMessage(null);
  };
  return <ResearchPanel title="观察轨迹" actions={<EvidenceLink work={work}>查看作品</EvidenceLink>}>
    <div className={styles.toolbar}><MetricPicker value={metric} onChange={value=>context.patch({inspect_metric:value})}/>{read.data&&<Cutoff value={read.data.knowledgeCutoff}/>}</div>
    <p className={styles.meta}>同一作品在不同时间、不同入口的真实观察。勾选两次，核对当时实际取得的值。</p>
    <ReadFeedback read={read}/>{message&&<p role="status" className={styles.notice}>{message}</p>}
    {read.data&&<div className={styles.tableScroll} tabIndex={0} aria-label="作品观察记录，可横向滚动"><table className={styles.table}>
      <colgroup><col style={{width:'6%'}}/><col style={{width:'22%'}}/><col style={{width:'28%'}}/><col style={{width:'16%'}}/><col style={{width:'14%'}}/><col style={{width:'14%'}}/></colgroup>
      <thead><tr><th>选择</th><th>观察时间</th><th>入口与来源</th><th>{metricLabels[metric]}</th><th>采样位置</th><th>记录情况</th></tr></thead>
      <tbody>{read.data.observations.map(item=><tr key={item.observationRef} data-selected={selected.includes(item.observationRef)}>
        <td><input className={styles.check} type="checkbox" aria-label={`选择 ${moment(item.observedAt)} 的观察`} checked={selected.includes(item.observationRef)} onChange={()=>toggle(item.observationRef)}/></td>
        <td><time className={styles.moment}>{moment(item.observedAt)}</time></td>
        <td><span className={styles.title}>{item.targetName??sourceName(item.sourceKind)}</span><div className={styles.subline}>{sourceName(item.sourceKind)}{item.ruleSlot?` · ${ruleLabels[item.ruleSlot]??'历史规则'}`:''}</div></td>
        <td><span className={styles.value}>{metricValue(item.metrics[metric])}</span><div className={styles.meta}>{item.metrics[metric].precision==='exact'?'来源明确计数':item.metrics[metric].state==='known'?'来源报告值':'本次未取得'}</div></td>
        <td>{item.position??'—'}</td>
        <td><span className={styles.tag}>{item.lateArrival?'后补记录':'实际观察'}</span></td>
      </tr>)}</tbody>
    </table>{read.data.observations.length===0&&<p className={styles.empty}>当前范围没有可读取的观察记录；不据此将指标写成零。</p>}</div>}
    <div className={styles.footer}><span className={styles.meta}>已选 {selected.length} / 2 次</span><div className={styles.rowActions}>
      <ActionButton variant="quiet" disabled={offset===0} onClick={()=>context.patch({inspect_trace_offset:String(Math.max(0,offset-50))})}>上一页</ActionButton>
      <ActionButton variant="quiet" disabled={read.data?.nextOffset==null} onClick={()=>{if(read.data?.nextOffset!=null)context.patch({inspect_trace_offset:String(read.data.nextOffset)});}}>下一页</ActionButton>
    </div></div>
    <ReadFeedback read={compared}/>{compared.data&&<div className={styles.notice}>
      <p><span className={styles.meta}>{moment(compared.data.left.observedAt)} → {moment(compared.data.right.observedAt)}</span><br/>{metricLabels[metric]} {metricValue(compared.data.left.metrics[metric])} → {metricValue(compared.data.right.metrics[metric])}</p>
      <Delta value={compared.data.metrics[metric]}/>
    </div>}
    <p className={styles.meta}>采样位置仅属于对应入口与轮次，不连成跨入口排名趋势。重新读取已有详情，不增加作品数或已取详情篇数。</p>
  </ResearchPanel>;
}
