const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const dir=process.argv[2];if(!dir)throw Error('usage: node check-inputs.cjs PRIVATE_INPUT_DIR');
const sha=x=>crypto.createHash('sha256').update(x).digest('hex'),read=name=>fs.readFileSync(path.join(dir,name),'utf8');
const manifest=JSON.parse(read('input-manifest.json')),results=[];
for(const s of manifest.samples){
 const packet=JSON.parse(read(s.sampleId+'.input.json')),fingerprint=packet.inputFingerprint,core={...packet};delete core.inputFingerprint;
 assert.equal(sha(JSON.stringify(core)),fingerprint);assert.equal(fingerprint,s.inputFingerprint);
 const requests=['B0','B1'].map(v=>{
  const bytes=read(`${s.sampleId}.${v}.request.json`);assert.equal(sha(bytes),s.variants[v]);const r=JSON.parse(bytes);
  assert.deepEqual(r.input,packet);assert.deepEqual(r.outputSchema,require('./contract.cjs'));
  assert.equal(r.system,fs.readFileSync(path.join(__dirname,'common.txt'),'utf8'));
  assert.equal(r.method,fs.readFileSync(path.join(__dirname,v+'.txt'),'utf8'));assert.equal(r.variant,v);
  const comparison={...r};delete comparison.method;delete comparison.variant;return comparison;
 });
 assert.deepEqual(requests[0],requests[1]);
 assert.equal(new Set([...packet.sources,...packet.comments].map(x=>x.ref)).size,packet.sources.length+packet.comments.length);
 assert.equal(packet.sources.filter(x=>x.kind==='platform_title').length,1);
 assert.equal(packet.metricsIncluded,false);assert.equal(packet.mediaMode,'derived_text_only');
 results.push({sampleId:s.sampleId,inputFingerprint:fingerprint,identicalExceptMethodAndVariant:true,requestBodyBytes:['B0','B1'].map(v=>Buffer.byteLength(read(`${s.sampleId}.${v}.request.json`))),providerCalls:0});
}
fs.writeFileSync(path.join(dir,'input-checks.json'),JSON.stringify(results,null,2)+'\n');console.log(JSON.stringify(results));
