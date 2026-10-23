import { useState } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { inspectionUrl } from './inspectionApi';
import { EvidenceLink,ReadFeedback } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import styles from './TargetResearch.module.css';

import { decodeDossier } from './dossierModel';
const fieldLabels:Record<string,string>={title:'平台标题',cover_headline:'封面文字',body:'正文',tags:'话题标签',ocr:'图片文字',transcript:'视频转写'};
export function DossierReadout({work}:{work:string}){
  const context=useTargetResearch();const [open,setOpen]=useState(false);
  const read=useInspectionRead(open?inspectionUrl(context.scope,`works/${encodeURIComponent(work)}/dossier`):null,decodeDossier);
  return <div><ActionButton variant="quiet" aria-expanded={open} onClick={()=>setOpen(value=>!value)}>{open?'收起底稿':'展开已取得的底稿'}</ActionButton>
    {open&&<><ReadFeedback read={read}/>{read.data&&<>
      <p className={styles.meta}>{read.data.partial?'材料仍有缺口；以下只展示已取得内容。':'以下为当前读取范围内的来源材料。'}</p>
      {read.data.fragments.length===0?<p className={styles.meta}>没有可读正文片段，不等于原作品没有正文。</p>:read.data.fragments.map(fragment=><details key={fragment.citationId}>
        <summary className={styles.meta}>{fieldLabels[fragment.field]??'来源材料'}</summary><blockquote className={styles.quote}>{fragment.text}</blockquote>
      </details>)}
      <p><EvidenceLink work={work}>到证据库核对原图与底稿</EvidenceLink></p>
    </>}</>}
  </div>;
}
