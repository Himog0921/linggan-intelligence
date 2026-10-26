import { useState } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { API_ROOT } from '../../../corpus/evidence/evidencePaths';
import { parseDossierPayload } from '../../../corpus/evidence/dossierDecode';
import { DossierReading } from '../../../corpus/evidence/DossierReading';
import { EvidenceLink,ReadFeedback } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';
export function DossierReadout({work,cutoff}:{work:string;cutoff?:string}) {
  const context=useTargetResearch();const [open,setOpen]=useState(false);
  const selectedCutoff=cutoff??context.scope.cutoff;
  const params=new URLSearchParams();if(context.scope.domain!=='all')params.set('domain',context.scope.domain);if(selectedCutoff)params.set('asOf',selectedCutoff);
  const read=useInspectionRead(open?`${API_ROOT}/${encodeURIComponent(work)}/research-dossier?${params}`:null,value=>{const payload=parseDossierPayload(value);if(payload.dossier.workRef!==work||(selectedCutoff&&Date.parse(payload.dossier.asOf)!==Date.parse(selectedCutoff)))throw new Error('底稿作品或截点不符');return payload;},true);
  return <div><ActionButton variant="quiet" aria-expanded={open} onClick={()=>setOpen(value=>!value)}>{open?'收起底稿':'展开已取得的底稿'}</ActionButton>
    {open&&<><ReadFeedback read={read}/>{read.data&&<>
      <p className={styles.meta}>{selectedCutoff?'以下按当前研究截点读取正式底稿。':'以下为当前正式底稿。'}</p>
      <h4>{read.data.dossier.note.displayTitle ?? '无标题'}</h4>
      {read.data.dossier.note.displayTitleBasis === 'cover' ? <p className={styles.meta}>封面整理标题</p> : null}
      <DossierReading dossier={read.data.dossier} titleRendered />
      <p><EvidenceLink work={work}>到证据库核对原图与底稿</EvidenceLink></p>
    </>}</>}
  </div>;
}
