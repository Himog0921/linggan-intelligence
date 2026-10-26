#!/usr/bin/env node
// Routes the existing proof suites. A path outside the known map fails open to
// MORE verification, never to an empty green run. Keep this map conservative
// until cross-module dependencies have executable coverage.
import { execFileSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const suites = ['ai', 'ocr_adoption', 'ocr_audit', 'web', 'web_real_chain', 'topic'];

// These patterns preserve the former pull_request.paths coverage while splitting
// the Web proof in two. Unknown paths and delivery changes run every proof.
const paths = {
  ai: [
    'crates/contracts/src/ai_*', 'crates/intelligence/**',
    'crates/storage-postgres/**', 'apps/api/src/local_web/**',
    'apps/worker/**', 'apps/pi-adapter/**', 'database/migrations/**',
    'scripts/runtime/**', 'scripts/local-runtime.sh',
    'scripts/test-research-dossier-postgres.sh',
  ],
  ocr_adoption: [
    'crates/evidence/src/ocr_adoption.rs', 'crates/evidence/src/ocr_adoption/**',
    'crates/evidence/src/ocr_adoption_persistence.rs', 'crates/evidence/src/material_reading.rs', 'crates/evidence/src/material_reading/**',
    'crates/evidence/src/material_processing.rs', 'crates/evidence/src/material_media_read.rs',
    'crates/evidence/src/material_evidence_fragment.rs', 'crates/evidence/src/research_dossier.rs',
    'apps/worker/src/bin/media_worker.rs', 'apps/worker/src/bin/media_worker_support/**',
    'apps/worker/tests/ocr_vl_postgres.rs', 'database/migrations/0142_ocr_adoption.sql',
    'scripts/test-ocr-adoption-postgres.sh', 'crates/evidence/src/ocr_layering.rs',
    'crates/evidence/src/lib.rs', 'crates/evidence/examples/ocr_adoption_replay.rs',
    'crates/evidence/tests/ocr_adoption.rs', 'crates/evidence/tests/ocr_adoption/**',
    'crates/evidence/Cargo.toml',
  ],
  ocr_audit: [
    'crates/intelligence/src/ocr_quality_audit*', 'crates/intelligence/src/ocr_quality_audit/**',
    'crates/intelligence/src/ai_foundation/**', 'crates/intelligence/src/pi_*',
    'crates/evidence/src/material_reading.rs', 'crates/evidence/src/material_reading/**', 'crates/evidence/src/ocr_adoption_persistence.rs', 'crates/evidence/src/cloud_ocr_adoption*',
    'crates/evidence/src/research_dossier.rs', 'crates/evidence/tests/cloud_ocr*',
    'apps/worker/tests/ocr_recognition_postgres.rs', 'apps/worker/tests/ocr_vl_postgres.rs',
    'database/migrations/0144_ocr_manual_recognition.sql',
    'apps/api/src/local_web/ocr_recognition*', 'apps/api/src/local_web/ocr_audit.rs',
    'apps/web/src/pages/settings/OcrAudit*', 'apps/web/src/pages/settings/AuditReadingReview*',
    'apps/web/src/pages/settings/ocrAuditApi.ts',
    'apps/web/src/pages/settings/AiSettingsExperience.tsx',
    'apps/web/src/pages/settings/aiSettingsModel.ts',
    'apps/web/src/pages/corpus/evidence/OcrRecognition*',
    'apps/web/src/pages/corpus/evidence/OcrReadingReview*',
    'apps/web/src/pages/corpus/evidence/ocrRecognition*',
    'apps/web/src/pages/corpus/evidence/ReadingPanel.tsx',
    'apps/web/src/shared/api.ts', 'apps/web/src/shared/decode.ts',
    'apps/web/src/app/pages.ts', 'apps/web/src/app/App.tsx',
    'apps/web/package.json', 'apps/web/package-lock.json',
    'apps/web/test/e2e/ocr*', 'apps/pi-adapter/**',
    'apps/worker/tests/ocr_quality_audit_postgres.rs', 'apps/worker/tests/support/ocr_audit_endpoint.rs',
    'database/migrations/0143_ocr_quality_audit.sql',
    'scripts/test-ocr-quality-audit-postgres.sh', 'scripts/local-runtime.sh',
    'scripts/runtime/tests/test_ai_guard.py',
  ],
  web: [
    'apps/web/**', 'docs/design/lids/**', 'docs/design/changes/lids-component-change.json',
    'scripts/lids*', 'scripts/sync-lids*', 'apps/api/src/**',
    'scripts/check-web-react-*', 'scripts/login-anatomy/**', 'apps/api/Cargo.toml',
    'scripts/runtime/prepare-web.sh', 'scripts/test-web-app-postgres.sh',
    'scripts/test-content-workbench-postgres.sh', 'scripts/test-collection-*.cjs',
    'scripts/test-collection-single-run-postgres.sh',
    'crates/evidence/src/material_social_read.rs', 'crates/evidence/src/research_dossier.rs',
    'crates/evidence/src/research_dossier/**', 'crates/evidence/src/work_resource_*.rs',
    'crates/evidence/src/material_*.rs', 'crates/evidence/src/content_reobservation.rs',
    'scripts/test-research-dossier-postgres.sh', 'crates/evidence/src/dossier_input.rs',
    'crates/evidence/src/dossier_input/**', 'crates/evidence/tests/dossier_input_postgres.rs',
    'apps/worker/src/bin/research_dossier.rs', 'database/migrations/0129_note_material_hashtags.sql',
    'database/migrations/0130_note_material_snapshot.sql',
    'crates/intelligence/src/ops_ledger/read/creation_read.rs',
    'crates/intelligence/Cargo.toml', 'crates/intelligence/src/note_interpretation_offline.rs',
    'crates/intelligence/src/note_interpretation_offline/**',
    'crates/intelligence/tests/note_interpretation_offline.rs',
    'crates/intelligence/src/comment_study_run/**',
    'crates/intelligence/tests/comment_study_start_postgres.rs',
    'scripts/test-comment-study-productization-postgres.sh',
    'crates/intelligence/src/content_workbench*', 'crates/intelligence/src/content_workbench/**',
    'crates/intelligence/src/content_workbench_sources/**', 'crates/evidence/src/lib.rs',
    'crates/intelligence/tests/content_workbench*',
    'crates/intelligence/tests/support/content_workbench_fixture.rs',
    'database/migrations/0127_content_workbench.sql', 'database/migrations/0128_content_workbench_ai.sql',
  ],
  web_real_chain: [],
  topic: [
    'apps/web/src/styles/**', 'apps/web/src/pages/topics/**', 'apps/web/test/e2e/topic*',
    'apps/web/package-lock.json', 'apps/api/src/local_web/topic_workspace*',
    'apps/api/src/local_web/web_app*',
  ],
};

// Keep all existing backend consumers covered when the web route grows. Only
// presentation tooling and page-local React files are exempt from the costly
// Rust/PostgreSQL proof; shared wiring, package changes and API clients are not.
const frontendOnlyWebPatterns = new Set([
  'apps/web/**', 'docs/design/lids/**', 'docs/design/changes/lids-component-change.json',
  'scripts/lids*', 'scripts/sync-lids*', 'scripts/check-web-react-*',
  'scripts/login-anatomy/**',
]);
paths.web_real_chain = [
  ...paths.web.filter(pattern => !frontendOnlyWebPatterns.has(pattern)),
  'apps/web/src/app/**', 'apps/web/src/corpus/**', 'apps/web/src/shared/**',
  'apps/web/src/main.tsx',
  'apps/web/package.json', 'apps/web/package-lock.json',
  // Existing OCR hosts and decoder participate in the actual reading/settings chain.
  'apps/web/src/pages/settings/AiSettingsExperience.tsx',
  'apps/web/src/pages/settings/aiSettingsModel.ts',
  'apps/web/src/pages/corpus/evidence/ReadingPanel.tsx',
  'apps/web/src/pages/**/*Api.ts', 'apps/web/src/pages/**/*Api.tsx',
  'apps/web/src/pages/**/api.ts', 'apps/web/src/pages/**/api.tsx',
  // These existing page components make HTTP requests directly. Keep this list
  // aligned with the source scan in ci-select.test.mjs as new clients appear.
  'apps/web/src/pages/collection/collectionCommand.ts',
  'apps/web/src/pages/collection/CollectionFrame.tsx',
  'apps/web/src/pages/collection/targets/inspection/useInspectionRead.ts',
  'apps/web/src/pages/collection/targets/inspection/ContentComparisonPanel.tsx',
  'apps/web/src/pages/collection/targets/useWorkspace.ts',
  'apps/web/src/pages/collection/tasks/runRequests.ts',
  'apps/web/src/pages/corpus/evidence/DossierSections.tsx',
  'apps/web/src/pages/login/sceneRuntime.ts',
  'apps/web/src/pages/login/AcceptInvitationPage.tsx',
  'apps/web/src/pages/login/LoginPage.tsx',
  'apps/web/src/pages/ops/targetSave.ts',
  'apps/web/src/pages/ops/OpsPage.tsx',
  'apps/web/src/pages/settings/AccountSecurityPage.tsx',
  'apps/web/src/pages/topics/topicRequest.ts',
  'apps/web/src/pages/topics/TopicsPage.tsx',
  // These scripts are launched by the Rust/PostgreSQL proof runners below.
  // A test-only edit must execute the test it changes.
  'apps/web/test/e2e/comments-real-chain.mjs',
  'apps/web/test/e2e/creators-real-chain.mjs',
  'apps/web/test/e2e/evidence-plugin-entry.mjs',
  'apps/web/test/e2e/evidence-real-chain.mjs',
  'apps/web/test/e2e/login-react.mjs',
  'apps/web/test/e2e/offline-preview.mjs',
  'apps/web/test/e2e/ops-real-chain.mjs',
  'apps/web/test/e2e/research-dossier-v2.mjs',
  'apps/web/test/e2e/settings-real-chain.mjs',
  'apps/web/test/e2e/settings-trials-real-chain.mjs',
  'apps/web/test/e2e/topics-real-chain.mjs',
  'apps/web/test/e2e/workbench-real-chain.mjs',
];

function globRegex(pattern) {
  const escaped = pattern.replace(/[.+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`^${escaped.replace(/\*\*|\*/g, star => star === '**' ? '.*' : '[^/]*')}$`);
}
const matchers = Object.fromEntries(suites.map(suite => [suite, paths[suite].map(globRegex)]));

export function selectSuites(changedPaths) {
  const selected = Object.fromEntries(suites.map(suite => [suite, false]));
  let full = changedPaths.length === 0;
  for (const path of changedPaths) {
    // Dossier/readings are shared by OCR, AI input and the three reading surfaces.
    // Their schema or public source contract must run every existing consumer proof.
    if (/^crates\/evidence\/src\/(?:research_dossier|material_reading|dossier_input)(?:\.rs$|\/)/.test(path)
      || /^database\/migrations\/\d+_(research_dossier_v2|comment_study_full_context|dossier_consumer_contracts)\.sql$/.test(path)) {
      full = true;
      continue;
    }
    if (/^(?:\.github\/|AGENTS\.md$|CLAUDE\.md$|codx\.md$|Cargo\.(?:toml|lock)$|\.nvmrc$|rust-toolchain(?:\.toml)?$|docs\/(?:governance|agents)\/|scripts\/(?:ci-|check-project-governance|verify-ui-design-handbook|verify-lids-alignment))/.test(path)) {
      full = true;
      continue;
    }
    const matched = suites.filter(suite => matchers[suite].some(pattern => pattern.test(path))
      && (!/^apps\/web\/.*\.css$/.test(path) || !['ocr_audit', 'web_real_chain'].includes(suite)));
    if (matched.length) {
      for (const suite of matched) selected[suite] = true;
    } else if (!/^docs\/.*\.md$/.test(path) && path !== 'README.md') {
      full = true;
    }
  }
  if (full) for (const suite of suites) selected[suite] = true;
  return { ...selected, full };
}

export function changedPaths(base, cwd = process.cwd()) {
  // Resolve to a commit ID before constructing a rev range. argv is never a shell.
  const options = { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] };
  const commit = execFileSync('git', ['rev-parse', '--verify', '--end-of-options', `${base}^{commit}`], options).trim();
  return execFileSync('git', ['diff', '--no-renames', '--name-only', '-z', `${commit}...HEAD`], options).split('\0').filter(Boolean);
}

function main() {
  const [base, ...rest] = process.argv.slice(2);
  if (!base || rest.length) throw new Error('Usage: node scripts/ci-select.mjs <base-ref>');
  const result = selectSuites(changedPaths(base));
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, Object.entries(result).map(([name, value]) => `${name}=${value}\n`).join(''));
  console.log(JSON.stringify(result, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
