import { useEffect,useState,type CSSProperties } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { MediaImage } from '../../../corpus/evidence/Media';
import { limitationCopy } from '../../../corpus/evidence/evidencePresent';
import { TargetSelect } from '../TargetSelect';
import { decodeWorkComparison,inspectionUrl,METRICS,type MetricKey,type Snapshot } from './inspectionApi';
import { age,metricLabels,metricValue,moment,number,reason } from './inspectionPresent';
import { Cutoff,EvidenceLink,MetricPicker,ReadFeedback,ResearchPanel } from './ResearchParts';
import { DossierReadout } from './DossierReadout';
import { ContentComparisonPanel } from './ContentComparisonPanel';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';

export function WorkComparisonPanel(){
  const context=useTargetResearch();const [message,setMessage]=useState<string|null>(null);
  const mode=context.params.get('inspect_mode')==='age_24h'?'age_24h':'current';
  const rawMetric=context.params.get('inspect_metric');
  const metric:MetricKey=METRICS.includes(rawMetric as MetricKey)?rawMetric as MetricKey:'likes';
  const read=useInspectionRead(context.selected.length>=2?inspectionUrl(context.scope,'work-comparison',{works:context.selected.join(','),mode}):null,decodeWorkComparison);
  const data=read.data;
  useEffect(()=>{if(data)context.pinCutoff(data.knowledgeCutoff);},[data,context]);
  const replace=(index:number,value:string)=>{
    if(context.selected.some((id,at)=>id===value&&at!==index)){setMessage('这篇已经在对照中，请选择其他作品。');return;}
    const next=[...context.selected];next[index]=value;setMessage(null);context.patch({inspect_works:next.join(','),inspect_comparison_request:null});
  };
  const references=data?.referenceStatistics[metric];
  return <ResearchPanel title="作品并排对照" actions={<div className={styles.modes}>
    <ActionButton variant="quiet" aria-pressed={mode==='current'} onClick={()=>context.patch({inspect_mode:'current'})}>当前累计</ActionButton>
    <ActionButton variant="quiet" aria-pressed={mode==='age_24h'} onClick={()=>context.patch({inspect_mode:'age_24h'})}>发布后 24 小时</ActionButton>
  </div>}>
    <div className={styles.toolbar}><MetricPicker value={metric} onChange={next=>context.patch({inspect_metric:next})}/>{data&&<Cutoff value={data.knowledgeCutoff}/>}</div>
    <p className={styles.meta}>{mode==='current'?'并列各作品截至标注时点的累计值，不是同期成绩。':'使用发布后 24 小时附近的真实观察；每篇实际年龄单独标注，不推算缺失记录。'}</p>
    {message&&<div role="status" className={styles.notice}>{message}</div>}
    <ReadFeedback read={read}/>
    {context.selected.length<2?<div className={styles.empty}><strong>还需选择一篇参照作品</strong><p>返回列表或分布图继续选择，已选作品会保留。</p><ActionButton onClick={context.back}>返回选择</ActionButton></div>:data&&<>
      <div className={styles.compareScroll} tabIndex={0} aria-label="作品并排对照，可横向滚动">
        <div className={styles.compareGrid} style={{'--compare-columns':data.items.length} as CSSProperties}>
          <div className={styles.compareRow}>{data.items.map((item,index)=><div className={styles.compareCell} key={item.workRef}>
            <div className={styles.footer}><span className={styles.meta}>{index===0?'当前作品':`参照 ${index}`}</span><ActionButton variant="quiet" onClick={()=>context.toggle(item.workRef)}>移除</ActionButton></div>
            <WorkIntroduction item={item}/>
            {context.data.catalog&&<TargetSelect name={`replace-${index}`} label={`替换${index===0?'当前作品':'参照'}`} value={item.workRef} onChange={value=>replace(index,value)}
              options={context.data.catalog.map(work=>({value:work.publicRef,label:work.title??'标题未取得'}))}/>}
          </div>)}</div>
          <div className={styles.compareLabel}>{metricLabels[metric]} · {mode==='current'?'各自标注时点累计':'同发布年龄'}</div>
          <div className={styles.compareRow}>{data.items.map((item,index)=><div className={styles.compareCell} key={item.workRef}>
            <div className={styles.metricValue} data-focus={index===0}>{metricValue(item.metrics[metric])}</div>
            <div className={styles.meta}><time className={styles.moment}>{moment(item.metrics[metric].observedAt)}</time><br/>{age(item.metrics[metric],item.publishedEpochMs)}</div>
            {item.reasons.map(code=><p className={styles.meta} key={code}>{reason(code)}</p>)}
          </div>)}</div>
          <div className={styles.compareLabel}>其他指标</div>
          <div className={styles.compareRow}>{data.items.map(item=><div className={styles.compareCell} key={item.workRef}>{METRICS.filter(key=>key!==metric).map(key=><div className={styles.footer} key={key}><span className={styles.meta}>{metricLabels[key]}</span><span className={styles.value} title={moment(item.metrics[key].observedAt)}>{metricValue(item.metrics[key])}</span></div>)}</div>)}</div>
          <div className={styles.compareLabel}>已取得内容与来源</div>
          <div className={styles.compareRow}>{data.items.map(item=><div className={styles.compareCell} key={item.workRef}>
            {item.resource&&<DossierReadout work={item.workRef} cutoff={data.knowledgeCutoff}/>}<p><EvidenceLink work={item.workRef}/></p>
          </div>)}</div>
        </div>
      </div>
      {references&&<div className={styles.inlineSummary}><span>所选参照的{metricLabels[metric]}中位数 <b>{number(references.median)}</b></span><span>可用参照 <b>{references.count}</b> 篇</span><span>不含第一列；仅说明所选作品，不代表作者整体规律。</span></div>}
      {context.data.target.kind==='creator'&&<ContentComparisonPanel/>}
      <div className={styles.footer}><ActionButton variant="quiet" onClick={context.back}>返回原列表或图表</ActionButton><ActionButton variant="quiet" onClick={read.reload}>重读固定截点</ActionButton></div>
    </>}
  </ResearchPanel>;
}

export function WorkIntroduction({item}:{item:Snapshot}){
  const resource=item.resource;const title=resource?.display.title??'标题未取得';
  const limitation=resource?.primaryLimitation;
  return <div className={styles.intro}>
    <MediaImage className={styles.cover} url={resource?.media.coverUrl??null} showStatus fallback="封面未取得"
      names={{loaded:`${title}的封面`,loading:'封面读取中',failed:'封面读取失败',absent:'封面未取得'}}/>
    <div><h3>{title}</h3><p className={styles.meta}>{resource?.display.creatorName??'作者待核对'}</p><time className={`${styles.meta} ${styles.moment}`}>{moment(resource?.display.publishedAt??null)}</time>
      {!resource&&<p className={styles.meta}>当前范围内不可读取，其他作品仍可对照。</p>}
      {limitation&&limitation!=='NONE'&&limitation!=='OTHER_LANES_NOT_EVALUATED'&&<p className={styles.meta}>{limitationCopy(limitation)}</p>}
    </div>
  </div>;
}
