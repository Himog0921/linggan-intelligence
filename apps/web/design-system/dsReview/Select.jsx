import React,{useEffect,useId,useRef,useState} from 'react';

// Non-editable combobox: focus stays on the trigger; selection commits only on
// Enter/click. The same control supplies review tools, FIND and the scope plate.
export function Select({value,onChange,options,label,disabled=false,appearance='',placeholder='请选择',footer,className=''}){
 const id=useId(),root=useRef(null),trigger=useRef(null),typeahead=useRef(''),typeTimer=useRef(null);
 const [open,setOpen]=useState(false),[active,setActive]=useState(-1),[up,setUp]=useState(false),[end,setEnd]=useState(false);
 const choices=options.map(o=>typeof o==='string'?{value:o,label:o}:o),selected=choices.findIndex(o=>o.value===value);
 const enabled=choices.map((o,n)=>o.disabled?-1:n).filter(n=>n>=0);
 function expand(){if(disabled)return;const r=root.current.getBoundingClientRect();setUp(window.innerHeight-r.bottom<Math.min(280,choices.length*44+70)&&r.top>300);setEnd(r.left+260>window.innerWidth);setActive(selected>=0&&!choices[selected].disabled?selected:enabled[0]??-1);setOpen(true)}
 function choose(n){if(n<0||choices[n]?.disabled)return;onChange?.(choices[n].value);setOpen(false);trigger.current?.focus()}
 useEffect(()=>{if(!open)return;const outside=e=>{if(!root.current?.contains(e.target))setOpen(false)};document.addEventListener('pointerdown',outside);return()=>document.removeEventListener('pointerdown',outside)},[open]);
 useEffect(()=>{if(open&&active>=0)document.getElementById(`${id}-${active}`)?.scrollIntoView({block:'nearest'})},[active,open,id]);
 useEffect(()=>()=>clearTimeout(typeTimer.current),[]);
 function key(e){
  if(e.key==='Escape'&&open){e.preventDefault();e.stopPropagation();setOpen(false);return}
  if(e.key==='Tab'){if(open&&footer&&!e.shiftKey){e.preventDefault();root.current.querySelector('.ds-select-footer')?.focus()}else setOpen(false);return}
  if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){
   e.preventDefault();if(!open){expand();return}const n=enabled.indexOf(active);
   setActive(e.key==='Home'?enabled[0]:e.key==='End'?enabled.at(-1):enabled[(n+(e.key==='ArrowDown'?1:-1)+enabled.length)%enabled.length]??-1);return;
  }
  if(e.key==='Enter'||e.key===' '){e.preventDefault();open?choose(active):expand();return}
  if(e.key.length===1&&!e.metaKey&&!e.ctrlKey&&!e.altKey){
   typeahead.current+=e.key.toLocaleLowerCase();clearTimeout(typeTimer.current);typeTimer.current=setTimeout(()=>{typeahead.current=''},600);
   const match=enabled.find(n=>choices[n].label.toLocaleLowerCase().startsWith(typeahead.current));
   if(match!==undefined){e.preventDefault();if(!open)expand();setActive(match)}
  }
 }
 return <span ref={root} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))setOpen(false)}} className={`ds-select ${appearance} ${className} ${open?'is-open':''}`}>
  <button ref={trigger} type="button" role="combobox" aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open?id:undefined} aria-activedescendant={open&&active>=0?`${id}-${active}`:undefined} disabled={disabled} onClick={()=>open?setOpen(false):expand()} onKeyDown={key} onBlur={e=>{if(!root.current?.contains(e.relatedTarget))setOpen(false)}} className="ds-select-trigger">
   <span>{choices[selected]?.label||placeholder}</span>{choices[selected]?.meta&&<small>{choices[selected].meta}</small>}<i aria-hidden="true"/>
  </button>
  {open&&<span className={`ds-select-popover ${up?'opens-up':''} ${end?'align-end':''}`}><span role="listbox" aria-label={label} id={id} className="ds-select-options">{choices.map((o,n)=><span role="option" aria-selected={n===selected} aria-disabled={o.disabled||undefined} id={`${id}-${n}`} key={o.value} className={n===active?'is-active':''} onPointerMove={()=>!o.disabled&&setActive(n)} onMouseDown={e=>e.preventDefault()} onClick={e=>{e.preventDefault();choose(n)}}><span>{o.label}</span>{o.meta&&<small>{o.meta}</small>}<b aria-hidden="true">{n===selected?'■':''}</b></span>)}</span>{footer&&<button className="ds-select-footer" type="button" onKeyDown={e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();setOpen(false);trigger.current?.focus()}}} onClick={()=>{setOpen(false);footer.onClick();trigger.current?.focus()}}>{footer.label}<small>{footer.meta}</small>↗</button>}</span>}
 </span>
}
