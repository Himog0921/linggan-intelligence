import {ActionButton} from '../../src/shared/ActionButton';
import {FindField} from '../../src/shared/FindField';
import {SelectField} from '../../src/shared/SelectField';
import React,{useEffect,useId,useRef} from 'react';
import {Select} from './Select.jsx';
export function Button({children,tone='secondary',className='',...props}){const variant=tone==='secondary'?'outline':tone;return <ActionButton variant={variant} className={className} busy={props['aria-busy']===true} {...props}>{children}</ActionButton>}
export function Mark({children,tone='ink'}){return <span className={`ds-mark ${tone}`}><i aria-hidden="true"/>{children}</span>}
export function Facts({items}){return <dl className="ds-facts">{items.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>}
export function Pill({children,onClick,active=false,...p}){return <button type="button" className={'ds-pill '+(active?'active':'')} onClick={onClick} {...p}>{children}</button>}
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
