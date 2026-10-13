export const DURATION=16;
export const SCENE={width:658,height:630,azimuth:45,elevation:34,
 main:{x:-78,y:-64,w:176,d:160,r:8},archive:{x:-170,y:-22,w:82,d:107,r:6},
 track:{x:-65,y:96,w:42,d:206,r:4},output:{x:108,y:-36,w:76,d:118,r:6},
 console:{x:70,y:102,w:88,d:68,r:6},
 drones:[{id:'drone-main',x:57,y:-96,z:263,size:1},{id:'drone-small',x:-179,y:26,z:172,size:.66}],
 colors:{ink:'#0f1415',signal:'#f74d20',paper:'#fff'}};
export const smooth=v=>{v=Math.max(0,Math.min(1,v));return v*v*(3-2*v);};
export const ramp=(t,a,b)=>smooth((t-a)/(b-a));
export const envelope=(t,a,b,c,d)=>ramp(t,a,b)*(1-ramp(t,c,d));
export function stateAt(seconds,s={}){
 const t=((seconds%DURATION)+DURATION)%DURATION;
 s.t=t;
 // Entry/exit resets are hidden behind actual hoods; no visible teleport.
 s.inputY=t<4?291-141*ramp(t,.15,2.9):t<5.1?150-27*ramp(t,4,5.1):123;
 s.inputVisible=1-ramp(t,4.7,5.1);
 s.scan=envelope(t,2.9,3.1,3.8,4.05);s.scanY=146-18*ramp(t,3.05,3.8);
 s.read=envelope(t,3.5,3.9,4.8,5.1);
 s.pulse=envelope(t,4,4.13,5.5,5.65);s.pulseAlong=ramp(t,4.1,5.55);
 s.core=envelope(t,5.2,6.3,7.1,8.1);s.slice=3.2*s.core;
 s.coreScan=30*ramp(t,5.4,6.95);s.coreScanVisible=envelope(t,5.35,5.5,6.95,7.1);
 s.evidence=envelope(t,7.05,7.8,12.6,14.4);s.signal=envelope(t,8.25,9.3,12.4,14.2);
 s.archive=envelope(t,6.7,7.6,10.8,12.8);
 s.result=26*ramp(t,11.1,12.8)*(1-ramp(t,14.15,15.6));
 s.ready=envelope(t,11.35,12.1,13.6,14.9);
 s.observe=envelope(t,1.2,1.8,2.2,2.8);
 s.droneX=1.8*Math.sin(t*Math.PI/8);s.droneY=1.4*(Math.cos(t*Math.PI/8)-1);s.droneZ=.75*Math.sin(t*Math.PI/4);
 s.rotor=t*Math.PI*8;
 return s;
}
