import React,{useEffect,useId,useRef} from 'react';
import {Button} from '../dsReview/ui.jsx';

export function OverlayPanel({title,description,shape='dialog',modal=true,confirm=false,onClose,onRestore,focusKey,children,footer,width}){
 const ref=useRef(null),heading=useRef(null),outside=useRef(false),id=useId(),restore=useRef(onRestore);restore.current=onRestore;
 useEffect(()=>{const el=ref.current;if(modal)el.showModal();else el.show();return()=>el.close()},[modal]);
 useEffect(()=>{if(modal||ref.current.contains(document.activeElement)||document.activeElement===document.body)(ref.current.querySelector('[data-initial]:not(:disabled)')||heading.current)?.focus()},[focusKey,modal]);
 useEffect(()=>()=>{queueMicrotask(()=>restore.current?.())},[]);
 useEffect(()=>{if(modal)return;const esc=e=>{if(e.key==='Escape'&&!e.defaultPrevented&&ref.current?.contains(e.target)&&!document.querySelector('dialog:modal')&&!e.target.closest('[role=listbox],[role=combobox]')){e.preventDefault();onClose('escape')}};document.addEventListener('keydown',esc);return()=>document.removeEventListener('keydown',esc)},[modal,onClose]);
 const isOutside=e=>{const r=ref.current.getBoundingClientRect();return e.target===ref.current&&(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)};
 return <dialog ref={ref} className={'ds-dialog q09-panel '+shape} role={confirm?'alertdialog':'dialog'} aria-modal={modal?true:undefined} aria-labelledby={id} aria-describedby={id+'-description'} style={{'--q09-panel-limit':width+'px'}} onCancel={e=>{e.preventDefault();e.stopPropagation();onClose('escape')}} onPointerDown={e=>{outside.current=isOutside(e)}} onClick={e=>{if(outside.current&&isOutside(e))onClose('backdrop');outside.current=false}} onKeyDown={e=>{if(!modal||e.key!=='Tab')return;const nodes=[...ref.current.querySelectorAll('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),[role=combobox]:not(:disabled),a[href],summary,[tabindex="0"]')].filter(el=>el.getClientRects().length);if(!nodes.length)return;if(e.shiftKey&&(document.activeElement===nodes[0]||document.activeElement===heading.current)){e.preventDefault();nodes.at(-1).focus()}else if(!e.shiftKey&&document.activeElement===nodes.at(-1)){e.preventDefault();nodes[0].focus()}}}>
  <header className="q09-panel-head" data-region="Z05-H"><div><span className="ds-kicker">Q09 · 合成交互演练</span><h2 id={id} ref={heading} tabIndex={-1}>{title}</h2><p id={id+'-description'}>{description}</p></div><Button tone="quiet" aria-label="关闭当前浮层" onClick={()=>onClose('button')}>关闭 ×</Button></header>
  <div className="q09-panel-body" data-region="Z05-B">{children}</div>
  <footer className="q09-panel-foot" data-region="Z05-A">{footer}</footer>
 </dialog>;
}
