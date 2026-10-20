import React,{useEffect,useState} from 'react';
import registry from '../generated/registry.json';
import './fonts.css';
import './system.css';
import './delivery.css';
import './focus.css';
import {InputMethodScope} from './InputMethod.jsx';
import {CHANNEL,variablesFor} from '../typeComparison/model.js';
import '../typeComparison/compare.css';

const type=registry.visual.typographyApplication;
const focus=registry.visual.focusApplication.parameters;
const material=registry.visual.materialApplication.parameters;
const geometry=registry.visual.materialApplication.geometry;
export const typographyStyle={
 '--accepted-ledger-row':registry.visual.ledgerApplication.decision.rowHeight+'px',
 ...Object.fromEntries(Object.entries(material).filter(([,v])=>typeof v==='number').map(([k,v])=>['--mat-'+k,k.endsWith('Opacity')?v:v+'px'])),
 '--mat-context-fill':geometry.contextFill,
 '--mat-context-mesh-size':`${material.contextMajor}px ${material.contextMajor}px,${material.contextFine}px ${material.contextFine}px`,
 '--mat-shell-plate-size':`${material.shellPitch}px ${material.shellPitch}px`,
 '--focus-warm':focus.warmFill,
 '--focus-inset':focus.keyboardInsetPx+'px',
 ...Object.fromEntries(Object.entries(type.families).map(([role,value])=>['--ty-'+role,value])),
 ...Object.fromEntries(Object.entries(type.roles).flatMap(([role,v])=>[['--ty-'+role+'-size',v.size+'px'],['--ty-'+role+'-weight',v.weight],['--ty-'+role+'-line',v.lineHeight]])),
 ...Object.fromEntries(Object.entries(type.weights).map(([role,value])=>['--ty-weight-'+role,value]))
};

export function TypographyScope({children}){
 const [comparison,setComparison]=useState(null);
 useEffect(()=>{
  if(window.parent===window||new URLSearchParams(location.search).get('typePreview')!=='1')return;
  function receive(event){if(event.origin!==location.origin||event.source!==window.parent||event.data?.channel!==CHANNEL||event.data.type!=='apply')return;const v=event.data.config;if(!v||v.variant!=='draft'||v.dimension!=='all')return;setComparison({...v,signature:event.data.signature});}
  window.addEventListener('message',receive);window.parent.postMessage({channel:CHANNEL,type:'ready'},location.origin);
  return()=>window.removeEventListener('message',receive);
 },[]);
 useEffect(()=>{if(!comparison)return;let active=true,fontsReady=false,lastReport='';
  const faces=['400 14px "Linggan OPPO"','400 14px "Linggan Albert"','500 14px "Linggan Manrope"','300 40px "Linggan Alumni"'];
  function report(){requestAnimationFrame(()=>{if(!active)return;const width=document.documentElement.clientWidth,key=width+':'+fontsReady;if(key===lastReport)return;lastReport=key;window.parent.postMessage({channel:CHANNEL,type:'applied',signature:comparison.signature,width,fontsReady},location.origin);});}
  const observer=new ResizeObserver(report);observer.observe(document.documentElement);
  Promise.allSettled(faces.map(f=>document.fonts.load(f))).then(()=>document.fonts.ready).then(()=>{if(!active)return;fontsReady=faces.every(f=>document.fonts.check(f));report();});
  return()=>{active=false;observer.disconnect()};
 },[comparison]);
 return <div className="type-system" style={{...typographyStyle,...(comparison?variablesFor(comparison):{})}} data-typography="TYPE-03" data-type-comparison={comparison?comparison.variant:undefined} data-type-dimension={comparison?.dimension} data-type-workspace={comparison?.workspace?'work':undefined}>
  <InputMethodScope>{children}</InputMethodScope>
  <footer className="type-license">本审阅软件使用 OPPO Sans · Alumni Sans · Albert Sans · Manrope <a href="?page=system&section=typography#font-license">字体来源与授权 ↗</a></footer>
 </div>
}
