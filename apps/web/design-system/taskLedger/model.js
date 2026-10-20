import registry from '../generated/registry.json';
const ledgerRule=registry.visual.ledgerApplication;
export const ROW_HEIGHT=ledgerRule.decision.rowHeight;
export const LEDGER_NARROW=ledgerRule.narrow;
import {rows,qty,commentLabel,matchFilter} from '../tasksReview/data.js';
export {qty,commentLabel,matchFilter};
export const INITIAL_RUNS=rows.map((r,i)=>({...r,received:i===0?140:r.comments}));
export const DOMAINS=['ADHD','考研自习'];
export const RANKS=['综合','最新','最多点赞','最多收藏','最多评论'];
export const TARGETS=Array.from({length:43},(_,i)=>({id:`${i%2?'creator':'keyword'}-${i+1}`,name:i===0?'亲子沟通':i===1?'林间观察':`示例${i%2?'创作者':'关键词'} ${i+1}`,kind:i%2?'创作者':'关键词'}));
const FIELD_DEFINITIONS=[
 ['name','任务名称',170,true],['id','运行编号',130],['domain','领域',72],['source','来源',98],['scope','固定 / 上限',92],['mode','评论要求',115],['rank','排序',80],
 ['details','详情 · 篇',74],['comments','评论 · 条',78],['reused','复用 · 篇',76],['added','新增评论',88],['cumulative','累计评论',88],
 ['stage','当前阶段',140,true],['next','下一步',170],['created','创建时间',112],['updated','最近进展',112],['action','操作',54,true]
];
export const FIELDS=ledgerRule.fields.map(id=>[id,FIELD_DEFINITIONS.find(f=>f[0]===id)[1],ledgerRule.widths[id],ledgerRule.priority.P0.includes(id)]);
export const DEFAULT_FIELDS=ledgerRule.fields;
export const freshDraft=(domain='all',exact=false)=>({source:exact?'selected':'keyword',domain:exact?'ADHD':DOMAINS.includes(domain)?domain:'',query:'',target:'',rank:'综合',limit:exact?'4':'50',mode:exact?'all_public':'sample_30'});
export const sourceShort=r=>({'临时关键词':'临时关键词','已有创作者目标':'创作者目标','已有关键词目标':'关键词目标','精确所选作品':'语料已选','历史记录':'历史记录'})[r.source]||r.source;
export function stage(r){return ({active:'获取作品详情',blocked:'资源受限',waiting:'等待调度',paused:'已暂停',ended:r.gap?'已结束 · 有缺口':'本次结束',legacy:'历史 · 已结束',stopped:r.leases>0?'已停止 · 交付中':'已停止'})[r.status]||'状态未知'}
export const tone=r=>r.status==='active'?'orange':r.status==='blocked'?'amber':['waiting','legacy','paused'].includes(r.status)?'muted':'ink';
export const canControl=r=>!['ended','legacy','stopped'].includes(r.status)&&Number.isInteger(r.revision);
export const stamp=(r,updated=false)=>(updated?(r.updatedDate||r.created.slice(0,5))+' '+r.updated:r.created.replace(' / ',' '));
export function validateDraft(d){const e={};if(!DOMAINS.includes(d.domain))e.domain='请选择本次采集归属的领域。';if(d.source==='keyword'&&(!d.query.trim()||Array.from(d.query.trim()).length>80))e.query='输入 1–80 字的有效关键词。';if(d.source==='target'&&!TARGETS.some(t=>t.id===d.target))e.target='请选择本领域的观察目标。';if(d.source!=='selected'&&(!/^\d+$/.test(String(d.limit))||Number(d.limit)<1||Number(d.limit)>200))e.limit='作品上限须为 1–200 的整数。';return e}
export function freezeDraft(d){const exact=d.source==='selected',creator=d.source==='target'&&d.target.startsWith('creator');return {source:d.source,domain:d.domain,target:d.target,query:d.query.trim(),rank:exact||creator?null:d.rank,limit:exact?4:Number(d.limit),mode:exact?'all_public':d.mode,workRefs:exact?['DEMO-W01','DEMO-W02','DEMO-W03','DEMO-W04']:null}}
export function acceptedRun(request){const d=request.frozen;return {...INITIAL_RUNS[2],id:`DEMO-N${request.ordinal}`,name:d.source==='selected'?'已选 4 篇作品':d.source==='keyword'?d.query:TARGETS.find(t=>t.id===d.target)?.name||'观察目标',domain:d.domain,source:d.source==='selected'?'精确所选作品':d.source==='keyword'?'临时关键词':d.target.startsWith('creator')?'已有创作者目标':'已有关键词目标',rank:d.rank,limit:d.limit,fixed:d.source==='selected'?4:null,mode:d.mode,created:'10.16 / 10:46',updated:'10:46',revision:1}}
export function applyControl(run,action){return {...run,status:action==='stop'?'stopped':action==='pause'?'paused':'waiting',revision:run.revision+1,queued:action==='stop'?0:run.queued,updated:'10:46',updatedDate:'10.16',stage:action==='stop'?'已停止，等待在途交付':action==='pause'?'本次已暂停':'等待调度',next:action==='stop'?'已得材料保留，本次不可继续':action==='pause'?'可继续未完成的固定范围':'等待有效租约',gap:action==='stop'?'未认领工作已取消；停止前已产生的材料仍可能继续交付。':run.gap}}
export function workRows(run){if(run.fixed===null)return [];const count=run.fixed,n=run.details??0;return Array.from({length:count},(_,i)=>({id:`${run.id}-W${String(i+1).padStart(3,'0')}`,ordinal:i+1,title:['从一个小步骤开始','把任务拆成可见的动作','为启动留出缓冲','把每天的进步记录下来'][i%4]+(i>3?` · ${i+1}`:''),author:`示例创作者 ${i%5+1}`,readable:i<n,reused:i<(run.reused??0),comments:i<n?Math.floor((run.comments??0)/Math.max(n,1))+(i<(run.comments??0)%Math.max(n,1)?1:0):null,gap:i<n?(run.status==='ended'&&run.gap?'公开评论范围有缺口':null):['waiting','active','paused'].includes(run.status)?'详情尚未取得':'详情材料缺失'}))}
export function readStored(key,fallback){try{const v=JSON.parse(sessionStorage.getItem(key));return v??fallback}catch{return fallback}}
export const makeId=prefix=>`${prefix}-${crypto.randomUUID()}`;
