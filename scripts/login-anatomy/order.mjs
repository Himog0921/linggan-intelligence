import {k} from './anatomy.mjs';

// Each part is a convex extrusion. Compare the near surfaces at the interior of
// their projected intersection, then topologically order the actual solids.
// This keeps a translated bridge above its bed and joint caps above their links.
export function orderGeometry(P){
  const o=k.iso([0,0,0],P),axes=[[1,0,0],[0,1,0],[0,0,1]].map(v=>k.iso(v,P));
  const rowX=axes.map(v=>v[0]-o[0]),rowY=axes.map(v=>v[1]-o[1]);
  const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0);
  const aa=dot(rowX,rowX),bb=dot(rowY,rowY),ab=dot(rowX,rowY),det=aa*bb-ab*ab;
  let V=[rowX[1]*rowY[2]-rowX[2]*rowY[1],rowX[2]*rowY[0]-rowX[0]*rowY[2],rowX[0]*rowY[1]-rowX[1]*rowY[0]];
  const len=Math.hypot(...V);V=V.map(x=>x/len);
  if(k.depthOf(V,P)<k.depthOf([0,0,0],P))V=V.map(x=>-x);
  const ray=point=>{
    const x=point[0]-o[0],y=point[1]-o[1],a=(bb*x-ab*y)/det,b=(aa*y-ab*x)/det;
    return rowX.map((v,i)=>v*a+rowY[i]*b);
  };
  return {P,V,ray};
}
function cross(a,b,c){return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);}
function hull(points){
  const sorted=points.sort((a,b)=>a[0]-b[0]||a[1]-b[1]),lo=[],hi=[];
  for(const p of sorted){while(lo.length>1&&cross(lo.at(-2),lo.at(-1),p)<=1e-8)lo.pop();lo.push(p);}
  for(const p of [...sorted].reverse()){while(hi.length>1&&cross(hi.at(-2),hi.at(-1),p)<=1e-8)hi.pop();hi.push(p);}
  return lo.slice(0,-1).concat(hi.slice(0,-1));
}
function intersection(a,b){
  let result=a;
  for(let i=0;i<b.length&&result.length;i++){
    const p=b[i],q=b[(i+1)%b.length],next=[];
    for(let j=0;j<result.length;j++){
      const u=result[j],v=result[(j+1)%result.length],du=cross(p,q,u),dv=cross(p,q,v);
      if(du>=-1e-7)next.push(u);
      if((du>=0)!==(dv>=0)){const t=du/(du-dv);next.push([u[0]+t*(v[0]-u[0]),u[1]+t*(v[1]-u[1])]);}
    }
    result=next;
  }
  return result;
}
function area(points){return Math.abs(points.reduce((s,p,i)=>{const q=points[(i+1)%points.length];return s+p[0]*q[1]-p[1]*q[0];},0))*.5;}
export function surfaceOf(item,G){
  const s=item.shape;
  if(item.kind==='prism'){
    const{F,ring,s0,s1}=s,points=[];
    for(const t of[s0,s1])for(const[u,v]of ring)points.push(F.o.map((n,i)=>n+F.a[i]*t+F.u[i]*u+F.v[i]*v));
    const faces=[ring.map((_,i)=>ring.length-1-i),ring.map((_,i)=>ring.length+i)];
    for(let i=0;i<ring.length;i++){const j=(i+1)%ring.length;faces.push([i,j,j+ring.length,i+ring.length]);}
    const center=points.reduce((p,q)=>p.map((n,i)=>n+q[i]/points.length),[0,0,0]);
    const planes=faces.map(face=>{
      const a=points[face[0]],b=points[face[1]],c=points[face[2]],u=b.map((n,i)=>n-a[i]),v=c.map((n,i)=>n-a[i]);
      let n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
      if(n.reduce((s,v,i)=>s+v*(a[i]-center[i]),0)<0)n=n.map(v=>-v);
      return {n,c:n.reduce((s,v,i)=>s+v*a[i],0)};
    });
    const outline=hull(points.map(p=>k.iso(p,G.P))),xs=outline.map(p=>p[0]),ys=outline.map(p=>p[1]);
    return {id:item.id,key:item.key,hull:outline,planes,box:[Math.min(...xs),Math.min(...ys),Math.max(...xs),Math.max(...ys)]};
  }
  let ring;
  if(item.kind==='slab'){
    const {p}=s,r=Math.min(p.r??0,p.w/2,p.d/2);ring=[];
    if(r)for(const [cx,cy,start]of[[p.x+p.w-r,p.y+r,-Math.PI/2],[p.x+p.w-r,p.y+p.d-r,0],[p.x+r,p.y+p.d-r,Math.PI/2],[p.x+r,p.y+r,Math.PI]])for(let i=0;i<=5;i++){const angle=start+i*Math.PI/10;ring.push([cx+r*Math.cos(angle),cy+r*Math.sin(angle)]);}
    else ring=[[p.x,p.y],[p.x+p.w,p.y],[p.x+p.w,p.y+p.d],[p.x,p.y+p.d]];
  }else if(item.kind==='cylinder')ring=Array.from({length:s.r>8?48:24},(_,i)=>{const a=i*Math.PI*2/(s.r>8?48:24);return[s.x+s.r*Math.cos(a),s.y+s.r*Math.sin(a)];});
  else ring=s.ring;
  const points=hull(ring.flatMap(p=>[k.iso([...p,s.z],G.P),k.iso([...p,s.z+s.h],G.P)]));
  const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]);
  const planes=ring.map((p,i)=>{const q=ring[(i+1)%ring.length],nx=q[1]-p[1],ny=p[0]-q[0];return {n:[nx,ny,0],c:nx*p[0]+ny*p[1]};});
  planes.push({n:[0,0,1],c:s.z+s.h},{n:[0,0,-1],c:-s.z});
  return {id:item.id,key:item.key,hull:points,planes,box:[Math.min(...xs),Math.min(...ys),Math.max(...xs),Math.max(...ys)]};
}
function near(surface,point,G){
  const q=G.ray(point);let lo=-Infinity,hi=Infinity;
  for(const {n,c}of surface.planes){
    const d=n[0]*G.V[0]+n[1]*G.V[1]+n[2]*G.V[2],v=c-n[0]*q[0]-n[1]*q[1]-n[2]*q[2];
    if(Math.abs(d)<1e-9){if(v<-.001)return null;continue;}
    const t=v/d;if(d>0)hi=Math.min(hi,t);else lo=Math.max(lo,t);
  }
  return hi<lo-.001?null:hi;
}
export function relationOf(a,b,G){
  if(a.box[2]<b.box[0]||b.box[2]<a.box[0]||a.box[3]<b.box[1]||b.box[3]<a.box[1])return null;
  const hit=intersection(a.hull,b.hull);if(hit.length<3||area(hit)<.015)return null;
  const point=hit.reduce((p,q)=>[p[0]+q[0]/hit.length,p[1]+q[1]/hit.length],[0,0]);
  const x=near(a,point,G),y=near(b,point,G);if(x===null||y===null||Math.abs(x-y)<.025)return null;
  return x>y?[b.id,a.id]:[a.id,b.id];
}
export function makeOrdering(fixed,P){
  const G=orderGeometry(P),staticSurfaces=fixed.map(item=>surfaceOf(item,G)),staticRules=[];
  for(let i=0;i<staticSurfaces.length;i++)for(let j=i+1;j<staticSurfaces.length;j++){const relation=relationOf(staticSurfaces[i],staticSurfaces[j],G);if(relation)staticRules.push(relation);}
  return active=>{
    const dynamic=active.map(item=>surfaceOf(item,G)),all=[...staticSurfaces,...dynamic],rules=[...staticRules];
    for(let i=0;i<dynamic.length;i++){
      for(const other of staticSurfaces){const r=relationOf(dynamic[i],other,G);if(r)rules.push(r);}
      for(let j=i+1;j<dynamic.length;j++){const r=relationOf(dynamic[i],dynamic[j],G);if(r)rules.push(r);}
    }
    const outgoing=new Map(all.map(x=>[x.id,[]])),degree=new Map(all.map(x=>[x.id,0])),keys=new Map(all.map(x=>[x.id,x.key]));
    for(const [back,front]of rules){outgoing.get(back).push(front);degree.set(front,degree.get(front)+1);}
    const ready=all.filter(x=>degree.get(x.id)===0).map(x=>x.id),result=[];
    while(ready.length){ready.sort((a,b)=>keys.get(a)-keys.get(b));const id=ready.shift();result.push(id);for(const next of outgoing.get(id)){degree.set(next,degree.get(next)-1);if(degree.get(next)===0)ready.push(next);}}
    if(result.length!==all.length){
      const seen=new Set(),stack=[],active=new Set();let cycle=null;
      function visit(id){if(active.has(id)){cycle=stack.slice(stack.indexOf(id)).concat(id);return true;}if(seen.has(id))return false;seen.add(id);stack.push(id);active.add(id);for(const next of outgoing.get(id))if(visit(next))return true;stack.pop();active.delete(id);return false;}
      for(const one of all)if(visit(one.id))break;
      throw new Error('Depth interlock: '+cycle?.join(' -> '));
    }
    return result;
  };
}
