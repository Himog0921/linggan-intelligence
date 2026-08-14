// @vitest-environment node

/**
 * B2 Derived schema isolated-database proof.
 *
 * Opt-in only. Creates and preserves a fresh database whose name is asserted
 * before the candidate migration is applied. Never targets the formal local
 * test database.
 *
 * Run:
 *   V2_B2_INTEGRATION_DB=1 npx vitest run src/lib/evidence/derived/b2-derived-schema.integration.test.ts
 */

import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PG_HOST = "127.0.0.1";
const PG_PORT = "54329";
const PG_USER = "postgres";
const PG_PASSWORD = "postgres";
const PG_ENV = { ...process.env, PGPASSWORD: PG_PASSWORD };

const FIXED_POINT = "d482109d35e36e769a754f17a785f0b39cc868d6";
const PROOF_DB_PREFIX = "content_workbench_v2_b2_schema_";
const FIXED_SCHEMA_PATH = join(tmpdir(), `v2-b2-fixed-schema-${Date.now()}.prisma`);
const MIGRATION_PATH = "prisma/migrations/20260810193000_add_v2_derived_schema/migration.sql";
const POSITIVE_SQL_PATH = "docs/architecture/v2/proofs/b2-derived-positive.sql";
const BLK_003_NEGATIVE_SQL_PATH = "docs/architecture/v2/proofs/b2-blk-003-negative.sql";
const BLK_016_NEGATIVE_SQL_PATH = "docs/architecture/v2/proofs/b2-blk-016-negative.sql";
const PRISMA_CLI_PATH = process.env.V2_B2_PRISMA_CLI ?? "node_modules/prisma/build/index.js";
const OPTED_IN = process.env.V2_B2_INTEGRATION_DB === "1";

let proofDatabase = "";
let proofDatabaseSequence = 0;
const preservedProofDatabases: string[] = [];

function databaseUrl(database: string): string {
  return `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${database}?schema=public`;
}

function psqlQuery(database: string, sql: string): string {
  return execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-t", "-A", "-c", sql],
    { env: PG_ENV, encoding: "utf8" },
  ).trim();
}

function psqlSql(database: string, sql: string): void {
  execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-c", sql],
    { env: PG_ENV, encoding: "utf8" },
  );
}

function errorOutput(error: unknown): string {
  if (!(error instanceof Error)) return String(error);
  const processError = error as Error & { stderr?: Buffer | string; stdout?: Buffer | string };
  const stderr = Buffer.isBuffer(processError.stderr)
    ? processError.stderr.toString("utf8")
    : processError.stderr ?? "";
  const stdout = Buffer.isBuffer(processError.stdout)
    ? processError.stdout.toString("utf8")
    : processError.stdout ?? "";
  return `${stderr}\n${stdout}\n${error.message}`;
}

function psqlFailure(database: string, sql: string): string {
  try {
    psqlSql(database, sql);
  } catch (error) {
    return errorOutput(error);
  }
  throw new Error(`Expected SQL to fail in ${database}, but it succeeded.`);
}

function psqlFile(database: string, filePath: string): string {
  return execFileSync(
    "psql",
    [
      "-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database,
      "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-f", filePath,
    ],
    { env: PG_ENV, encoding: "utf8" },
  ).trim();
}

function psqlFileFailure(database: string, filePath: string): string {
  try {
    psqlFile(database, filePath);
  } catch (error) {
    return errorOutput(error);
  }
  throw new Error(`Expected SQL file ${filePath} to fail in ${database}, but it succeeded.`);
}

function assertProofDatabase(database: string): void {
  if (!database.startsWith(PROOF_DB_PREFIX)) {
    throw new Error(`Unsafe B2 proof database name: ${database}`);
  }
  expect(psqlQuery(database, "SELECT current_database();")).toBe(database);
}

function pushFixedPointSchema(database: string): void {
  execFileSync(
    process.execPath,
    [PRISMA_CLI_PATH, "db", "push", "--url", databaseUrl(database), "--schema", FIXED_SCHEMA_PATH],
    { cwd: process.cwd(), stdio: "pipe", timeout: 120_000 },
  );
}

function seedCompleteChain(database: string): void {
  expect(psqlFile(database, POSITIVE_SQL_PATH)).toContain("B2_DERIVED_POSITIVE_PASS");
}

beforeAll(() => {
  if (!OPTED_IN) return;

  writeFileSync(
    FIXED_SCHEMA_PATH,
    execFileSync("git", ["show", `${FIXED_POINT}:prisma/schema.prisma`], { encoding: "utf8" }),
  );
}, 30_000);

beforeEach(() => {
  proofDatabaseSequence += 1;
  proofDatabase = `${PROOF_DB_PREFIX}${Date.now()}_${proofDatabaseSequence}`;
  psqlSql("postgres", `CREATE DATABASE "${proofDatabase}";`);
  preservedProofDatabases.push(proofDatabase);
  assertProofDatabase(proofDatabase);
  pushFixedPointSchema(proofDatabase);
  psqlFile(proofDatabase, MIGRATION_PATH);
  seedCompleteChain(proofDatabase);
}, 180_000);

afterAll(() => {
  if (existsSync(FIXED_SCHEMA_PATH)) unlinkSync(FIXED_SCHEMA_PATH);
  for (const database of preservedProofDatabases) {
    console.log(`[b2-schema] Preserving proof database ${database}.`);
    console.log(
      `[b2-schema] Manual cleanup: dropdb --if-exists --host=${PG_HOST} --port=${PG_PORT} --username=${PG_USER} ${database}`,
    );
  }
});

const describeIntegration = OPTED_IN ? describe : describe.skip;

describeIntegration("B2 Derived schema isolated database", () => {
  it("persists one complete same-source B2 chain", () => {
    assertProofDatabase(proofDatabase);
    expect(psqlQuery(proofDatabase, `
      SELECT concat_ws(',',
        (SELECT count(*) FROM "NormalizationRun"),
        (SELECT count(*) FROM "NormalizationRunCurrent"),
        (SELECT count(*) FROM "CanonicalObservation"),
        (SELECT count(*) FROM "ContractEvaluation"),
        (SELECT count(*) FROM "ContractEvaluationCurrent"),
        (SELECT count(*) FROM "ContractEvaluationNormalizationRun"),
        (SELECT count(*) FROM "ContractEvaluationInput")
      );
    `)).toBe("1,1,1,1,1,1,1");
  });

  it("rejects all four cross-source relationships", () => {
    const auditedCrossSnapshotFailure = psqlFileFailure(proofDatabase, BLK_016_NEGATIVE_SQL_PATH);
    expect(auditedCrossSnapshotFailure).toContain("23503");
    expect(auditedCrossSnapshotFailure).toContain(
      "NormalizationRunCurrent_workspace_snapshot_run_fkey",
    );
    expect(auditedCrossSnapshotFailure).toContain(
      "NormalizationRunCurrent_workspace_snapshot_record_fkey",
    );
    expect(auditedCrossSnapshotFailure).toContain("EXPECTED_NEGATIVE_SUITE_COMPLETE");

    psqlSql(proofDatabase, `
      INSERT INTO "RawSnapshot"
        ("id", "workspaceId", "captureId", "platform", "targetKey", "observedAt")
      VALUES
        ('snapshot-b', 'workspace-a', 'capture-b', 'xhs', 'xhs:note:note-b', now()),
        ('snapshot-other-workspace', 'workspace-b', 'capture-c', 'xhs', 'xhs:note:note-c', now());

      INSERT INTO "RawRecord"
        ("id", "workspaceId", "rawSnapshotId", "recordKind", "platform", "payload",
         "payloadHash", "observedAt", "idempotencyKey")
      VALUES
        ('record-b', 'workspace-a', 'snapshot-b', 'note', 'xhs', '{}'::jsonb,
         'raw-hash-b', now(), 'record-key-b'),
        ('record-other-workspace', 'workspace-b', 'snapshot-other-workspace', 'note', 'xhs', '{}'::jsonb,
         'raw-hash-c', now(), 'record-key-c');

      INSERT INTO "NormalizationRun"
        ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
         "canonicalSchemaVersion", "status", "inputPayloadHash", "outputPayloadHash")
      VALUES
        ('run-b', 'workspace-a', 'snapshot-b', 'record-b', 'xhs-note', '1',
         '1', 'normalized', 'raw-hash-b', 'canonical-hash-b'),
        ('run-a-secondary', 'workspace-a', 'snapshot-a', 'record-a', 'xhs-note-secondary', '1',
         '1', 'normalized', 'raw-hash-a', 'canonical-hash-a-secondary'),
        ('run-a-tertiary', 'workspace-a', 'snapshot-a', 'record-a', 'xhs-note-tertiary', '1',
         '1', 'normalized', 'raw-hash-a', 'canonical-hash-a-tertiary');
    `);

    const crossWorkspace = psqlFailure(proofDatabase, `
      INSERT INTO "NormalizationRun"
        ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
         "canonicalSchemaVersion", "status", "inputPayloadHash")
      VALUES
        ('run-cross-workspace', 'workspace-b', 'snapshot-a', 'record-a', 'xhs-note-cross-workspace', '1',
         '1', 'normalized', 'raw-hash-a');
    `);
    expect(crossWorkspace).toContain("23503");
    expect(crossWorkspace).toContain("NormalizationRun_workspace_snapshot_fkey");

    const runToWrongRecord = psqlFailure(proofDatabase, `
      INSERT INTO "NormalizationRun"
        ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
         "canonicalSchemaVersion", "status", "inputPayloadHash")
      VALUES
        ('run-wrong-record', 'workspace-a', 'snapshot-b', 'record-a', 'xhs-note-wrong-record', '1',
         '1', 'normalized', 'raw-hash-a');
    `);
    expect(runToWrongRecord).toContain("23503");
    expect(runToWrongRecord).toContain("NormalizationRun_workspace_snapshot_record_fkey");

    const currentToWrongRun = psqlFailure(proofDatabase, `
      INSERT INTO "NormalizationRunCurrent"
        ("workspaceId", "rawRecordId", "rawSnapshotId", "normalizationRunId", "adapterId",
         "adapterVersion", "canonicalSchemaVersion")
      VALUES
        ('workspace-a', 'record-b', 'snapshot-b', 'run-a-secondary', 'xhs-note-secondary', '1', '1');
    `);
    expect(currentToWrongRun).toContain("23503");
    expect(currentToWrongRun).toContain("NormalizationRunCurrent_workspace_snapshot_run_fkey");

    const currentToWrongRecord = psqlFailure(proofDatabase, `
      INSERT INTO "NormalizationRunCurrent"
        ("workspaceId", "rawRecordId", "rawSnapshotId", "normalizationRunId", "adapterId",
         "adapterVersion", "canonicalSchemaVersion")
      VALUES
        ('workspace-a', 'record-b', 'snapshot-a', 'run-a-tertiary', 'xhs-note-tertiary', '1', '1');
    `);
    expect(currentToWrongRecord).toContain("23503");
    expect(currentToWrongRecord).toContain("NormalizationRunCurrent_workspace_snapshot_record_fkey");

    const observationToWrongRecord = psqlFailure(proofDatabase, `
      INSERT INTO "CanonicalObservation"
        ("id", "workspaceId", "normalizationRunId", "rawSnapshotId", "rawRecordId",
         "observationKind", "subjectKey", "observedAt", "schemaVersion", "payload",
         "payloadHash", "qualityStatus")
      VALUES
        ('observation-wrong-record', 'workspace-a', 'run-a', 'snapshot-a', 'record-b',
         'content', 'xhs:note:note-b', now(), '1', '{}'::jsonb,
         'canonical-hash-b', 'complete');
    `);
    expect(observationToWrongRecord).toContain("23503");
    expect(observationToWrongRecord).toContain("CanonicalObservation_workspace_snapshot_record_fkey");

    expect(psqlQuery(proofDatabase, `SELECT count(*) FROM "NormalizationRun";`)).toBe("4");
    expect(psqlQuery(proofDatabase, `SELECT count(*) FROM "NormalizationRunCurrent";`)).toBe("1");
    expect(psqlQuery(proofDatabase, `SELECT count(*) FROM "CanonicalObservation";`)).toBe("1");
  });

  it("rejects the superseded three-column to two-column FK shape", () => {
    const mismatchedForeignKey = psqlFileFailure(proofDatabase, BLK_003_NEGATIVE_SQL_PATH);
    expect(mismatchedForeignKey).toContain("42830");
    expect(mismatchedForeignKey).toContain(
      "number of referencing and referenced columns for foreign key disagree",
    );
    expect(psqlQuery(proofDatabase, `
      SELECT count(*) FROM pg_constraint
      WHERE conname IN ('RawRecord_mismatch_probe_key', 'NormalizationRunCurrent_mismatch_probe_fkey');
    `)).toBe("0");
  });

  it("keeps B2 facts immutable and limits Current tables to UPDATE capability", () => {
    const immutableRows = [
      { table: "NormalizationRun", predicate: `"id" = 'run-a'` },
      { table: "CanonicalObservation", predicate: `"id" = 'observation-a'` },
      { table: "ContractEvaluation", predicate: `"id" = 'evaluation-a'` },
      { table: "ContractEvaluationNormalizationRun", predicate: `"id" = 'evaluation-run-a'` },
      { table: "ContractEvaluationInput", predicate: `"id" = 'evaluation-input-a'` },
    ];

    for (const row of immutableRows) {
      const updateFailure = psqlFailure(
        proofDatabase,
        `BEGIN; UPDATE "${row.table}" SET "createdAt" = "createdAt" WHERE ${row.predicate}; ROLLBACK;`,
      );
      expect(updateFailure).toContain("55000");
      expect(updateFailure).toContain("V2 Derived rows are immutable");

      const deleteFailure = psqlFailure(
        proofDatabase,
        `BEGIN; DELETE FROM "${row.table}" WHERE ${row.predicate}; ROLLBACK;`,
      );
      expect(deleteFailure).toContain("55000");
      expect(deleteFailure).toContain("V2 Derived rows are immutable");
    }

    psqlSql(proofDatabase, `
      UPDATE "NormalizationRunCurrent"
      SET "updatedAt" = "updatedAt" + interval '1 second'
      WHERE "workspaceId" = 'workspace-a' AND "rawRecordId" = 'record-a';
      UPDATE "ContractEvaluationCurrent"
      SET "updatedAt" = "updatedAt" + interval '1 second'
      WHERE "workspaceId" = 'workspace-a'
        AND "rawSnapshotId" = 'snapshot-a'
        AND "contractId" = 'xhs.note-detail'
        AND "contractVersion" = 1;
    `);
    expect(psqlQuery(proofDatabase, `
      SELECT count(*) FROM "NormalizationRunCurrent"
      WHERE "workspaceId" = 'workspace-a' AND "rawRecordId" = 'record-a';
    `)).toBe("1");
    expect(psqlQuery(proofDatabase, `
      SELECT count(*) FROM "ContractEvaluationCurrent"
      WHERE "workspaceId" = 'workspace-a'
        AND "rawSnapshotId" = 'snapshot-a'
        AND "contractId" = 'xhs.note-detail'
        AND "contractVersion" = 1;
    `)).toBe("1");

    for (const table of ["NormalizationRunCurrent", "ContractEvaluationCurrent"]) {
      const deleteFailure = psqlFailure(proofDatabase, `BEGIN; DELETE FROM "${table}"; ROLLBACK;`);
      expect(deleteFailure).toContain("55000");
      expect(deleteFailure).toContain("V2 Derived Current rows cannot be deleted");
    }
  });

  it("rejects invalid normalization and evaluation terminal states", () => {
    const invalidRunStatus = psqlFailure(proofDatabase, `
      BEGIN;
      INSERT INTO "NormalizationRun"
        ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
         "canonicalSchemaVersion", "status", "inputPayloadHash")
      VALUES
        ('run-invalid-status', 'workspace-a', 'snapshot-a', 'record-a', 'xhs-note-invalid', '1',
         '1', 'running', 'raw-hash-a');
      ROLLBACK;
    `);
    expect(invalidRunStatus).toContain("23514");
    expect(invalidRunStatus).toContain("NormalizationRun_status_check");

    const acceptedNotApplicable = psqlFailure(proofDatabase, `
      BEGIN;
      INSERT INTO "ContractEvaluation"
        ("id", "workspaceId", "rawSnapshotId", "contractId", "contractVersion",
         "evaluatorVersion", "canonicalSchemaVersion", "evaluationInputHash", "decision", "completeness")
      VALUES
        ('evaluation-invalid-accepted', 'workspace-a', 'snapshot-a', 'invalid.accepted', 1,
         '1', '1', 'invalid-evaluation-hash-a', 'accepted', 'not_applicable');
      ROLLBACK;
    `);
    expect(acceptedNotApplicable).toContain("23514");
    expect(acceptedNotApplicable).toContain("ContractEvaluation_decision_completeness_check");

    const rejectedFull = psqlFailure(proofDatabase, `
      BEGIN;
      INSERT INTO "ContractEvaluation"
        ("id", "workspaceId", "rawSnapshotId", "contractId", "contractVersion",
         "evaluatorVersion", "canonicalSchemaVersion", "evaluationInputHash", "decision", "completeness")
      VALUES
        ('evaluation-invalid-rejected', 'workspace-a', 'snapshot-a', 'invalid.rejected', 1,
         '1', '1', 'invalid-evaluation-hash-b', 'rejected', 'full');
      ROLLBACK;
    `);
    expect(rejectedFull).toContain("23514");
    expect(rejectedFull).toContain("ContractEvaluation_decision_completeness_check");
  });

  it("rejects cross-source evaluation links and Current contract drift", () => {
    psqlSql(proofDatabase, `
      INSERT INTO "RawSnapshot"
        ("id", "workspaceId", "captureId", "platform", "targetKey", "observedAt")
      VALUES
        ('snapshot-b', 'workspace-a', 'capture-b', 'xhs', 'xhs:note:note-b', now());

      INSERT INTO "RawRecord"
        ("id", "workspaceId", "rawSnapshotId", "recordKind", "platform", "payload",
         "payloadHash", "observedAt", "idempotencyKey")
      VALUES
        ('record-b', 'workspace-a', 'snapshot-b', 'note', 'xhs', '{}'::jsonb,
         'raw-hash-b', now(), 'record-key-b');

      INSERT INTO "NormalizationRun"
        ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
         "canonicalSchemaVersion", "status", "inputPayloadHash", "outputPayloadHash")
      VALUES
        ('run-b', 'workspace-a', 'snapshot-b', 'record-b', 'xhs-note', '1',
         '1', 'normalized', 'raw-hash-b', 'canonical-hash-b');

      INSERT INTO "CanonicalObservation"
        ("id", "workspaceId", "normalizationRunId", "rawSnapshotId", "rawRecordId",
         "observationKind", "subjectKey", "observedAt", "schemaVersion", "payload",
         "payloadHash", "qualityStatus")
      VALUES
        ('observation-b', 'workspace-a', 'run-b', 'snapshot-b', 'record-b',
         'content', 'xhs:note:note-b', now(), '1', '{}'::jsonb,
         'canonical-hash-b', 'complete');
    `);

    const evaluationCrossWorkspace = psqlFailure(proofDatabase, `
      INSERT INTO "ContractEvaluation"
        ("id", "workspaceId", "rawSnapshotId", "contractId", "contractVersion",
         "evaluatorVersion", "canonicalSchemaVersion", "evaluationInputHash", "decision", "completeness")
      VALUES
        ('evaluation-cross-workspace', 'workspace-b', 'snapshot-a', 'xhs.note-detail', 2,
         '1', '1', 'evaluation-hash-cross-workspace', 'accepted', 'full');
    `);
    expect(evaluationCrossWorkspace).toContain("23503");
    expect(evaluationCrossWorkspace).toContain("ContractEvaluation_workspace_snapshot_fkey");

    const evaluationToWrongRun = psqlFailure(proofDatabase, `
      INSERT INTO "ContractEvaluationNormalizationRun"
        ("id", "evaluationId", "normalizationRunId", "workspaceId", "rawSnapshotId",
         "status", "inputPayloadHash", "outputPayloadHash")
      VALUES
        ('evaluation-run-wrong-source', 'evaluation-a', 'run-b', 'workspace-a', 'snapshot-a',
         'normalized', 'raw-hash-b', 'canonical-hash-b');
    `);
    expect(evaluationToWrongRun).toContain("23503");
    expect(evaluationToWrongRun).toContain("ContractEvaluationNormalizationRun_workspace_run_fkey");

    const evaluationToWrongObservation = psqlFailure(proofDatabase, `
      INSERT INTO "ContractEvaluationInput"
        ("id", "workspaceId", "rawSnapshotId", "contractEvaluationId",
         "canonicalObservationId", "ordinal", "canonicalOutputHash")
      VALUES
        ('evaluation-input-wrong-source', 'workspace-a', 'snapshot-a', 'evaluation-a',
         'observation-b', 1, 'canonical-hash-b');
    `);
    expect(evaluationToWrongObservation).toContain("23503");
    expect(evaluationToWrongObservation).toContain("ContractEvaluationInput_workspace_observation_fkey");

    const currentContractDrift = psqlFailure(proofDatabase, `
      INSERT INTO "ContractEvaluationCurrent"
        ("workspaceId", "rawSnapshotId", "contractId", "contractVersion",
         "contractEvaluationId", "revision")
      VALUES
        ('workspace-a', 'snapshot-a', 'xhs.wrong-contract', 1, 'evaluation-a', 1);
    `);
    expect(currentContractDrift).toContain("23503");
    expect(currentContractDrift).toContain("ContractEvaluationCurrent_workspace_evaluation_fkey");
  });
});
