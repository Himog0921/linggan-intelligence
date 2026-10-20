import test from 'node:test';
import assert from 'node:assert/strict';
import { inspectSkillPackage } from '../src/skills.mjs';

const packet=(manifest,extras=[])=>({contract:'ai-skill-package.v1',directory:'example-skill',files:[{path:'SKILL.md',content:manifest},...extras]});
const valid='---\nname: example-skill\ndescription: |\n  A multiline description\n  of a local skill.\nallowed-tools: [shell]\n---\nRead references/example.md.\n';
test('pinned loader parses YAML, matches the directory and formats fixed skill content',async()=>{
  const value=await inspectSkillPackage(packet(valid,[{path:'references/example.md',content:'An example.'}]));
  assert.equal(value.name,'example-skill');
  assert.match(value.description,/multiline description\nof a local skill/);
  assert.match(value.frontmatter,/allowed-tools/);
  assert.equal(Object.hasOwn(value,'tools'),false,'frontmatter grants no tools');
  assert.match(value.invocation,/<skill name="example-skill" location="\/skills\/example-skill\/SKILL.md">/);
  assert.match(value.invocation,/Read references\/example.md/);
  assert.match(value.invocation,/<reference path="references\/example.md">\nAn example\./);
});
test('invalid metadata is rejected rather than accepted as the SDK warning',async()=>{
  for(const manifest of [valid.replace('name: example-skill','name: different'),valid.replace('description: |','description: []\nextra: |'),valid.replace('name: example-skill','name: Bad_Name'),`---\nname: example-skill\ndescription: ${'🙂'.repeat(513)}\n---\nBody`,valid.replace('name: example-skill','name: [malformed')]) {
    await assert.rejects(inspectSkillPackage(packet(manifest)),/skill_metadata_invalid/);
  }
});
test('paths, duplicate files, scripts and unbounded packages cannot reach the loader',async()=>{
  for(const path of ['../escape.md','references/../escape.md','/absolute.md','Scripts/action.md','references/scripts/run.md','C:/escape.md','bad\\name.md']) {
    await assert.rejects(inspectSkillPackage(packet(valid,[{path,content:'body'}])),/skill_path_invalid/);
  }
  await assert.rejects(inspectSkillPackage(packet(valid,[{path:'SKILL.md',content:valid}])),/skill_path_invalid/);
  await assert.rejects(inspectSkillPackage(packet(valid,[{path:'too-large.md',content:'x'.repeat(262145)}])),/skill_package_too_large/);
});
