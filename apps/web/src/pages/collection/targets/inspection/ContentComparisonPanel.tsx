import { useRef, useState } from 'react';
import { ApiError, requestJson, isRecord } from '../../../../shared/api';
import { ActionButton } from '../../../../shared/ActionButton';
import { DossierReadout, type DossierView, decodeDossier } from './DossierReadout';
import { inspectionUrl, array, flag, nullable, num, record, ref, str } from './inspectionApi';
import { EvidenceLink, ReadFeedback } from './ResearchParts';
import { useTargetResearch } from './TargetResearchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';

type Citation = {workRef:string;citationId:string};
type Claim = {kind:string;statement:string;targetCitations:Citation[];referenceCitations:Citation[]};
type Analysis = {relation:string;claims:Claim[];limitations:string[];partial:boolean;methodVersion:string};
type Preview = {
  targetRef:string;workRefs:string[];knowledgeCutoff:string;materialRevisions:string[];
  partial:boolean;taskConfigRef:string|null;taskRevision:number|null;
  canStart:boolean;reasons:string[];methodVersion:string;
};
type ContentRead = {
  requestRef:string;batchRef:string;knowledgeCutoff:string;
  state:string;waitReason:string|null;failureCode:string|null;
  result:Analysis|null;materials:DossierView[];outdated:boolean;methodVersion:string;
};
function decodePreview(v:unknown):Preview{
  const r=record(v);
  return {targetRef:ref(r.targetRef),workRefs:array(r.workRefs,ref),knowledgeCutoff:str(r.knowledgeCutoff),
    materialRevisions:array(r.materialRevisions,str),partial:flag(r.partial),
    taskConfigRef:nullable(r.taskConfigRef,ref),taskRevision:nullable(r.taskRevision,num),
    canStart:flag(r.canStart),reasons:array(r.reasons,str),methodVersion:str(r.methodVersion)};
}
function citation(v:unknown):Citation {const r=record(v);return {workRef:ref(r.workRef),citationId:str(r.citationId)};}
function claim(v:unknown):Claim {const r=record(v);return {kind:str(r.kind),statement:str(r.statement),
  targetCitations:array(r.targetCitations,citation),referenceCitations:array(r.referenceCitations,citation)};}
function decodeResult(v:unknown):ContentRead {
  const r=record(v);return {requestRef:ref(r.requestRef),batchRef:ref(r.batchRef),knowledgeCutoff:str(r.knowledgeCutoff),
    state:str(r.state),waitReason:nullable(r.waitReason,str),failureCode:nullable(r.failureCode,str),
    result:nullable(r.result,value=>{const a=record(value);return {relation:str(a.relation),claims:array(a.claims,claim),
      limitations:array(a.limitations,str),partial:flag(a.partial),methodVersion:str(a.methodVersion)};}),
    materials:array(r.materials,decodeDossier),outdated:flag(r.outdated),methodVersion:str(r.methodVersion)};
}
const kindNames:Record<string,string>={
  shared_point:'共同表达',added_step:'补充步骤',added_example:'补充例子',
  added_condition:'增加适用条件',added_counterexample:'补充反例',
  changed_scene:'换了场景',expression_only:'表达调整',different_problem:'讨论的问题不同',
};
const relationNames:Record<string,string>={
  mostly_repeated:'主要表达重复',supplemented:'有实质补充',changed_angle:'处理角度改变',
  different_problem:'问题并不相同',undetermined:'目前无法确定',
};
const help:Record<string,string>={
  task_not_connected:'尚未配置内容对照任务；仍可手动并排。',
  egress_not_approved:'当前未获准发送该范围材料；可继续查看现有底稿。',
  comparison_source_unavailable:'选中作品缺少足够可读材料；请先查看各篇底稿覆盖。',
  comparison_source_changed:'来源材料已更新，需重新预览后再提交。',
  comparison_requires_single_attempt:'当前任务重试策略不符合内容对照要求。',
  comparison_not_admitted:'服务器未接纳此次研究。',
};
function explanation(value:string):string{return help[value]??'当前研究暂不能执行；请在 AI 设置核对任务和授权。';}

export function ContentComparisonPanel(){
  const context=useTargetResearch();
  const works=context.selected;
  const params=context.params;
  const [requestRef,setRequestRef]=useState<string|null>(params.get('inspect_comparison_request'));
  const [pending,setPending]=useState(false);
  const [unknown,setUnknown]=useState(false);
  const [error,setError]=useState<string|null>(null);
  const command=useRef<string|null>(params.get('inspect_comparison_command'));
  const previousSelection=useRef(works.join(','));
  // A comparison belongs to the exact selected works, not a different later selection.
  if(previousSelection.current!==works.join(',')){
    previousSelection.current=works.join(',');
    command.current=null;
    if(requestRef!==null)setRequestRef(null);
    if(error!==null)setError(null);
    if(unknown)setUnknown(false);
  }
  const previewPath=works.length>=2
    ?inspectionUrl(context.scope,'content-comparison/preview',{works:works.join(',')}):null;
  const preview=useInspectionRead(previewPath,decodePreview);
  const readPath=requestRef
    ?inspectionUrl(context.scope,'content-comparison/requests/'+requestRef):null;
  const read=useInspectionRead(readPath,decodeResult);
  const matched=read.data?.materials.map(material=>material.workRef).join(',')===works.join(',');
  const begin=async()=>{
    if(!preview.data?.canStart||pending)return;
    const input=preview.data;
    if(input.workRefs.join(',')!==works.join(',')){setError('选择已改变，请重新读取研究准备信息。');return;}
    if(!input.taskConfigRef||input.taskRevision===null)return;
    const commandRef=command.current??crypto.randomUUID();
    command.current=commandRef;
    setPending(true);setError(null);
    try{
      const output=await requestJson(
        inspectionUrl(context.scope,'content-comparison/requests',{as_of:input.knowledgeCutoff}),
        {method:'POST',body:{
          commandRef,targetRef:input.targetRef,domainRef:context.scope.domain==='all'?null:context.scope.domain,
          workRefs:input.workRefs,knowledgeCutoff:input.knowledgeCutoff,
          expectedMaterialRevisions:input.materialRevisions,taskConfigRef:input.taskConfigRef,
          expectedTaskRevision:input.taskRevision,
        }}
      );
      if(!isRecord(output)||typeof output.requestRef!=='string'
        ||!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(output.requestRef))throw new Error('服务器结果不可确认');
      const id=output.requestRef;
      setRequestRef(id);setUnknown(false);command.current=null;
      context.patch({inspect_comparison_request:id,inspect_comparison_command:null});
    }catch(caught){
      if(caught instanceof ApiError&&caught.answered){
        const payload=isRecord(caught.responseBody)?caught.responseBody:null;
        const reasons=payload&&Array.isArray(payload.reasons)?payload.reasons.filter(v=>typeof v==='string') as string[]:[];
        setUnknown(false);command.current=null;
        setError(reasons.length?reasons.map(explanation).join('；'):'本次内容对照未被接纳，当前作品指标与原文仍可继续查看。');
      }else {
        // With an unknown response, keep the same command ID for an explicit retry.
        setUnknown(true);
        setError('提交结果尚未确认。重试将使用同一请求编号，不会创建另一份研究。');
      }
    }finally{setPending(false);}
  };
  const result=matched?read.data?.result:null;
  return <section className={styles.contentResearch} aria-label="内容新增与重复表达对照">
    <div className={styles.contentResearchHead}>
      <div><h3>内容新增与重复表达</h3><p className={styles.meta}>仅比较所选作品的已取得内容；与点赞、收藏等表现对照独立。</p></div>
      {!requestRef&&<ActionButton variant="outline" disabled={!preview.data?.canStart||pending||works.length<2} onClick={()=>{void begin();}}>
        {pending?'正在提交…':unknown?'按原请求核对':'比较内容'}</ActionButton>}
      {requestRef&&<ActionButton variant="quiet" onClick={read.reload}>读取研究结果</ActionButton>}
    </div>
    {!requestRef&&<ReadFeedback read={preview}/>}
    {preview.data?.partial&&<p className={styles.meta}>部分作品材料仍不完整；分析只可引用已取得内容。</p>}
    {!requestRef&&preview.data&&!preview.data.canStart&&<p className={styles.meta}>
      {preview.data.reasons.map(explanation).join('；')||'当前没有可用的模型研究设置。'} 手动并排对照不受影响。
    </p>}
    {error&&<p className={styles.notice} role={unknown?'status':'alert'}>{error}</p>}
    {requestRef&&<><ReadFeedback read={read}/>
      {matched&&read.data&&<>
        {read.data.outdated&&<p className={styles.notice}>研究基于当时的材料版本；已有材料更新，可重新选择作品并发起新的比较。</p>}
        <p className={styles.meta}>{read.data.state==='succeeded'?'研究完成':read.data.state==='failed'?'研究失败':read.data.state==='cancelled'?'研究已取消':'研究已提交，等待任务执行'}
          {read.data.waitReason?' · 等待原因已记录':''}
          {read.data.failureCode?' · 当前未得到可发布的对照结果':''}</p>
        {result&&<><strong>{relationNames[result.relation]??'内容关系尚不确定'}</strong>
          {result.claims.length===0?<p className={styles.meta}>没有可核验的双侧内容差异结论。</p>:result.claims.map((item,index)=>
            <article className={styles.comparisonClaim} key={index}>
              <span className={styles.tag}>{kindNames[item.kind]??'内容差异'}</span>
              <p>{item.statement}</p>
              <div className={styles.claimSources}>
                <div><b>当前作品依据</b>{item.targetCitations.map((citation,i)=><CitationExcerpt key={i} citation={citation} materials={read.data!.materials}/>)}</div>
                <div><b>参照作品依据</b>{item.referenceCitations.map((citation,i)=><CitationExcerpt key={i} citation={citation} materials={read.data!.materials}/>)}</div>
              </div>
            </article>)}
          {result.limitations.map((limitation,index)=><p key={index} className={styles.meta}>{limitation}</p>)}
        </>}
        {read.data.materials.map(material=><DossierReadout key={material.workRef} work={material.workRef}/>)}
      </>}
      {read.data&&!matched&&<p className={styles.notice}>本次研究所属作品已发生变化；旧请求结果不用于当前选择。</p>}
    </>}
  </section>;
}
function CitationExcerpt({citation,materials}:{citation:Citation;materials:DossierView[]}){
  const source=materials.find(m=>m.workRef===citation.workRef)?.fragments.find(f=>f.citationId===citation.citationId);
  if(!source)return null;
  return <div className={styles.claimExcerpt}><blockquote>{source.text}</blockquote><EvidenceLink work={citation.workRef}>核对来源</EvidenceLink></div>;
}
