import React,{useEffect,useRef,useState} from 'react';
import './tertiary-tabs.css';

// Page partitions belong to the breadcrumb module; filters and object tabs keep their own owners.
export function TertiaryTabs({items,value,onChange,label='页面三级分区',panelId,disabled=false,revealOnFocus=false}){
 const refs=useRef([]);
 const [focus,setFocus]=useState(items.findIndex(item=>item.id===value));
 function reveal(index){const tab=refs.current[index],rail=tab?.closest('.ds-tertiary-tabs');if(!tab||!rail)return;const r=tab.getBoundingClientRect(),c=rail.getBoundingClientRect();if(r.left<c.left)rail.scrollLeft-=c.left-r.left;else if(r.right>c.right)rail.scrollLeft+=r.right-c.right}
 useEffect(()=>{const index=items.findIndex(item=>item.id===value);setFocus(index);if(revealOnFocus)reveal(index)},[value,items,revealOnFocus]);
 function keydown(event,index){
  if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
  event.preventDefault();
  const next=event.key==='Home'?0:event.key==='End'?items.length-1:(index+(event.key==='ArrowRight'?1:-1)+items.length)%items.length;
  setFocus(next);refs.current[next]?.focus();
 }
 return <nav className="ds-tertiary-tabs" aria-label={label} data-navigation-level="3"><div role="tablist" aria-label={label}>{items.map((item,index)=><button key={item.id} ref={el=>refs.current[index]=el} role="tab" type="button" disabled={disabled} aria-selected={item.id===value} aria-controls={panelId} tabIndex={index===focus?0:-1} onFocus={()=>{if(revealOnFocus)reveal(index)}} onKeyDown={event=>keydown(event,index)} onClick={()=>onChange(item.id)} title={item.name}>{item.name}</button>)}</div></nav>;
}
