import { isRecord } from '../../../../shared/api';
import { parseWorkItem } from '../../../corpus/evidence/evidenceDecode';
import type { WorkItem } from '../../../corpus/evidence/evidenceTypes';

export type MetricKey = 'likes' | 'comments' | 'collects' | 'shares';
export const METRICS: readonly MetricKey[] = ['likes','comments','collects','shares'];
export interface Metric { value:number|null; state:string; observationRef:string|null; observedAt:string|null; observedEpochMs:number|null; sourceKind:string|null; precision:string }
export type Metrics = Record<MetricKey,Metric>;
export interface Difference { left:Metric; right:Metric; delta:number|null; elapsedSeconds:number|null; percent:number|null; comparable:boolean; reason:string|null }
export type Differences = Record<MetricKey,Difference>;
export interface Round { roundRef:string; packageRef:string; ruleRef:string|null; ruleRevisionRef:string|null; slot:string|null; observedAt:string; observedEpochMs:number; acceptedAt:string; coverage:string; hits:number; conditions:Record<string,unknown> }
export interface RoundPage { targetRef:string; domainRef:string|null; knowledgeCutoff:string; methodVersion:string; rounds:Round[]; nextOffset:number|null }
export interface RoundWork { workRef:string; title:string|null; creator:string|null; position:number|null; metrics:Metrics; firstSeenInScope:boolean|null; libraryPresence:string; seenBeforeLeft:boolean }
export interface RoundResult { targetRef:string; knowledgeCutoff:string; methodVersion:string; round:Round; works:RoundWork[]; truncated:boolean }
export interface ComparedRow { workRef:string; membership:string; discoveryLabel:string; left:RoundWork|null; right:RoundWork|null; metrics:Differences }
export interface RoundComparison { targetRef:string; knowledgeCutoff:string; methodVersion:string; left:Round; right:Round; comparisonState:string; reasons:string[]; rows:ComparedRow[]; commonCount:number|null; enteredCount:number|null; absentCount:number|null; truncated:boolean }
export interface Observation { observationRef:string; packageRef:string; roundRef:string; targetRef:string|null; targetName:string|null; ruleSlot:string|null; sourceKind:string; observedAt:string; observedEpochMs:number; acceptedAt:string; metrics:Metrics; position:number|null; lateArrival:boolean }
export interface TracePage { workRef:string; knowledgeCutoff:string; observations:Observation[]; nextOffset:number|null; methodVersion:string }
export interface ObservationComparison { workRef:string; knowledgeCutoff:string; methodVersion:string; left:Observation; right:Observation; metrics:Differences }
export type WorkMode = 'current' | 'age_24h';
export interface Snapshot { workRef:string; status:string; resource:WorkItem|null; metrics:Metrics; publishedEpochMs:number|null; reasons:string[] }
export interface WorkComparison { targetRef:string; knowledgeCutoff:string; methodVersion:string; mode:WorkMode; ageHours:number|null; toleranceMinutes:number|null; items:Snapshot[]; referenceStatistics:Record<MetricKey,{median:number|null;count:number}> }
export interface ReadScope { targetRef:string; domain:string; cutoff?:string|null }

export function record(value:unknown):Record<string,unknown> { if(!isRecord(value))throw new Error('检查器返回内容无法解析');return value; }
export function str(value:unknown):string {if(typeof value!=='string')throw new Error('检查器文字字段无效');return value;}
export function num(value:unknown):number {if(typeof value!=='number'||!Number.isFinite(value))throw new Error('检查器数字字段无效');return value;}
export function flag(value:unknown):boolean {if(typeof value!=='boolean')throw new Error('检查器状态字段无效');return value;}
export function nullable<T>(value:unknown,decode:(v:unknown)=>T):T|null {return value===null?null:decode(value);}
export function array<T>(value:unknown,decode:(v:unknown)=>T):T[]{if(!Array.isArray(value))throw new Error('检查器集合字段无效');return value.map(decode);}
export function ref(value:unknown):string {const id=str(value);if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id))throw new Error('作品引用无效');return id;}
function metric(value:unknown):Metric {const r=record(value);const state=str(r.state);const count=nullable(r.value,num);if(!['known','unknown'].includes(state)||(state==='known'&&(count===null||!Number.isSafeInteger(count)||count<0))||(state==='unknown'&&count!==null))throw new Error('指标数值与状态不一致');return {value:count,state,observationRef:nullable(r.observationRef,str),observedAt:nullable(r.observedAt,str),observedEpochMs:nullable(r.observedEpochMs,num),sourceKind:nullable(r.sourceKind,str),precision:str(r.precision)};}
function metrics(value:unknown):Metrics {const r=record(value);return {likes:metric(r.likes),comments:metric(r.comments),collects:metric(r.collects),shares:metric(r.shares)};}
function difference(value:unknown):Difference {const r=record(value);return {left:metric(r.left),right:metric(r.right),delta:nullable(r.delta,num),elapsedSeconds:nullable(r.elapsedSeconds,num),percent:nullable(r.percent,num),comparable:flag(r.comparable),reason:nullable(r.reason,str)};}
function differences(value:unknown):Differences {const r=record(value);return {likes:difference(r.likes),comments:difference(r.comments),collects:difference(r.collects),shares:difference(r.shares)};}
export function round(value:unknown):Round {const r=record(value);return {roundRef:ref(r.roundRef),packageRef:ref(r.packageRef),ruleRef:nullable(r.ruleRef,ref),ruleRevisionRef:nullable(r.ruleRevisionRef,ref),slot:nullable(r.slot,str),observedAt:str(r.observedAt),observedEpochMs:num(r.observedEpochMs),acceptedAt:str(r.acceptedAt),coverage:str(r.coverage),hits:num(r.hits),conditions:record(r.conditions)};}
export function decodeRounds(value:unknown):RoundPage {const r=record(value);return {targetRef:ref(r.targetRef),domainRef:nullable(r.domainRef,ref),knowledgeCutoff:str(r.knowledgeCutoff),methodVersion:str(r.methodVersion),rounds:array(r.rounds,round),nextOffset:nullable(r.nextOffset,num)};}
function roundWork(value:unknown):RoundWork {const r=record(value);return {workRef:ref(r.workRef),title:nullable(r.title,str),creator:nullable(r.creator,str),position:nullable(r.position,num),metrics:metrics(r.metrics),firstSeenInScope:nullable(r.firstSeenInScope,flag),libraryPresence:str(r.libraryPresence),seenBeforeLeft:flag(r.seenBeforeLeft)};}
export function decodeRoundResult(value:unknown):RoundResult {const r=record(value);return {targetRef:ref(r.targetRef),knowledgeCutoff:str(r.knowledgeCutoff),methodVersion:str(r.methodVersion),round:round(r.round),works:array(r.works,roundWork),truncated:flag(r.truncated)};}
function comparedRow(value:unknown):ComparedRow {const r=record(value);return {workRef:ref(r.workRef),membership:str(r.membership),discoveryLabel:str(r.discoveryLabel),left:nullable(r.left,roundWork),right:nullable(r.right,roundWork),metrics:differences(r.metrics)};}
export function decodeRoundComparison(value:unknown):RoundComparison {const r=record(value);return {targetRef:ref(r.targetRef),knowledgeCutoff:str(r.knowledgeCutoff),methodVersion:str(r.methodVersion),left:round(r.left),right:round(r.right),comparisonState:str(r.comparisonState),reasons:array(r.reasons,str),rows:array(r.rows,comparedRow),commonCount:nullable(r.commonCount,num),enteredCount:nullable(r.enteredCount,num),absentCount:nullable(r.absentCount,num),truncated:flag(r.truncated)};}
function observation(value:unknown):Observation {const r=record(value);return {observationRef:ref(r.observationRef),packageRef:ref(r.packageRef),roundRef:ref(r.roundRef),targetRef:nullable(r.targetRef,ref),targetName:nullable(r.targetName,str),ruleSlot:nullable(r.ruleSlot,str),sourceKind:str(r.sourceKind),observedAt:str(r.observedAt),observedEpochMs:num(r.observedEpochMs),acceptedAt:str(r.acceptedAt),metrics:metrics(r.metrics),position:nullable(r.position,num),lateArrival:flag(r.lateArrival)};}
export function decodeTrace(value:unknown):TracePage {const r=record(value);return {workRef:ref(r.workRef),knowledgeCutoff:str(r.knowledgeCutoff),observations:array(r.observations,observation),nextOffset:nullable(r.nextOffset,num),methodVersion:str(r.methodVersion)};}
export function decodeObservationComparison(value:unknown):ObservationComparison {const r=record(value);return {workRef:ref(r.workRef),knowledgeCutoff:str(r.knowledgeCutoff),methodVersion:str(r.methodVersion),left:observation(r.left),right:observation(r.right),metrics:differences(r.metrics)};}
function snapshot(value:unknown):Snapshot {const r=record(value);const workRef=ref(r.workRef);const resource=nullable(r.resource,v=>parseWorkItem(record(v)));if(resource!==null&&resource.ref!==workRef)throw new Error('对照作品身份不符');return {workRef,status:str(r.status),resource,metrics:metrics(r.metrics),publishedEpochMs:nullable(r.publishedEpochMs,num),reasons:array(r.reasons,str)};}
function statistic(value:unknown){const r=record(value);return {median:nullable(r.median,num),count:num(r.count)};}
export function decodeWorkComparison(value:unknown):WorkComparison {const r=record(value);const mode=str(r.mode);if(mode!=='current'&&mode!=='age_24h')throw new Error('对照口径无效');const s=record(r.referenceStatistics);return {targetRef:ref(r.targetRef),knowledgeCutoff:str(r.knowledgeCutoff),methodVersion:str(r.methodVersion),mode,ageHours:nullable(r.ageHours,num),toleranceMinutes:nullable(r.toleranceMinutes,num),items:array(r.items,snapshot),referenceStatistics:{likes:statistic(s.likes),comments:statistic(s.comments),collects:statistic(s.collects),shares:statistic(s.shares)}};}

export function inspectionUrl(scope:ReadScope,path:string,fields:Record<string,string|number|null|undefined>={}):string {
  const params=new URLSearchParams({domain:scope.domain});
  if(scope.cutoff)params.set('as_of',scope.cutoff);
  for(const [key,value] of Object.entries(fields))if(value!==null&&value!==undefined)params.set(key,String(value));
  return `/api/collection/workspace/target/${encodeURIComponent(scope.targetRef)}/inspection/${path}?${params}`;
}
