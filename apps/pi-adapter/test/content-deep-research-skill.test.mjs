import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inspectSkillPackage } from '../src/skills.mjs';

// The L2 Skill package as it is uploaded through "选择做法目录"; read locally, no network or model.
const root = new URL('../../../', import.meta.url);
const skillDir = fileURLToPath(new URL('docs/data-contracts/deep-content-research/skill/content-deep-research/', root));
const files = readdirSync(skillDir, { recursive: true, withFileTypes: true })
  .filter(entry => entry.isFile())
  .map(entry => {
    const full = join(entry.parentPath, entry.name);
    return { path: relative(skillDir, full).split(sep).join('/'), content: readFileSync(full, 'utf8') };
  })
  .sort((a, b) => a.path.localeCompare(b.path));

// Keep in sync with transport.rs estimate_input_tokens (the adapter's reproducible estimate).
const estimate = value => {
  let total = 0, run = 0;
  for (const ch of value) {
    if (/^[A-Za-z0-9]$/.test(ch)) run += 1;
    else { total += Math.ceil(run / 3) + 1; run = 0; }
  }
  return total + Math.ceil(run / 3);
};

test('content-deep-research loads as one Skill with every mode and reference inlined', async () => {
  const value = await inspectSkillPackage({ contract: 'ai-skill-package.v1', directory: 'content-deep-research', files });
  assert.equal(value.name, 'content-deep-research');
  assert.equal(value.disableModelInvocation, false);
  for (const path of ['modes/content-design.md', 'modes/content-value.md', 'modes/creation-method.md', 'references/finding-types.md']) {
    assert.ok(value.invocation.includes(`<reference path="${path}">`), path);
  }
  assert.ok(!value.invocation.includes('画面文字'), 'retired frame text (DEC-0013)');
  assert.ok(!value.invocation.includes('frame_text'), 'retired frame text (DEC-0013)');
  for (const field of ['payload.researchMode', 'payload.baseMode', 'payload.subQuestions', 'payload.materials', 'payload.core', 'payload.bannedPatterns', 'payload.causalPatterns']) {
    assert.ok(value.body.includes(field), field);
  }

  const migration = readFileSync(new URL('database/migrations/0224_content_understanding_deep_v2.sql', root), 'utf8');
  const systemText = migration.match(/'content_understanding_deep',2,\s*'([^']+)'/)?.[1];
  assert.ok(systemText, 'prompt v2 system_text');
  const system = `${systemText}\n\n${value.invocation}`;
  const tokens = estimate(system);
  console.log(`L2 skill size: chars=${[...system].length} bytes=${Buffer.byteLength(system)} estimatedTokens=${tokens}`);
  assert.ok(tokens < 6000, `estimated system prompt ${tokens} tokens`);
});
