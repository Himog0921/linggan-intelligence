import {k} from './anatomy.mjs';
import {SCENE} from './model.mjs';

export const makeProjection=()=>k.fitProjection(
 [SCENE.main,SCENE.archive,SCENE.track,SCENE.output,SCENE.console].flatMap(p=>k.boxCorners(p,0,28)).concat(
 k.boxCorners({x:-66,y:-109,w:8,d:123,r:2},86,219),
 k.boxCorners({x:-27,y:-71,w:110,d:8,r:2},73,203),
 [[32,-121,263],[82,-71,279],[-197,8,172],[-161,44,184],[186,-58,152]]),
 SCENE.width,SCENE.height,{pad:28,azimuth:SCENE.azimuth,elevation:SCENE.elevation});
export function makeParts(P){
 const items=[];
 const add=(id,kind,shape,paths,at,o={})=>{
  const svg=k.solidSvg(paths,{tone:o.tone??'mid',crease:'faint',inner:o.inner??'',className:o.className});
  const item={id,kind,shape,paths,at,svg,group:o.group??id.split('.')[0],motion:o.motion??null,flightRoot:o.flightRoot??false,ground:o.ground??false};items.push(item);return item;
 };
 const slab=(id,p,z,h,o={})=>add(id,'slab',{p,z,h},k.slabOf(p,z,h,P,5,Math.min(.7,h*.2)),[p.x+p.w/2,p.y+p.d/2,z+h/2],o);
 const round=(id,x,y,r,z,h,o={})=>add(id,'cylinder',{x,y,r,z,h},k.cylinder(x,y,r,z,h,P,r>5?48:28,Math.min(.4,h*.2)),[x,y,z+h/2],o);
 const poly=(id,ring,z,h,o={})=>add(id,'poly',{ring,z,h},k.extrude(ring,z,h,P,{bevel:.4}),[ring.reduce((s,p)=>s+p[0],0)/ring.length,ring.reduce((s,p)=>s+p[1],0)/ring.length,z+h/2],o);
 // Oriented convex prisms are used for the physical sloped console, not a line drawn on a box.
 const prism=(id,F,ring,s0,s1,o={})=>{
  const points=[];for(const s of [s0,s1])for(const [u,v]of ring)points.push(F.o.map((n,i)=>n+F.a[i]*s+F.u[i]*u+F.v[i]*v));
  const faces=[ring.map((_,i)=>ring.length-1-i),ring.map((_,i)=>ring.length+i)];
  for(let i=0;i<ring.length;i++){const j=(i+1)%ring.length;faces.push([i,j,j+ring.length,i+ring.length]);}
  const cam=k.cameraOf(P),view=[Math.cos(SCENE.azimuth*Math.PI/180)*cam.cosE,Math.sin(SCENE.azimuth*Math.PI/180)*cam.cosE,cam.sinE];
  const center=points.reduce((s,p)=>s.map((n,i)=>n+p[i]/points.length),[0,0,0]);
  let sides='',top='',crease='';const visible=[];
  for(const face of faces){
   const a=points[face[0]],b=points[face[1]],c=points[face[2]],ab=b.map((v,i)=>v-a[i]),ac=c.map((v,i)=>v-a[i]);
   let n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];
   const fc=face.reduce((s,i)=>s.map((v,j)=>v+points[i][j]/face.length),[0,0,0]);if(n.reduce((s,v,i)=>s+v*(fc[i]-center[i]),0)<0)n=n.map(v=>-v);
   if(n.reduce((s,v,i)=>s+v*view[i],0)<=0)continue;
   const d=k.pathOf(face.map(i=>k.iso(points[i],P)),true);if(n[2]>0)top+=d;else sides+=d;crease+=d;visible.push(face);
  }
  const edges=new Map();for(const face of visible)for(let i=0;i<face.length;i++){const a=face[i],b=face[(i+1)%face.length],key=[a,b].sort((a,b)=>a-b).join('-');edges.set(key,(edges.get(key)??0)+1);}
  const outline=[...edges].filter(([,n])=>n===1).map(([key])=>{const[a,b]=key.split('-').map(Number);return k.pathOf([k.iso(points[a],P),k.iso(points[b],P)]);}).join('');
  return add(id,'prism',{F,ring,s0,s1}, {fill:sides+top,shades:[sides,'','',''],top,crease,outline},center,o);
 };
 const bolts=(p,z,inset=5)=>k.lineSvg(k.corners(p,inset).map(([x,y])=>k.ring(x,y,1.2,z,P,16)).join(''),{tone:'lo'})+k.dotsSvg(k.corners(p,inset).map(([x,y])=>k.iso([x,y,z],P)),{size:.35,tone:'mid'});
 const seam=(p,z)=>k.lineSvg(k.sideSeam(p,z,P,5),{tone:'lo'});
 const closed=(p,z,tone='faint')=>k.lineSvg(k.planOutline(p,z,P,5),{tone});
 const printed=(p,z,variant=0)=>closed(k.insetPlan(p,2),z,'lo')+k.lineSvg([0,1,2].map(i=>k.lineOnTop([p.x+5,p.y+6+i*5],[p.x+p.w-5-(i===variant?5:0),p.y+6+i*5],z,P)).join(''),{tone:'mid',free:true});
 const topPort=(id,p,z)=>slab(id,p,z,.8,{className:'dark-metal',inner:closed(k.insetPlan(p,1),z+.8,'mid')});
 const foundation=(id,p,top=26)=>{
  for(const [i,[x,y]]of k.corners(p,9).entries()){round(id+'.foot-'+i,x,y,4.2,0,3,{className:'rubber',ground:true});round(id+'.leg-'+i,x,y,3.2,3,2,{tone:'lo'});}
  slab(id+'.floor',p,5,4,{tone:'mid',inner:seam(p,7)});
  const q=k.insetPlan(p,2);slab(id+'.middle',q,9,top-13,{tone:'mid',inner:seam(q,top-8)});
  slab(id+'.deck',p,top-4,4,{tone:'mid',inner:bolts(p,top,7)+closed(k.insetPlan(p,4),top)});
 };
 foundation('platform',SCENE.main);foundation('archive-base',SCENE.archive);foundation('output-base',SCENE.output);foundation('console-base',SCENE.console);

 // The joints between independently readable devices are narrow physical bridges.
 for(const [id,x,y,w,d]of[['archive-link',-88,36,10,12],['output-link',98,16,10,12],['console-link',78,96,16,6]]){
  slab(id+'.foot',{x:x+2,y:y+2,w:w-4,d:d-4,r:.8},0,5,{ground:true,tone:'lo'});
  slab(id+'.support',{x:x+2,y:y+2,w:w-4,d:d-4,r:.8},5,17,{tone:'lo'});
  slab(id+'.bridge',{x,y,w,d,r:1.2},22,4,{inner:closed({x:x+2,y:y+2,w:w-4,d:d-4,r:.8},26)});
 }
 const stage={x:-67,y:-40,w:152,d:114,r:9};slab('platform.processing-frame',stage,26,8,{tone:'mid',inner:seam(stage,30)});
 slab('platform.processing-deck',stage,34,3,{tone:'mid',inner:bolts(stage,37,8)+closed(k.insetPlan(stage,5),37)});
 const accessFace={x:-52,y:74,w:30,d:.3,r:.2};
 slab('platform.access-panel',accessFace,27.5,5.5,{tone:'lo',inner:`<g transform="${k.sideMatrix([-49,74.3],0,P,'left')}"><rect class="panel-inset" x="0" y="-31.5" width="24" height="2.5" rx=".3"/><rect class="vent-slot" x="3" y="-31" width="10" height="1" rx=".3"/></g>`});
 for(const [i,[x,y]]of k.corners(stage,9).entries())slab('platform.corner-lock-'+i,{x:x-2,y:y-2,w:4,d:4,r:1},37,.7,{className:'dark-metal'});
 for(const [i,p]of[{x:-76,y:63,w:7,d:21,r:1},{x:87,y:43,w:9,d:23,r:1},{x:-9,y:85,w:27,d:8,r:1}].entries())topPort('platform.service-port-'+i,p,26);
 const scale={x:-46,y:69,w:72,d:4,r:.4};slab('platform.scale',scale,37,.4,{tone:'lo',inner:closed(scale,37.4,'lo')+k.lineSvg(k.topTicks(-44,24,4,5,69,37.4,[1.2,2.8],P).minor,{tone:'lo'})+k.lineSvg(k.topTicks(-44,24,4,5,69,37.4,[1.2,2.8],P).major,{tone:'mid'})});

 // Long low track; decks, carriers and gate walls never pass through each other.
 const track=SCENE.track;
 for(const [i,[x,y]]of k.corners(track,8).entries()){round('track.foot-'+i,x,y,3.2,0,4,{className:'rubber',ground:true});}
 slab('track.floor',track,4,4,{tone:'mid',inner:seam(track,6)});
 slab('track.bed',{x:-61,y:96,w:34,d:206,r:1},8,.6,{className:'dark-metal'});
 for(const x of [-65,-27])slab('track.guide-'+x,{x,y:96,w:4,d:206,r:1.2},8,5,{tone:'mid',inner:closed({x:x+.9,y:98,w:2.2,d:202,r:.5},13,'lo')});
 for(let i=0;i<14;i++)slab('track.segment-'+i,{x:-60,y:99+i*14,w:32,d:2,r:.5},8.6,1.4,{className:'track-segment'});
 for(const [id,y,d,h]of[['entry',268,34,10],['receiver',96,31,13]]){
  for(const x of [-61,-30])slab('track.'+id+'-wall-'+x,{x,y,w:3,d,r:.6},10,h,{tone:'mid'});
  slab('track.'+id+'-hood',{x:-61,y,w:34,d,r:1},10+h,3,{tone:'mid',inner:bolts({x:-61,y,w:34,d,r:1},13+h,4)});
 }
 for(const [i,y]of[170,250].entries())slab('track.signal-inlay-'+i,{x:-65,y,w:4,d:9,r:.3},13,.2,{className:'orange-mark'});
 const paper={x:-58,y:267,w:16,d:21,r:1};slab('input.card',paper,10,.7,{tone:'lo',motion:'input',inner:printed(paper,10.7,1)+`<path class="paper-image" d="${k.planOutline({x:-54,y:277,w:6,d:6,r:.3},10.7,P)}"/>`});
 const comment={x:-40,y:269,w:10,d:17,r:.7};
 slab('input.comment-card',comment,10,.7,{tone:'lo',motion:'input',inner:closed(k.insetPlan(comment,1),10.7,'lo')+k.lineSvg([0,1,2].map(i=>k.lineOnTop([-38,274+i*3],[-32-(i===2?2:0),274+i*3],10.7,P)).join(''),{tone:'mid',free:true})+`<path class="paper-tab" d="${k.planOutline({x:-38,y:271,w:4,d:1,r:.1},10.7,P)}"/>`});

 // Portal scanner sits across the track, with a real open throat and elevated reader.
 for(const x of [-70,-23]){
  slab('scanner.foot-'+x,{x,y:123,w:5,d:13,r:1},4,6,{tone:'mid'});
  slab('scanner.post-'+x,{x,y:124,w:5,d:11,r:1},10,52,{tone:'mid',inner:seam({x,y:124,w:5,d:11,r:1},47)});
  slab('scanner.latch-'+x,{x:x+.5,y:126,w:4,d:5,r:.5},62,2,{className:'dark-metal'});
 }
 slab('scanner.bridge',{x:-70,y:124,w:52,d:11,r:1.5},64,6,{tone:'mid',inner:bolts({x:-70,y:124,w:52,d:11,r:1.5},70,3)});
 slab('scanner.read-window',{x:-64,y:134.99,w:40,d:.6,r:.2},64.5,3,{className:'dark-metal'});
 slab('scanner.status',{x:-18,y:126,w:.4,d:6,r:.2},32,18,{className:'orange-mark scanner-status'});
 const scannerSide=k.sideMatrix([-17.99,127],0,P,'right');
 const vent=`<g transform="${scannerSide}">${[0,1,2,3].map(i=>`<rect class="vent-slot" x="0" y="${-54+i*4}" width="4" height="1.3" rx=".5"/>`).join('')}</g>`;
 // Plane-mounted details are attached to their actual scanner side face.
 items.find(i=>i.id==='scanner.post--23').svg=items.find(i=>i.id==='scanner.post--23').svg.replace('</g>',vent+'</g>');

 // Archive: stepped rear layers, an inset panel and a restrained record selection.
 const archiveBody={x:-155,y:5,w:55,d:18,r:2};
 slab('archive.heel',{x:-160,y:0,w:65,d:30,r:2},26,5,{tone:'mid',inner:bolts({x:-160,y:0,w:65,d:30,r:2},31,4)});
 slab('archive.back',archiveBody,31,82,{tone:'mid',inner:seam(archiveBody,102)});
 slab('archive.layer-one',{x:-158,y:7,w:3,d:14,r:.5},34,76,{tone:'lo'});
 slab('archive.layer-two',{x:-161,y:9,w:3,d:10,r:.4},37,70,{tone:'lo'});
 slab('archive.screen',{x:-147,y:23,w:39,d:.4,r:.3},43,58,{className:'archive-window',inner:archiveContents(P)});
 slab('archive.side-strip',{x:-105,y:23,w:2,d:.4,r:.2},45,54,{className:'orange-mark'});
 topPort('archive.top-slot',{x:-148,y:10,w:36,d:5,r:.4},113);
 for(const x of [-155,-109])slab('archive.top-clip-'+x,{x,y:11,w:6,d:5,r:.5},113,1.5,{tone:'mid'});
 topPort('archive.connector',{x:-122,y:24,w:14,d:3,r:.4},31);

 // Double screens: two thick shells, narrow rims, true stands and different faces.
 const left={x:-66,y:-109,w:8,d:123,r:2};
 slab('evidence.mount',{x:-66,y:-28,w:16,d:30,r:2},37,4,{tone:'mid',inner:bolts({x:-66,y:-28,w:16,d:30,r:2},41,3)});
 for(const y of [-20,-9])slab('evidence.stem-'+y,{x:-61,y,w:5,d:4,r:.7},41,45,{tone:'mid'});
 slab('evidence.hinge',{x:-66,y:-25,w:8,d:27,r:1},86,4,{tone:'lo'});
 slab('evidence.shell',left,90,129,{tone:'mid',inner:seam(left,212)});
 slab('evidence.face',{x:-58,y:-104,w:.35,d:113,r:.5},98,112,{className:'screen-glass',inner:evidenceContents(P)});
 slab('evidence.jack',{x:-58,y:-35,w:.4,d:16,r:.2},93,2,{className:'dark-metal'});
 for(const y of [-106,7])slab('evidence.frame-clip-'+y,{x:-58,y,w:1,d:3,r:.2},212,2,{tone:'lo'});
 const right={x:-27,y:-71,w:110,d:8,r:2};
 slab('signal.mount',{x:20,y:-62,w:25,d:18,r:2},26,4,{tone:'mid',inner:bolts({x:20,y:-62,w:25,d:18,r:2},30,4)});
 for(const x of [27,38])slab('signal.stem-'+x,{x,y:-57,w:4,d:6,r:.7},30,43,{tone:'mid'});
 slab('signal.hinge',{x:25,y:-71,w:20,d:20,r:1},73,4,{tone:'lo'});
 slab('signal.shell',right,77,126,{tone:'mid',inner:seam(right,196)});
 slab('signal.face',{x:-22,y:-63,w:100,d:.35,r:.5},84,111,{className:'screen-glass',inner:signalContents(P)});
 slab('signal.jack',{x:25,y:-63,w:12,d:.4,r:.2},80,2,{className:'dark-metal'});

 // The core has thin translucent walls and plates sliding on a real central spindle.
 const seat={x:-28,y:-3,w:64,d:64,r:4};slab('core.seat',seat,37,4,{className:'dark-metal',inner:closed(k.insetPlan(seat,3),41,'mid')});
 for(const [i,[x,y]]of k.corners(seat,2).entries())round('core.spigot-'+i,x,y,1,41,3,{tone:'lo'});
 slab('core.spindle',{x:-14,y:11,w:32,d:32,r:.6},41,49,{className:'core-inner'});
 for(let i=0;i<3;i++){
  const z=53+i*15,o={className:'core-slice',motion:'slice'};
  slab('core.slice-'+i+'-back',{x:-20,y:5,w:44,d:6,r:.5},z,.65,o);
  slab('core.slice-'+i+'-front',{x:-20,y:43,w:44,d:6,r:.5},z,.65,o);
  slab('core.slice-'+i+'-left',{x:-20,y:11,w:6,d:32,r:.3},z,.65,o);
  slab('core.slice-'+i+'-right',{x:18,y:11,w:6,d:32,r:.3},z,.65,o);
 }
 for(const [id,p]of[['back',{x:-24,y:1,w:56,d:1,r:.2}],['left',{x:-24,y:2,w:1,d:54,r:.2}],['right',{x:31,y:2,w:1,d:54,r:.2}],['front',{x:-24,y:56,w:56,d:1,r:.2}]])slab('core.shell-'+id,p,41,56,{className:'core-glass'});
 slab('core.cap',{x:-24,y:1,w:56,d:56,r:.2},97,1,{className:'core-glass'});

 // Low console: a real wedge whose controls and display lie on its sloped top.
 const F={o:[0,0,0],a:[1,0,0],u:[0,1,0],v:[0,0,1]};
 const slope=y=>57-(y-111)*.28;
 prism('console.body',F,[[111,26],[163,26],[163,slope(163)],[111,57]],76,151,{tone:'mid'});
 prism('console.display',F,[[115,slope(115)+.08],[142,slope(142)+.08],[142,slope(142)+.45],[115,slope(115)+.45]],82,145,{className:'console-screen',inner:consoleContents(P,slope)});
 for(const [i,x]of[84,99,114].entries())prism('console.key-'+i,F,[[149,slope(149)+.1],[157,slope(157)+.1],[157,slope(157)+1.3],[149,slope(149)+1.3]],x,x+9,{tone:'lo'});
 prism('console.dial-socket',F,[[145,slope(145)+.08],[157,slope(157)+.08],[157,48.36],[145,48.36]],130,142,{tone:'lo'});
 round('console.dial',136,151,4.7,48.36,2,{tone:'mid',inner:k.lineSvg(k.knurl(136,151,4.7,48.36,50.36,24,P),{tone:'lo'})});
 topPort('console.port',{x:76,y:104,w:11,d:4,r:.4},26);
 slab('console.front-panel',{x:91,y:163,w:45,d:.25,r:.2},29,8,{tone:'lo',inner:`<g transform="${k.sideMatrix([95,163.25],0,P,'left')}">${[0,1,2].map(i=>`<rect class="vent-slot" x="${i*7}" y="-34" width="5" height="1.4" rx=".4"/>`).join('')}<rect class="panel-inset" x="26" y="-34.5" width="10" height="3" rx=".4"/></g>`});

 // Output terminal: an open paper mouth behind an opaque upper cover.
 const out={x:122,y:17,w:46,d:33,r:2};
 slab('result.floor',out,26,3,{tone:'mid'});
 for(const x of [122,163])slab('result.side-'+x,{x,y:17,w:5,d:33,r:1},29,30,{tone:'mid'});
 slab('result.back',{x:127,y:17,w:36,d:5,r:.7},29,30,{tone:'mid'});
 slab('result.upper-case',{x:127,y:22,w:36,d:28,r:1},32,27,{tone:'mid',inner:seam({x:127,y:22,w:36,d:28,r:1},54)});
 slab('result.cap',out,59,3,{tone:'mid',inner:bolts(out,62,5)+closed(k.insetPlan(out,4),62)});
 slab('result.service-panel',{x:132,y:50,w:26,d:.3,r:.3},37,15,{tone:'lo',inner:`<g transform="${k.sideMatrix([135,50.3],0,P,'left')}"><rect class="panel-inset" x="0" y="-49" width="20" height="9" rx=".4"/>${[0,1,2].map(i=>`<rect class="vent-slot" x="3" y="${-47+i*2.5}" width="14" height="1" rx=".3"/>`).join('')}</g>`});
 topPort('result.status-screen',{x:135,y:25,w:22,d:15,r:1},62);
 const report={x:129,y:22,w:32,d:25,r:.6};slab('result.report',report,29.1,.6,{tone:'lo',motion:'result',inner:printed(report,29.7,2)+`<path class="paper-tab" d="${k.planOutline({x:133,y:42,w:9,d:2,r:.2},29.7,P)}"/>`});
 slab('result.lip',{x:127,y:50,w:36,d:3,r:.5},29,.1,{className:'dark-metal'});
 const aux={x:114,y:-23,w:29,d:29,r:2};slab('output.archive-mini',aux,26,35,{tone:'mid',inner:seam(aux,52)});
 slab('output.archive-mini-panel',{x:119,y:6,w:19,d:.3,r:.4},34,21,{tone:'lo',inner:`<g transform="${k.sideMatrix([121,6.3],0,P,'left')}"><rect class="panel-inset" x="0" y="-53" width="15" height="17" rx=".4"/><path class="archive-text" data-free="true" d="M2 -49 H11 M2 -46 H12 M2 -40 H10"/></g>`});
 topPort('output.archive-mini-slot',{x:118,y:-18,w:21,d:5,r:.5},61);
 slab('output.archive-mini-band',{x:114,y:-16,w:.5,d:6,r:.2},33,22,{className:'orange-mark'});
 topPort('output.archive-mini-interface',{x:123,y:7,w:13,d:4,r:.4},26);
 for(const y of [-32,8]){
  slab('output.panel-shoe-'+y,{x:174,y,w:8,d:7,r:.8},26,3,{tone:'lo'});
  slab('output.panel-stem-'+y,{x:177,y:y+2,w:2,d:3,r:.4},29,41,{tone:'lo'});
 }
 slab('output.side-panel',{x:177,y:-49,w:2,d:66,r:1},70,78,{tone:'lo',inner:sidePanelContents(P)});

 // Drones are powered free bodies. Motors are seated on arms, lenses under the chassis.
 const rotorPlanes=[];
 for(const drone of SCENE.drones){
  const{id,x,y,z,size:q}=drone,o={group:id,motion:id};
  const bp={x:x-7*q,y:y-8*q,w:14*q,d:16*q,r:2*q};
  slab(id+'.body',bp,z,7*q,{...o,tone:'mid',flightRoot:true,inner:bolts(bp,z+7*q,2*q)});
  round(id+'.lens-seat',x,y,3.6*q,z-2.5*q,2.5*q,{...o,className:'dark-metal'});
  round(id+'.lens',x,y,2.5*q,z-4*q,1.5*q,{...o,className:'dark-metal',inner:k.lineSvg(k.ring(x,y,1.3*q,z-2.5*q,P,16),{tone:'mid'})});
  for(const [i,[dx,dy]]of[[-1,-1],[1,-1],[-1,1],[1,1]].entries()){
   const mx=x+15*q*dx,my=y+14*q*dy;
   const ax=x+8.1*q*dx,ay=y+5*q*dy;
   const ring=capsule([ax,ay],[mx,my],1.1*q);
   poly(id+'.arm-'+i,ring,z+2.8*q,1.7*q,{...o,tone:'mid'});
   round(id+'.motor-'+i,mx,my,2.1*q,z+4.5*q,3.2*q,{...o,className:'dark-metal'});
   round(id+'.hub-'+i,mx,my,1.4*q,z+7.7*q,.7*q,{...o,tone:'mid'});
   rotorPlanes.push({id:id+'-rotor-'+i,drone:id,x:mx,y:my,z:z+8.4*q,size:q,phase:i%2*Math.PI/2});
  }
  for(const dx of [-1,1])slab(id+'.landing-'+dx,{x:x+dx*5*q-.7*q,y:y-5*q,w:1.4*q,d:10*q,r:.5*q},z-1.4*q,1.4*q,{...o,tone:'lo'});
 }
 return {items,rotorPlanes};
}

function capsule(a,b,r){
 const angle=Math.atan2(b[1]-a[1],b[0]-a[0]),ring=[];
 for(let i=0;i<=12;i++){const q=angle-Math.PI/2+i*Math.PI/12;ring.push([b[0]+r*Math.cos(q),b[1]+r*Math.sin(q)]);}
 for(let i=0;i<=12;i++){const q=angle+Math.PI/2+i*Math.PI/12;ring.push([a[0]+r*Math.cos(q),a[1]+r*Math.sin(q)]);}return ring;
}
function archiveContents(P){
 return `<g transform="${k.sideMatrix([-145,23.4],0,P,'left')}"><rect x="0" y="-96" width="35" height="48" rx=".5" class="archive-paper"/><path class="archive-text" data-free="true" d="M4 -90 H25 M4 -84 H29 M4 -80 H24 M4 -69 H29 M4 -65 H24 M4 -58 H25"/><rect class="archive-selection" x="3" y="-73" width="29" height="12" rx=".5"/><rect class="paper-tab" x="4" y="-90" width="4" height="1"/></g>`;
}
function evidenceContents(P){
 return `<g transform="${k.sideMatrix([-57.65,9],0,P,'right')}"><defs><clipPath id="evidence-window"><rect x="0" y="-202" width="99" height="99" rx="1"/></clipPath></defs><g clip-path="url(#evidence-window)"><text class="screen-title" x="2" y="-188">REAL CONTENT</text><text class="screen-subtitle" x="2" y="-178">EVIDENCE / COMPARE</text><path class="screen-rule" d="M2 -171 H94" data-free="true"/>${[0,1,2].map(i=>`<rect class="screen-card" x="2" y="${-165+i*18}" width="93" height="14" rx="1"/><path class="screen-ink" data-free="true" d="M8 ${-160+i*18} H${71-i*8} M8 ${-156+i*18} H${85-i*13}"/>`).join('')}<rect class="evidence-selection" x="2" y="-147" width="93" height="14" rx="1"/><path class="evidence-mark" d="M2 -147 V-133"/><text class="screen-tiny" x="2" y="-105">SOURCE · CONTEXT · CONTRAST</text></g></g>`;
}
function signalContents(P){
 const grid=[0,1,2,3].map(i=>`M5 ${-153+i*16} H86`).join('');
 const curves=[0,1,2,3,4].map(i=>{
  const y=-102-i*7;return `<path class="signal-curve" d="M5 ${y} L17 ${y-2} L28 ${y+1} L37 ${y-13} L44 ${y-5} L54 ${y-28} L63 ${y-6} L73 ${y-12} L86 ${y-8}"/>`;
 }).join('');
 return `<g transform="${k.sideMatrix([-17,-62.65],17,P,'left')}"><defs><clipPath id="signal-window"><rect x="0" y="-172" width="86" height="83" rx="1"/></clipPath></defs><g clip-path="url(#signal-window)"><text class="screen-subtitle" x="4" y="-163">SIGNAL / OBSERVATION</text><rect class="signal-grid" x="5" y="-158" width="81" height="64"/><path class="signal-grid" d="${grid}"/>${curves}<path class="signal-highlight" d="M37 -136 L44 -128 L54 -151 L63 -129"/><circle class="signal-point" cx="54" cy="-151" r="1.7"/><circle class="signal-target" cx="54" cy="-151" r="4"/></g></g>`;
}
function consoleContents(P,slope){
 const pts=[[87,120],[138,120],[138,137],[87,137]].map(([x,y])=>[x,y,slope(y)+.48]);
 const line=(a,b)=>k.pathOf([k.iso([a[0],a[1],slope(a[1])+.5],P),k.iso([b[0],b[1],slope(b[1])+.5],P)]);
 return `<path class="console-rule" data-free="true" d="${line([89,120],[133,120])+line([89,124],[118,124])+line([89,130],[123,130])}"/><path class="console-ready" d="${k.pathOf([[130,128],[138,128],[138,134],[130,134]].map(([x,y])=>k.iso([x,y,slope(y)+.5],P)),true)}"/>`;
}
function sidePanelContents(P){
 return `<g transform="${k.sideMatrix([179,12],0,P,'right')}"><rect class="side-panel-rule" x="1" y="-137" width=".5" height="47"/><text class="side-label" x="6" y="-132">TRENDS</text><text class="side-label" x="6" y="-122">PEOPLE</text><text class="side-label" x="6" y="-112">TOPICS</text><text class="side-label" x="6" y="-102">EVIDENCE</text><rect class="side-panel-mark" x="1" y="-91" width="2" height="9"/></g>`;
}

export function effectPaths(P){
 const channel=[[-44,120,26],[-44,96,26],[-44,74,26],[-44,74,34],[-44,74,37],[-44,55,37],[-28,55,37]],endpoints=channel.map(p=>k.iso(p,P));
 const lengths=[];let total=0;for(let i=1;i<endpoints.length;i++){total+=Math.hypot(endpoints[i][0]-endpoints[i-1][0],endpoints[i][1]-endpoints[i-1][1]);lengths.push(total);}
 return {channel:k.pathOf(endpoints),channelPoints:endpoints,channelLengths:lengths,channelTotal:total,
  channelHosts:['track.receiver-hood','platform.deck','platform.processing-frame','platform.processing-deck','platform.processing-deck','platform.processing-deck'],
  scan:k.pathOf([k.iso([-59,145,10.82],P),k.iso([-29,145,10.82],P)]),
  coreScan:k.planOutline({x:-21,y:4,w:50,d:50,r:.3},55,P),
  target:k.planOutline({x:-146,y:9,w:14,d:10,r:1},113.8,P),
  droneOrigin:k.iso([-179,26,169.36],P),observationPoint:k.iso([-139,14,113.8],P),
  droneRay:k.pathOf([k.iso([-179,26,169.36],P),k.iso([-139,14,113.8],P)])};
}
