// @vitest-environment node

/**
 * B2-B-02 dark service proof. Opt-in only; creates and preserves one fresh
 * isolated database. It never targets content_workbench_local.
 *
 * Run:
 *   V2_B2_SERVICE_INTEGRATION_DB=1 npx vitest run src/lib/evidence/derived/b2-derived-service.integration.test.ts
 */

import { execFileSync } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createPrismaClient } from "@/lib/db";
import { Prisma, type PrismaClient } from "@/lib/prisma-client";

import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_LIST_SCAN } from "../contracts/xhs-collection-contracts";
import { sha256Hex } from "../ingress/base64";
import { canonicalJsonBytes, type JsonValue } from "../ingress/canonical-json";
import type { CapturePackagePayloadV2 } from "../ingress/types";
import {
  B2DerivedService,
  B2SourceInvariantError,
  createEvidenceAnalysisReader,
  type EvidenceAnalysisReader,
} from "./b2-derived-service";

const PG_HOST = "127.0.0.1";
const PG_PORT = "54329";
const PG_USER = "postgres";
const PG_PASSWORD = "postgres";
const PG_ENV = { ...process.env, PGPASSWORD: PG_PASSWORD };
const BASE_SCHEMA_FIXED_POINT = "d482109d35e36e769a754f17a785f0b39cc868d6";
const PROOF_DB_PREFIX = "content_workbench_v2_b2_service_";
const FIXED_SCHEMA_PATH = join(tmpdir(), `v2-b2-service-fixed-schema-${Date.now()}.prisma`);
const MIGRATION_PATH = "prisma/migrations/20260810193000_add_v2_derived_schema/migration.sql";
const PRISMA_CLI_PATH = process.env.V2_B2_PRISMA_CLI ?? "node_modules/prisma/build/index.js";
const OPTED_IN = process.env.V2_B2_SERVICE_INTEGRATION_DB === "1";

let proofDatabase = "";
let client: PrismaClient;
let auditSequence = 0;

function databaseUrl(database: string): string {
  return `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${database}?schema=public`;
}

function psql(database: string, sql: string): string {
  return execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-t", "-A", "-v", "ON_ERROR_STOP=1", "-c", sql],
    { env: PG_ENV, encoding: "utf8" },
  ).trim();
}

function evidenceReader(readClient: PrismaClient = client): EvidenceAnalysisReader {
  return createEvidenceAnalysisReader({
    async readCapturePackageAndAudit(input) {
      const snapshot = await readClient.rawSnapshot.findFirstOrThrow({
        where: { workspaceId: input.workspaceId, id: input.rawSnapshotId },
        select: {
          integrityStatus: true,
          capturePackage: {
            select: {
              packagePayload: true,
              checksumAlgorithm: true,
              checksumValue: true,
              contentLength: true,
            },
          },
        },
      });
      if (!snapshot.capturePackage) throw new Error("test fixture has no package");
      if (!snapshot.integrityStatus) throw new Error("test fixture has no integrity status");
      const accessAuditId = `audit:${input.rawSnapshotId}:${auditSequence += 1}`;
      await readClient.$executeRaw`
        INSERT INTO "B2ReadAuditProof" ("id", "rawSnapshotId")
        VALUES (${accessAuditId}, ${input.rawSnapshotId})
      `;
      return {
        ...input,
        accessAuditId,
        lifecycleStatus: "ACTIVE",
        integrityStatus: snapshot.integrityStatus,
        packageBytes: Uint8Array.from(snapshot.capturePackage.packagePayload),
        packageChecksumAlgorithm: snapshot.capturePackage.checksumAlgorithm,
        packageChecksumValue: snapshot.capturePackage.checksumValue,
        packageContentLength: snapshot.capturePackage.contentLength,
      };
    },
  });
}

async function seedListScanSnapshot(
  suffix: string,
  payload: JsonValue = {
    noteId: `note-${suffix}`,
    title: "title",
    content: "body",
    url: `https://xhs.test/${suffix}`,
  },
  options: {
    packageSequence?: number;
    databaseSequence?: number;
    snapshotContractHash?: string;
    recordKind?: "note" | "metric";
    terminal?: CapturePackagePayloadV2["header"]["report"]["terminal"];
  } = {},
): Promise<{ workspaceId: string; rawSnapshotId: string }> {
  const workspaceId = `workspace-${suffix}`;
  const rawSnapshotId = `snapshot-${suffix}`;
  const observedAtText = "2026-08-05T11:58:00.000Z";
  const record = {
    idempotencyKey: `record-key-${suffix}`,
    recordKind: options.recordKind ?? "note",
    platform: "xhs" as const,
    targetKey: `xhs:note:${suffix}`,
    externalRecordId: `note-${suffix}`,
    sequence: options.packageSequence ?? 0,
    payload,
    observedAt: observedAtText,
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header: {
      protocolVersion: "capture-submission/v2",
      ingressKind: "manual_import",
      captureId: `capture-${suffix}`,
      platform: "xhs",
      target: {
        expectedTargetKey: `xhs:note:${suffix}`,
        observedTargetKey: `xhs:note:${suffix}`,
      },
      observedAt: observedAtText,
      collectorVersion: "integration-1.0.0",
      contractId: XHS_LIST_SCAN.id,
      contractVersion: XHS_LIST_SCAN.version,
      contractHash: computeContractHash(XHS_LIST_SCAN),
      sourceSummary: "isolated B2 proof",
      report: {
        startedAt: observedAtText,
        completedAt: observedAtText,
        terminal: options.terminal ?? { state: "completed", reason: "limit_reached", retryable: false },
        slots: [{ slotId: "note_list", status: "observed", reason: null }],
        counters: { requested: 1, discovered: 1, emitted: 1, deduplicated: 0, failed: 0 },
        diagnostics: {},
      },
    },
    records: [record],
    artifacts: [],
  };
  const packageBytes = canonicalJsonBytes(capturePackage);
  const packageHash = sha256Hex(packageBytes);
  const packageRow = await client.capturePackage.create({
    data: {
      workspaceId,
      packagePayload: Uint8Array.from(packageBytes),
      checksumAlgorithm: "sha256",
      checksumValue: packageHash,
      contentLength: packageBytes.length,
      restricted: false,
    },
  });
  await client.rawSnapshot.create({
    data: {
      id: rawSnapshotId,
      workspaceId,
      captureId: capturePackage.header.captureId,
      platform: "xhs",
      targetKey: capturePackage.header.target.expectedTargetKey,
      observedAt: new Date(observedAtText),
      capturePackageId: packageRow.id,
      checksumAlgorithm: "sha256",
      checksumValue: packageHash,
      contentLength: packageBytes.length,
      integrityStatus: "verified",
      contractId: XHS_LIST_SCAN.id,
      contractVersion: XHS_LIST_SCAN.version,
      contractHash: options.snapshotContractHash ?? computeContractHash(XHS_LIST_SCAN),
    },
  });
  await client.rawRecord.create({
    data: {
      workspaceId,
      rawSnapshotId,
      recordKind: record.recordKind,
      platform: "xhs",
      targetKey: record.targetKey,
      externalRecordId: record.externalRecordId,
      sequence: options.databaseSequence ?? record.sequence,
      payload: payload as Prisma.InputJsonValue,
      payloadHash: sha256Hex(canonicalJsonBytes(payload)),
      observedAt: new Date(observedAtText),
      idempotencyKey: record.idempotencyKey,
    },
  });
  return { workspaceId, rawSnapshotId };
}

async function counts(rawSnapshotId: string): Promise<number[]> {
  const [runs, currents, observations, evaluations, evaluationCurrents, edges, inputs] = await Promise.all([
    client.normalizationRun.count({ where: { rawSnapshotId } }),
    client.normalizationRunCurrent.count({ where: { rawSnapshotId } }),
    client.canonicalObservation.count({ where: { rawSnapshotId } }),
    client.contractEvaluation.count({ where: { rawSnapshotId } }),
    client.contractEvaluationCurrent.count({ where: { rawSnapshotId } }),
    client.contractEvaluationNormalizationRun.count({ where: { rawSnapshotId } }),
    client.contractEvaluationInput.count({ where: { rawSnapshotId } }),
  ]);
  return [runs, currents, observations, evaluations, evaluationCurrents, edges, inputs];
}

beforeAll(async () => {
  if (!OPTED_IN) return;
  proofDatabase = `${PROOF_DB_PREFIX}${Date.now()}`;
  writeFileSync(
    FIXED_SCHEMA_PATH,
    execFileSync("git", ["show", `${BASE_SCHEMA_FIXED_POINT}:prisma/schema.prisma`], { encoding: "utf8" }),
  );
  psql("postgres", `CREATE DATABASE "${proofDatabase}";`);
  expect(psql(proofDatabase, "SELECT current_database();")).toBe(proofDatabase);
  execFileSync(
    process.execPath,
    [PRISMA_CLI_PATH, "db", "push", "--url", databaseUrl(proofDatabase), "--schema", FIXED_SCHEMA_PATH],
    { cwd: process.cwd(), stdio: "pipe", timeout: 120_000 },
  );
  execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", proofDatabase, "-v", "ON_ERROR_STOP=1", "-f", MIGRATION_PATH],
    { env: PG_ENV, stdio: "pipe" },
  );
  client = createPrismaClient(databaseUrl(proofDatabase));
  psql(proofDatabase, `
    CREATE TABLE "B2ReadAuditProof" (
      "id" text PRIMARY KEY,
      "rawSnapshotId" text NOT NULL
    );
  `);
}, 180_000);

afterAll(async () => {
  if (!OPTED_IN) return;
  await client.$disconnect();
  if (existsSync(FIXED_SCHEMA_PATH)) unlinkSync(FIXED_SCHEMA_PATH);
  console.log(`[b2-service] Preserving proof database ${proofDatabase}.`);
  console.log(`[b2-service] Manual cleanup: dropdb --if-exists --host=${PG_HOST} --port=${PG_PORT} --username=${PG_USER} ${proofDatabase}`);
});

const describeIntegration = OPTED_IN ? describe : describe.skip;

describeIntegration("B2-B-02 isolated service", () => {
  it("atomically commits, replays, and explicitly retries an accepted chain", async () => {
    const identity = await seedListScanSnapshot("accepted");
    const service = new B2DerivedService(client, evidenceReader());

    const committed = await service.derive({ ...identity, retryMode: "replay" });
    expect(committed).toMatchObject({
      status: "committed",
      decision: "accepted",
      completeness: "full",
      revision: 1,
    });
    expect(await counts(identity.rawSnapshotId)).toEqual([1, 1, 1, 1, 1, 1, 1]);

    const replay = await service.derive({ ...identity, retryMode: "replay" });
    expect(replay).toMatchObject({
      status: "replay",
      contractEvaluationId: committed.status === "committed" ? committed.contractEvaluationId : "never",
      revision: 1,
    });
    expect(await counts(identity.rawSnapshotId)).toEqual([1, 1, 1, 1, 1, 1, 1]);

    const retried = await service.derive({ ...identity, retryMode: "explicit_retry" });
    expect(retried).toMatchObject({ status: "committed", decision: "accepted", revision: 2 });
    expect(await counts(identity.rawSnapshotId)).toEqual([2, 1, 2, 2, 1, 2, 2]);
  });

  it("rolls back every Derived row when a late infrastructure write fails", async () => {
    const identity = await seedListScanSnapshot("rollback");
    const injected = new Error("injected evaluation write failure");
    const failingDb = {
      $transaction: (callback: (tx: Prisma.TransactionClient) => Promise<unknown>, options: unknown) =>
        client.$transaction(async (tx) => callback(new Proxy(tx, {
          get(target, property, receiver) {
            if (property !== "contractEvaluation") return Reflect.get(target, property, receiver);
            return new Proxy(target.contractEvaluation, {
              get(delegate, delegateProperty, delegateReceiver) {
                if (delegateProperty === "create") return async () => { throw injected; };
                const value = Reflect.get(delegate, delegateProperty, delegateReceiver);
                return typeof value === "function" ? value.bind(delegate) : value;
              },
            });
          },
        })), options as never),
    };
    const service = new B2DerivedService(failingDb as never, evidenceReader());

    await expect(service.derive({ ...identity, retryMode: "replay" })).rejects.toBe(injected);
    expect(await counts(identity.rawSnapshotId)).toEqual([0, 0, 0, 0, 0, 0, 0]);
    expect(Number(psql(proofDatabase, `
      SELECT count(*) FROM "B2ReadAuditProof"
      WHERE "rawSnapshotId" = '${identity.rawSnapshotId}';
    `))).toBe(1);
  });

  it("rejects package/record and contract-hash drift before any Derived write", async () => {
    const sequenceDrift = await seedListScanSnapshot(
      "sequence-drift",
      undefined,
      { packageSequence: 0, databaseSequence: 1 },
    );
    const hashDrift = await seedListScanSnapshot(
      "contract-hash-drift",
      undefined,
      { snapshotContractHash: "f".repeat(64) },
    );
    const service = new B2DerivedService(client, evidenceReader());

    await expect(service.derive({ ...sequenceDrift, retryMode: "replay" }))
      .rejects.toBeInstanceOf(B2SourceInvariantError);
    await expect(service.derive({ ...hashDrift, retryMode: "replay" }))
      .rejects.toBeInstanceOf(B2SourceInvariantError);
    expect(await counts(sequenceDrift.rawSnapshotId)).toEqual([0, 0, 0, 0, 0, 0, 0]);
    expect(await counts(hashDrift.rawSnapshotId)).toEqual([0, 0, 0, 0, 0, 0, 0]);
  });

  it("keeps only the latest rejected attempt in each evaluation", async () => {
    const identity = await seedListScanSnapshot("rejected", { title: "missing identity" });
    const service = new B2DerivedService(client, evidenceReader());

    const first = await service.derive({ ...identity, retryMode: "replay" });
    expect(first).toMatchObject({
      status: "committed",
      decision: "rejected",
      completeness: "not_applicable",
      revision: 1,
    });
    expect(await counts(identity.rawSnapshotId)).toEqual([1, 0, 0, 1, 1, 1, 0]);

    const second = await service.derive({ ...identity, retryMode: "explicit_retry" });
    expect(second).toMatchObject({ status: "committed", decision: "rejected", revision: 2 });
    expect(await counts(identity.rawSnapshotId)).toEqual([2, 0, 0, 2, 1, 2, 0]);
    const latestEvaluation = await client.contractEvaluationCurrent.findFirstOrThrow({
      where: { rawSnapshotId: identity.rawSnapshotId },
    });
    const linked = await client.contractEvaluationNormalizationRun.findMany({
      where: { evaluationId: latestEvaluation.contractEvaluationId },
      include: { normalizationRun: true },
    });
    expect(linked).toHaveLength(1);
    expect(linked[0].normalizationRun.attemptNumber).toBe(2);
  });

  it("persists an unsupported record kind as rejected without a Current or Observation", async () => {
    const identity = await seedListScanSnapshot(
      "unsupported-kind",
      { viewCount: 10 },
      { recordKind: "metric" },
    );
    const result = await new B2DerivedService(client, evidenceReader()).derive({
      ...identity,
      retryMode: "replay",
    });

    expect(result).toMatchObject({
      status: "committed",
      decision: "rejected",
      completeness: "not_applicable",
    });
    expect(await counts(identity.rawSnapshotId)).toEqual([1, 0, 0, 1, 1, 1, 0]);
    const run = await client.normalizationRun.findFirstOrThrow({
      where: { rawSnapshotId: identity.rawSnapshotId },
    });
    expect(run).toMatchObject({
      adapterId: "xhs.unsupported",
      status: "rejected",
      parseErrors: [{ path: "recordKind", code: "record_kind_unsupported" }],
    });
  });

  it("records normalized_current_missing, then accepts an explicit retry", async () => {
    const identity = await seedListScanSnapshot("orphan-normalized");
    const record = await client.rawRecord.findFirstOrThrow({ where: { rawSnapshotId: identity.rawSnapshotId } });
    await client.normalizationRun.create({
      data: {
        workspaceId: identity.workspaceId,
        rawSnapshotId: identity.rawSnapshotId,
        rawRecordId: record.id,
        adapterId: "xhs.note",
        adapterVersion: "1.0.0",
        canonicalSchemaVersion: "xhs.canonical/1",
        attemptNumber: 1,
        status: "normalized",
        inputPayloadHash: record.payloadHash!,
        outputPayloadHash: "b".repeat(64),
      },
    });
    const service = new B2DerivedService(client, evidenceReader());

    const rejected = await service.derive({ ...identity, retryMode: "replay" });
    expect(rejected).toMatchObject({ status: "committed", decision: "rejected", revision: 1 });
    expect(await counts(identity.rawSnapshotId)).toEqual([1, 0, 0, 1, 1, 1, 0]);

    const accepted = await service.derive({ ...identity, retryMode: "explicit_retry" });
    expect(accepted).toMatchObject({ status: "committed", decision: "accepted", revision: 2 });
    expect(await counts(identity.rawSnapshotId)).toEqual([2, 1, 1, 2, 1, 2, 1]);
  });

  it("allows a rejected evaluation to replace an earlier accepted Current", async () => {
    const identity = await seedListScanSnapshot("revocation");
    const service = new B2DerivedService(client, evidenceReader());
    const accepted = await service.derive({ ...identity, retryMode: "replay" });
    expect(accepted).toMatchObject({ status: "committed", decision: "accepted", revision: 1 });
    const record = await client.rawRecord.findFirstOrThrow({ where: { rawSnapshotId: identity.rawSnapshotId } });
    const oldRun = await client.normalizationRun.create({
      data: {
        workspaceId: identity.workspaceId,
        rawSnapshotId: identity.rawSnapshotId,
        rawRecordId: record.id,
        adapterId: "xhs.note",
        adapterVersion: "0.9.0",
        canonicalSchemaVersion: "xhs.canonical/0",
        attemptNumber: 1,
        status: "normalized",
        inputPayloadHash: record.payloadHash!,
        outputPayloadHash: "c".repeat(64),
      },
    });
    await client.normalizationRunCurrent.update({
      where: {
        workspaceId_rawRecordId: {
          workspaceId: identity.workspaceId,
          rawRecordId: record.id,
        },
      },
      data: {
        normalizationRunId: oldRun.id,
        adapterId: oldRun.adapterId,
        adapterVersion: oldRun.adapterVersion,
        canonicalSchemaVersion: oldRun.canonicalSchemaVersion,
      },
    });

    const rejected = await service.derive({ ...identity, retryMode: "replay" });
    expect(rejected).toMatchObject({ status: "committed", decision: "rejected", revision: 2 });
    const current = await client.contractEvaluationCurrent.findFirstOrThrow({
      where: { rawSnapshotId: identity.rawSnapshotId },
      include: { contractEvaluation: true },
    });
    expect(current.contractEvaluation.decision).toBe("rejected");
    expect(current.contractEvaluation.rejectionCode).toBe("schema_version_mismatch");
  });

  it("does not replay an accepted evaluation after observation binding drift", async () => {
    const identity = await seedListScanSnapshot("observation-binding-drift");
    const service = new B2DerivedService(client, evidenceReader());
    await expect(service.derive({ ...identity, retryMode: "replay" }))
      .resolves.toMatchObject({ status: "committed", decision: "accepted" });
    const record = await client.rawRecord.findFirstOrThrow({
      where: { rawSnapshotId: identity.rawSnapshotId },
    });
    const existing = await client.normalizationRun.findFirstOrThrow({
      where: { rawRecordId: record.id },
    });
    const driftedRun = await client.normalizationRun.create({
      data: {
        workspaceId: identity.workspaceId,
        rawSnapshotId: identity.rawSnapshotId,
        rawRecordId: record.id,
        adapterId: existing.adapterId,
        adapterVersion: existing.adapterVersion,
        canonicalSchemaVersion: existing.canonicalSchemaVersion,
        attemptNumber: 2,
        status: "normalized",
        inputPayloadHash: existing.inputPayloadHash,
        outputPayloadHash: existing.outputPayloadHash,
      },
    });
    const driftedHash = "d".repeat(64);
    await client.canonicalObservation.create({
      data: {
        workspaceId: identity.workspaceId,
        normalizationRunId: driftedRun.id,
        rawSnapshotId: identity.rawSnapshotId,
        rawRecordId: record.id,
        observationKind: "note",
        subjectKey: "xhs:note:drifted",
        observedAt: record.observedAt,
        schemaVersion: driftedRun.canonicalSchemaVersion,
        payload: { drifted: true },
        payloadHash: driftedHash,
        qualityStatus: "complete",
        fieldPresence: { noteIdentity: true },
      },
    });
    await client.normalizationRunCurrent.update({
      where: {
        workspaceId_rawRecordId: {
          workspaceId: identity.workspaceId,
          rawRecordId: record.id,
        },
      },
      data: { normalizationRunId: driftedRun.id },
    });

    await expect(service.derive({ ...identity, retryMode: "replay" }))
      .rejects.toThrow("Evaluation member/observation binding changed");
    expect(await client.contractEvaluation.count({
      where: { rawSnapshotId: identity.rawSnapshotId },
    })).toBe(1);
  });

  it("converges concurrent identical submissions to committed plus replay", async () => {
    const identity = await seedListScanSnapshot("concurrent");
    const secondClient = createPrismaClient(databaseUrl(proofDatabase));
    try {
      const [left, right] = await Promise.all([
        new B2DerivedService(client, evidenceReader(client)).derive({ ...identity, retryMode: "replay" }),
        new B2DerivedService(secondClient, evidenceReader(secondClient)).derive({ ...identity, retryMode: "replay" }),
      ]);
      expect([left.status, right.status].sort()).toEqual(["committed", "replay"]);
      expect(await counts(identity.rawSnapshotId)).toEqual([1, 1, 1, 1, 1, 1, 1]);
    } finally {
      await secondClient.$disconnect();
    }
  });
});
