import { pinnedSdkAvailable } from './sdk-version.mjs';
// Bundled package metadata and in-memory uploads only; no tools, user file paths or providers.
import { BACKGROUND_CONTEXT, loadSkills, formatSkillInvocation } from '@earendil-works/pi-agent-core';
import { pathToFileURL } from 'node:url';
import { posix } from 'node:path';
import limits from '../../../crates/contracts/src/ai_limits.json' with {type:'json'};

export async function inspectSkillPackage(packet) {
  if(!pinnedSdkAvailable())throw new Error('skill_loader_version_mismatch');
  if (!packet || packet.contract!=='ai-skill-package.v1' || typeof packet.directory!=='string'
      || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(packet.directory) || packet.directory.length>64
      || !Array.isArray(packet.files) || packet.files.length<1 || packet.files.length>limits.maxSkillFiles) {
    throw new Error('skill_package_invalid');
  }
  const root=`/skills/${packet.directory}`;
  const entries=new Map();
  let total=0;
  for (const file of packet.files) {
    if (typeof file?.path!=='string' || typeof file?.content!=='string'
        || file.path.includes('\\') || file.path.startsWith('/') || file.path.includes(':')
        || file.path.split('/').some(p=>!p || p==='.' || p==='..' || p.toLowerCase()==='scripts')
        || file.path!==file.path.normalize('NFC') || file.path.includes('\0') || entries.has(`${root}/${file.path}`)) {
      throw new Error('skill_path_invalid');
    }
    if(file.content!==file.content.normalize('NFC') || file.content.includes('\r')
        || /[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/u.test(file.content)) throw new Error('skill_text_invalid');
    const bytes=Buffer.byteLength(file.content); total+=bytes;
    if (bytes>limits.maxSkillFileBytes || total>limits.maxSkillTotalBytes) throw new Error('skill_package_too_large');
    entries.set(`${root}/${file.path}`,file.content);
  }
  const path=`${root}/SKILL.md`;
  if (!entries.has(path)) throw new Error('skill_manifest_missing');
  const ok=value=>({ok:true,value});
  const missing=()=>({ok:false,error:{code:'not_found',message:'not_found'}});
  const env={
    async fileInfo(path) {
      if (entries.has(path)) return ok({path,name:posix.basename(path),kind:'file'});
      if (path===root || [...entries.keys()].some(p=>p.startsWith(`${path}/`))) return ok({path,name:posix.basename(path),kind:'directory'});
      return missing();
    },
    async readTextFile(path) {return entries.has(path)?ok(entries.get(path)):missing();},
    async listDir(path) {
      const children=new Map();
      for(const file of entries.keys()) if(file.startsWith(`${path}/`)) {
        const [name,...rest]=file.slice(path.length+1).split('/');
        children.set(name,{path:`${path}/${name}`,name,kind:rest.length?'directory':'file'});
      }
      return ok([...children.values()]);
    },
    async joinPath(parts) {return ok(posix.join(...parts));},
  };
  const loaded=await loadSkills(env,root,BACKGROUND_CONTEXT);
  if(loaded.diagnostics.length || loaded.skills.length!==1 || loaded.skills[0].filePath!==path) {
    // Parser prose can contain the user's source. Only fixed codes cross the process boundary.
    throw new Error('skill_metadata_invalid');
  }
  const skill=loaded.skills[0];
  if(!skill.content.trim()) throw new Error('skill_body_empty');
  const manifest=entries.get(path);
  const end=manifest.startsWith('---\n')?manifest.indexOf('\n---',4):-1;
  if(end<0) throw new Error('skill_metadata_invalid');
  const attr=value=>value.replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
  const references=[...entries].filter(([file])=>file!==path)
    .sort(([a],[b])=>Buffer.compare(Buffer.from(a),Buffer.from(b)))
    .map(([file,content])=>`<reference path="${attr(file.slice(root.length+1))}">\n${content}\n</reference>`).join('\n\n');
  const invocationSkill={...skill,content:references?`${skill.content}\n\n${references}`:skill.content};
  return {contract:'ai-skill-inspection.v1',name:skill.name,description:skill.description,body:skill.content,
    frontmatter:manifest.slice(4,end),disableModelInvocation:skill.disableModelInvocation===true,
    invocation:formatSkillInvocation(invocationSkill)};
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try {
    const chunks=[];let bytes=0;
    for await(const chunk of process.stdin) {
      bytes+=chunk.length;
      if(bytes>8*limits.maxSkillTotalBytes) throw new Error('skill_package_too_large');
      chunks.push(chunk);
    }
    const result=await inspectSkillPackage(JSON.parse(Buffer.concat(chunks).toString('utf8')));
    process.stdout.write(JSON.stringify(result));
  } catch {
    process.stdout.write(JSON.stringify({error:'skill_package_invalid'}));
    process.exitCode=1;
  }
}
