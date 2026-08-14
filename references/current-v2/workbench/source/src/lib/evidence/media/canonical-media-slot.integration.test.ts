// @vitest-environment node

/**
 * B3 CanonicalMediaSlot isolated-database proof.
 *
 * Opt-in only. A fresh database is asserted before the fixed-point schema and
 * the complete candidate migration are applied. The proof database is kept.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createPrismaClient } from "@/lib/db";
import type { PrismaClient } from "@/lib/prisma-client";

import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";
import { canonicalJsonString } from "../ingress/canonical-json";
import type { CapturePackagePayloadV2 } from "../ingress/types";
import { CanonicalMediaSlotWriter } from "./canonical-media-slot-writer";
import {
  type EvidenceArtifactAuditReceipt,
  type EvidenceArtifactAuditSource,
  MediaArtifactInvariantError,
  VerifiedArtifactReader,
  VerifiedMediaArtifacts,
} from "./verified-artifact-reader";

const PG_HOST = "127.0.0.1";
const PG_PORT = "54329";
const PG_USER = "postgres";
const PG_PASSWORD = "postgres";
const PG_ENV = { ...process.env, PGPASSWORD: PG_PASSWORD };
const FIXED_POINT = "38721ece5de80c781a4e059d67beda1388af64a6";
const PROOF_DB_PREFIX = "content_workbench_b3_media_slot_proof_";
const PROOF_DB = `${PROOF_DB_PREFIX}${Date.now()}`;
const PROOF_DB_URL = `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${PROOF_DB}?schema=public`;
const FIXED_SCHEMA_PATH = join(tmpdir(), `b3-media-fixed-schema-${Date.now()}.prisma`);
const MIGRATION_PATH = "prisma/migrations/20260811120000_add_canonical_media_slot/migration.sql";
const PRISMA_CLI_PATH = process.env.B3_MEDIA_PRISMA_CLI ?? "node_modules/prisma/build/index.js";
const OPTED_IN = process.env.B3_MEDIA_INTEGRATION_DB === "1";

let db: ReturnType<typeof createPrismaClient>;
let db2: ReturnType<typeof createPrismaClient>;
let base: SeededFixture;

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

function psqlFile(database: string, path: string): void {
  execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=verbose", "-f", path],
    { env: PG_ENV, encoding: "utf8" },
  );
}

function psqlFailure(database: string, sql: string): string {
  try {
    psqlSql(database, sql);
  } catch (error) {
    if (!(error instanceof Error)) return String(error);
    const processError = error as Error & { stderr?: Buffer | string; stdout?: Buffer | string };
    const stderr = Buffer.isBuffer(processError.stderr) ? processError.stderr.toString("utf8") : processError.stderr ?? "";
    const stdout = Buffer.isBuffer(processError.stdout) ? processError.stdout.toString("utf8") : processError.stdout ?? "";
    return `${stderr}\n${stdout}\n${error.message}`;
  }
  throw new Error("Expected SQL statement to fail.");
}

function assertProofDatabase(): void {
  if (!PROOF_DB.startsWith(PROOF_DB_PREFIX) || PROOF_DB === "content_workbench_local") {
    throw new Error(`Unsafe B3 proof database name: ${PROOF_DB}`);
  }
  expect(psqlQuery(PROOF_DB, "SELECT current_database();")).toBe(PROOF_DB);
}

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

type MediaCandidate = {
  subject: { kind: "note"; noteId: string; platformContentId: string };
  slotId: string;
  purpose: string;
  kind: string;
  ordinal?: number;
  observedAddress: string;
  coverProvenance: string;
};

function candidate(
  noteId: string,
  ordinal = 0,
  overrides: Partial<MediaCandidate> = {},
): MediaCandidate {
  const purpose = overrides.purpose ?? (ordinal === 0 ? "cover" : "body");
  const kind = overrides.kind ?? "image";
  return {
    subject: { kind: "note", noteId, platformContentId: noteId },
    slotId: `note:${noteId}:${purpose}:${kind}:${ordinal}`,
    purpose,
    kind,
    ordinal,
    observedAddress: `https://example.invalid/${noteId}/${ordinal}.jpg`,
    coverProvenance: purpose === "cover" ? "platform_explicit" : "not_cover",
    ...overrides,
  };
}

type FixtureOptions = {
  candidates?: Array<Record<string, unknown>>;
  packagedArtifactChecksum?: string;
  descriptorChecksum?: string;
  lifecycleStatus?: EvidenceArtifactAuditReceipt["lifecycleStatus"];
  secondNote?: boolean;
};

type SeededFixture = {
  label: string;
  workspaceId: string;
  rawSnapshotId: string;
  capturePackageId: string;
  artifactId: string;
  canonicalObservationId: string;
  secondCanonicalObservationId?: string;
  noteId: string;
  lifecycleStatus: EvidenceArtifactAuditReceipt["lifecycleStatus"];
};

async function seedFixture(label: string, options: FixtureOptions = {}): Promise<SeededFixture> {
  const workspaceId = `ws-${label}`;
  const rawSnapshotId = `snapshot-${label}`;
  const capturePackageId = `package-${label}`;
  const artifactId = `artifact-${label}`;
  const recordId = `record-${label}`;
  const normalizationRunId = `run-${label}`;
  const canonicalObservationId = `observation-${label}`;
  const secondCanonicalObservationId = `observation-${label}-2`;
  const noteId = `note-${label}`;
  const secondNoteId = `note-${label}-2`;
  const observedAt = "2026-08-11T08:00:00.000Z";
  const payload = { noteId, platformContentId: noteId, type: "normal", title: label };
  const record = {
    idempotencyKey: `record-key-${label}`,
    recordKind: "note" as const,
    platform: "xhs" as const,
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 0,
    payload,
    observedAt,
  };
  const secondPayload = { noteId: secondNoteId, platformContentId: secondNoteId, type: "normal", title: `${label}-2` };
  const secondRecord = {
    idempotencyKey: `record-key-${label}-2`,
    recordKind: "note" as const,
    platform: "xhs" as const,
    targetKey: `xhs:note/${secondNoteId}`,
    externalRecordId: secondNoteId,
    sequence: 1,
    payload: secondPayload,
    observedAt,
  };
  const records = options.secondNote ? [record, secondRecord] : [record];
  const inventory = {
    schemaVersion: "xhs.media-inventory/v2",
    candidates: options.candidates ?? [
      candidate(noteId),
      ...(options.secondNote ? [candidate(secondNoteId)] : []),
    ],
  };
  const artifactBytes = Buffer.from(canonicalJsonString(inventory), "utf8");
  const actualArtifactChecksum = sha256(artifactBytes);
  const packagedArtifactChecksum = options.packagedArtifactChecksum ?? actualArtifactChecksum;
  const packagedArtifact = {
    kind: "media_inventory" as const,
    encoding: "base64" as const,
    artifactPayload: artifactBytes.toString("base64"),
    artifactChecksum: packagedArtifactChecksum,
    contentLength: artifactBytes.length,
    restricted: false,
  };
  const contractHash = computeContractHash(XHS_NOTE_DETAIL);
  const header: CapturePackagePayloadV2["header"] = {
    protocolVersion: "capture-submission/v2",
    captureId: `capture-${label}`,
    platform: "xhs",
    target: { expectedTargetKey: record.targetKey, observedTargetKey: record.targetKey },
    observedAt,
    collectorVersion: "proof-v1",
    contractId: XHS_NOTE_DETAIL.id,
    contractVersion: XHS_NOTE_DETAIL.version,
    contractHash,
    report: {
      startedAt: "2026-08-11T07:59:00.000Z",
      completedAt: observedAt,
      terminal: { state: "completed", reason: "source_exhausted", retryable: false },
      slots: [
        { slotId: "note", status: "observed", reason: null },
        { slotId: "comments", status: "not_applicable", reason: "not_requested" },
      ],
      counters: { requested: records.length, discovered: records.length, emitted: records.length, deduplicated: 0, failed: 0 },
      diagnostics: {},
    },
    ingressKind: "manual_import",
    sourceSummary: "B3 isolated proof",
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records,
    artifacts: [packagedArtifact],
  };
  const packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  const packageChecksum = sha256(packageBytes);

  await db.$executeRawUnsafe(
    `INSERT INTO "CapturePackage" ("id","workspaceId","packagePayload","checksumAlgorithm","checksumValue","contentLength") VALUES ($1,$2,$3::bytea,'sha256',$4,$5)`,
    capturePackageId, workspaceId, packageBytes, packageChecksum, packageBytes.length,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "RawSnapshot" ("id","workspaceId","captureId","platform","targetKey","observedAt","capturePackageId","checksumAlgorithm","checksumValue","contentLength","integrityStatus","contractId","contractVersion","contractHash") VALUES ($1,$2,$3,'xhs',$4,$5,$6,'sha256',$7,$8,'verified',$9,$10,$11)`,
    rawSnapshotId, workspaceId, header.captureId, record.targetKey, new Date(observedAt), capturePackageId,
    packageChecksum, packageBytes.length, XHS_NOTE_DETAIL.id, XHS_NOTE_DETAIL.version, contractHash,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "RawRecord" ("id","workspaceId","rawSnapshotId","recordKind","platform","targetKey","externalRecordId","sequence","payload","payloadHash","observedAt","idempotencyKey") VALUES ($1,$2,$3,'note','xhs',$4,$5,0,$6::jsonb,$7,$8,$9)`,
    recordId, workspaceId, rawSnapshotId, record.targetKey, noteId, JSON.stringify(payload),
    sha256(canonicalJsonString(payload)), new Date(observedAt), record.idempotencyKey,
  );
  if (options.secondNote) {
    await db.$executeRawUnsafe(
      `INSERT INTO "RawRecord" ("id","workspaceId","rawSnapshotId","recordKind","platform","targetKey","externalRecordId","sequence","payload","payloadHash","observedAt","idempotencyKey") VALUES ($1,$2,$3,'note','xhs',$4,$5,1,$6::jsonb,$7,$8,$9)`,
      `${recordId}-2`, workspaceId, rawSnapshotId, secondRecord.targetKey, secondNoteId,
      JSON.stringify(secondPayload), sha256(canonicalJsonString(secondPayload)),
      new Date(observedAt), secondRecord.idempotencyKey,
    );
  }
  await db.$executeRawUnsafe(
    `INSERT INTO "CaptureArtifact" ("id","workspaceId","rawSnapshotId","kind","artifactChecksum","restricted") VALUES ($1,$2,$3,'media_inventory',$4,false)`,
    artifactId, workspaceId, rawSnapshotId, options.descriptorChecksum ?? packagedArtifactChecksum,
  );
  if (options.secondNote) {
    await db.$executeRawUnsafe(
      `INSERT INTO "NormalizationRun" ("id","workspaceId","rawSnapshotId","rawRecordId","adapterId","adapterVersion","canonicalSchemaVersion","status","inputPayloadHash","outputPayloadHash") VALUES ($1,$2,$3,$4,'xhs.note','proof-v1','proof-v1','normalized',$5,$6)`,
      `${normalizationRunId}-2`, workspaceId, rawSnapshotId, `${recordId}-2`,
      sha256(canonicalJsonString(secondPayload)), sha256(canonicalJsonString({ payload: secondPayload })),
    );
  }
  await db.$executeRawUnsafe(
    `INSERT INTO "NormalizationRun" ("id","workspaceId","rawSnapshotId","rawRecordId","adapterId","adapterVersion","canonicalSchemaVersion","status","inputPayloadHash","outputPayloadHash") VALUES ($1,$2,$3,$4,'xhs.note','proof-v1','proof-v1','normalized',$5,$6)`,
    normalizationRunId, workspaceId, rawSnapshotId, recordId,
    sha256(canonicalJsonString(payload)), sha256(canonicalJsonString({ payload })),
  );
  if (options.secondNote) {
    await db.$executeRawUnsafe(
      `INSERT INTO "CanonicalObservation" ("id","workspaceId","normalizationRunId","rawSnapshotId","rawRecordId","observationKind","subjectKey","observedAt","schemaVersion","payload","payloadHash","qualityStatus") VALUES ($1,$2,$3,$4,$5,'note',$6,$7,'proof-v1',$8::jsonb,$9,'complete')`,
      secondCanonicalObservationId, workspaceId, `${normalizationRunId}-2`, rawSnapshotId, `${recordId}-2`,
      `xhs:note:${encodeURIComponent(secondNoteId)}`, new Date(observedAt),
      JSON.stringify({ payload: secondPayload }), sha256(canonicalJsonString({ payload: secondPayload })),
    );
  }
  await db.$executeRawUnsafe(
    `INSERT INTO "CanonicalObservation" ("id","workspaceId","normalizationRunId","rawSnapshotId","rawRecordId","observationKind","subjectKey","observedAt","schemaVersion","payload","payloadHash","qualityStatus") VALUES ($1,$2,$3,$4,$5,'note',$6,$7,'proof-v1',$8::jsonb,$9,'complete')`,
    canonicalObservationId, workspaceId, normalizationRunId, rawSnapshotId, recordId,
    `xhs:note:${encodeURIComponent(noteId)}`, new Date(observedAt), JSON.stringify({ payload }),
    sha256(canonicalJsonString({ payload })),
  );

  return {
    label,
    workspaceId,
    rawSnapshotId,
    capturePackageId,
    artifactId,
    canonicalObservationId,
    secondCanonicalObservationId: options.secondNote ? secondCanonicalObservationId : undefined,
    noteId,
    lifecycleStatus: options.lifecycleStatus ?? "ACTIVE",
  };
}

function databaseAuditSource(
  client: PrismaClient,
  fixture: SeededFixture,
  mutate?: (receipt: EvidenceArtifactAuditReceipt) => EvidenceArtifactAuditReceipt,
): EvidenceArtifactAuditSource {
  return {
    async readCaptureArtifactsAndAudit(input) {
      const snapshot = await client.rawSnapshot.findFirstOrThrow({
        where: { id: input.rawSnapshotId, workspaceId: input.workspaceId },
        select: {
          id: true,
          workspaceId: true,
          captureId: true,
          platform: true,
          integrityStatus: true,
          checksumAlgorithm: true,
          checksumValue: true,
          contentLength: true,
          contractId: true,
          contractVersion: true,
          contractHash: true,
          capturePackageId: true,
          records: {
            select: {
              id: true,
              workspaceId: true,
              rawSnapshotId: true,
              recordKind: true,
              platform: true,
              targetKey: true,
              externalRecordId: true,
              sequence: true,
              payload: true,
              payloadHash: true,
              observedAt: true,
              idempotencyKey: true,
            },
          },
          captureArtifacts: {
            select: {
              id: true,
              workspaceId: true,
              rawSnapshotId: true,
              kind: true,
              artifactChecksum: true,
              restricted: true,
            },
          },
        },
      });
      if (!snapshot.capturePackageId) throw new Error("Proof snapshot has no CapturePackage.");
      const rows = await client.$queryRawUnsafe<Array<{
        packagePayload: unknown;
        checksumAlgorithm: string;
        checksumValue: string;
        contentLength: number;
      }>>(
        `SELECT "packagePayload","checksumAlgorithm","checksumValue","contentLength" FROM "CapturePackage" WHERE "workspaceId"=$1 AND "id"=$2`,
        input.workspaceId,
        snapshot.capturePackageId,
      );
      if (rows.length !== 1) throw new Error("Proof CapturePackage is missing.");
      const rawPayload = rows[0].packagePayload;
      const receipt: EvidenceArtifactAuditReceipt = {
        workspaceId: snapshot.workspaceId,
        rawSnapshotId: snapshot.id,
        capturePackageId: snapshot.capturePackageId,
        accessAuditId: `audit-${fixture.label}`,
        lifecycleStatus: fixture.lifecycleStatus,
        integrityStatus: snapshot.integrityStatus ?? "",
        packageBytes: Buffer.isBuffer(rawPayload) ? rawPayload : Buffer.from(rawPayload as string, "base64"),
        packageChecksumAlgorithm: rows[0].checksumAlgorithm,
        packageChecksumValue: rows[0].checksumValue,
        packageContentLength: rows[0].contentLength,
        snapshot,
        artifacts: snapshot.captureArtifacts,
      };
      return mutate ? mutate(structuredClone(receipt)) : receipt;
    },
  };
}

async function verifiedFor(
  fixture: SeededFixture,
  mutate?: (receipt: EvidenceArtifactAuditReceipt) => EvidenceArtifactAuditReceipt,
) {
  return new VerifiedArtifactReader(databaseAuditSource(db, fixture, mutate)).readAndVerify({
    workspaceId: fixture.workspaceId,
    rawSnapshotId: fixture.rawSnapshotId,
  });
}

beforeAll(async () => {
  if (!OPTED_IN) return;
  psqlSql("postgres", `CREATE DATABASE "${PROOF_DB}";`);
  assertProofDatabase();
  writeFileSync(
    FIXED_SCHEMA_PATH,
    execFileSync("git", ["show", `${FIXED_POINT}:prisma/schema.prisma`], { encoding: "utf8" }),
  );
  execFileSync(
    process.execPath,
    [PRISMA_CLI_PATH, "db", "push", "--url", PROOF_DB_URL, "--schema", FIXED_SCHEMA_PATH],
    { cwd: process.cwd(), stdio: "pipe", timeout: 120_000 },
  );
  psqlFile(PROOF_DB, MIGRATION_PATH);
  db = createPrismaClient(PROOF_DB_URL);
  db2 = createPrismaClient(PROOF_DB_URL);
  base = await seedFixture("base");
}, 180_000);

afterAll(async () => {
  if (db) await db.$disconnect();
  if (db2) await db2.$disconnect();
  if (existsSync(FIXED_SCHEMA_PATH)) unlinkSync(FIXED_SCHEMA_PATH);
  if (OPTED_IN) {
    console.log(`[b3-media] Proof DB preserved: ${PROOF_DB}`);
    console.log(`[b3-media] Manual cleanup: dropdb --if-exists --host=${PG_HOST} --port=${PG_PORT} --username=${PG_USER} ${PROOF_DB}`);
  }
});

const describeIntegration = OPTED_IN ? describe : describe.skip;

describeIntegration("B3 canonical media isolated proof", () => {
  it("01 accepts audited media Artifact and writes one observed slot", async () => {
    const result = await verifiedFor(base);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    const written = await new CanonicalMediaSlotWriter(db).writeSlots({
      canonicalObservationId: base.canonicalObservationId,
      verified: result.verified,
    });
    expect(written).toMatchObject({ inserted: 1, replayed: 0 });
  });

  it("02 rejects an audited package checksum mismatch", async () => {
    const result = await verifiedFor(base, (receipt) => ({ ...receipt, packageChecksumValue: "0".repeat(64) }));
    expect(result).toMatchObject({ ok: false, reason: "capture_package_invalid" });
  });

  it("03 rejects a CaptureArtifact descriptor mismatch", async () => {
    const fixture = await seedFixture("descriptor-mismatch", { descriptorChecksum: "a".repeat(64) });
    expect(await verifiedFor(fixture)).toMatchObject({ ok: false, reason: "artifact_descriptor_mismatch" });
  });

  it("04 rejects package bytes changed after audit metadata was issued", async () => {
    const result = await verifiedFor(base, (receipt) => {
      const changed = Uint8Array.from(receipt.packageBytes);
      changed[changed.length - 1] ^= 1;
      return { ...receipt, packageBytes: changed };
    });
    expect(result).toMatchObject({ ok: false, reason: "capture_package_invalid" });
  });

  it("05 rejects an Artifact payload whose declared checksum is false", async () => {
    const fixture = await seedFixture("artifact-checksum", {
      packagedArtifactChecksum: "b".repeat(64),
      descriptorChecksum: "b".repeat(64),
    });
    expect(await verifiedFor(fixture)).toMatchObject({ ok: false, reason: "artifact_checksum_mismatch" });
  });

  it("06 rejects a URL-shaped slot identity", async () => {
    const fixture = await seedFixture("url-slot", {
      candidates: [candidate("note-url-slot", 0, { slotId: "https://evil.invalid/media" })],
    });
    expect(await verifiedFor(fixture)).toMatchObject({ ok: false, reason: "media_slot_mismatch" });
  });

  it("07 rejects a candidate without a producer ordinal", async () => {
    const withoutOrdinal = candidate("note-no-ordinal") as Record<string, unknown>;
    delete withoutOrdinal.ordinal;
    const fixture = await seedFixture("no-ordinal", { candidates: [withoutOrdinal] });
    expect(await verifiedFor(fixture)).toMatchObject({ ok: false, reason: "media_candidate_shape_invalid" });
  });

  it("08 rejects a media subject absent from same-package note records", async () => {
    const fixture = await seedFixture("wrong-subject", { candidates: [candidate("different-note")] });
    expect(await verifiedFor(fixture)).toMatchObject({ ok: false, reason: "media_subject_mismatch" });
  });

  it("09 rejects a same-shaped forged verifier capability before database access", async () => {
    const forged = Object.create(VerifiedMediaArtifacts.prototype) as VerifiedMediaArtifacts;
    await expect(new CanonicalMediaSlotWriter(db).writeSlots({
      canonicalObservationId: base.canonicalObservationId,
      verified: forged,
    })).rejects.toThrow(MediaArtifactInvariantError);
  });

  it("10 replays an identical observation slot without a second row", async () => {
    const before = await db.canonicalMediaSlot.count({ where: { canonicalObservationId: base.canonicalObservationId } });
    const result = await verifiedFor(base);
    if (!result.ok) throw new Error(result.detail);
    const written = await new CanonicalMediaSlotWriter(db).writeSlots({ canonicalObservationId: base.canonicalObservationId, verified: result.verified });
    expect(written).toMatchObject({ inserted: 0, replayed: 1 });
    expect(await db.canonicalMediaSlot.count({ where: { canonicalObservationId: base.canonicalObservationId } })).toBe(before);
  });

  it("11 rejects same slot identity with changed immutable fields", async () => {
    const fixture = await seedFixture("field-conflict");
    await db.canonicalMediaSlot.create({
      data: {
        workspaceId: fixture.workspaceId,
        canonicalObservationId: fixture.canonicalObservationId,
        slotId: `note:${fixture.noteId}:cover:image:0`,
        status: "observed",
        kind: "video",
        ordinal: 0,
      },
    });
    const result = await verifiedFor(fixture);
    if (!result.ok) throw new Error(result.detail);
    await expect(new CanonicalMediaSlotWriter(db).writeSlots({ canonicalObservationId: fixture.canonicalObservationId, verified: result.verified }))
      .rejects.toThrow(/replay fields changed/);
  });

  it("12 rejects cross-workspace CanonicalObservation binding with zero writes", async () => {
    const other = await seedFixture("other-workspace");
    const result = await verifiedFor(base);
    if (!result.ok) throw new Error(result.detail);
    const before = await db.canonicalMediaSlot.count({ where: { canonicalObservationId: other.canonicalObservationId } });
    await expect(new CanonicalMediaSlotWriter(db).writeSlots({ canonicalObservationId: other.canonicalObservationId, verified: result.verified }))
      .rejects.toThrow(/not bound/);
    expect(await db.canonicalMediaSlot.count({ where: { canonicalObservationId: other.canonicalObservationId } })).toBe(before);
  });

  it("13 rejects direct cross-workspace FK insertion with SQLSTATE 23503", () => {
    const failure = psqlFailure(PROOF_DB, `INSERT INTO "CanonicalMediaSlot" ("id","workspaceId","canonicalObservationId","slotId","status","kind","ordinal") VALUES ('cross-workspace-slot','wrong-workspace','${base.canonicalObservationId}','cross','observed','image',0);`);
    expect(failure).toContain("23503");
    expect(failure).toContain("CanonicalMediaSlot_workspace_observation_fkey");
  });

  it("14 rejects UPDATE and DELETE with exact SQLSTATE 55000", () => {
    const id = psqlQuery(PROOF_DB, `SELECT id FROM "CanonicalMediaSlot" WHERE "canonicalObservationId"='${base.canonicalObservationId}' LIMIT 1;`);
    expect(psqlFailure(PROOF_DB, `UPDATE "CanonicalMediaSlot" SET kind='video' WHERE id='${id}';`)).toContain("55000");
    expect(psqlFailure(PROOF_DB, `DELETE FROM "CanonicalMediaSlot" WHERE id='${id}';`)).toContain("55000");
    expect(psqlQuery(PROOF_DB, `SELECT count(*) FROM "CanonicalMediaSlot" WHERE id='${id}';`)).toBe("1");
  });

  it("15 rolls back all slots when a later insert fails", async () => {
    const noteId = "note-rollback";
    const fixture = await seedFixture("rollback", { candidates: [candidate(noteId, 0), candidate(noteId, 1)] });
    const result = await verifiedFor(fixture);
    if (!result.ok) throw new Error(result.detail);
    psqlSql(PROOF_DB, `CREATE OR REPLACE FUNCTION _b3_media_fail() RETURNS trigger AS $$ BEGIN IF NEW."ordinal"=1 THEN RAISE EXCEPTION 'injected' USING ERRCODE='55000'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql; CREATE TRIGGER _b3_media_fail_trigger BEFORE INSERT ON "CanonicalMediaSlot" FOR EACH ROW EXECUTE FUNCTION _b3_media_fail();`);
    try {
      await expect(new CanonicalMediaSlotWriter(db).writeSlots({ canonicalObservationId: fixture.canonicalObservationId, verified: result.verified })).rejects.toThrow(/injected/);
    } finally {
      psqlSql(PROOF_DB, `DROP TRIGGER IF EXISTS _b3_media_fail_trigger ON "CanonicalMediaSlot"; DROP FUNCTION IF EXISTS _b3_media_fail();`);
    }
    expect(await db.canonicalMediaSlot.count({ where: { canonicalObservationId: fixture.canonicalObservationId } })).toBe(0);
  });

  it("16 concurrent writers converge to inserted plus replayed", async () => {
    const fixture = await seedFixture("concurrent");
    const result = await verifiedFor(fixture);
    if (!result.ok) throw new Error(result.detail);
    const [left, right] = await Promise.all([
      new CanonicalMediaSlotWriter(db).writeSlots({ canonicalObservationId: fixture.canonicalObservationId, verified: result.verified }),
      new CanonicalMediaSlotWriter(db2).writeSlots({ canonicalObservationId: fixture.canonicalObservationId, verified: result.verified }),
    ]);
    expect(left.inserted + right.inserted).toBe(1);
    expect(left.replayed + right.replayed).toBe(1);
    expect(await db.canonicalMediaSlot.count({ where: { canonicalObservationId: fixture.canonicalObservationId } })).toBe(1);
  });

  it("17 accepts archived verified Evidence and rejects redacted Evidence", async () => {
    const archived = await seedFixture("archived", { lifecycleStatus: "ARCHIVED" });
    const redacted = await seedFixture("redacted", { lifecycleStatus: "REDACTED" });
    expect((await verifiedFor(archived)).ok).toBe(true);
    expect(await verifiedFor(redacted)).toMatchObject({ ok: false, reason: "evidence_ineligible" });
  });

  it("18 leaves the existing Media Domain and Projection tables untouched", async () => {
    expect(await db.mediaItem.count()).toBe(0);
    expect(await db.mediaOrigin.count()).toBe(0);
    expect(await db.contentMediaUsage.count()).toBe(0);
    expect(psqlQuery(PROOF_DB, "SELECT count(*) FROM \"CanonicalMediaSlot\";")).not.toBe("0");
  });

  it("19 isolates two note subjects carried by the same CapturePackage", async () => {
    const fixture = await seedFixture("multi-subject", { secondNote: true });
    const result = await verifiedFor(fixture);
    if (!result.ok) throw new Error(result.detail);
    if (!fixture.secondCanonicalObservationId) throw new Error("Second proof observation is missing.");

    const writer = new CanonicalMediaSlotWriter(db);
    await expect(writer.writeSlots({
      canonicalObservationId: fixture.canonicalObservationId,
      verified: result.verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
    await expect(writer.writeSlots({
      canonicalObservationId: fixture.secondCanonicalObservationId,
      verified: result.verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
    expect(await db.canonicalMediaSlot.count({
      where: { canonicalObservationId: fixture.canonicalObservationId },
    })).toBe(1);
    expect(await db.canonicalMediaSlot.count({
      where: { canonicalObservationId: fixture.secondCanonicalObservationId },
    })).toBe(1);
  });
});
