import { decode, list, num, obj, optNum, optObj, optStr, str, type Json } from '../../../shared/decode';
import type { CommentKey } from './studyApi';
export interface ObservationPoint { date: string; newObservedCommentCount: number | null; studiedCommentCount: number | null; acceptedSignalCount: number | null; coveredWorkCount: number | null; observationCoverage: string | null }
export interface ObservationSeries { timezone: string | null; points: ObservationPoint[] }
export interface VoicePreview { requestRef: string; commentKey: CommentKey; quote: string; proposition: string; workTitle: string }
export interface ProblemPreview {
 problemRef: string; label: string; state: string | null; authors: number | null; works: number | null;
 evidence: {requestRef: string; workRef: string; commentExternalId: string; quote: string; proposition: string}[];
}
/** CONTRACT §3：四段之和等于可研究评论数。旧服务没有此块时为 null，页面退回单段。 */
export interface ResearchBreakdown { withResult: number; running: number; failedWithoutResult: number; never: number }
export interface RecentSignal { requestRef: string; proposition: string; kind: string; authorDisplayName: string | null; acceptedAt: string | null; commentKey: CommentKey }
export interface StudyOverview {
 asOf: string; corpusSummary: {displayableCommentCount:number;eligibleCommentCount:number;rawReadableCommentCount:number};
 indexCoverage:{indexedCount:number;pendingCount:number};
 researchSummary:{studiedCommentCount:number;succeededCommentCount:number;noSignalCommentCount:number;currentSignalCount:number};
 knowledgeSummary:{problemCount:number;pendingCandidateCount:number;solutionSignalCount:number;experienceSignalCount:number;solutionWorkCount:number;experienceWorkCount:number;solutionPreview:VoicePreview[];experiencePreview:VoicePreview[];problemPreview:ProblemPreview[]};
 observationSeries:ObservationSeries;yesterdayNewProblemCount:number;
 researchBreakdown: ResearchBreakdown | null;
 /** null：服务未提供此块（旧版本）；空数组：已提供但当前没有近期信号。 */
 recentSignals: RecentSignal[] | null;
}
function breakdown(row: Json | null): ResearchBreakdown | null {
 if (!row) return null;
 const parts = ['withResult', 'running', 'failedWithoutResult', 'never'].map(key => optNum(row, key));
 if (parts.some(value => value == null)) return null;
 const [withResult, running, failedWithoutResult, never] = parts as number[];
 return { withResult: withResult!, running: running!, failedWithoutResult: failedWithoutResult!, never: never! };
}
function recent(root: Json): RecentSignal[] | null {
 if (root.recentSignals == null) return null;
 return list(root, 'recentSignals', row => { const key = obj(row, 'commentKey'); return {
  requestRef: str(row, 'requestRef'), proposition: str(row, 'proposition'), kind: str(row, 'kind'),
  authorDisplayName: optStr(row, 'authorDisplayName'), acceptedAt: optStr(row, 'acceptedAt'),
  commentKey: { workRef: str(key, 'workRef'), commentExternalId: str(key, 'commentExternalId') },
 }; });
}
export function parseOverview(value:unknown,domain:string):StudyOverview{return decode(value,root=>{
 if(str(root,'domainRef')!==domain)throw new Error('概览与当前领域不符');
 if(str(root,'contract')!=='comment-research.overview.v1')throw new Error('概览数据版本不符');
 const corpus=obj(root,'corpusSummary'),coverage=obj(root,'indexCoverage'),research=obj(root,'researchSummary'),knowledge=obj(root,'knowledgeSummary'),series=obj(root,'observationSeries');
 const voices=(key:string)=>list(knowledge,key,row=>{const k=obj(row,'commentKey');return {requestRef:str(row,'requestRef'),commentKey:{workRef:str(k,'workRef'),commentExternalId:str(k,'commentExternalId')},quote:str(row,'quote'),proposition:str(row,'proposition'),workTitle:str(row,'workTitle')};});
 return {asOf:str(root,'asOf'),corpusSummary:{displayableCommentCount:num(corpus,'displayableCommentCount'),eligibleCommentCount:num(corpus,'eligibleCommentCount'),rawReadableCommentCount:num(corpus,'rawReadableCommentCount')},indexCoverage:{indexedCount:num(coverage,'indexedCount'),pendingCount:num(coverage,'pendingCount')},researchSummary:{studiedCommentCount:num(research,'studiedCommentCount'),succeededCommentCount:num(research,'succeededCommentCount'),noSignalCommentCount:num(research,'noSignalCommentCount'),currentSignalCount:num(research,'currentSignalCount')},knowledgeSummary:{problemCount:num(knowledge,'problemCount'),pendingCandidateCount:num(knowledge,'pendingCandidateCount'),solutionSignalCount:num(knowledge,'solutionSignalCount'),experienceSignalCount:num(knowledge,'experienceSignalCount'),solutionWorkCount:num(knowledge,'solutionWorkCount'),experienceWorkCount:num(knowledge,'experienceWorkCount'),solutionPreview:voices('solutionPreview'),experiencePreview:voices('experiencePreview'),problemPreview:list(knowledge,'problemPreview',row=>({problemRef:str(row,'problemRef'),label:str(row,'label'),state:optStr(row,'state'),authors:optNum(row,'authorCount'),works:optNum(row,'workCount'),evidence:list(row,'evidence',e=>({requestRef:str(e,'requestRef'),workRef:str(e,'workRef'),commentExternalId:str(e,'commentExternalId'),quote:str(e,'quote'),proposition:str(e,'proposition')}))}))},yesterdayNewProblemCount:num(root,'yesterdayNewProblemCount'),observationSeries:{timezone:optStr(series,'timezone'),points:list(series,'points',p=>({date:str(p,'date'),newObservedCommentCount:num(p,'newObservedCommentCount'),studiedCommentCount:num(p,'studiedCommentCount'),acceptedSignalCount:num(p,'acceptedSignalCount'),coveredWorkCount:num(p,'coveredWorkCount'),observationCoverage:str(p,'observationCoverage')}))},
  researchBreakdown:breakdown(optObj(root,'researchBreakdown')),recentSignals:recent(root)};
});}
