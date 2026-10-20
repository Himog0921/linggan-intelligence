import React,{useEffect,useRef,useState} from 'react';
export function PersonalMenu({onNotice=()=>{}}){
 const [menu,setMenu]=useState(false),personal=useRef(null),personalTrigger=useRef(null);
 useEffect(()=>{if(!menu)return;const outside=e=>{if(!personal.current?.contains(e.target))setMenu(false)};document.addEventListener('pointerdown',outside);return()=>document.removeEventListener('pointerdown',outside)},[menu]);
 return <details ref={personal} className="ds-real-personal" open={menu} onToggle={e=>setMenu(e.currentTarget.open)} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))setMenu(false)}} onKeyDown={e=>{if(e.key==='Escape'&&menu){e.preventDefault();e.stopPropagation();setMenu(false);personalTrigger.current?.focus()}}}><summary ref={personalTrigger}><span>个人菜单</span><i aria-hidden="true"/></summary><div className="ds-real-personal-popover"><button type="button" onClick={()=>{setMenu(false);onNotice('设置入口示例。');personalTrigger.current?.focus()}}>设置<span aria-hidden="true">↗</span></button></div></details>
}
