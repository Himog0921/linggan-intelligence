import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, renameSync } from 'node:fs';
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
    '.github/workflows/delivery.yml', '.github/workflows/web-app.yml', '.github/workflows/web-real-chain.yml',
    'scripts/ci-preflight.sh', 'scripts/ci-select.test.mjs', 'scripts/check-project-governance.sh',
    'scripts/verify-ui-design-handbook.sh', 'scripts/verify-lids-alignment.py',
    'crates/new-domain/src/lib.rs', 'plugins/new-plugin/background.js', 'docs/new-runtime-input.json',
  ]) assert.deepEqual(selected([path]), suites, path);
  assert.deepEqual(selected([]), suites, 'empty or indeterminate scope must not silently skip everything');
});

test('existing domain proof coverage is retained and unions across files', () => {
  const cases = [
    ['apps/web/src/pages/collection/TasksPage.tsx', ['web']],
    ['apps/web/src/pages/topics/TopicsPage.tsx', ['web', 'web_real_chain', 'topic']],
    ['apps/web/src/styles/theme.css', ['web', 'topic']],
    ['apps/web/package-lock.json', ['ocr_audit', 'web', 'web_real_chain', 'topic']],
    ['apps/web/test/e2e/topic-main.mjs', ['web', 'topic']],
    ['apps/api/src/local_web/topic_workspace.rs', ['ai', 'web', 'web_real_chain', 'topic']],
    ['apps/api/src/local_web/web_app_tests.rs', ['ai', 'web', 'web_real_chain', 'topic']],
    ['apps/api/src/local_web/ocr_recognition.rs', ['ai', 'ocr_audit', 'web', 'web_real_chain']],
    ['apps/api/src/main.rs', ['web', 'web_real_chain']],
    ['apps/pi-adapter/src/index.ts', ['ai', 'ocr_audit']],
    ['crates/intelligence/src/ai_foundation/executor/ocr_audit.rs', ['ai', 'ocr_audit']],
    ['crates/intelligence/src/content_workbench/source_usage.rs', ['ai', 'web', 'web_real_chain']],
    ['crates/intelligence/src/comment_study_run/submit.rs', ['ai', 'web', 'web_real_chain']],
    ['crates/evidence/src/ocr_adoption/selection.rs', ['ocr_adoption']],
    ['crates/evidence/src/ocr_adoption_persistence.rs', ['ocr_adoption', 'ocr_audit']],
    ['crates/evidence/src/material_media_read.rs', ['ocr_adoption', 'web', 'web_real_chain']],
    ['crates/evidence/src/research_dossier.rs', suites],
    ['apps/worker/tests/ocr_vl_postgres.rs', ['ai', 'ocr_adoption', 'ocr_audit']],
    ['database/migrations/0142_ocr_adoption.sql', ['ai', 'ocr_adoption']],
    ['database/migrations/0143_ocr_quality_audit.sql', ['ai', 'ocr_audit']],
    ['scripts/test-research-dossier-postgres.sh', ['ai', 'web', 'web_real_chain']],
    ['scripts/runtime/prepare-web.sh', ['ai', 'web', 'web_real_chain']],
    ['scripts/local-runtime.sh', ['ai', 'ocr_audit']],
    ['docs/design/lids/tokens.md', ['web']],
    ['docs/design/changes/lids-component-change.json', ['web']],
  ];
  for (const [path, expected] of cases) assert.deepEqual(selected([path]), expected, path);
  assert.deepEqual(selected(['docs/README.md', 'apps/web/src/pages/topics/TopicsPage.tsx', 'crates/evidence/src/ocr_adoption.rs']), ['ocr_adoption', 'web', 'web_real_chain', 'topic']);
});

test('page-local UI changes avoid OCR and PostgreSQL proofs; shared and API changes retain them', () => {
  const cases = [
    ['apps/web/src/pages/settings/TeamSettingsPage.module.css', ['web']],
    ['apps/web/src/pages/settings/TeamSettingsPage.tsx', ['web']],
    ['apps/web/src/pages/settings/AiSettingsExperience.tsx', ['ocr_audit', 'web', 'web_real_chain']],
    ['apps/web/src/pages/settings/aiSettingsModel.ts', ['ocr_audit', 'web', 'web_real_chain']],
    ['apps/web/src/pages/corpus/evidence/ReadingPanel.tsx', ['ocr_audit', 'web', 'web_real_chain']],
    ['apps/web/test/e2e/team-member-scroll.mjs', ['web']],
    ['apps/web/src/pages/settings/teamSettingsApi.ts', ['web', 'web_real_chain']],
    ['apps/web/src/app/AppShell.module.css', ['web']],
    ['apps/web/src/app/App.tsx', ['ocr_audit', 'web', 'web_real_chain']],
    ['apps/web/src/main.tsx', ['web', 'web_real_chain']],
    ['apps/web/src/corpus/corpusShellApi.ts', ['web', 'web_real_chain']],
    ['apps/web/src/shared/api.ts', ['ocr_audit', 'web', 'web_real_chain']],
    ['apps/web/src/pages/corpus/evidence/DossierSections.tsx', ['web', 'web_real_chain']],
    ['apps/web/src/pages/settings/OcrAudit.module.css', ['web']],
    ['apps/web/src/pages/settings/OcrAuditSettings.tsx', ['ocr_audit', 'web']],
  ];
  for (const [path, expected] of cases) assert.deepEqual(selected([path]), expected, path);
  assert.deepEqual(selected(['apps/web/src/pages/settings/TeamSettingsPage.tsx', 'apps/api/src/main.rs']), ['web', 'web_real_chain']);
});

test('page files with direct HTTP calls cannot silently enter the UI-only route', () => {
  const walk = directory => readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? walk(path) : entry.isFile() ? [path] : [];
  });
  for (const file of walk(join(root, 'apps/web/src/pages'))) {
    if (!/\.tsx?$/.test(file) || /\.test\.tsx?$/.test(file)) continue;
    if (!/\b(?:fetch|requestJson)\s*\(/.test(readFileSync(file, 'utf8'))) continue;
    const path = file.slice(root.length).replaceAll('\\', '/');
    assert.ok(selected([path]).includes('web_real_chain'), `${path} makes direct HTTP calls`);
  }
});

test('the canonical dossier and reading schema exercise every current consumer', () => {
  for (const path of [
    'crates/evidence/src/research_dossier.rs',
    'crates/evidence/src/research_dossier/content/ocr.rs',
    'crates/evidence/src/material_reading.rs',
    'crates/evidence/src/material_reading/read.rs',
    'crates/evidence/src/dossier_input.rs',
    'crates/evidence/src/dossier_input/backfill.rs',
    'database/migrations/0146_research_dossier_v2.sql',
    'database/migrations/0147_comment_study_full_context.sql',
    'database/migrations/0148_dossier_consumer_contracts.sql',
  ]) assert.deepEqual(selected([path]), suites, path);
  assert.deepEqual(selected(['apps/worker/src/bin/research_dossier.rs']), ['ai', 'web', 'web_real_chain']);
  assert.deepEqual(selected(['crates/evidence/tests/dossier_input_postgres.rs']), ['web', 'web_real_chain']);
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
const jobs = { ai: 'ai', 'ocr-adoption': 'ocr_adoption', 'ocr-audit': 'ocr_audit', web: 'web', 'web-real-chain': 'web_real_chain', topic: 'topic' };
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
    for (const job of Object.keys(jobs)) {
      assert.notEqual(gateResult(suites, needs => { needs[job].result = status; }), 0, `${job}: ${status}`);
    }
    assert.notEqual(gateResult(suites, needs => { needs.preflight.result = status; }), 0, `preflight ${status}`);
  }
  assert.notEqual(gateResult([], needs => { delete needs.preflight.outputs.web; }), 0);
  assert.notEqual(gateResult([], needs => { delete needs.preflight.outputs.web_real_chain; }), 0);
  assert.notEqual(gateResult([], needs => { needs.web.result = 'failure'; }), 0);
});

test('active workflows have a single PR entrypoint and all proofs depend on preflight', () => {
  const workflows = ['ai-gen1-foundation', 'ocr-adoption', 'ocr-quality-audit', 'web-app', 'web-real-chain', 'topic-map-compact-ui'];
  for (const workflow of workflows) {
    const source = readFileSync(join(root, `.github/workflows/${workflow}.yml`), 'utf8');
    assert.match(source, /on:\n  workflow_call:/);
    assert.doesNotMatch(source, /pull_request:\n|check-project-governance\.sh|verify-ui-design-handbook\.sh|git diff --check/);
    assert.ok(delivery.includes(`uses: ./.github/workflows/${workflow}.yml`));
  }
  for (const [job, output] of Object.entries(jobs)) {
    const needs = job === 'web-real-chain' ? '[preflight, web]' : 'preflight';
    assert.ok(delivery.includes(`  ${job}:\n    needs: ${needs}\n    if: needs.preflight.outputs.${output} == 'true'`));
  }
  assert.match(delivery, /ready_for_review/);
  assert.doesNotMatch(delivery, /\.draft/);
  assert.match(delivery, /needs: \[preflight, ai, ocr-adoption, ocr-audit, web, web-real-chain, topic\]/);
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
  const workflows = ['delivery', 'ai-gen1-foundation', 'ocr-adoption', 'ocr-quality-audit', 'web-app', 'web-real-chain', 'topic-map-compact-ui'];
  for (const workflow of workflows) {
    const source = readFileSync(join(root, `.github/workflows/${workflow}.yml`), 'utf8');
    const checkouts = source.match(/uses: actions\/checkout@/g) ?? [];
    const headRefs = source.match(/ref: \$\{\{ github\.event\.pull_request\.head\.sha \}\}/g) ?? [];
    assert.ok(checkouts.length, `${workflow} has checkout`);
    assert.equal(checkouts.length, headRefs.length, `${workflow} checks out the event head SHA`);
  }
  assert.equal((delivery.match(/DELIVERY_BASE_SHA: \$\{\{ github\.event\.pull_request\.base\.sha \}\}/g) ?? []).length, 2);
});

test('isolated Rust proofs reject empty selections and preserve listing or execution failures', () => {
  const shell = String.raw`
    source "$1"
    cargo() {
      if [[ "$*" == *' --list' ]]; then
        case "$PROOF_FIXTURE" in
          empty) printf '0 tests, 0 benchmarks\n' ;;
          list_failure) return 43 ;;
          *) printf 'source_frozen: test\n\n1 test, 0 benchmarks\n' ;;
        esac
      else
        printf '%s\n' "$@"
        [[ "$PROOF_FIXTURE" != execution_failure ]] || return 37
      fi
    }
    cargo_proof -p synthetic --test canonical source_frozen --locked -- --ignored --exact --nocapture
  `;
  const invoke = mode => spawnSync('bash', ['-c', shell, 'proof-fixture', join(root, 'scripts/cargo-proof.sh')], {
    encoding: 'utf8', env: { ...process.env, PROOF_FIXTURE: mode },
  });
  const empty = invoke('empty');
  assert.equal(empty.status, 1); assert.equal(empty.stdout, '');
  assert.match(empty.stderr, /selected no tests/);
  assert.equal(invoke('list_failure').status, 43);
  assert.equal(invoke('execution_failure').status, 37);
  const successful = invoke('selected');
  assert.equal(successful.status, 0);
  assert.deepEqual(successful.stdout.trim().split('\n'), [
    'test', '-p', 'synthetic', '--test', 'canonical', 'source_frozen', '--locked', '--', '--ignored', '--exact', '--nocapture',
  ]);
});

test('Delivery executes current creator, saved-source and OCR proof targets without empty-test filters', () => {
  const runner = readFileSync(join(root, 'scripts/test-research-dossier-postgres.sh'), 'utf8');
  const readonly = runner.split('if [[ "${1:-}" == "--ai-readonly" ]]; then')[1]?.split('\n  fi')[0];
  assert.ok(readonly, 'inspect the actual Delivery mode');
  assert.match(readonly, /cargo_proof -p linggan-intelligence --test creator_discovery_input_postgres /);
  assert.match(readonly, /cargo_proof -p linggan-intelligence --lib topic_map_research::saved_source_postgres:: /);
  assert.match(readonly, /export CREATOR_PROOF_NODE=/);
  assert.match(runner, /source scripts\/cargo-proof\.sh/);
  assert.doesNotMatch(runner, /\bcargo test\b/, 'all filtered dossier proof calls guard against zero selected tests');
  const workflow = readFileSync(join(root, '.github/workflows/ai-gen1-foundation.yml'), 'utf8');
  assert.match(workflow, /test-research-dossier-postgres\.sh --ai-readonly/);
  const ocr = readFileSync(join(root, 'scripts/test-ocr-adoption-postgres.sh'), 'utf8');
  const name = ocr.match(/test_name=(synthetic_[a-z_]+)/)?.[1];
  assert.ok(name);
  assert.ok(readFileSync(join(root, 'apps/worker/tests/ocr_vl_postgres.rs'), 'utf8').includes(`async fn ${name}()`));
  assert.match(ocr, /source scripts\/cargo-proof\.sh/);
  assert.match(ocr, /cargo_proof .*"\$test_name" --locked -- --ignored --exact --nocapture/);
  assert.doesNotMatch(readFileSync(join(root, '.github/workflows/ocr-adoption.yml'), 'utf8'), /--lib ocr_adoption_persistence::/);
});

test('OCR audit runner filters name actual Rust tests, including split modules', () => {
  const runner = readFileSync(join(root, 'scripts/test-ocr-quality-audit-postgres.sh'), 'utf8');
  const calls = [...runner.matchAll(/cargo_proof -p linggan-worker --test (\w+) (\w+) --locked/g)];
  assert.ok(calls.length > 0, 'inspect the actual audit proof calls');
  assert.equal(calls.length, [...runner.matchAll(/^\s*cargo_proof\b/gm)].length, 'every audit proof call must be checked');
  const rustSources = directory => readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return rustSources(path);
    return entry.isFile() && entry.name.endsWith('.rs') ? [readFileSync(path, 'utf8')] : [];
  });
  for (const [, target, filter] of calls) {
    const entrypoint = join(root, 'apps/worker/tests', `${target}.rs`);
    const modules = join(root, 'apps/worker/tests', target);
    const sources = [readFileSync(entrypoint, 'utf8'), ...(existsSync(modules) ? rustSources(modules) : [])];
    const names = sources.flatMap(source => [...source.matchAll(
      /#\[(?:tokio::)?test(?:\([^]*?\))?\]\s*(?:#\[[^\]]*\]\s*)*(?:async\s+)?fn\s+(\w+)\s*\(/g,
    )].map(match => match[1]));
    assert.ok(names.includes(filter), `${target}: ${filter} must select a declared Rust test`);
  }
});
