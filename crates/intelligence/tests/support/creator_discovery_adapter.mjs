// Synthetic local process: parses the real Pi request and never opens a network connection.
import {promises as fs} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
let raw='';for await(const part of process.stdin)raw+=part;
const request=JSON.parse(raw);
if(!request.baseUrl.startsWith('http://127.0.0.1:')||request.operation!=='analyze')process.exit(2);
const input=JSON.parse(request.prompt);
const barrier=request.prompt.match(/SYNTHETIC_BARRIER:([0-9a-f-]{36})/);
if(barrier){
  const tempRoot=await fs.realpath(os.tmpdir());
  const barrierDir=await fs.realpath(path.join(os.tmpdir(),`creator-discovery-barrier-${barrier[1]}`));
  if(!barrierDir.startsWith(tempRoot+path.sep))process.exit(3);
  await fs.writeFile(path.join(barrierDir,'ready'),'adapter-started',{flag:'wx'});
  let released=false;
  for(let i=0;i<500;i++){
    try{await fs.access(path.join(barrierDir,'release'));released=true;break;}catch{}
    await new Promise(resolve=>setTimeout(resolve,20));
  }
  if(!released)process.exit(4);
}
const ids=input.fragments.map(f=>f.fragmentId);
const field=(value)=>({value,reason:'合成模型验证来源引用',evidenceFragmentIds:ids.slice(0,1)});
const output=input.workRef?{relevance:field('related'),traits:{personal_experience:field('yes'),professional_output:field('unknown'),explicit_promotion:field('unknown')},topicHints:[]}:{focus:field('unknown'),institution_or_brand:field('unknown')};
process.stdout.write(JSON.stringify({version:request.version,ok:true,text:request.prompt.includes('SYNTHETIC_INVALID')?'invalid JSON':JSON.stringify(output),failureCode:null,modelIds:null,modelListOrigin:null,usage:{inputTokens:111,outputTokens:22,costUsd:null},elapsedMs:1}));
