import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {validateContracts,validateChange,componentFingerprints,ROOT} from './lids.mjs';
const c=JSON.parse(await fs.readFile(ROOT+'/docs/design/lids/component-contracts.json','utf8'));
test('contracts keep query policy and certification boundaries',()=>{
 validateContracts(c);const changed=structuredClone(c);changed.components.C16.debounceMs=50;assert.throws(()=>validateContracts(changed),/query semantics/);
 const certified=structuredClone(c);certified.components.C09.certified=true;assert.throws(()=>validateContracts(certified),/CMP/);
 const escaped=structuredClone(c);escaped.components.C16.implementation='../outside.tsx';assert.throws(()=>validateContracts(escaped),/escapes/);
});
test('a changed implementation, test, rule or contract invalidates its previous declaration',async()=>{
 const base=await componentFingerprints(c,async p=>p),record={kind:'implementation_fix',reason:'existing standard compliance',authorization:'Mog scoped task',componentFingerprints:base};
 validateChange(record,base);
 for(const changed of [c.components.C16.implementation,c.components.C16.styles,c.components.C16.tests[0],'docs/design/lids/focus.json']){
  const fingerprints=await componentFingerprints(c,async p=>p+(p===changed?' changed':''));assert.throws(()=>validateChange(record,fingerprints),/stale/);
 }
 const contract=structuredClone(c);contract.components.C16.states.push('another-state');const next=await componentFingerprints(contract,async p=>p);assert.throws(()=>validateChange(record,next),/stale/);
});
test('page exceptions require bounded scope; missing reason is rejected',()=>{
 assert.throws(()=>validateChange({kind:'page_exception',reason:'local',authorization:'Mog',componentFingerprints:{}},{}),/exceptionScope/);
 assert.throws(()=>validateChange({kind:'shared_standard',reason:'',authorization:'Mog'},{}),/reason/);
});
