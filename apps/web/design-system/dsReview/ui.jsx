import {ActionButton} from '../../src/shared/ActionButton';
import {ActionFeedback} from '../../src/shared/ActionFeedback';
import {EmptyState} from '../../src/shared/EmptyState';
import {InlineNotice} from '../../src/shared/InlineNotice';
import {LoadingState} from '../../src/shared/LoadingState';
import {FindField} from '../../src/shared/FindField';
import {SelectField} from '../../src/shared/SelectField';
import {ViewSwitch} from '../../src/shared/ViewSwitch';
import {IntelligenceIcon} from '../../src/shared/IntelligenceIcon';
import React,{useEffect,useId,useRef} from 'react';
import {Select} from './Select.jsx';
export function Button({children,tone='secondary',className='',...props}){const variant=tone==='secondary'?'outline':tone;return <ActionButton variant={variant} className={className} busy={props['aria-busy']===true} {...props}>{children}</ActionButton>}
export function Mark({children,tone='ink'}){return <span className={`ds-mark ${tone}`}><i aria-hidden="true"/>{children}</span>}
export function Facts({items}){return <dl className="ds-facts">{items.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>}
export function Pill({children,onClick,active=false,...p}){return <button type="button" className={'ds-pill '+(active?'active':'')} onClick={onClick} {...p}>{children}</button>}
export function IconFamily({spec}){
 const [expanded,setExpanded]=React.useState(false);
 return <div className="lf-icon-family"><p>{spec.geometry.grid} 单位网格 · {spec.geometry.strokeWidth} 线宽 · 最多 {spec.geometry.maxLayers} 层。单次操作反馈 {spec.motion.feedbackMs}ms，状态标记保持静态。</p><div className="lf-icon-grid">{spec.icons.map(item=><button key={item.name} type="button" aria-label={`检查${item.label}图标`} aria-expanded={item.name==='chevron'?expanded:undefined} onClick={()=>{if(item.name==='chevron')setExpanded(value=>!value)}}><IntelligenceIcon name={item.name} size={32} expanded={item.name==='chevron'&&expanded}/><strong>{item.label}</strong><span><IntelligenceIcon name={item.name} size={18} expanded={item.name==='chevron'&&expanded}/><IntelligenceIcon name={item.name} size={15} expanded={item.name==='chevron'&&expanded}/></span></button>)}</div><p>悬停或键盘定位查看反馈，点击章节图标切换朝向。示例不执行刷新、复制、导出或业务命令。</p></div>;
}
export function Modal({title,onClose,children,footer,modal=true,regionLabels=false,onRegion,dirty=false}){
 const [leave,setLeave]=React.useState(false);const requestClose=()=>dirty?setLeave(true):onClose();
 const ref=useRef(null),opener=useRef(document.activeElement),heading=useRef(null),id=useId(),close=useRef(onClose);close.current=requestClose;
 useEffect(()=>{const el=ref.current;if(modal)el.showModal();else el.show();heading.current?.focus();return()=>{el.close();if(opener.current?.isConnected)opener.current.focus()}},[modal]);
 useEffect(()=>{if(modal)return;const escape=e=>{if(e.key==='Escape'&&!document.querySelector('dialog:modal')){e.preventDefault();close.current()}};document.addEventListener('keydown',escape);return()=>document.removeEventListener('keydown',escape)},[modal]);
 return <dialog ref={ref} className={'ds-dialog '+(!modal?'nonmodal':'')} aria-labelledby={id} onCancel={e=>{e.preventDefault();e.stopPropagation();requestClose()}} onClick={e=>{if(modal&&e.target===ref.current){const r=ref.current.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)requestClose()}}} onKeyDown={e=>{if(!modal||e.key!=='Tab')return;const nodes=[...ref.current.querySelectorAll('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href]')].filter(x=>x.getClientRects().length);if(e.shiftKey&&(document.activeElement===nodes[0]||document.activeElement===heading.current)){e.preventDefault();nodes.at(-1)?.focus()}else if(!e.shiftKey&&document.activeElement===nodes.at(-1)){e.preventDefault();nodes[0]?.focus()}}}>
 <div className="ds-dialog-head" data-region={onRegion?"Z05-H":undefined} onClick={()=>onRegion?.("Z05-H")}>{regionLabels&&<button type="button" className="ds-zone-label" onClick={e=>{e.stopPropagation();onRegion?.("Z05-H")}}>Z05-H 详情身份</button>}<div><span className="ds-kicker">交互结构 · 本地演示</span><h2 ref={heading} tabIndex={-1} id={id}>{title}</h2></div><Button tone="quiet" onClick={requestClose}>关闭 ×</Button></div><div className="ds-dialog-body">{leave?<section role="alert" aria-label="未保存保护"><h3>保留这份未保存内容？</h3><p>继续编辑保留输入；放弃后离开才会关闭当前面板。</p><Button onClick={()=>setLeave(false)}>继续编辑</Button><Button tone="danger" onClick={onClose}>放弃并离开</Button></section>:children}</div>{!leave&&footer&&<div className="ds-dialog-foot" data-region={onRegion?"Z05-A":undefined} onClick={()=>onRegion?.("Z05-A")}>{regionLabels&&<button type="button" className="ds-zone-label" onClick={e=>{e.stopPropagation();onRegion?.("Z05-A")}}>Z05-A 详情动作</button>}{footer}</div>}</dialog>
}
export function Find({value,onChange,trigger='debounce_250',placeholder='昵称、平台账号、公开简介',label='演示搜索',withScope=true,holdCommit=false,onScopeChange,submitLabel,disabled=false,busy=false}){
 const [scope,setScope]=React.useState('author');const handle=useRef(null);
 return <FindField className="ds-find" disabled={disabled} busy={busy} handle={handle} value={value||''} onCommit={v=>{if(!holdCommit)onChange?.(v)}} trigger={trigger==='debounce_250'?'debounce_250':'submit'}
 label={label} placeholder={scope==='work'?'标题、正文或内容方向':placeholder} shortcut submitLabel={submitLabel} clearLabel="清空演示搜索"
 contextKey={scope} scope={withScope?<SelectField compact label="演示搜索范围" value={scope} options={[{value:'author',label:'创作者资料'},{value:'work',label:'作品内容'}]} onChange={next=>{handle.current?.cancel();setScope(next);onScopeChange?.(next,handle.current?.read())}}/>:undefined}/>;
}
export function ViewDemo({value='table',onChange,disabled=false,count}){
 const options=[{key:'table',label:'表格',count},{key:'covers',label:'封面'},{key:'list',label:'列表',disabled,reason:disabled?'当前范围没有列表视图':undefined}];
 return <ViewSwitch label="演示视图" value={value} options={options} onChange={next=>onChange?.(next)}/>;
}
export function SelectDemo({value='recent',onChange,disabled=false,compact=false}){
 const options=[{value:'recent',label:'最近收录'},{value:'relevant',label:'相关作品数'},{value:'blocked',label:'暂不可选',disabled:true}];
 return <SelectField label="演示当前排序" compact={compact} disabled={disabled} value={value} options={options} onChange={next=>onChange?.(next)}/>;
}
export function EmptyDemo({kind='no_match',onChange}){
 const title={first_use:'从已有入口开始',no_match:'当前条件没有结果',no_material:'尚无可阅读材料',unavailable_feature:'当前能力未开放'}[kind]||'当前条件没有结果';
 return <EmptyState kind={kind} title={title} action={<Button onClick={()=>onChange?.('当前范围')}>返回当前范围</Button>}>空、未接通和读取失败分别说明；读取失败不显示成没有材料。</EmptyState>;
}
export function LoadingDemo({phase='initial',onChange}){
 return <LoadingState phase={phase} subject="领域配置"
  error={phase==='stale_error'?'领域配置和关联关系当前读不到；':undefined}
  action={phase==='stale_error'?<Button onClick={()=>onChange?.('重新读取')}>重新读取</Button>:undefined}>
  {phase==='initial'?undefined:<p className="ds-muted">上次读取结果：3 个领域 · 2 个运行中。</p>}
 </LoadingState>;
}
export function NoticeDemo({tone='info',onChange}){
 const text={info:'当前只检索了有界范围；未找到候选不能解释为作者从未讲过。',warning:'当前结果仅覆盖部分来源；未覆盖的部分仍按未知处理。',error:'读取失败，不能据此认定没有材料。'}[tone];
 return <InlineNotice tone={tone} action={<Button onClick={()=>onChange?.('重新核对')}>重新核对</Button>}>{text}</InlineNotice>;
}
export function ReceiptDemo({state='accepted',onChange}){
 const text={pending:'正在提交详情缺口补采，结果以服务器回执为准。',accepted:'请求已由服务器接纳；详情取得数在实际回执入库后更新。',applied:'观察关系已保存；以服务器记录为准。',rejected:'没有完成：服务器拒绝本次写入，输入已保留。',unconfirmed:'提交结果尚未确认，请使用原请求核对，不重复派单。',partial:'部分作品已排入；其余因暂无可执行地址没有提交。'}[state]||'请求已由服务器接纳；详情取得数在实际回执入库后更新。';
 return <ActionFeedback state={state} requestRef={state==='unconfirmed'?'req-7f21':undefined} code={state==='rejected'?'revision_conflict':undefined}
  action={state==='unconfirmed'||state==='rejected'?<Button onClick={()=>onChange?.('核对原回执')}>核对原回执</Button>:undefined}>{text}</ActionFeedback>;
}
