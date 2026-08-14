/**
 * EvidenceIngress isolated-database integration test.
 *
 * Creates fresh proof databases from the fixed-point schema, applies the exact
 * Release-A append-only trigger section, then executes the Release-B candidate
 * migration with stop-on-error semantics.
 *
 * Opt-in via V2_B1_INTEGRATION_DB=1. Ordinary npm test does NOT create or
 * drop any external database.
 *
 * Run:
 *   V2_B1_INTEGRATION_DB=1 npx vitest run src/lib/evidence/ingress/evidence-ingress.integration.test.ts
 */

import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { execFileSync, execSync } from "node:child_process";
import { writeFileSync, unlinkSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createPrismaClient } from "@/lib/db";
import { EvidenceIngress } from "./evidence-ingress";
import {
  AlwaysValidAuthorityValidator,
  testRegistry,
  buildBaseHeader,
  buildSubmission,
  buildTestRecord,
} from "./test-fixtures";
import { sha256Hex } from "./base64";
import type {
  CaptureArtifactSubmissionV2,
  CaptureHeaderV2,
  CapturePackagePayloadV2,
  CaptureSubmissionV2,
  IngressKindV2,
} from "./types";
import { buildExecutionIngressBinding } from "../adapters/execution-adapter";
import {
  submitExecutionEvidence,
  submitManualImportEvidence,
  submitRecoveryEvidence,
} from "./evidence-ingress-orchestrator";
import {
  sha256Hex as sha256Text,
  signStationRequest,
  verifyExecutionStationIngressSession,
} from "@/lib/services/execution-station-signature-service";
import { encryptStationSigningSecret } from "@/lib/services/execution-station-signing-secret";
import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";

const PG_HOST = "127.0.0.1";
const PG_PORT = "54329";
const PG_USER = "postgres";
const PG_PASSWORD = "postgres";
const PG_ENV = { ...process.env, PGPASSWORD: PG_PASSWORD };

const FIXED_POINT = "a274907ef1016a3af27ec0b7f3badaaa37cec554";
const PROOF_DB_PREFIX = "content_workbench_v2_b1_release_b_core_";
const DB_NAME = `content_workbench_v2_b1_release_b_core_${Date.now()}`;
const PREFLIGHT_DB_NAME = `${DB_NAME}_preflight`;
const DB_URL = `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${DB_NAME}?schema=public`;
const FIXED_SCHEMA_PATH = join(tmpdir(), `v2-b1-fixed-schema-${Date.now()}.prisma`);
const RELEASE_B_MIGRATION_PATH = "prisma/migrations/20260805180000_add_v2_evidence_release_b_prepare/migration.sql";
const EXECUTION_AUTHORITY_MIGRATION_PATH = "prisma/migrations/20260810143000_add_execution_job_plan_version/migration.sql";
const OPTED_IN = process.env.V2_B1_INTEGRATION_DB === "1";
const EXECUTION_STATION_ID = "station-001";
const EXECUTION_STATION_TOKEN = "station-token-integration";
const EXECUTION_SIGNING_SECRET = "station-signing-secret-integration";
const EXECUTION_AUTHORIZATION_ID = "plugin-auth-001";
const EXECUTION_AUTHORIZATION_TOKEN = "plugin-auth-token-integration";
const EXECUTION_SYNC_PATH = "/api/execution-stations/sync";

function psqlQuery(database: string, sql: string): string {
  return execFileSync("psql", [
    "-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER,
    "-d", database, "-t", "-A", "-c", sql,
  ], { env: PG_ENV, encoding: "utf8" }).trim();
}

function psqlFile(database: string, filePath: string): void {
  execFileSync("psql", [
    "-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER,
    "-d", database, "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-f", filePath,
  ], { env: PG_ENV, encoding: "utf8" });
}

function psqlSql(database: string, sql: string): void {
  execFileSync("psql", [
    "-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER,
    "-d", database, "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-c", sql,
  ], { env: PG_ENV, encoding: "utf8" });
}

function psqlFailure(database: string, sql: string): string {
  try {
    psqlSql(database, sql);
  } catch (error) {
    return errorOutput(error);
  }
  throw new Error(`Expected SQL to fail in ${database}, but it succeeded.`);
}

function databaseUrl(database: string): string {
  return `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${database}?schema=public`;
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

function assertProofDatabaseIdentity(database: string): string {
  if (!database.startsWith(PROOF_DB_PREFIX)) {
    throw new Error(`Unsafe integration database name: ${database}`);
  }
  const actual = psqlQuery(database, "SELECT current_database();");
  if (actual !== database) {
    throw new Error(`Database identity mismatch: expected ${database}, received ${actual}`);
  }
  return actual;
}

function pushFixedPointSchema(database: string): void {
  execFileSync(
    "npx",
    ["prisma", "db", "push", "--url", databaseUrl(database), "--schema", FIXED_SCHEMA_PATH],
    {
      cwd: process.cwd(),
      stdio: ["pipe", "pipe", "pipe"],
      timeout: 120_000,
    },
  );
}

function releaseAAppendOnlySql(): string {
  const migration = execFileSync(
    "git",
    ["show", `${FIXED_POINT}:prisma/migrations/20260805143000_add_v2_evidence_release_a/migration.sql`],
    { encoding: "utf8" },
  );
  const startMarker = 'CREATE OR REPLACE FUNCTION "reject_v2_evidence_mutation"()';
  const endMarker = 'FOR EACH ROW EXECUTE FUNCTION "reject_v2_evidence_mutation"();';
  const start = migration.indexOf(startMarker);
  const finalStatement = migration.lastIndexOf(endMarker);
  if (start < 0 || finalStatement < start) {
    throw new Error("Unable to locate the authoritative Release-A append-only SQL section.");
  }
  return migration.slice(start, finalStatement + endMarker.length);
}

function createAndVerifyBlankDatabase(database: string): void {
  psqlSql("postgres", `CREATE DATABASE "${database}";`);
  assertProofDatabaseIdentity(database);
}

function executionSignedHeaders(bodyText: string, signingSecret = EXECUTION_SIGNING_SECRET): Headers {
  const timestamp = new Date().toISOString();
  const nonce = `integration-${Date.now()}-${Math.random()}`;
  const bodyHash = sha256Text(bodyText);
  return new Headers({
    "x-cw-station-id": EXECUTION_STATION_ID,
    "x-cw-timestamp": timestamp,
    "x-cw-nonce": nonce,
    "x-cw-body-sha256": bodyHash,
    "x-cw-signature": signStationRequest({
      method: "POST",
      path: EXECUTION_SYNC_PATH,
      timestamp,
      nonce,
      bodyHash,
      signingSecret,
    }),
  });
}

let prismaClient: ReturnType<typeof createPrismaClient>;
let prismaClient2: ReturnType<typeof createPrismaClient>;
let ingress: EvidenceIngress;
let dbCreated = false;
let preflightDbCreated = false;
let preflightFailure = "";

// Reusable test artifact: a small binary payload with verifiable checksum.
const TEST_ARTIFACT_BYTES = Buffer.from("v2-b1-release-b-core-artifact-payload");
const TEST_ARTIFACT_BASE64 = TEST_ARTIFACT_BYTES.toString("base64");
const TEST_ARTIFACT_CHECKSUM = sha256Hex(TEST_ARTIFACT_BYTES);

function buildTestArtifact(overrides: Partial<CaptureArtifactSubmissionV2> = {}): CaptureArtifactSubmissionV2 {
  return {
    kind: "context",
    encoding: "base64",
    artifactPayload: TEST_ARTIFACT_BASE64,
    artifactChecksum: TEST_ARTIFACT_CHECKSUM,
    contentLength: TEST_ARTIFACT_BYTES.length,
    restricted: false,
    ...overrides,
  };
}

function buildRealXhsSubmission(
  kind: IngressKindV2,
  options: {
    headerOverrides?: Partial<CaptureHeaderV2>;
    records?: CapturePackagePayloadV2["records"];
    artifacts?: CapturePackagePayloadV2["artifacts"];
  } = {},
): CaptureSubmissionV2 {
  const base = buildBaseHeader(kind);
  return buildSubmission(kind, {
    records: options.records,
    artifacts: options.artifacts,
    headerOverrides: {
      ...options.headerOverrides,
      contractId: XHS_NOTE_DETAIL.id,
      contractVersion: XHS_NOTE_DETAIL.version,
      contractHash: computeContractHash(XHS_NOTE_DETAIL),
      report: {
        ...base.report,
        slots: [
          { slotId: "note", status: "observed", reason: null },
          { slotId: "comments", status: "not_applicable", reason: "fixture_without_comments" },
        ],
      },
    },
  });
}

beforeAll(async () => {
  if (!OPTED_IN) {
    console.log("[integration] SKIP: set V2_B1_INTEGRATION_DB=1 to run isolated DB tests");
    return;
  }

  // 1. Extract the fixed-point schema before touching either proof database.
  const schemaContent = execFileSync(
    "git",
    ["show", `${FIXED_POINT}:prisma/schema.prisma`],
    { encoding: "utf8" },
  );
  writeFileSync(FIXED_SCHEMA_PATH, schemaContent);

  // 2. Create the positive proof database and assert its identity before the
  // first target-schema DDL statement.
  createAndVerifyBlankDatabase(DB_NAME);
  dbCreated = true;
  pushFixedPointSchema(DB_NAME);
  psqlSql(DB_NAME, releaseAAppendOnlySql());
  psqlFile(DB_NAME, RELEASE_B_MIGRATION_PATH);
  psqlFile(DB_NAME, EXECUTION_AUTHORITY_MIGRATION_PATH);

  // 3. Create a second fresh database to prove the candidate migration itself
  // aborts before DDL when historical cross-workspace rows exist.
  createAndVerifyBlankDatabase(PREFLIGHT_DB_NAME);
  preflightDbCreated = true;
  pushFixedPointSchema(PREFLIGHT_DB_NAME);
  psqlSql(PREFLIGHT_DB_NAME, `
    INSERT INTO "RawSnapshot"
      ("id", "workspaceId", "captureId", "platform", "targetKey", "observedAt", "schemaVersion")
    VALUES
      ('preflight-snapshot', 'preflight-parent-workspace', 'preflight-capture', 'xhs', 'xhs:note/preflight', now(), 'v3');
    INSERT INTO "RawRecord"
      ("id", "workspaceId", "rawSnapshotId", "recordType", "platform", "payload", "observedAt", "idempotencyKey")
    VALUES
      ('preflight-record', 'preflight-other-workspace', 'preflight-snapshot', 'note', 'xhs', '{}'::jsonb, now(), 'preflight-key');
  `);
  try {
    psqlFile(PREFLIGHT_DB_NAME, RELEASE_B_MIGRATION_PATH);
    throw new Error("Release-B migration unexpectedly accepted cross-workspace historical data.");
  } catch (error) {
    preflightFailure = errorOutput(error);
    if (!preflightFailure.includes("Release-B prepare blocked")) throw error;
  }

  // 4. Create two independent Prisma clients for concurrency proof.
  prismaClient = createPrismaClient(DB_URL);
  prismaClient2 = createPrismaClient(DB_URL);

  await prismaClient.pluginAuthorization.create({
    data: {
      id: EXECUTION_AUTHORIZATION_ID,
      workspaceId: "ws-test-001",
      deviceId: "integration-device-001",
      authorizationTokenHash: sha256Text(EXECUTION_AUTHORIZATION_TOKEN),
      status: "active",
    },
  });
  await prismaClient.executionStation.create({
    data: {
      id: EXECUTION_STATION_ID,
      stationKey: "integration-station-key-001",
      workspaceId: "ws-test-001",
      pluginAuthorizationId: EXECUTION_AUTHORIZATION_ID,
      displayName: "B1 integration station",
      capabilities: "[]",
      stationTokenHash: sha256Text(EXECUTION_STATION_TOKEN),
      signingSecretEncrypted: encryptStationSigningSecret(EXECUTION_SIGNING_SECRET),
      signingSecretVersion: 1,
    },
  });

  ingress = new EvidenceIngress(
    prismaClient,
    testRegistry(),
    new AlwaysValidAuthorityValidator(),
  );
}, 180_000);

afterAll(async () => {
  if (prismaClient) await prismaClient.$disconnect();
  if (prismaClient2) await prismaClient2.$disconnect();
  // Clean up temp schema file
  try { if (existsSync(FIXED_SCHEMA_PATH)) unlinkSync(FIXED_SCHEMA_PATH); } catch { /* best effort */ }
  if (dbCreated) {
    console.log(`[integration] Preserving database ${DB_NAME} for review.`);
    console.log(`[integration] Manual cleanup: dropdb --if-exists --host=${PG_HOST} --port=${PG_PORT} --username=${PG_USER} ${DB_NAME}`);
  }
  if (preflightDbCreated) {
    console.log(`[integration] Preserving database ${PREFLIGHT_DB_NAME} for review.`);
    console.log(`[integration] Manual cleanup: dropdb --if-exists --host=${PG_HOST} --port=${PG_PORT} --username=${PG_USER} ${PREFLIGHT_DB_NAME}`);
  }
});

function countTable(table: string): number {
  return parseInt(psqlQuery(DB_NAME, `SELECT count(*) FROM "${table}";`), 10);
}

const describeIntegration = OPTED_IN ? describe : describe.skip;

describeIntegration("EvidenceIngress isolated database integration", () => {

  it("confirms the retained positive proof database identity", () => {
    const db = psqlQuery(DB_NAME, "SELECT current_database();");
    expect(db).toBe(DB_NAME);
    console.log(`[integration] Verified current_database() = ${db}`);
  });

  it("proves Release-B preflight aborts before schema DDL", () => {
    expect(preflightFailure).toContain("Release-B prepare blocked");
    expect(psqlQuery(PREFLIGHT_DB_NAME, `
      SELECT "is_nullable"
      FROM information_schema.columns
      WHERE table_schema = 'public'
        AND table_name = 'RawSnapshot'
        AND column_name = 'schemaVersion';
    `)).toBe("NO");
    expect(psqlQuery(PREFLIGHT_DB_NAME, `
      SELECT count(*)
      FROM pg_constraint
      WHERE conname = 'RawRecord_rawSnapshotId_fkey';
    `)).toBe("1");
    expect(psqlQuery(PREFLIGHT_DB_NAME, `
      SELECT count(*)
      FROM pg_constraint
      WHERE conname = 'RawRecord_workspace_snapshot_fkey';
    `)).toBe("0");
  });

  // ── Baseline ──────────────────────────────────────────────────────────────
  let baseline: Record<string, number>;

  it("records pre-write baseline counts (all zero)", () => {
    baseline = {
      CapturePackage: countTable("CapturePackage"),
      RawSnapshot: countTable("RawSnapshot"),
      RawRecord: countTable("RawRecord"),
      CaptureArtifact: countTable("CaptureArtifact"),
      EvidenceIngressReceipt: countTable("EvidenceIngressReceipt"),
      OutboxEvent: countTable("OutboxEvent"),
    };
    expect(baseline.CapturePackage).toBe(0);
    expect(baseline.RawSnapshot).toBe(0);
    console.log(`[integration] Baseline: ${JSON.stringify(baseline)}`);
  });

  // ── §3.4: execution with artifact ────────────────────────────────────────

  it("commits execution through the real signed-session execution adapter", async () => {
    const leaseExpiresAt = new Date(Date.now() + 5 * 60_000);
    await prismaClient.executionJob.create({
      data: {
        id: "job-001",
        workspaceId: "ws-test-001",
        platform: "xhs",
        targetType: "note",
        targetKey: "xhs:note/abc123",
        jobType: "collect_detail",
        collectionProfile: "note_full",
        requiredFields: {},
        payload: {},
        executionPlanVersion: "plan-v1",
        lane: "manual_hot",
        status: "in_progress",
      },
    });
    await prismaClient.executionQueueEntry.create({
      data: {
        jobId: "job-001",
        workspaceId: "ws-test-001",
        platform: "xhs",
        source: "manual",
        lane: "manual_hot",
        status: "in_progress",
        reservedByStationId: "station-001",
        leaseToken: "lease-token-abc",
        leaseEpoch: 1,
        leaseExpiresAt,
        lastAttemptId: "att-001",
      },
    });
    await prismaClient.taskAttempt.create({
      data: {
        id: "att-001",
        jobId: "job-001",
        stationId: "station-001",
        leaseToken: "lease-token-abc",
        leaseEpoch: 1,
      },
    });
    await prismaClient.executionTaskRuntime.create({
      data: {
        jobId: "job-001",
        workspaceId: "ws-test-001",
        status: "running",
        activeExecutor: EXECUTION_STATION_ID,
        assignedStationId: EXECUTION_STATION_ID,
        leaseToken: "lease-token-abc",
        leaseEpoch: 1,
        leaseExpiresAt,
        currentAttemptId: "att-001",
        startedAt: new Date(),
      },
    });

    const record = buildTestRecord({});
    const artifact = buildTestArtifact();
    const sub = buildRealXhsSubmission("execution", {
      records: [record],
      artifacts: [artifact],
    });
    const bodyText = JSON.stringify(sub.body);
    const executionRequest = {
      stationId: EXECUTION_STATION_ID,
      stationToken: EXECUTION_STATION_TOKEN,
      pluginAuthorizationId: EXECUTION_AUTHORIZATION_ID,
      pluginAuthorizationToken: EXECUTION_AUTHORIZATION_TOKEN,
      method: "POST",
      path: EXECUTION_SYNC_PATH,
      headers: executionSignedHeaders(bodyText),
      bodyText,
    };
    const result = await submitExecutionEvidence(executionRequest, { db: prismaClient });

    expect(result.status).toBe("committed");
    if (result.status === "committed") {
      expect(result.integrityStatus).toBe("verified");

      // Verify artifact was persisted
      const snap = await prismaClient.rawSnapshot.findUnique({
        where: { id: result.rawSnapshotId },
        include: { captureArtifacts: true },
      });
      expect(snap!.captureArtifacts).toHaveLength(1);
      expect(snap!.captureArtifacts[0].artifactChecksum).toBe(TEST_ARTIFACT_CHECKSUM);
      expect(snap!.captureArtifacts[0].restricted).toBe(false);

      // Execution fields populated
      expect(snap!.jobId).toBeTruthy();
      expect(snap!.attemptId).toBeTruthy();
      expect(snap!.stationId).toBeTruthy();
      expect(snap!.ingressKind).toBe("execution");
      expect(snap!.sourcePrincipal).toBe("execution-station:station-001");
      expect(snap!.leaseEpoch).toBe(1);
      expect(snap!.executionPlanVersion).toBe("plan-v1");

      expect(snap!.capturePackageId).toMatch(/^c[a-z0-9]{24}$/);
      expect(snap!.capturePackageId).not.toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
      );
      const packagePayloadHex = psqlQuery(
        DB_NAME,
        `SELECT encode("packagePayload", 'hex') FROM "CapturePackage" WHERE id = '${snap!.capturePackageId}';`,
      );
      expect(Buffer.from(packagePayloadHex, "hex")).toEqual(
        Buffer.from(sub.body.capturePackage.packagePayload, "base64"),
      );

      const [job, queue, attempt, runtime] = await Promise.all([
        prismaClient.executionJob.findUniqueOrThrow({ where: { id: "job-001" } }),
        prismaClient.executionQueueEntry.findUniqueOrThrow({ where: { jobId: "job-001" } }),
        prismaClient.taskAttempt.findUniqueOrThrow({ where: { id: "att-001" } }),
        prismaClient.executionTaskRuntime.findUniqueOrThrow({ where: { jobId: "job-001" } }),
      ]);
      expect(job.status).toBe("raw_committed");
      expect(queue.status).toBe("raw_committed");
      expect(queue.leaseToken).toBeNull();
      expect(attempt.result).toBe("success");
      expect(attempt.endedAt).not.toBeNull();
      expect(runtime.status).toBe("completed");
      expect(runtime.progress).toBe(100);
      expect(runtime.leaseToken).toBeNull();

      const replay = await submitExecutionEvidence(
        { ...executionRequest, headers: executionSignedHeaders(bodyText) },
        { db: prismaClient },
      );
      expect(replay.status).toBe("replay");
      expect(await prismaClient.outboxEvent.count({
        where: { aggregateId: result.rawSnapshotId },
      })).toBe(0);
    }
  });

  it("retains Evidence and recovers control after a real transaction failure past lease expiry", async () => {
    const jobId = "job-control-pending-001";
    const attemptId = "att-control-pending-001";
    const leaseToken = "lease-control-pending-001";
    const captureId = "cap-control-pending-001";
    const leaseExpiresAt = new Date(Date.now() + 5 * 60_000);
    await prismaClient.executionJob.create({
      data: {
        id: jobId,
        workspaceId: "ws-test-001",
        platform: "xhs",
        targetType: "note",
        targetKey: "xhs:note/control-pending",
        jobType: "collect_detail",
        collectionProfile: "note_full",
        requiredFields: {},
        payload: {},
        executionPlanVersion: "plan-control-pending-v1",
        lane: "manual_hot",
        status: "in_progress",
      },
    });
    await prismaClient.executionQueueEntry.create({
      data: {
        jobId,
        workspaceId: "ws-test-001",
        platform: "xhs",
        source: "manual",
        lane: "manual_hot",
        status: "in_progress",
        reservedByStationId: EXECUTION_STATION_ID,
        leaseToken,
        leaseEpoch: 1,
        leaseExpiresAt,
        lastAttemptId: attemptId,
      },
    });
    await prismaClient.taskAttempt.create({
      data: {
        id: attemptId,
        jobId,
        stationId: EXECUTION_STATION_ID,
        leaseToken,
        leaseEpoch: 1,
      },
    });
    await prismaClient.executionTaskRuntime.create({
      data: {
        jobId,
        workspaceId: "ws-test-001",
        status: "running",
        activeExecutor: EXECUTION_STATION_ID,
        assignedStationId: EXECUTION_STATION_ID,
        leaseToken,
        leaseEpoch: 1,
        leaseExpiresAt,
        currentAttemptId: attemptId,
        startedAt: new Date(),
      },
    });

    const sub = buildRealXhsSubmission("execution", {
      records: [buildTestRecord({ idempotencyKey: "control-pending-record" })],
      artifacts: [buildTestArtifact()],
      headerOverrides: {
        captureId,
        jobId,
        attemptId,
        leaseEpoch: 1,
        executionPlanVersion: "plan-control-pending-v1",
      } as never,
    });
    const bodyText = JSON.stringify(sub.body);
    const request = {
      stationId: EXECUTION_STATION_ID,
      stationToken: EXECUTION_STATION_TOKEN,
      pluginAuthorizationId: EXECUTION_AUTHORIZATION_ID,
      pluginAuthorizationToken: EXECUTION_AUTHORIZATION_TOKEN,
      method: "POST",
      path: EXECUTION_SYNC_PATH,
      headers: executionSignedHeaders(bodyText),
      bodyText,
    };
    const evidenceBefore = {
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
      outbox: countTable("OutboxEvent"),
    };

    psqlSql(DB_NAME, `
      CREATE OR REPLACE FUNCTION "reject_b1_control_pending_once"()
      RETURNS trigger AS $$
      BEGIN
        IF NEW."jobId" = '${jobId}' AND NEW."status" = 'completed' THEN
          RAISE EXCEPTION 'B1 integration control failure' USING ERRCODE = 'P0001';
        END IF;
        RETURN NEW;
      END;
      $$ LANGUAGE plpgsql;
      CREATE TRIGGER "reject_b1_control_pending_once_trigger"
      BEFORE UPDATE ON "ExecutionTaskRuntime"
      FOR EACH ROW EXECUTE FUNCTION "reject_b1_control_pending_once"();
    `);

    let pendingResult;
    try {
      pendingResult = await submitExecutionEvidence(request, { db: prismaClient });
    } finally {
      psqlSql(DB_NAME, `
        DROP TRIGGER IF EXISTS "reject_b1_control_pending_once_trigger" ON "ExecutionTaskRuntime";
        DROP FUNCTION IF EXISTS "reject_b1_control_pending_once"();
      `);
    }
    expect(pendingResult).toMatchObject({
      status: "evidence_committed_control_pending",
      retryable: true,
    });
    const afterFailure = {
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
      outbox: countTable("OutboxEvent"),
    };
    expect(afterFailure).toEqual({
      packages: evidenceBefore.packages + 1,
      snapshots: evidenceBefore.snapshots + 1,
      records: evidenceBefore.records + 1,
      artifacts: evidenceBefore.artifacts + 1,
      receipts: evidenceBefore.receipts + 1,
      outbox: evidenceBefore.outbox,
    });

    const failedControl = await Promise.all([
      prismaClient.executionJob.findUniqueOrThrow({ where: { id: jobId } }),
      prismaClient.executionQueueEntry.findUniqueOrThrow({ where: { jobId } }),
      prismaClient.taskAttempt.findUniqueOrThrow({ where: { id: attemptId } }),
      prismaClient.executionTaskRuntime.findUniqueOrThrow({ where: { jobId } }),
    ]);
    expect(failedControl[0].status).toBe("in_progress");
    expect(failedControl[1].status).toBe("in_progress");
    expect(failedControl[1].leaseToken).toBe(leaseToken);
    expect(failedControl[2].result).toBeNull();
    expect(failedControl[2].endedAt).toBeNull();
    expect(failedControl[3].status).toBe("running");

    await prismaClient.executionQueueEntry.update({
      where: { jobId },
      data: { leaseExpiresAt: new Date(Date.now() - 60_000) },
    });
    const recovered = await submitExecutionEvidence(
      { ...request, headers: executionSignedHeaders(bodyText) },
      { db: prismaClient },
    );
    expect(recovered.status).toBe("replay");
    expect({
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
      outbox: countTable("OutboxEvent"),
    }).toEqual(afterFailure);

    const recoveredControl = await Promise.all([
      prismaClient.executionJob.findUniqueOrThrow({ where: { id: jobId } }),
      prismaClient.executionQueueEntry.findUniqueOrThrow({ where: { jobId } }),
      prismaClient.taskAttempt.findUniqueOrThrow({ where: { id: attemptId } }),
      prismaClient.executionTaskRuntime.findUniqueOrThrow({ where: { jobId } }),
    ]);
    expect(recoveredControl[0].status).toBe("raw_committed");
    expect(recoveredControl[1].status).toBe("raw_committed");
    expect(recoveredControl[1].leaseToken).toBeNull();
    expect(recoveredControl[2].result).toBe("success");
    expect(recoveredControl[3].status).toBe("completed");
  });

  it("rejects bad token, HMAC and authorization binding before any Evidence write", async () => {
    const sub = buildSubmission("execution", {
      records: [buildTestRecord({ idempotencyKey: "bad-auth-record" })],
      artifacts: [buildTestArtifact()],
      headerOverrides: { captureId: "cap-execution-bad-auth" } as never,
    });
    const bodyText = JSON.stringify(sub.body);
    const before = {
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
    };
    const base = {
      stationId: EXECUTION_STATION_ID,
      stationToken: EXECUTION_STATION_TOKEN,
      pluginAuthorizationId: EXECUTION_AUTHORIZATION_ID,
      pluginAuthorizationToken: EXECUTION_AUTHORIZATION_TOKEN,
      method: "POST",
      path: EXECUTION_SYNC_PATH,
      bodyText,
    };

    await expect(verifyExecutionStationIngressSession({
      ...base,
      stationToken: "wrong-station-token",
      headers: executionSignedHeaders(bodyText),
    }, prismaClient)).rejects.toMatchObject({ code: "STATION_TOKEN_INVALID" });
    await expect(verifyExecutionStationIngressSession({
      ...base,
      headers: executionSignedHeaders(bodyText, "wrong-signing-secret"),
    }, prismaClient)).rejects.toMatchObject({ code: "SIGNATURE_INVALID" });
    await expect(verifyExecutionStationIngressSession({
      ...base,
      pluginAuthorizationToken: "wrong-authorization-token",
      headers: executionSignedHeaders(bodyText),
    }, prismaClient)).rejects.toMatchObject({ code: "PLUGIN_AUTHORIZATION_INVALID" });

    expect({
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
    }).toEqual(before);
  });

  it("rejects a lease that becomes stale after binding with zero Evidence writes", async () => {
    const jobId = "job-stale-001";
    const attemptId = "att-stale-001";
    const sub = buildSubmission("execution", {
      records: [buildTestRecord({ idempotencyKey: "stale-execution-record" })],
      artifacts: [buildTestArtifact()],
      headerOverrides: {
        captureId: "cap-execution-stale",
        jobId,
        attemptId,
        executionPlanVersion: "plan-stale-v1",
      } as never,
    });
    await prismaClient.executionJob.create({
      data: {
        id: jobId,
        workspaceId: "ws-test-001",
        platform: "xhs",
        targetType: "note",
        targetKey: "xhs:note/stale",
        jobType: "collect_detail",
        collectionProfile: "note_full",
        requiredFields: {},
        payload: {},
        executionPlanVersion: "plan-stale-v1",
        lane: "manual_hot",
        status: "in_progress",
      },
    });
    await prismaClient.executionQueueEntry.create({
      data: {
        jobId,
        workspaceId: "ws-test-001",
        platform: "xhs",
        source: "manual",
        lane: "manual_hot",
        status: "in_progress",
        reservedByStationId: EXECUTION_STATION_ID,
        leaseToken: "lease-stale-original",
        leaseEpoch: 1,
        leaseExpiresAt: new Date(Date.now() + 5 * 60_000),
        lastAttemptId: attemptId,
      },
    });
    await prismaClient.taskAttempt.create({
      data: {
        id: attemptId,
        jobId,
        stationId: EXECUTION_STATION_ID,
        leaseToken: "lease-stale-original",
        leaseEpoch: 1,
      },
    });
    const bodyText = JSON.stringify(sub.body);
    const verifiedSession = await verifyExecutionStationIngressSession({
      stationId: EXECUTION_STATION_ID,
      stationToken: EXECUTION_STATION_TOKEN,
      pluginAuthorizationId: EXECUTION_AUTHORIZATION_ID,
      pluginAuthorizationToken: EXECUTION_AUTHORIZATION_TOKEN,
      method: "POST",
      path: EXECUTION_SYNC_PATH,
      headers: executionSignedHeaders(bodyText),
      bodyText,
    }, prismaClient);
    const binding = await buildExecutionIngressBinding(
      bodyText,
      verifiedSession,
      prismaClient,
    );
    const before = {
      packages: countTable("CapturePackage"),
      snapshots: countTable("RawSnapshot"),
      records: countTable("RawRecord"),
      artifacts: countTable("CaptureArtifact"),
      receipts: countTable("EvidenceIngressReceipt"),
    };
    try {
      await prismaClient.executionQueueEntry.update({
        where: { jobId },
        data: { leaseToken: "lease-stale-new" },
      });

      const result = await new EvidenceIngress(
        prismaClient,
        testRegistry(),
        binding.authorityValidator,
      ).submit(binding.submission);
      expect(result).toEqual({
        status: "rejected",
        reason: "execution_authority_invalid",
        retryable: false,
      });
      expect({
        packages: countTable("CapturePackage"),
        snapshots: countTable("RawSnapshot"),
        records: countTable("RawRecord"),
        artifacts: countTable("CaptureArtifact"),
        receipts: countTable("EvidenceIngressReceipt"),
      }).toEqual(before);
    } finally {
      await prismaClient.executionJob.delete({ where: { id: jobId } });
    }
  });

  // ── §3.4: manual_import with artifact ────────────────────────────────────

  it("commits manual_import with artifact, all execution fields null", async () => {
    const record = buildTestRecord({ idempotencyKey: "mi-art-1" });
    const artifact = buildTestArtifact({ kind: "platform_response" });
    const sub = buildRealXhsSubmission("manual_import", {
      records: [record],
      artifacts: [artifact],
      headerOverrides: { captureId: "cap-mi-art" } as never,
    });
    const result = await submitManualImportEvidence({
      body: sub.body,
      context: {
        workspaceId: "ws-test-001",
        userId: "manual-user-001",
        pluginAuthorization: {
          id: "plugin-auth-001",
          workspaceId: "ws-test-001",
        },
      },
    }, { db: prismaClient });

    expect(result.status).toBe("committed");
    const snap = await prismaClient.rawSnapshot.findUnique({
      where: { id: (result as { rawSnapshotId: string }).rawSnapshotId },
      include: { captureArtifacts: true, records: true },
    });
    expect(snap!.captureArtifacts).toHaveLength(1);
    expect(snap!.jobId).toBeNull();
    expect(snap!.attemptId).toBeNull();
    expect(snap!.stationId).toBeNull();
    expect(snap!.leaseEpoch).toBeNull();
    expect(snap!.executionPlanVersion).toBeNull();
    expect(snap!.records.length).toBeGreaterThan(0);
    expect(snap!.records.every((record) => record.jobId === null)).toBe(true);
    expect(snap!.ingressKind).toBe("manual_import");
    expect(snap!.sourceSummary).toBe("manual import test");
    expect(snap!.sourcePrincipal).toBe("user:manual-user-001");
    expect(snap!.importerIdentity).toBe("plugin-authorization:plugin-auth-001");
  });

  it("commits recovery with artifact without fabricated execution fields", async () => {
    const sub = buildRealXhsSubmission("recovery", {
      records: [buildTestRecord({ idempotencyKey: "recovery-art-1" })],
      artifacts: [buildTestArtifact({ kind: "page_snapshot" })],
      headerOverrides: { captureId: "cap-recovery-art" } as never,
    });
    const result = await submitRecoveryEvidence({
      body: sub.body,
      context: {
        workspaceId: "ws-test-001",
        userId: "recovery-user-001",
        role: "owner",
      },
    }, { db: prismaClient });

    expect(result.status).toBe("committed");
    if (result.status !== "committed") throw new Error("Expected recovery commit");
    const snap = await prismaClient.rawSnapshot.findUniqueOrThrow({
      where: { id: result.rawSnapshotId },
      include: { captureArtifacts: true },
    });
    expect(snap.captureArtifacts).toHaveLength(1);
    expect(snap.ingressKind).toBe("recovery");
    expect(snap.recoveryCaptureId).toBe("rec-cap-001");
    expect(snap.sourcePrincipal).toBe("user:recovery-user-001");
    expect(snap.recoveryAuthorizedBy).toBe("user:recovery-user-001");
    expect(snap.jobId).toBeNull();
    expect(snap.attemptId).toBeNull();
    expect(snap.stationId).toBeNull();
    expect(snap.leaseEpoch).toBeNull();
    expect(snap.executionPlanVersion).toBeNull();
  });

  it("commits migration with artifact without creating an execution relation", async () => {
    const sub = buildSubmission("migration", {
      records: [buildTestRecord({ idempotencyKey: "migration-art-1" })],
      artifacts: [buildTestArtifact({ kind: "media_inventory" })],
      headerOverrides: { captureId: "cap-migration-art" } as never,
    });
    const result = await ingress.submit(sub);

    expect(result.status).toBe("committed");
    if (result.status !== "committed") throw new Error("Expected migration commit");
    const snap = await prismaClient.rawSnapshot.findUniqueOrThrow({
      where: { id: result.rawSnapshotId },
      include: { captureArtifacts: true },
    });
    expect(snap.captureArtifacts).toHaveLength(1);
    expect(snap.ingressKind).toBe("migration");
    expect(snap.sourceSummary).toBe("migration source");
    expect(snap.migrationAuthorization).toBe("migration-auth-001");
    expect(snap.jobId).toBeNull();
    expect(snap.attemptId).toBeNull();
    expect(snap.stationId).toBeNull();
    expect(snap.leaseEpoch).toBeNull();
    expect(snap.executionPlanVersion).toBeNull();
  });

  // ── §5.4: Replay with artifact — no new rows ─────────────────────────────

  it("replay with artifact adds zero new rows", async () => {
    const before = {
      cp: countTable("CapturePackage"),
      snap: countTable("RawSnapshot"),
      rec: countTable("RawRecord"),
      art: countTable("CaptureArtifact"),
      receipt: countTable("EvidenceIngressReceipt"),
    };

    const record = buildTestRecord({});
    const artifact = buildTestArtifact();
    const sub = buildSubmission("manual_import", {
      records: [record],
      artifacts: [artifact],
      headerOverrides: { captureId: "cap-replay-art" } as never,
    });

    const first = await ingress.submit(sub);
    expect(first.status).toBe("committed");

    const second = await ingress.submit(sub);
    expect(second.status).toBe("replay");
    if (second.status === "replay") {
      expect(second.rawSnapshotId).toBe((first as { rawSnapshotId: string }).rawSnapshotId);
    }

    const after = {
      cp: countTable("CapturePackage"),
      snap: countTable("RawSnapshot"),
      rec: countTable("RawRecord"),
      art: countTable("CaptureArtifact"),
      receipt: countTable("EvidenceIngressReceipt"),
    };
    expect(after.cp).toBe(before.cp + 1);
    expect(after.snap).toBe(before.snap + 1);
    expect(after.art).toBe(before.art + 1);
    expect(after.receipt).toBe(before.receipt + 1);
  });

  // ── §5.4: Conflict with artifact — both variants stored ─────────────────

  it("conflict with artifact stores both variants", async () => {
    const record1 = buildTestRecord({ payload: { title: "con-art-A" } });
    const art1 = buildTestArtifact();
    const sub1 = buildSubmission("manual_import", {
      records: [record1],
      artifacts: [art1],
      headerOverrides: { captureId: "cap-con-art" } as never,
    });
    const first = await ingress.submit(sub1);
    expect(first.status).toBe("committed");

    const record2 = buildTestRecord({ payload: { title: "con-art-B" } });
    const art2 = buildTestArtifact({ kind: "dom_fragment" });
    const sub2 = buildSubmission("manual_import", {
      records: [record2],
      artifacts: [art2],
      headerOverrides: { captureId: "cap-con-art" } as never,
    });
    const second = await ingress.submit(sub2);
    expect(second.status).toBe("conflict");
  });

  // ── R2-004: Transaction failure injection WITH artifact ──────────────────

  it("rolls back all 5 tables including artifact on transaction failure (R2-004)", async () => {
    psqlSql(DB_NAME, `
      CREATE OR REPLACE FUNCTION _test_cause_receipt_error()
      RETURNS trigger AS $$
      BEGIN
        IF NEW."captureId" = 'cap-rollback-art' THEN
          RAISE EXCEPTION 'test_injected_failure: receipt insert blocked'
            USING ERRCODE = '55000';
        END IF;
        RETURN NEW;
      END;
      $$ LANGUAGE plpgsql;

      CREATE TRIGGER _test_receipt_error_trigger
      BEFORE INSERT ON "EvidenceIngressReceipt"
      FOR EACH ROW EXECUTE FUNCTION _test_cause_receipt_error();
    `);

    const before = {
      cp: countTable("CapturePackage"),
      snap: countTable("RawSnapshot"),
      rec: countTable("RawRecord"),
      art: countTable("CaptureArtifact"),
      receipt: countTable("EvidenceIngressReceipt"),
    };

    const record = buildTestRecord({ idempotencyKey: "fail-art-rec" });
    const artifact = buildTestArtifact();
    const sub = buildSubmission("manual_import", {
      records: [record],
      artifacts: [artifact],
      headerOverrides: { captureId: "cap-rollback-art" } as never,
    });

    try {
      await expect(ingress.submit(sub)).rejects.toThrow(/test_injected_failure/);
    } finally {
      psqlSql(DB_NAME, `
        DROP TRIGGER IF EXISTS _test_receipt_error_trigger ON "EvidenceIngressReceipt";
        DROP FUNCTION IF EXISTS _test_cause_receipt_error();
      `);
    }

    // Zero residual — including artifact
    const after = {
      cp: countTable("CapturePackage"),
      snap: countTable("RawSnapshot"),
      rec: countTable("RawRecord"),
      art: countTable("CaptureArtifact"),
      receipt: countTable("EvidenceIngressReceipt"),
    };
    expect(after.cp).toBe(before.cp);
    expect(after.snap).toBe(before.snap);
    expect(after.rec).toBe(before.rec);
    expect(after.art).toBe(before.art);
    expect(after.receipt).toBe(before.receipt);
  });

  // ── R2-004: Append-only trigger — assert SQLSTATE 55000 ──────────────────

  it("rejects CaptureArtifact UPDATE", async () => {
    // First ensure at least one artifact exists
    const artCount = countTable("CaptureArtifact");
    expect(artCount).toBeGreaterThan(0);
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "CaptureArtifact" ORDER BY "createdAt" LIMIT 1;`);
    expect(testId).toBeTruthy();

    // psql emits errors to stderr; execSync with stdio:'pipe' captures them
    try {
      execSync(
        `psql -h ${PG_HOST} -p ${PG_PORT} -U ${PG_USER} -d "${DB_NAME}" -v ON_ERROR_STOP=1 -c "UPDATE \\\"CaptureArtifact\\\" SET kind = 'tampered' WHERE id = '${testId}';"`,
        { env: PG_ENV, stdio: "pipe", encoding: "utf8" },
      );
      expect(false).toBe(true);
    } catch (err) {
      const stderr = (err as { stderr?: Buffer | string }).stderr;
      const msg = typeof stderr === "string" ? stderr : Buffer.isBuffer(stderr) ? stderr.toString("utf8") : String(err);
      expect(msg).toContain("append-only");
    }
  });

  it("rejects CaptureArtifact DELETE", async () => {
    const artCount = countTable("CaptureArtifact");
    expect(artCount).toBeGreaterThan(0);
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "CaptureArtifact" ORDER BY "createdAt" LIMIT 1;`);
    expect(testId).toBeTruthy();

    try {
      execSync(
        `psql -h ${PG_HOST} -p ${PG_PORT} -U ${PG_USER} -d "${DB_NAME}" -v ON_ERROR_STOP=1 -c "DELETE FROM \\\"CaptureArtifact\\\" WHERE id = '${testId}';"`,
        { env: PG_ENV, stdio: "pipe", encoding: "utf8" },
      );
      expect(false).toBe(true);
    } catch (err) {
      const stderr = (err as { stderr?: Buffer | string }).stderr;
      const msg = typeof stderr === "string" ? stderr : Buffer.isBuffer(stderr) ? stderr.toString("utf8") : String(err);
      expect(msg).toContain("append-only");
    }
  });

  it("rejects CapturePackage UPDATE", async () => {
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "CapturePackage" ORDER BY "createdAt" LIMIT 1;`);
    expect(testId).toBeTruthy();
    try {
      execSync(
        `psql -h ${PG_HOST} -p ${PG_PORT} -U ${PG_USER} -d "${DB_NAME}" -v ON_ERROR_STOP=1 -c "UPDATE \\\"CapturePackage\\\" SET \\\"checksumValue\\\" = 'tampered' WHERE id = '${testId}';"`,
        { env: PG_ENV, stdio: "pipe", encoding: "utf8" },
      );
      expect(false).toBe(true);
    } catch (err) {
      const stderr = (err as { stderr?: Buffer | string }).stderr;
      const msg = typeof stderr === "string" ? stderr : Buffer.isBuffer(stderr) ? stderr.toString("utf8") : String(err);
      expect(msg).toContain("append-only");
    }
  });

  it("rejects CapturePackage DELETE with SQLSTATE 55000", () => {
    const before = countTable("CapturePackage");
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "CapturePackage" ORDER BY "createdAt" LIMIT 1;`);
    expect(testId).toBeTruthy();
    expect(psqlFailure(DB_NAME, `DELETE FROM "CapturePackage" WHERE id = '${testId}';`)).toContain("55000");
    expect(countTable("CapturePackage")).toBe(before);
  });

  it("surfaces SQLSTATE 55000 for CaptureArtifact mutation", () => {
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "CaptureArtifact" ORDER BY "createdAt" LIMIT 1;`);
    expect(testId).toBeTruthy();
    expect(psqlFailure(
      DB_NAME,
      `UPDATE "CaptureArtifact" SET kind = 'tampered' WHERE id = '${testId}';`,
    )).toContain("55000");
  });

  it("rejects EvidenceIngressReceipt UPDATE and DELETE with SQLSTATE 55000", () => {
    const before = countTable("EvidenceIngressReceipt");
    const testId = psqlQuery(DB_NAME, `SELECT id FROM "EvidenceIngressReceipt" ORDER BY "receivedAt" LIMIT 1;`);
    expect(testId).toBeTruthy();
    expect(psqlFailure(
      DB_NAME,
      `UPDATE "EvidenceIngressReceipt" SET "collectorVersion" = 'tampered' WHERE id = '${testId}';`,
    )).toContain("55000");
    expect(psqlFailure(DB_NAME, `DELETE FROM "EvidenceIngressReceipt" WHERE id = '${testId}';`)).toContain("55000");
    expect(countTable("EvidenceIngressReceipt")).toBe(before);
  });

  // ── R2-001: Cross-workspace preflight rejection ──────────────────────────

  it("compound FK rejects a new cross-workspace RawRecord after migration", async () => {
    // Insert a RawSnapshot and then attempt to insert a RawRecord with
    // a different workspaceId. The compound FK (workspaceId, rawSnapshotId)
    // should reject this.
    const snapId = `snap-cross-test-${Date.now()}`;
    await prismaClient.rawSnapshot.create({
      data: {
        id: snapId,
        workspaceId: "ws-cross-test",
        captureId: `cap-cross-${Date.now()}`,
        platform: "xhs",
        targetKey: "xhs:note/cross",
        observedAt: new Date(),
      },
    });

    // Attempt cross-workspace insert — must be rejected by compound FK
    try {
      psqlSql(DB_NAME, `
        INSERT INTO "RawRecord" ("id", "workspaceId", "rawSnapshotId", "platform", "payload", "observedAt", "idempotencyKey")
        VALUES ('rec-cross-ws', 'wrong-workspace', '${snapId}', 'xhs', '{}'::jsonb, now(), 'cross-key')
      `);
      expect(false).toBe(true);
    } catch (err) {
      const msg = err instanceof Error ? (err as Error & { stderr?: string }).stderr ?? err.message : String(err);
      // FK violation (23503) or the specific compound constraint error
      expect(msg).toMatch(/23503|violates foreign key|is not present/);
    }
  });

  // ── Concurrent proof (R2-004: must run and report results) ───────────────

  it("concurrent same hash → committed + replay", async () => {
    const ingress2 = new EvidenceIngress(
      prismaClient2,
      testRegistry(),
      new AlwaysValidAuthorityValidator(),
    );

    const record = buildTestRecord({});
    const artifact = buildTestArtifact();
    const sub = buildSubmission("manual_import", {
      records: [record],
      artifacts: [artifact],
      headerOverrides: { captureId: "cap-cc-same" } as never,
    });

    const [r1, r2] = await Promise.all([
      ingress.submit(sub),
      ingress2.submit(sub),
    ]);

    const statuses = [r1.status, r2.status].sort();
    expect(statuses).toContain("committed");
    expect(statuses).toContain("replay");
    console.log(`[integration] Concurrent same-hash: ${r1.status} / ${r2.status}`);
  });

  it("concurrent different hash → verified + conflict", async () => {
    const ingress2 = new EvidenceIngress(
      prismaClient2,
      testRegistry(),
      new AlwaysValidAuthorityValidator(),
    );

    const rec1 = buildTestRecord({ payload: { title: "cc-A" } });
    const art1 = buildTestArtifact();
    const sub1 = buildSubmission("manual_import", {
      records: [rec1], artifacts: [art1],
      headerOverrides: { captureId: "cap-cc-diff" } as never,
    });

    const rec2 = buildTestRecord({ payload: { title: "cc-B" } });
    const art2 = buildTestArtifact({ kind: "dom_fragment" });
    const sub2 = buildSubmission("manual_import", {
      records: [rec2], artifacts: [art2],
      headerOverrides: { captureId: "cap-cc-diff" } as never,
    });

    const [r1, r2] = await Promise.all([
      ingress.submit(sub1),
      ingress2.submit(sub2),
    ]);

    const statuses = [r1.status, r2.status].sort();
    expect(statuses).toContain("committed");
    expect(statuses).toContain("conflict");
    console.log(`[integration] Concurrent diff-hash: ${r1.status} / ${r2.status}`);
  });

  // ── No derived data ──────────────────────────────────────────────────────

  it("creates zero OutboxEvent rows", () => {
    expect(countTable("OutboxEvent")).toBe(0);
  });

  // ── RawSnapshot/RawRecord: no append-only triggers ───────────────────────

  it("RawSnapshot and RawRecord have zero triggers (V1 coexistence)", () => {
    expect(parseInt(psqlQuery(DB_NAME,
      `SELECT count(*) FROM information_schema.triggers WHERE event_object_table = 'RawSnapshot';`
    ), 10)).toBe(0);
    expect(parseInt(psqlQuery(DB_NAME,
      `SELECT count(*) FROM information_schema.triggers WHERE event_object_table = 'RawRecord';`
    ), 10)).toBe(0);
  });

  // ── V1 coexistence ───────────────────────────────────────────────────────

  it("V1 execution write coexists with V2 Evidence", async () => {
    const beforeSnap = countTable("RawSnapshot");
    const beforeRec = countTable("RawRecord");

    const ts = Date.now();
    await prismaClient.rawSnapshot.create({
      data: {
        id: `v1-co-${ts}`,
        workspaceId: "ws-v1-co",
        jobId: "v1-job",
        attemptId: "v1-att",
        captureId: `v1-cap-${ts}`,
        platform: "xhs",
        targetKey: "xhs:note/v1-co",
        observedAt: new Date(),
        qualityStatus: "ok",
        source: "plugin_sync",
        schemaVersion: "v3",
        records: {
          create: [{
            id: `v1-rec-${ts}`,
            jobId: "v1-job",
            recordType: "note",
            platform: "xhs",
            payload: { v1: true },
            observedAt: new Date(),
            idempotencyKey: `v1-idem-${ts}`,
          }],
        },
      },
    });

    expect(countTable("RawSnapshot")).toBe(beforeSnap + 1);
    expect(countTable("RawRecord")).toBe(beforeRec + 1);
  });

  // ── Final report ─────────────────────────────────────────────────────────

  it("reports exact database name and final counts", () => {
    const counts = {
      database: DB_NAME,
      CapturePackage: countTable("CapturePackage"),
      RawSnapshot: countTable("RawSnapshot"),
      RawRecord: countTable("RawRecord"),
      CaptureArtifact: countTable("CaptureArtifact"),
      EvidenceIngressReceipt: countTable("EvidenceIngressReceipt"),
      OutboxEvent: countTable("OutboxEvent"),
    };
    console.log(JSON.stringify(counts, null, 2));

    expect(counts.CapturePackage).toBeGreaterThan(0);
    expect(counts.RawSnapshot).toBeGreaterThan(0);
    expect(counts.CaptureArtifact).toBeGreaterThan(0);
    expect(counts.EvidenceIngressReceipt).toBeGreaterThan(0);
    expect(counts.OutboxEvent).toBe(0);
  });
});
