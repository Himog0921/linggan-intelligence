import { array,flag,num,nullable,record,ref,str } from './inspectionApi';
import { choice,parseBlocks,parseIssue } from '../../../corpus/evidence/dossierBlockDecode';
import type { DossierBlock,DossierIssue } from '../../../corpus/evidence/dossierTypes';
export interface SourceFragment { citationId:string;sourceRef:string;sourceVersion:string;field:string;start:number;end:number;structure:DossierBlock;issues:DossierIssue[];speaker:string;readingRef:string|null;unitRef:string|null;knowledgeCutoff:string;sourceLocation:{slotKey:string|null;page:number|null}|null }
export interface DossierView { workRef:string;knowledgeCutoff:string;materialRevision:string;dossierRevision:string;ownerContract:string;fragments:SourceFragment[];partial:boolean;limitations:string[] }
export function decodeDossier(value:unknown):DossierView {
  const r=record(value);
  if(r.ownerContract!=='research-dossier.v2')throw new Error('底稿合同不受支持');
  return {workRef:ref(r.workRef),knowledgeCutoff:str(r.knowledgeCutoff),materialRevision:str(r.materialRevision),dossierRevision:str(r.dossierRevision),ownerContract:str(r.ownerContract),
    fragments:array(r.fragments,value=>{const f=record(value);const blocks=parseBlocks({blocks:[f.structure]});const structure=blocks[0];if(!structure||structure.ref!==f.citationId)throw new Error('来源区块身份不符');return {
      citationId:str(f.citationId),sourceRef:ref(f.sourceRef),sourceVersion:str(f.sourceVersion),field:str(f.field),start:num(f.start),end:num(f.end),structure,
      issues:array(f.issues,value=>parseIssue(record(value))),speaker:choice(f.speaker,['author','other','unconfirmed'],'speaker'),readingRef:nullable(f.readingRef,ref),unitRef:nullable(f.unitRef,ref),knowledgeCutoff:str(f.knowledgeCutoff),sourceLocation:nullable(f.sourceLocation,value=>{const location=record(value);return {slotKey:nullable(location.slotKey,str),page:nullable(location.page,num)};}),
    };}),partial:flag(r.partial),limitations:array(r.limitations,str)};
}
