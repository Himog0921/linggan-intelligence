import { useEffect,useRef,type ReactNode } from 'react';
import { Link } from 'react-router';
import { ActionButton } from '../../../../shared/ActionButton';
import { InspectorSelect as TargetSelect } from './InspectorSelect';
import type { InspectionRead } from './useInspectionRead';
import { METRICS,type MetricKey,type Difference } from './inspectionApi';
import { duration,metricLabels,moment,number,reason } from './inspectionPresent';
import { useTargetResearch } from './researchContext';
import styles from './TargetResearch.module.css';

export function ResearchPanel({title,actions,children}:{title:string;actions?:ReactNode;children:ReactNode}){
  const context=useTargetResearch();const heading=useRef<HTMLHeadingElement>(null);
  useEffect(()=>{heading.current?.focus();},[title]);
  return <section className={styles.root} aria-label={title} onKeyDown={event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();context.back();}}}>
    <header className={styles.panelHead}><ActionButton variant="quiet" onClick={context.back}>返回</ActionButton><h2 tabIndex={-1} ref={heading}>{title}</h2>{actions}</header>
    {children}
  </section>;
}
export function ReadFeedback<T>({read}:{read:InspectionRead<T>}){
  if(read.error)return <div className={styles.notice} data-tone="error" role="alert"><p>{read.error}{read.stale?' 以下保留上次读取结果。':''}</p><ActionButton variant="quiet" onClick={read.reload}>重新读取</ActionButton></div>;
  if(read.pending)return <p className={styles.meta} role="status">{read.data?'正在刷新，以下为上次结果…':'正在读取真实记录…'}</p>;
  return null;
}
export function MetricPicker({value,onChange}:{value:MetricKey;onChange:(value:MetricKey)=>void}){
  return <TargetSelect name="inspection-metric" label="比较指标" value={value} onChange={next=>{if(METRICS.includes(next as MetricKey))onChange(next as MetricKey);}} options={METRICS.map(key=>({value:key,label:metricLabels[key]}))}/>;
}
export function Delta({value}:{value:Difference}){
  return <div><b className={styles.delta} data-positive={value.delta!==null&&value.delta>0}>{value.comparable&&value.delta!==null?`${value.delta>0?'↑ ':value.delta<0?'↓ ':''}${number(Math.abs(value.delta))}`:'—'}</b>
    <div className={styles.meta}>{value.comparable?duration(value.elapsedSeconds):value.reason?reason(value.reason):'暂不能比较'}</div></div>;
}
export function EvidenceLink({work,children='查看原文'}:{work:string;children?:ReactNode}){
  const {scope}=useTargetResearch();const query=new URLSearchParams({work});if(scope.domain!=='all')query.set('domain',scope.domain);
  return <Link className={styles.link} to={`/corpus/evidence?${query}`} target="_blank" rel="noopener noreferrer" aria-label="在新标签页打开原文，保留当前对照">{children} ↗</Link>;
}
export function Cutoff({value}:{value:string}){return <span className={styles.cutoff}>材料截点 <time className={styles.moment}>{moment(value)}</time></span>;}
