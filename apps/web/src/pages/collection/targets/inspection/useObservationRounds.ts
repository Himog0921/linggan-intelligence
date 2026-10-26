import { useEffect } from 'react';
import { decodeRoundComparison,decodeRoundResult,decodeRounds,inspectionUrl,type Round } from './inspectionApi';
import { moment,ruleLabels } from './inspectionPresent';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';

export function previousRound(rounds:Round[],current:Round|undefined){
  if(!current?.ruleRef)return undefined;
  return rounds.find(item=>item.roundRef!==current.roundRef&&item.ruleRef===current.ruleRef&&item.observedEpochMs<current.observedEpochMs);
}

/** One scope and material cutoff for the list, selected result and preceding same-rule comparison. */
export function useObservationRounds(){
  const context=useTargetResearch();
  const rule=context.params.get('inspect_rule')||null;
  const offset=Math.max(0,Number(context.params.get('inspect_round_offset')??'0')||0);
  const page=useInspectionRead(inspectionUrl(context.scope,'rounds',{rule,offset}),decodeRounds);
  const right=context.params.get('inspect_right_round');
  const result=useInspectionRead(right?inspectionUrl(context.scope,`rounds/${right}`):null,decodeRoundResult);
  const current=result.data?.round??page.data?.rounds.find(item=>item.roundRef===right);
  const previous=previousRound(page.data?.rounds??[],current);
  const compared=useInspectionRead(right&&previous?inspectionUrl(context.scope,'round-comparison',{left:previous.roundRef,right}):null,decodeRoundComparison);
  useEffect(()=>{
    if(!page.data)return;
    const updates:Record<string,string|null>={};
    if(!context.params.has('inspect_as_of'))updates.inspect_as_of=page.data.knowledgeCutoff;
    if(!right&&page.data.rounds[0])updates.inspect_right_round=page.data.rounds[0].roundRef;
    if(Object.keys(updates).length)context.patch(updates,true);
  },[page.data,right,context]);
  const known=new Map((page.data?.rounds??[]).map(item=>[item.roundRef,item]));
  if(current)known.set(current.roundRef,current);
  const options=[...known.values()].sort((a,b)=>b.observedEpochMs-a.observedEpochMs).map(item=>({value:item.roundRef,
    label:`${moment(item.observedAt)} · ${ruleLabels[item.slot??'']??'规则未记录'}${item.coverage==='complete'?'':' · 部分／未知覆盖'}`}));
  return {page,result,compared,previous,options,rule,offset,right};
}
