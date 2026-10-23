import { useEffect,useState } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import { array,flag,inspectionUrl,nullable,num,record,ref,str } from './inspectionApi';
import { DossierReadout } from './DossierReadout';
import { Cutoff,EvidenceLink,ReadFeedback,ResearchPanel } from './ResearchParts';
import { useTargetResearch } from './researchContext';
import { useInspectionRead } from './useInspectionRead';
import { moment } from './inspectionPresent';
import styles from './TargetResearch.module.css';

function decodeRelated(value:unknown){const r=record(value);return {workRef:ref(r.workRef),knowledgeCutoff:str(r.knowledgeCutoff),scannedCount:num(r.scannedCount),truncated:flag(r.truncated),retrievalBasis:str(r.retrievalBasis),candidates:array(r.candidates,value=>{const w=record(value);return {workRef:ref(w.workRef),title:nullable(w.title,str),publishedAt:nullable(w.publishedAt,str),temporalRelation:str(w.temporalRelation),selectionBasis:str(w.selectionBasis),materialAvailable:flag(w.materialAvailable)};})};}
export function RelatedWorksPanel({work}:{work:string}){
  const context=useTargetResearch();const [refs,setRefs]=useState<string[]>([]);const [message,setMessage]=useState<string|null>(null);
  const read=useInspectionRead(inspectionUrl(context.scope,`works/${work}/related`),decodeRelated);
  useEffect(()=>{if(read.data)context.pinCutoff(read.data.knowledgeCutoff);},[read.data,context]);
  const title=context.data.catalog?.find(item=>item.publicRef===work)?.title??'当前作品';
  const select=(id:string)=>{if(refs.includes(id)){setRefs(refs.filter(value=>value!==id));setMessage(null);}else if(refs.length<2){setRefs([...refs,id]);setMessage(null);}else setMessage('最多选择两篇参照；请先移除一篇。');};
  const open=()=>{if(refs.length===0)return;context.patch({inspect_works:[work,...refs].join(','),inspect_panel:'compare',inspect_work:null});};
  return <ResearchPanel title="新作与相近旧作" actions={<EvidenceLink work={work}/> }>
    <h3>{title}</h3><DossierReadout work={work}/>
    <ReadFeedback read={read}/>{message&&<p role="status" className={styles.notice}>{message}</p>}
    {read.data&&<>
      <div className={styles.inlineSummary}><span>当前作者检索范围 <b>{read.data.scannedCount}</b> 篇</span><span>文字检索候选，内容关系尚待核验</span><Cutoff value={read.data.knowledgeCutoff}/></div>
      {read.data.truncated&&<p className={styles.notice}>当前只检索了有界范围，未找到候选不能解释为作者从未讲过。</p>}
      <div className={styles.tableScroll} tabIndex={0} aria-label="相近旧作候选"><table className={styles.table}><thead><tr><th style={{width:'8%'}}>选择</th><th style={{width:'49%'}}>参照作品</th><th>发布时间</th><th>材料</th></tr></thead><tbody>
        {read.data.candidates.map(item=><tr key={item.workRef} data-selected={refs.includes(item.workRef)}><td><input className={styles.check} type="checkbox" checked={refs.includes(item.workRef)} onChange={()=>select(item.workRef)} aria-label={`选择参照 ${item.title??'标题未取得'}`}/></td><td><span className={styles.title}>{item.title??'标题未取得'}</span><div className={styles.subline}>{item.temporalRelation==='older'?'发布时间早于当前作品':'发布时间待确认，不直接称为旧作'}<EvidenceLink work={item.workRef}/></div></td><td className={styles.moment}>{moment(item.publishedAt)}</td><td>{item.materialAvailable?'已取得详情':'材料待取得'}</td></tr>)}
      </tbody></table></div>
      {read.data.candidates.length===0&&<div className={styles.empty}><strong>当前已检索材料中未找到充分参照</strong><p>文字召回不能代替全文语义核对；仍可返回作品列表手动选择。</p></div>}
    </>}
    <div className={styles.footer}><span className={styles.meta}>内容差异与数据高低分开核对；高互动不代表新增知识。</span><ActionButton variant="primary" disabled={refs.length===0} onClick={open}>对照所选作品</ActionButton></div>
  </ResearchPanel>;
}
