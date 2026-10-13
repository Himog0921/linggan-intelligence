(() => {
 const host=document.getElementById('station-scene');if(!host)return;
 const mobile=matchMedia('(max-width:800px)'),reduced=matchMedia('(prefers-reduced-motion:reduce)');
 const fallback=host.querySelector('.station-fallback'),template=document.getElementById('station-template');
 let svg=null,nodes=null,frame=0,last=0,elapsed=0,visible=true,disposed=false;
 const state={},origin=PROJECTION.origin,basis=AXES;
 const translate=(node,x=0,y=0,z=0)=>node.setAttribute('transform',`translate(${basis[0][0]*x+basis[1][0]*y+basis[2][0]*z} ${basis[0][1]*x+basis[1][1]*y+basis[2][1]*z})`);
 function render(seconds){
  if(!nodes)return;stateAt(seconds,state);
  for(const item of nodes.moving){
   if(item.motion==='input'){translate(item.node,0,state.inputY-291,0);item.node.setAttribute('opacity',state.inputVisible);}
   else if(item.motion==='slice')translate(item.node,0,0,state.slice);
   else if(item.motion==='result')translate(item.node,0,state.result,0);
   else {const q=item.motion==='drone-small'?.66:1;translate(item.node,state.droneX*q,state.droneY*q,state.droneZ*q);}
  }
  const selected=nodes.order[Math.min(nodes.order.length-1,Math.floor(state.t*8))];
  if(selected!==nodes.currentOrder){for(const node of selected)nodes.parts.appendChild(node);nodes.currentOrder=selected;}
  nodes.input.setAttribute('opacity',state.inputVisible);
  for(const scan of nodes.scans){scan.setAttribute('opacity',state.scan);translate(scan,0,state.scanY-145-(state.inputY-291),0);}
  nodes.read.style.opacity=.25+.75*state.read;
  for(const n of nodes.core)n.style.opacity=.22+.1*state.core;
  nodes.coreScan.setAttribute('opacity',state.coreScanVisible);translate(nodes.coreScan,0,0,state.coreScan);
  nodes.archive.style.opacity=state.archive;
  nodes.evidence.style.opacity=state.evidence;nodes.evidenceMark.style.opacity=state.evidence;
  nodes.signal.style.opacity=state.signal;nodes.point.style.opacity=state.signal;nodes.target.style.opacity=.7*state.signal;
  nodes.ready.style.opacity=.16+.84*state.ready;
  nodes.ray.style.opacity=.18*state.observe;nodes.observeTarget.style.opacity=.6*state.observe;
  const rayX=EFFECTS.droneOrigin[0]+.66*(basis[0][0]*state.droneX+basis[1][0]*state.droneY+basis[2][0]*state.droneZ),rayY=EFFECTS.droneOrigin[1]+.66*(basis[0][1]*state.droneX+basis[1][1]*state.droneY+basis[2][1]*state.droneZ);
  nodes.ray.setAttribute('x1',rayX);nodes.ray.setAttribute('y1',rayY);nodes.rayOrigin.setAttribute('cx',rayX);nodes.rayOrigin.setAttribute('cy',rayY);nodes.rayOrigin.style.opacity=.6*state.observe;
  const path=EFFECTS.channelPoints,lengths=EFFECTS.channelLengths,distance=state.pulseAlong*EFFECTS.channelTotal;
  let segment=0;while(segment<lengths.length-1&&distance>lengths[segment])segment++;
  const start=segment?lengths[segment-1]:0,q=(distance-start)/(lengths[segment]-start),a=path[segment],b=path[segment+1];
  for(let i=0;i<nodes.travels.length;i++){
   const node=nodes.travels[i];node.setAttribute('opacity',i===segment?state.pulse:0);
   if(i===segment){node.setAttribute('cx',a[0]+(b[0]-a[0])*q);node.setAttribute('cy',a[1]+(b[1]-a[1])*q);}
  }
  for(const rotor of nodes.rotors){
   const angle=state.rotor+rotor.phase,c=Math.cos(angle),s=Math.sin(angle),q=rotor.size;
   // A flat propeller on its own horizontal plane: exact affine plane projection.
   rotor.blades.setAttribute('transform',`matrix(${(basis[0][0]*c+basis[1][0]*s)*q} ${(basis[0][1]*c+basis[1][1]*s)*q} ${(-basis[0][0]*s+basis[1][0]*c)*q} ${(-basis[0][1]*s+basis[1][1]*c)*q} 0 0)`);
   const f=rotor.drone==='drone-small'?.66:1;translate(rotor.node,state.droneX*f,state.droneY*f,state.droneZ*f);
  }
  host.dataset.time=state.t.toFixed(3);
 }
 function stop(){if(frame)cancelAnimationFrame(frame);frame=0;last=0;}
 function tick(now){frame=0;if(disposed||!svg||mobile.matches||reduced.matches||document.hidden||!visible){last=0;return;}if(last)elapsed+=(now-last)/1000;last=now;render(elapsed);frame=requestAnimationFrame(tick);}
 function sync(){
  stop();if(disposed)return;
  if(mobile.matches){if(svg){svg.remove();svg=null;nodes=null;}fallback.hidden=false;host.dataset.status='mobile';return;}
  if(!svg){
   try {
    svg=template.content.firstElementChild.cloneNode(true);
    host.appendChild(svg);
    const byId=Object.create(null);for(const node of svg.querySelectorAll('[data-part]'))byId[node.dataset.part]=node;
    const paintStates=PAINT_STATES.map(order=>order.map(index=>byId[PART_IDS[index]]));
    nodes={byId,parts:svg.querySelector('#station-parts'),order:PAINT_SEQUENCE.map(index=>paintStates[index]),currentOrder:null,
     moving:MOVING.map(item=>({motion:item.motion,node:byId[item.id]})),input:byId['input.card'],
     scans:[...svg.querySelectorAll('.read-scan')],read:svg.querySelector('.scanner-status'),core:[...svg.querySelectorAll('.core-glass')],
     coreScan:svg.querySelector('#core-scan'),archive:svg.querySelector('.archive-selection'),
     evidence:svg.querySelector('.evidence-selection'),evidenceMark:svg.querySelector('.evidence-mark'),
     signal:svg.querySelector('.signal-highlight'),point:svg.querySelector('.signal-point'),target:svg.querySelector('.signal-target'),
     ready:svg.querySelector('.console-ready'),ray:svg.querySelector('#drone-ray'),rayOrigin:svg.querySelector('#drone-ray-origin'),observeTarget:svg.querySelector('#observation-target'),travels:effectsTravelNodes(svg),
     rotors:ROTORS.map(rotor=>({...rotor,node:svg.querySelector('#'+rotor.id),blades:svg.querySelector('#'+rotor.id+'-blades')}))};
    render(reduced.matches?8.4:elapsed);fallback.hidden=true;
   }catch(error){if(svg)svg.remove();svg=null;nodes=null;fallback.hidden=false;host.dataset.status='fallback';return;}
  }
  if(reduced.matches){render(8.4);host.dataset.status='still';return;}
  host.dataset.status=document.hidden||!visible?'paused':'running';
  if(!document.hidden&&visible)frame=requestAnimationFrame(tick);
 }
 function effectsTravelNodes(svg){return EFFECTS.channelHosts.map((_,i)=>svg.querySelector('#signal-travel-'+i));}
 const observer=new IntersectionObserver(entries=>{visible=entries[0].isIntersecting;sync();},{threshold:0});
 function destroy(){if(disposed)return;disposed=true;stop();observer.disconnect();mobile.removeEventListener('change',sync);reduced.removeEventListener('change',sync);document.removeEventListener('visibilitychange',sync);window.removeEventListener('pagehide',pagehide);window.removeEventListener('pageshow',pageshow);if(svg)svg.remove();svg=null;nodes=null;fallback.hidden=false;}
 function pagehide(event){if(event.persisted){stop();host.dataset.status='paused';}else destroy();}
 function pageshow(event){if(event.persisted)sync();}
 mobile.addEventListener('change',sync);reduced.addEventListener('change',sync);document.addEventListener('visibilitychange',sync);window.addEventListener('pagehide',pagehide);window.addEventListener('pageshow',pageshow);
 observer.observe(host);
 // The form has already initialized; SVG installation waits for a frame after parsing.
 frame=requestAnimationFrame(sync);
 // Lifecycle API for future embedding, with no product controls or focus handlers.
 host.station={destroy,seek(seconds){elapsed=seconds;last=0;render(seconds);},get time(){return elapsed;}};
})();
