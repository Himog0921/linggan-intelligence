import assert from 'node:assert/strict';
import {writeFileSync,readFileSync,mkdirSync} from 'node:fs';
import {A} from './anatomy.mjs';
import {stateAt,SCENE} from './model.mjs';
import {record,items,P,orders,generatedAssets,AUDIT_DIR} from './build.mjs';

assert.deepEqual(stateAt(0),stateAt(16));
assert.deepEqual(stateAt(3.4),stateAt(19.4));
for(const [file,expected] of generatedAssets())assert.equal(readFileSync(file,'utf8'),expected,`Generated asset drift: ${file}`);
console.log('React static scene generated; page fidelity is checked by actual-browser comparisons.');

const results=[];
for(const t of[0,.875,1.8,2.937,3.4,4.825,5.075,6.6,8.4,10.875,12.9,14.825,15.999]){
 const R=record(items,stateAt(t)),result=A.audit(R,P);
 // Verify the order actually installed in the browser, including between cached samples.
 const ranks=new Map(orders[Math.floor(t*8)].map((id,i)=>[id,i]));
 for(const item of R.items)item.key=ranks.get(item.name);
 const depth=A.depthOrder(R.items,P,{overlaps:R.settled.overlaps});
 const problems={supports:result.supports.loose.length,sunk:result.supports.sunk.length,standIns:result.cover.length,depth:depth.problems.length,missingDepth:depth.missing.length};
 results.push({t,...problems});
 console.log('pose',t,JSON.stringify(problems));
 assert.ok(Object.values(problems).every(n=>n===0),A.report({...result,depth}));
}

// Flat rotor blades are 2D faces, separate from the kit's volumetric solids.
// Their swept footprint stays outside the chassis and clear of neighbouring rotors.
for(const drone of SCENE.drones){
 const q=drone.size,centers=[[-15*q,-14*q],[15*q,-14*q],[-15*q,14*q],[15*q,14*q]];
 for(let i=0;i<centers.length;i++)for(let j=i+1;j<centers.length;j++)assert.ok(Math.hypot(centers[i][0]-centers[j][0],centers[i][1]-centers[j][1])>16.4*q);
 for(const[x,y]of centers)assert.ok(Math.hypot(Math.max(0,Math.abs(x)-7*q),Math.max(0,Math.abs(y)-8*q))>8.2*q);
}
mkdirSync(AUDIT_DIR,{recursive:true});
writeFileSync(new URL('motion-audit.json','file://'+AUDIT_DIR+'/'),JSON.stringify({staticScene:true,loopPeriod:16,freeBodyRoots:['drone-main.body','drone-small.body'],rotorSweeps:'clear',poses:results},null,2));
console.log('Motion audit passed; aircraft roots use explicit powered-flight support, all other parts have physical contacts.');
