import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync, renameSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runInNewContext } from 'node:vm';
import test from 'node:test';
import { changedPaths, selectSuites, suites } from './ci-select.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
const selected = paths => suites.filter(suite => selectSuites(paths)[suite]);

test('ordinary documentation needs only preflight, including deletions', () => {
  assert.deepEqual(selected(['docs/README.md', 'docs/current-state.md', 'docs/plans/active/example.md', 'README.md']), []);
});

test('governance, delivery infrastructure and unknown paths select every proof', () => {
  for (const path of [
    'AGENTS.md', 'CLAUDE.md', 'codx.md', 'Cargo.toml', 'Cargo.lock', '.nvmrc',
    'rust-toolchain.toml', 'docs/governance/agent-collaboration.md', 'docs/agents/issue-tracker.md',
    '.github/workflows/delivery.yml', '.github/workflows/web-app.yml',
    'scripts/ci-preflight.sh', 'scripts/ci-select.test.mjs', 'scripts/check-project-governance.sh',
    'scripts/verify-ui-design-handbook.sh', 'scripts/verify-lids-alignment.py',
    'crates/new-domain/src/lib.rs', 'plugins/new-plugin/background.js', 'docs/new-runtime-input.json',
  ]) assert.deepEqual(selected([path]), suites, path);
  assert.deepEqual(selected([]), suites, 'empty or indeterminate scope must not silently skip everything');
});

test('existing domain proof coverage is retained and unions across files', () => {
  const cases = [
    ['apps/web/src/pages/collection/TasksPage.tsx', ['ocr_audit', 'web']],
    ['apps/web/src/pages/topics/TopicsPage.tsx', ['ocr_audit', 'web', 'topic']],
    ['apps/web/src/styles/theme.css', ['ocr_audit', 'web', 'topic']],
    ['apps/web/package-lock.json', ['ocr_audit', 'web', 'topic']],
    ['apps/web/test/e2e/topic-main.mjs', ['ocr_audit', 'web', 'topic']],
    ['apps/api/src/local_web/topic_workspace.rs', ['ai', 'web', 'topic']],
    ['apps/api/src/local_web/web_app_tests.rs', ['ai', 'web', 'topic']],
    ['apps/api/src/local_web/ocr_recognition.rs', ['ai', 'ocr_audit', 'web']],
    ['apps/api/src/main.rs', ['web']],
    ['apps/pi-adapter/src/index.ts', ['ai', 'ocr_audit']],
    ['crates/intelligence/src/ai_foundation/executor/ocr_audit.rs', ['ai', 'ocr_audit']],
    ['crates/intelligence/src/content_workbench/source_usage.rs', ['ai', 'web']],
    ['crates/intelligence/src/comment_study_run/submit.rs', ['ai', 'web']],
    ['crates/evidence/src/ocr_adoption/selection.rs', ['ocr_adoption']],
    ['crates/evidence/src/ocr_adoption_persistence/read.rs', ['ocr_adoption', 'ocr_audit']],
    ['crates/evidence/src/material_media_read.rs', ['ocr_adoption', 'web']],
    ['crates/evidence/src/research_dossier.rs', ['ocr_adoption', 'ocr_audit', 'web']],
    ['apps/worker/tests/ocr_vl_postgres.rs', ['ai', 'ocr_adoption', 'ocr_audit']],
    ['database/migrations/0142_ocr_adoption.sql', ['ai', 'ocr_adoption']],
    ['database/migrations/0143_ocr_quality_audit.sql', ['ai', 'ocr_audit']],
    ['scripts/test-research-dossier-postgres.sh', ['ai', 'web']],
    ['scripts/runtime/prepare-web.sh', ['ai', 'web']],
    ['scripts/local-runtime.sh', ['ai', 'ocr_audit']],
    ['docs/design/lids/tokens.md', ['web']],
    ['docs/design/changes/lids-component-change.json', ['web']],
  ];
  for (const [path, expected] of cases) assert.deepEqual(selected([path]), expected, path);
  assert.deepEqual(selected(['docs/README.md', 'apps/web/src/pages/topics/TopicsPage.tsx', 'crates/evidence/src/ocr_adoption.rs']), ['ocr_adoption', 'ocr_audit', 'web', 'topic']);
});

test('Git diff uses merge-base, includes deleted and renamed paths, and preserves unusual filenames', () => {
  const cwd = mkdtempSync(join(tmpdir(), 'linggan-ci-select-'));
  const git = (...args) => execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  try {
    git('init', '-q');
    git('config', 'user.email', 'ci-routing@example.invalid');
    git('config', 'user.name', 'CI routing fixture');
    writeFileSync(join(cwd, 'old name\nline.txt'), 'synthetic\n');
    writeFileSync(join(cwd, 'deleted.txt'), 'synthetic\n');
    git('add', '.');
    git('commit', '-qm', 'base');
    const base = git('rev-parse', 'HEAD');
    git('checkout', '-qb', 'delivery');
    renameSync(join(cwd, 'old name\nline.txt'), join(cwd, 'new name.txt'));
    rmSync(join(cwd, 'deleted.txt'));
    git('add', '-A');
    git('commit', '-qm', 'rename and delete');
    const head = git('rev-parse', 'HEAD');
    git('checkout', '-q', '--detach', base);
    writeFileSync(join(cwd, 'base-only.txt'), 'synthetic base advance\n');
    git('add', '.');
    git('commit', '-qm', 'base advances independently');
    const advancedBase = git('rev-parse', 'HEAD');
    git('checkout', '-q', '--detach', head);
    assert.deepEqual(changedPaths(advancedBase, cwd).sort(), ['deleted.txt', 'new name.txt', 'old name\nline.txt'].sort());
    assert.throws(() => changedPaths('missing-revision', cwd));
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

const delivery = readFileSync(join(root, '.github/workflows/delivery.yml'), 'utf8');
const gate = delivery.split("node --input-type=module <<'JS'\n")[1]?.split('\n          JS')[0];
const jobs = { ai: 'ai', 'ocr-adoption': 'ocr_adoption', 'ocr-audit': 'ocr_audit', web: 'web', topic: 'topic' };
function gateResult(selectedSuites, mutate = () => {}) {
  const needs = { preflight: { result: 'success', outputs: {} } };
  for (const [job, suite] of Object.entries(jobs)) {
    const enabled = selectedSuites.includes(suite);
    needs.preflight.outputs[suite] = String(enabled);
    needs[job] = { result: enabled ? 'success' : 'skipped' };
  }
  mutate(needs);
  return spawnSync(process.execPath, ['--input-type=module', '-e', gate], {
    encoding: 'utf8', env: { ...process.env, DELIVERY_NEEDS: JSON.stringify(needs) },
  }).status;
}

test('the actual final gate rejects missing, failed, cancelled or skipped selected proofs', () => {
  assert.ok(gate, 'extract the gate script directly from the workflow');
  assert.equal(gateResult([]), 0, 'ordinary docs');
  assert.equal(gateResult(suites), 0, 'full verification');
  assert.equal(gateResult(['web', 'ocr_audit']), 0, 'selected modules');
  for (const status of ['failure', 'cancelled', 'skipped']) {
    assert.notEqual(gateResult(suites, needs => { needs.web.result = status; }), 0, status);
    assert.notEqual(gateResult(suites, needs => { needs.preflight.result = status; }), 0, `preflight ${status}`);
  }
  assert.notEqual(gateResult([], needs => { delete needs.preflight.outputs.web; }), 0);
  assert.notEqual(gateResult([], needs => { needs.web.result = 'failure'; }), 0);
});

test('active workflows have a single PR entrypoint and all proofs depend on preflight', () => {
  const workflows = ['ai-gen1-foundation', 'ocr-adoption', 'ocr-quality-audit', 'web-app', 'topic-map-compact-ui'];
  for (const workflow of workflows) {
    const source = readFileSync(join(root, `.github/workflows/${workflow}.yml`), 'utf8');
    assert.match(source, /on:\n  workflow_call:/);
    assert.doesNotMatch(source, /pull_request:\n|check-project-governance\.sh|verify-ui-design-handbook\.sh|git diff --check/);
    assert.ok(delivery.includes(`uses: ./.github/workflows/${workflow}.yml`));
  }
  for (const [job, output] of Object.entries(jobs)) {
    assert.ok(delivery.includes(`  ${job}:\n    needs: preflight\n    if: needs.preflight.outputs.${output} == 'true'`));
  }
  assert.match(delivery, /ready_for_review/);
  assert.doesNotMatch(delivery, /\.draft/);
  assert.match(delivery, /needs: \[preflight, ai, ocr-adoption, ocr-audit, web, topic\]/);
  assert.match(delivery, /if: always\(\)/);
});

test('metadata edits cannot cancel proof or overwrite its required gate; base retargets rerun it', () => {
  // Evaluate the actual workflow expressions, not a parallel implementation.
  const group = delivery.match(/^  group: \$\{\{ (.*) \}\}$/m)?.[1];
  const preflightIf = delivery.match(/  preflight:\n[^\n]+\n    if: (.*)/)?.[1];
  const gateBlock = delivery.split('\n  gate:\n')[1];
  const gateName = gateBlock.match(/    name: \$\{\{ (.*) \}\}/)?.[1];
  const gateIf = gateBlock.match(/    if: (.*)/)?.[1];
  for (const expression of [group, preflightIf, gateName, gateIf]) assert.ok(expression, 'workflow expression found');
  const evaluate = (expression, action, changes = {}) => runInNewContext(expression, {
    github: { event: { action, changes, pull_request: { number: 481 } }, run_id: 8123 },
    format: (template, value) => template.replace('{0}', String(value)),
    always: () => true,
  });
  for (const changes of [{ body: { from: 'old body' } }, { title: { from: 'old title' } }, {}]) {
    assert.equal(evaluate(group, 'edited', changes), 'delivery-metadata-8123');
    assert.equal(Boolean(evaluate(preflightIf, 'edited', changes)), false);
    assert.equal(Boolean(evaluate(gateIf, 'edited', changes)), false);
    assert.notEqual(evaluate(gateName, 'edited', changes), 'Delivery gate');
  }
  const events = [
    ['opened', {}], ['synchronize', {}], ['reopened', {}], ['ready_for_review', {}],
    ['edited', { base: { ref: { from: 'old-base' }, sha: { from: 'old-sha' } } }],
    ['edited', { base: { ref: { from: 'old-base' } }, body: { from: 'old body' } }],
  ];
  for (const [action, changes] of events) {
    assert.equal(evaluate(group, action, changes), 'delivery-481');
    assert.equal(Boolean(evaluate(preflightIf, action, changes)), true);
    assert.equal(Boolean(evaluate(gateIf, action, changes)), true);
    assert.equal(evaluate(gateName, action, changes), 'Delivery gate');
  }
});

test('preflight and every reusable proof check out the same PR event head', () => {
  const workflows = ['delivery', 'ai-gen1-foundation', 'ocr-adoption', 'ocr-quality-audit', 'web-app', 'topic-map-compact-ui'];
  for (const workflow of workflows) {
    const source = readFileSync(join(root, `.github/workflows/${workflow}.yml`), 'utf8');
    const checkouts = source.match(/uses: actions\/checkout@/g) ?? [];
    const headRefs = source.match(/ref: \$\{\{ github\.event\.pull_request\.head\.sha \}\}/g) ?? [];
    assert.ok(checkouts.length, `${workflow} has checkout`);
    assert.equal(checkouts.length, headRefs.length, `${workflow} checks out the event head SHA`);
  }
  assert.equal((delivery.match(/DELIVERY_BASE_SHA: \$\{\{ github\.event\.pull_request\.base\.sha \}\}/g) ?? []).length, 2);
});
