import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {tmpdir} from 'node:os';
import {k} from './anatomy.mjs';
import {A} from './anatomy.mjs';
import {stateAt,SCENE} from './model.mjs';
import {makeParts,makeProjection,effectPaths} from './parts.mjs';
import {makeOrdering} from './order.mjs';
const HERE=dirname(fileURLToPath(import.meta.url));
export const OUTPUT=join(HERE,'../../apps/web/src/pages/login/assets/station.svg'),AUDIT_DIR=process.env.LINGGAN_LOGIN_AUDIT_DIR||join(tmpdir(),'linggan-login-anatomy-audit');
const P=makeProjection(),{items,rotorPlanes}=makeParts(P),effects=effectPaths(P);
const axes=[[1,0,0],[0,1,0],[0,0,1]].map(v=>{const p=k.iso(v,P);return p.map((n,i)=>n-P.origin[i]);});
// Signal marks belong to their physical faces. The containing solid supplies occlusion.
const innerOn=(item,markup)=>{const edge=item.svg.indexOf('<path class="iso-line iso-edge');item.svg=item.svg.slice(0,edge)+markup+item.svg.slice(edge);};
effects.channelHosts.forEach((id,index)=>{
 const a=effects.channelPoints[index],b=effects.channelPoints[index+1];
 const start=index===0?`<circle cx="${a[0]}" cy="${a[1]}" r="1" fill="#bfc8cb"/>`:'';
 const end=index===effects.channelHosts.length-1?`<circle cx="${b[0]}" cy="${b[1]}" r="1" fill="#bfc8cb"/>`:'';
 innerOn(items.find(item=>item.id===id),`<path class="signal-channel" d="${k.pathOf([a,b])}"/>${start}${end}<circle id="signal-travel-${index}" class="signal-travel" r="1.8" cx="${a[0]}" cy="${a[1]}"/>`);
});
for(const item of items.filter(item=>item.motion==='input')){
 const clip='scan-clip-'+item.id.replaceAll('.','-');
 innerOn(item,`<defs><clipPath id="${clip}"><path d="${k.planOutline(item.shape.p,10.82,P)}"/></clipPath></defs><g clip-path="url(#${clip})"><path id="read-scan-${clip}" class="read-scan" d="${effects.scan}"/></g>`);
}
export function offsetFor(item,s){
 if(item.motion==='input')return[0,s.inputY-291,0];
 if(item.motion==='result')return[0,s.result,0];
 if(item.motion==='slice')return[0,0,s.slice];
 if(item.motion?.startsWith('drone-')){const q=item.motion==='drone-small'?.66:1;return[s.droneX*q,s.droneY*q,s.droneZ*q];}
 return [0,0,0];
}
export function movedItem(item,s){
 const off=offsetFor(item,s),shape=structuredClone(item.shape);
 if(item.kind==='slab'){shape.p.x+=off[0];shape.p.y+=off[1];shape.z+=off[2];}
 else if(item.kind==='cylinder'){shape.x+=off[0];shape.y+=off[1];shape.z+=off[2];}
 else if(item.kind==='poly'){shape.ring=shape.ring.map(p=>[p[0]+off[0],p[1]+off[1]]);shape.z+=off[2];}
 else shape.F.o=shape.F.o.map((v,i)=>v+off[i]);
 return {...item,shape,at:item.at.map((v,i)=>v+off[i]),key:k.depthOf(item.at.map((v,i)=>v+off[i]),P)};
}
export function record(parts=items,s=stateAt(0)){
 // Only real floor feet and the two deliberately powered free-body roots seed support.
 const R=A.recorder(P,{ground:shape=>shape.ground||shape.flightRoot});
 for(const item of parts){
  const data=movedItem(item,s),v=data.shape,meta={name:item.id,ground:item.ground,flightRoot:item.flightRoot};
  const shape=item.kind==='slab'?A.slab(v.p,v.z,v.h,meta,5):item.kind==='cylinder'?A.cylinder(v.x,v.y,v.r,v.z,v.h,meta):item.kind==='poly'?A.extruded(v.ring,v.z,v.h,meta):A.prism(v.F,v.ring,v.s0,v.s1,meta);
  const move=offsetFor(item,s);
  const svg=move.some(Boolean)?`<g transform="${k.translateAlong(P,move)}">${item.svg}</g>`:item.svg;
  R.put({name:item.id,piece:item.group,at:data.at,svg,shapes:[R.solid(shape)]});
 }
 A.settle(R,P);return R;
}
const R=record(),keys=new Map(R.items.map(i=>[i.name,i.key]));items.forEach(i=>i.key=keys.get(i.id));
if(process.argv.includes('--diagnose')){console.log(A.report(A.audit(R,P)));process.exit(0);}
const fixed=items.filter(i=>!i.motion),moving=items.filter(i=>i.motion),ordering=makeOrdering(fixed,P),orders=[];
for(let f=0;f<128;f++){const state=stateAt(f/8);orders.push(ordering(moving.map(i=>movedItem(i,state))));}
const byId=new Map(items.map(i=>[i.id,i]));
const partIds=items.map(i=>i.id),partIndices=new Map(partIds.map((id,i)=>[id,i])),paintStates=[],paintSequence=[],paintIndex=new Map();
for(const order of orders){const key=order.join('|');let index=paintIndex.get(key);if(index===undefined){index=paintStates.length;paintIndex.set(key,index);paintStates.push(order.map(id=>partIndices.get(id)));}paintSequence.push(index);}
const partSvg=item=>item.svg.replace('<g ',`<g id="${item.id}" data-part="${item.id}" `);
const art=orders[0].map(id=>partSvg(byId.get(id))).join('');
const halos=[SCENE.main,SCENE.archive,SCENE.output,SCENE.console,SCENE.track].map(p=>`<path class="iso-halo" d="${k.haloOf(p,0,3,P)}"/>`).join('');
const rotors=rotorPlanes.map(rotor=>{
 const at=k.iso([rotor.x,rotor.y,rotor.z],P),outline=k.ring(rotor.x,rotor.y,8.2*rotor.size,rotor.z,P,48);
 const c=Math.cos(rotor.phase),s=Math.sin(rotor.phase),q=rotor.size;
 const matrix=[(axes[0][0]*c+axes[1][0]*s)*q,(axes[0][1]*c+axes[1][1]*s)*q,(-axes[0][0]*s+axes[1][0]*c)*q,(-axes[0][1]*s+axes[1][1]*c)*q,0,0];
 return `<g id="${rotor.id}"><path class="rotor-disc" d="${outline}"/><g transform="translate(${at.join(' ')})"><g id="${rotor.id}-blades" transform="matrix(${matrix.join(' ')})"><path class="rotor-blade" d="M-8 -.7 L-2 -1.3 L-1 .5 L-7 1.3 Z M8 .7 L2 1.3 L1 -.5 L7 -1.3 Z"/></g></g></g>`;
}).join('');
const svg=`<svg class="station-live iso-svg" id="station-svg" viewBox="0 0 ${SCENE.width} ${SCENE.height}" preserveAspectRatio="xMidYMid meet" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><g>${halos}</g><g id="station-parts">${art}</g><path id="core-scan" class="core-scan" d="${effects.coreScan}"/><g>${rotors}</g><line id="drone-ray" class="drone-ray" x1="${effects.droneOrigin[0]}" y1="${effects.droneOrigin[1]}" x2="${effects.observationPoint[0]}" y2="${effects.observationPoint[1]}"/><circle id="drone-ray-origin" cx="${effects.droneOrigin[0]}" cy="${effects.droneOrigin[1]}" r=".7" fill="#46545a" opacity="0"/><g id="observation-target" class="observation-target"><path d="${effects.target}"/><circle cx="${effects.observationPoint[0]}" cy="${effects.observationPoint[1]}" r=".75" fill="#f74d20"/></g></svg>`;
export {items,P,orders};
export function generatedAssets(){
 const model=readFileSync(join(HERE,'model.mjs'),'utf8');
 const keys=[...new Set([...model.matchAll(/s\.([a-zA-Z]+)\s*=/g)].map(m=>m[1]))];
 const typed=model.replace('smooth=v=>','smooth=(v:number)=>').replace('ramp=(t,a,b)=>','ramp=(t:number,a:number,b:number)=>').replace('envelope=(t,a,b,c,d)=>','envelope=(t:number,a:number,b:number,c:number,d:number)=>').replace('stateAt(seconds,s={})','stateAt(seconds:number,s={} as SceneState)');
 const typedModel='// Generated by scripts/login-anatomy/build.mjs; edit model.mjs.\nexport type SceneState = Record<'+keys.map(k=>JSON.stringify(k)).join(' | ')+', number>;\n'+typed;
 return new Map([
  [OUTPUT,svg],
  [join(dirname(OUTPUT),'scene-data.json'),JSON.stringify({axes,moving:moving.map(({id,motion})=>({id,motion})),effects,rotors:rotorPlanes,partIds,paintStates,paintSequence})],
  [join(dirname(OUTPUT),'../sceneModel.ts'),typedModel],
 ]);
}
if(process.argv[1]===fileURLToPath(import.meta.url)){
 mkdirSync(dirname(OUTPUT),{recursive:true});mkdirSync(AUDIT_DIR,{recursive:true});
 for(const [file,content] of generatedAssets())writeFileSync(file,content);
 writeFileSync(join(AUDIT_DIR,'parts-manifest.json'),JSON.stringify({projection:P,solids:items.length,freeRoots:items.filter(i=>i.flightRoot).map(i=>i.id),parts:items.map(({id,kind,shape,motion})=>({id,kind,shape,motion}))},null,2));
 writeFileSync(join(AUDIT_DIR,'station.svg'),svg);
 console.log(`Built ${items.length} physical parts, ${moving.length} moving parts; ${paintStates.length} painter orders as static React scene assets.`);
 if(process.argv.includes('--audit')){const result=A.auditOrExit(R,P);writeFileSync(join(AUDIT_DIR,'audit.json'),JSON.stringify(result,null,2));}
}
