import { useState } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import type { Drawer } from '../api';
import type { Commands } from '../useWorkspace';
import { inspectionUrl,record,ref,flag,array,str } from './inspectionApi';
import { useInspectionRead } from './useInspectionRead';
import { ReadFeedback } from './ResearchParts';
import styles from './TargetResearch.module.css';

interface Preview {
  targetRef:string;domainRef:string;
  works:{workRef:string;status:string}[];
  eligibleRefs:string[];truncated:boolean;
}
function decode(value:unknown):Preview{
  const r=record(value);return {
    targetRef:ref(r.targetRef),domainRef:ref(r.domainRef),
    works:array(r.works,value=>{const w=record(value);return {workRef:ref(w.workRef),status:str(w.status)};}),
    eligibleRefs:array(r.eligibleRefs,ref),truncated:flag(r.truncated),
  };
}
export function ExactDetailGaps({data,params,commands}:{data:Drawer;params:URLSearchParams;commands:Commands}){
  const [open,setOpen]=useState(false);
  const target=data.target.targetRef;
  const location=inspectionUrl({targetRef:target,domain:params.get('domain')??'all',cutoff:null},'detail-gaps');
  const read=useInspectionRead(open?location:null,decode);
  const result=read.data;
  return <section className={styles.gapScope} aria-label="仅补尚未取得的作品详情">
    <div className={styles.footer}>
      <div><strong>详情缺口补采</strong><p className={styles.meta}>与持续巡查独立；当前筛选的作品不会自动进入新的任务。</p></div>
      <ActionButton variant="outline" aria-expanded={open} onClick={()=>setOpen(value=>!value)}>
        {open?'收起可执行范围':'核查可补采范围'}</ActionButton>
    </div>
    {open&&<><ReadFeedback read={read}/>
      {result&&<>
        <div className={styles.inlineSummary}><span>可提交 <b>{result.eligibleRefs.length}</b></span>
          <span>已有任务在途 <b>{result.works.filter(w=>w.status==='in_flight').length}</b></span>
          <span>缺有效访问地址 <b>{result.works.filter(w=>w.status==='locator_unavailable').length}</b></span>
        </div>
        {result.truncated&&<p className={styles.notice}>当前仅展示前 200 篇待补详情，其余待后续核查；不会静默扩大此次提交范围。</p>}
        <p className={styles.meta}>本次只针对服务端明确仍缺详情且可排入的作品；补取详情时沿用获准的媒体和最多 30 条评论窗口。评论单独缺口不会混入。</p>
        <div className={styles.previewList}><ul>{result.works.map(row=>{
          const work=data.catalog?.find(item=>item.publicRef===row.workRef);
          return <li key={row.workRef}><span>{work?.title??'标题未取得'}</span><span>{row.status==='requestable'?'可提交':row.status==='in_flight'?'已有任务在途':'暂无可执行地址'}</span></li>;
        })}</ul></div>
        {commands.feedback?.action==='detail-gaps-exact'&&<p role="status" className={styles.notice}>
          {commands.feedback.unknown?'提交结果尚未确认，请使用原请求核对，不重复派单。':commands.feedback.applied?'请求已由服务器接纳；详情取得数在实际回执入库后更新。':commands.feedback.text}
        </p>}
        <ActionButton variant="primary" disabled={!result.eligibleRefs.length||commands.busy} onClick={()=>{
          void commands.run('detail-gaps-exact',{
            row_target_ref:target,domain_ref:result.domainRef,
            content_public_ref:result.eligibleRefs,
          }).then(()=>{read.reload();});
        }}>确认补采当前可执行的 {result.eligibleRefs.length} 篇</ActionButton>
      </>}
    </>}
  </section>;
}
