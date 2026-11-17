import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { classify } from './check-opportunity-clippy.mjs';

function diagnostic(path, level = 'warning', primary = true) {
  return { reason: 'compiler-message', message: { level, code: { code: 'clippy::example' }, spans: [{ is_primary: primary, file_name: path, line_start: 17 }] } };
}

test('classifies only primary diagnostic spans in the requested paths', () => {
  for (const path of ['crates/intelligence/src/opportunity.rs', 'crates/intelligence/src/opportunity/card/read.rs', 'crates/intelligence/tests/opportunity_metrics_postgres/card.rs', 'crates/intelligence/tests/support/opportunity_dataset.rs', 'apps/api/src/local_web/opportunity_api.rs', 'apps/worker/src/bin/opportunity.rs']) {
    assert.equal(classify(diagnostic(path))[0].target, true, path);
  }
  assert.equal(classify(diagnostic('crates/evidence/src/lib.rs'))[0].target, false);
  assert.equal(classify(diagnostic('crates/intelligence/src/opportunity.rs.backup'))[0].target, false);
  assert.deepEqual(classify(diagnostic('crates/intelligence/src/opportunity.rs', 'warning', false)), []);
  assert.deepEqual(classify(diagnostic('crates/intelligence/src/opportunity.rs', 'note')), []);
  assert.equal(classify(diagnostic('crates/intelligence/src/opportunity.rs', 'error'))[0].target, true);
});

test('gate ignores other diagnostics but rejects target warnings and compiler failure', () => {
  const dir = mkdtempSync(join(tmpdir(), 'opportunity-clippy-test-'));
  try {
    const cargo = join(dir, 'cargo');
    writeFileSync(cargo, '#!/bin/sh\nprintf "%s\\n" "$PROOF_DIAGNOSTIC"\nexit "$PROOF_EXIT"\n');
    chmodSync(cargo, 0o700);
    const gate = fileURLToPath(new URL('./check-opportunity-clippy.mjs', import.meta.url));
    const invoke = (message, exit) => spawnSync(process.execPath, [gate], {
      encoding: 'utf8',
      env: { ...process.env, PATH: [dir, process.env.PATH].join(delimiter), PROOF_DIAGNOSTIC: JSON.stringify(message), PROOF_EXIT: String(exit) },
    });
    const outside = invoke(diagnostic('crates/evidence/src/lib.rs'), 0);
    assert.equal(outside.status, 0);
    assert.match(outside.stderr, /Opportunity diagnostics: 0; other diagnostics: 1/);
    const target = invoke(diagnostic('crates/intelligence/src/opportunity/card.rs'), 0);
    assert.equal(target.status, 1);
    assert.match(target.stderr, /card\.rs:17 clippy::example/);
    assert.equal(invoke(diagnostic('crates/evidence/src/lib.rs', 'error'), 101).status, 1);
  } finally {
    rmSync(dir, { recursive: true });
  }
});
