import { useEffect,useRef,useState,type ReactNode } from 'react';
import { ActionButton } from '../../../../shared/ActionButton';
import styles from './TargetResearch.module.css';

export function InspectorHeaderActions({children}:{children:ReactNode}){
  const [open,setOpen]=useState(false);
  const host=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null);
  useEffect(()=>{
    if(!open)return;
    const close=(event:PointerEvent)=>{if(event.target instanceof Node&&!host.current?.contains(event.target))setOpen(false);};
    document.addEventListener('pointerdown',close);
    return()=>document.removeEventListener('pointerdown',close);
  },[open]);
  return <div ref={host} className={styles.headerMore} onKeyDown={event=>{
    if(event.key==='Escape'&&open){event.preventDefault();event.stopPropagation();setOpen(false);trigger.current?.focus();}
  }} onBlur={event=>{if(event.relatedTarget instanceof Node&&!event.currentTarget.contains(event.relatedTarget))setOpen(false);}}>
    <ActionButton ref={trigger} aria-expanded={open} aria-controls="inspector-more-actions" onClick={()=>setOpen(value=>!value)}>更多</ActionButton>
    {open&&<div id="inspector-more-actions" aria-label="目标更多操作">{children}</div>}
  </div>;
}
