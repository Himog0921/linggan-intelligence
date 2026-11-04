// Local-only export of already shared samples; never provider dispatch.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const [sourceRoot,outputDir]=process.argv.slice(2);
if(!sourceRoot||!outputDir)throw Error('usage: node freeze-inputs.cjs SHARE_ROOT PRIVATE_OUTPUT');
const sha=x=>crypto.createHash('sha256').update(x).digest('hex');
const manifest=JSON.parse(fs.readFileSync(path.join(sourceRoot,'manifest.json'),'utf8'));
const hashes=new Map(manifest.files.map(x=>[x.path,x]));
function read(rel){const b=fs.readFileSync(path.join(sourceRoot,rel)),m=hashes.get(rel);if(!m||m.bytes!==b.length||m.sha256!==sha(b))throw Error('source_manifest_mismatch:'+rel);return JSON.parse(b);}
fs.mkdirSync(outputDir,{recursive:true});
const out=[];
for(const sampleId of ['N02','N04','N06']){
 const dossier=read(`samples/${sampleId}/dossier.share.json`),comments=read(`samples/${sampleId}/comments.json`);
 if(comments.items.length>100)throw Error('offline_comment_scope_exceeds_100:'+sampleId);
 const sources=[],excluded=[];
 if(typeof dossier.note?.platformTitle==='string' && dossier.note.platformTitle.trim()){
  sources.push({ref:sampleId+':T001',text:dossier.note.platformTitle,kind:'platform_title',speaker:'author',
   origin:'share_projection_note.platformTitle',titleSource:dossier.note.titleSource,
   citationScope:'local_experiment_ref_not_canonical'});
 }
 for(const m of dossier.materials)for(const b of m.blocks){
  if(b.kind==='gap'){excluded.push({ref:b.shareRef,reason:'diagnostic_gap_not_source_text'});continue;}
  if(typeof b.text!=='string'||!b.text.trim())throw Error('unhandled_block:'+b.shareRef);
  sources.push({ref:b.shareRef,text:b.text,kind:b.kind,speaker:b.speaker,materialRef:m.shareRef,materialKind:m.kind,reading:m.reading});
 }
 const packet={contract:'note-understanding.input.offline.v0.1',sampleId,sourceState:'shared_snapshot_not_live',mediaMode:'derived_text_only',
   sources,comments:comments.items.map(c=>({ref:c.commentId,text:c.body,isNoteAuthor:c.isNoteAuthor,parentRef:c.parentCommentId,relation:c.relation})),
   coverage:dossier.coverage,readingView:dossier.readingView,excluded,commentScope:{selectionRule:comments.selectionRule,recordCount:comments.items.length,authorRecordCount:comments.items.filter(c=>c.isNoteAuthor).length},
   metricsIncluded:false};
 const inputFingerprint=sha(JSON.stringify(packet));packet.inputFingerprint=inputFingerprint;
 fs.writeFileSync(path.join(outputDir,sampleId+'.input.json'),JSON.stringify(packet,null,2)+'\n');
 const variants={};
 for(const v of ['B0','B1']){
  const common=fs.readFileSync(path.join(__dirname,'common.txt'),'utf8'),method=fs.readFileSync(path.join(__dirname,v+'.txt'),'utf8');
  const request={kind:'offline_unexecuted_request',variant:v,inputFingerprint,system:common,method,input:packet,outputSchema:require('./contract.cjs')};
  const bytes=JSON.stringify(request,null,2)+'\n';fs.writeFileSync(path.join(outputDir,`${sampleId}.${v}.request.json`),bytes);variants[v]=sha(bytes);
 }
 out.push({sampleId,inputFingerprint,sourceBlockCount:sources.length,excludedGapCount:excluded.length,commentCount:packet.comments.length,sourceFileSha256:hashes.get(`samples/${sampleId}/dossier.share.json`).sha256,variants});
}
fs.writeFileSync(path.join(outputDir,'input-manifest.json'),JSON.stringify({kind:'offline_frozen_inputs',providerCalls:0,samples:out},null,2)+'\n');
console.log(JSON.stringify(out,null,2));
