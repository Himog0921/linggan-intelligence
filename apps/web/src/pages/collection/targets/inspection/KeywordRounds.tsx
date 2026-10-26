import { previousRound } from './useObservationRounds';
import { useEffect } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { InspectorSelect as TargetSelect } from './InspectorSelect';
import { decodeRoundComparison,decodeRoundResult,decodeRounds,inspectionUrl,METRICS,type MetricKey,type Round,type RoundWork } from './inspectionApi';
import { discoveryLabels,metricLabels,metricValue,moment,number,reason,ruleLabels } from './inspectionPresent';
import { Cutoff,Delta,MetricPicker,ReadFeedback } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';

export function KeywordRounds(){
  const context=useTargetResearch();const rules=context.data.rules??[];
  const rule=context.params.get('inspect_rule')||null;
  const comparing=context.params.get('inspect_round_view')==='compare';
  const offset=Math.max(0,Number(context.params.get('inspect_round_offset')??'0')||0);
  const page=useInspectionRead(inspectionUrl(context.scope,'rounds',{rule,offset}),decodeRounds);
  const left=context.params.get('inspect_left_round');const right=context.params.get('inspect_right_round');
  const result=useInspectionRead(right&&!comparing?inspectionUrl(context.scope,`rounds/${right}`):null,decodeRoundResult);
  const compared=useInspectionRead(left&&right&&comparing?inspectionUrl(context.scope,'round-comparison',{left,right}):null,decodeRoundComparison);
  const rawMetric=context.params.get('inspect_metric');const metric:MetricKey=METRICS.includes(rawMetric as MetricKey)?rawMetric as MetricKey:'likes';
  useEffect(()=>{
    if(!page.data)return;
    const updates:Record<string,string|null>={};
    if(!context.params.has('inspect_as_of'))updates.inspect_as_of=page.data.knowledgeCutoff;
    if(rule&&!context.params.has('inspect_rule'))updates.inspect_rule=rule;
    if(!right&&page.data.rounds[0])updates.inspect_right_round=page.data.rounds[0].roundRef;
    if(comparing&&!left){const newest=page.data.rounds.find(item=>item.roundRef===(right??page.data?.rounds[0]?.roundRef));
      const previous=previousRound(page.data.rounds,newest);
      if(previous)updates.inspect_left_round=previous.roundRef;}
    if(Object.keys(updates).length)context.patch(updates,true);
  },[page.data,rule,right,left,comparing,context]);
  const known=new Map((page.data?.rounds??[]).map(item=>[item.roundRef,item]));
  for(const item of [result.data?.round,compared.data?.left,compared.data?.right])if(item)known.set(item.roundRef,item);
  const options=[...known.values()].sort((a,b)=>b.observedEpochMs-a.observedEpochMs).map(item=>({value:item.roundRef,label:`${moment(item.observedAt)}${item.coverage==='complete'?'':' · 部分／未知覆盖'}`}));
  const refresh=()=>context.patch({inspect_as_of:null,inspect_left_round:null,inspect_right_round:null,inspect_round_offset:null});
  return <section className={styles.root} aria-label="关键词轮次观察">
    <div className={styles.toolbar}>
      {rules.length>0&&<TargetSelect name="inspection-rule" label="观察规则" value={rule??''} onChange={value=>context.patch({inspect_rule:value,inspect_left_round:null,inspect_right_round:null,inspect_round_offset:null})} options={[{value:'',label:'全部规则'},...rules.map(item=>({value:item.ruleRef,label:ruleLabels[item.slot]??'历史规则'}))]}/>}
      <ActionButton variant="quiet" aria-pressed={!comparing} onClick={()=>context.patch({inspect_round_view:'latest'})}>本轮结果</ActionButton>
      <ActionButton variant="quiet" aria-pressed={comparing} onClick={()=>context.patch({inspect_round_view:'compare'})}>与上轮对照</ActionButton>
      <ActionButton variant="quiet" onClick={refresh}>读取最新轮次</ActionButton>
    </div>
    <ReadFeedback read={page}/>
    {options.length>0&&<div className={styles.toolbar}>
      {comparing&&<TargetSelect name="inspection-left-round" label="上轮" value={left??''} onChange={value=>context.patch({inspect_left_round:value||null})} options={[{value:'',label:'选择较早观察'},...options.map(option=>({...option,disabled:option.value===right||(known.get(option.value)?.observedEpochMs??Infinity)>(known.get(right??'')?.observedEpochMs??-Infinity)}))]}/>}
      <TargetSelect name="inspection-right-round" label="本轮" value={right??''} onChange={value=>context.patch({inspect_right_round:value||null,inspect_left_round:null})} options={[{value:'',label:'选择本轮'},...options]}/>
      <MetricPicker value={metric} onChange={value=>context.patch({inspect_metric:value})}/>
    </div>}
    {page.data?.rounds.length===0&&<p className={styles.empty}>尚无可还原轮次，可查看作品目录。</p>}
    {comparing&&!left&&right&&<p className={styles.notice}>当前可选轮次中，未找到本轮之前的同规则观察。可更换本轮或查看“本轮结果”。</p>}
    {comparing?<><ReadFeedback read={compared}/>{compared.data&&<>
      <div className={styles.inlineSummary}><span>共同命中 <b>{number(compared.data.commonCount)}</b></span><span>本轮新出现 <b>{number(compared.data.enteredCount)}</b></span><span>本轮未命中 <b>{number(compared.data.absentCount)}</b></span><Cutoff value={compared.data.knowledgeCutoff}/></div>
      {compared.data.reasons.length>0&&<div className={styles.notice}><p>{compared.data.reasons.map(reason).join('；')}。保留两侧原值。</p></div>}
      <RoundConditions left={compared.data.left} right={compared.data.right}/>
      <div className={styles.tableScroll} tabIndex={0} aria-label="关键词两轮作品对照，可横向滚动"><table className={styles.table}>
        <colgroup><col style={{width:'36%'}}/><col style={{width:'22%'}}/><col style={{width:'13%'}}/><col style={{width:'13%'}}/><col style={{width:'16%'}}/></colgroup>
        <thead><tr><th>作品</th><th>本轮情况</th><th>上轮{metricLabels[metric]}</th><th>本轮{metricLabels[metric]}</th><th>变化</th></tr></thead>
        <tbody>{compared.data.rows.map(row=><tr key={row.workRef}>
          <td><button id={`round-work-${row.workRef}-trace`} type="button" className={styles.titleButton} onClick={()=>context.open('trace',row.workRef)}>{row.right?.title??row.left?.title??'标题未取得'}</button><div className={styles.subline}>{row.right?.creator??row.left?.creator??'这两轮未记录作者'}<ActionButton variant="quiet" onClick={()=>context.toggle(row.workRef)}>{context.selected.includes(row.workRef)?'移出对照':'加入对照'}</ActionButton></div></td>
          <td><span className={styles.tag} data-new={row.discoveryLabel==='new_in_library'}>{discoveryLabels[row.discoveryLabel]??'范围待核对'}</span><div className={styles.meta}>采样位置 {row.left?.position??'—'} → {row.right?.position??'—'}</div></td>
          <td className={styles.value}>{row.left?metricValue(row.left.metrics[metric]):'未命中'}</td><td className={styles.value}>{row.right?metricValue(row.right.metrics[metric]):'未命中'}</td><td><Delta value={row.metrics[metric]}/></td>
        </tr>)}</tbody>
      </table></div>
    </>}</>:<><ReadFeedback read={result}/>{result.data&&<>
      <div className={styles.inlineSummary}><span>本轮命中 <b>{result.data.round.hits}</b></span><span>{result.data.round.coverage==='complete'?'本轮范围已结束':'本轮覆盖不完整'}</span><Cutoff value={result.data.knowledgeCutoff}/></div>
      <RoundConditions right={result.data.round}/>
      {result.data.truncated&&<p className={styles.notice}>当前仅展示有界部分作品，不将此名单当成完整结果。</p>}
      <SingleRound rows={result.data.works} metric={metric}/>
    </>}</>}
    {page.data&&<div className={styles.footer}><span className={styles.meta}>未命中不表示删除。</span><div className={styles.rowActions}>
      <ActionButton variant="quiet" disabled={offset===0} onClick={()=>context.patch({inspect_round_offset:String(Math.max(0,offset-50)),inspect_right_round:null,inspect_left_round:null})}>较新轮次</ActionButton>
      <ActionButton variant="quiet" disabled={page.data.nextOffset===null} onClick={()=>{if(page.data?.nextOffset!=null)context.patch({inspect_round_offset:String(page.data.nextOffset),inspect_right_round:null,inspect_left_round:null});}}>更早轮次</ActionButton>
    </div></div>}
  </section>;
}

function SingleRound({rows,metric}:{rows:RoundWork[];metric:MetricKey}){
  const context=useTargetResearch();
  return <div className={styles.tableScroll} tabIndex={0} aria-label="本轮命中作品，可横向滚动"><table className={styles.table}><thead><tr><th style={{width:'44%'}}>作品</th><th>采样位置</th><th>{metricLabels[metric]}</th><th style={{width:'25%'}}>来源情况</th></tr></thead><tbody>{rows.map(row=><tr key={row.workRef}>
    <td><button id={`round-work-${row.workRef}-trace`} type="button" className={styles.titleButton} onClick={()=>context.open('trace',row.workRef)}>{row.title??'标题未取得'}</button><div className={styles.subline}>{row.creator??'本轮未记录作者'}<ActionButton variant="quiet" onClick={()=>context.toggle(row.workRef)}>{context.selected.includes(row.workRef)?'移出对照':'加入对照'}</ActionButton></div></td>
    <td>{row.position??'—'}</td><td className={styles.value}>{metricValue(row.metrics[metric])}</td><td><span className={styles.tag}>{row.libraryPresence==='new'?'新入库':row.firstSeenInScope===true&&row.libraryPresence==='existing'?'库内已有 · 本规则首次发现':row.firstSeenInScope===false?'再次命中':'首次范围未能确认'}</span></td>
  </tr>)}</tbody></table>{rows.length===0&&<p className={styles.empty}>本轮没有取得合格作品；覆盖情况以上方实际回执为准。</p>}</div>;
}
function RoundConditions({left,right}:{left?:Round;right:Round}){
  const show=(round:Round)=>{const c=round.conditions;const val=(key:string)=>typeof c[key]==='number'||typeof c[key]==='string'?String(c[key]):'未记录';return `${typeof c.ranking==='string'?(ruleLabels[c.ranking]??c.ranking):'排序未记录'} · 下拉 ${val('scrollRounds')} · 取样上限 ${val('maximumQuota')} · 发布时间范围 ${val('publishedWithinDays')} 天`;};
  return <details className={styles.conditions}><summary>查看两轮实际口径与覆盖</summary><dl>{left&&<><dt>上轮</dt><dd>{moment(left.observedAt)} · {show(left)}</dd></>}<dt>本轮</dt><dd>{moment(right.observedAt)} · {show(right)}</dd></dl></details>;
}
