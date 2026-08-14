// @vitest-environment node

/** B3-MEDIA-SRC-003 fresh-database proof. Opt-in only; proof DB is preserved. */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createPrismaClient } from "@/lib/db";
import type { PrismaClient } from "@/lib/prisma-client";
import { parseCanonicalMediaProcessingPayload } from "@/lib/services/media-processing-queue-service";

import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";
import { canonicalJsonString } from "../ingress/canonical-json";
import type { CapturePackagePayloadV2 } from "../ingress/types";
import { CanonicalMediaAdapter } from "./canonical-media-adapter";
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
const PROOF_DB_PREFIX = "content_workbench_b3_media_domain_proof_";
const PROOF_DB = `${PROOF_DB_PREFIX}${Date.now()}`;
const PROOF_DB_URL = `postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${PROOF_DB}?schema=public`;
const FIXED_SCHEMA_PATH = join(tmpdir(), `b3-domain-fixed-schema-${Date.now()}.prisma`);
const MIGRATION_PATH = "prisma/migrations/20260811120000_add_canonical_media_slot/migration.sql";
const PRISMA_CLI_PATH = process.env.B3_MEDIA_PRISMA_CLI ?? "node_modules/prisma/build/index.js";
const OPTED_IN = process.env.B3_MEDIA_DOMAIN_INTEGRATION_DB === "1";

let db: ReturnType<typeof createPrismaClient>;
let db2: ReturnType<typeof createPrismaClient>;

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
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-v", "ON_ERROR_STOP=1", "-c", sql],
    { env: PG_ENV, encoding: "utf8" },
  );
}

function psqlFile(database: string, path: string): void {
  execFileSync(
    "psql",
    ["-h", PG_HOST, "-p", PG_PORT, "-U", PG_USER, "-d", database, "-v", "ON_ERROR_STOP=1", "-f", path],
    { env: PG_ENV, encoding: "utf8" },
  );
}

function assertProofDatabase(): void {
  if (!PROOF_DB.startsWith(PROOF_DB_PREFIX) || PROOF_DB === "content_workbench_local") {
    throw new Error(`Unsafe proof database name: ${PROOF_DB}`);
  }
  expect(psqlQuery(PROOF_DB, "SELECT current_database();")).toBe(PROOF_DB);
}

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

type InventoryCandidate = {
  subject: { kind: "note"; noteId: string; platformContentId: string };
  slotId: string;
  purpose: "cover" | "body" | "video" | "live_photo";
  kind: "image" | "video" | "live_photo";
  ordinal: number;
  observedAddress: string;
  coverProvenance: "platform_explicit" | "first_observed_image" | "not_cover";
};

function inventoryCandidate(
  noteId: string,
  input: {
    purpose?: InventoryCandidate["purpose"];
    kind?: InventoryCandidate["kind"];
    ordinal?: number;
    observedAddress?: string;
  } = {},
): InventoryCandidate {
  const purpose = input.purpose ?? "cover";
  const kind = input.kind ?? "image";
  const ordinal = input.ordinal ?? 0;
  return {
    subject: { kind: "note", noteId, platformContentId: noteId },
    slotId: `note:${noteId}:${purpose}:${kind}:${ordinal}`,
    purpose,
    kind,
    ordinal,
    observedAddress: input.observedAddress ?? `https://example.invalid/${noteId}/${purpose}-${ordinal}`,
    coverProvenance: purpose === "cover" ? "platform_explicit" : "not_cover",
  };
}

type SeedOptions = {
  workspaceId?: string;
  noteId?: string;
  observedAt?: string;
  candidates?: InventoryCandidate[];
  secondNote?: boolean;
};

type SeededFixture = {
  label: string;
  workspaceId: string;
  rawSnapshotId: string;
  capturePackageId: string;
  canonicalObservationId: string;
  secondCanonicalObservationId: string | null;
  noteId: string;
};

async function seedFixture(label: string, options: SeedOptions = {}): Promise<SeededFixture> {
  const workspaceId = options.workspaceId ?? `ws-domain-${label}`;
  const noteId = options.noteId ?? `note-domain-${label}`;
  const secondNoteId = `${noteId}-2`;
  const observedAt = options.observedAt ?? "2026-08-11T08:00:00.000Z";
  const rawSnapshotId = `snapshot-domain-${label}`;
  const capturePackageId = `package-domain-${label}`;
  const artifactId = `artifact-domain-${label}`;
  const recordId = `record-domain-${label}`;
  const runId = `run-domain-${label}`;
  const observationId = `observation-domain-${label}`;
  const secondObservationId = `observation-domain-${label}-2`;
  const payload = { noteId, platformContentId: noteId, type: "normal", title: label };
  const records: CapturePackagePayloadV2["records"] = [{
    idempotencyKey: `key-${label}`,
    recordKind: "note",
    platform: "xhs",
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 0,
    payload,
    observedAt,
  }];
  if (options.secondNote) {
    records.push({
      idempotencyKey: `key-${label}-2`,
      recordKind: "note",
      platform: "xhs",
      targetKey: `xhs:note/${secondNoteId}`,
      externalRecordId: secondNoteId,
      sequence: 1,
      payload: { noteId: secondNoteId, platformContentId: secondNoteId, type: "normal", title: `${label}-2` },
      observedAt,
    });
  }
  const candidates = options.candidates ?? [
    inventoryCandidate(noteId),
    ...(options.secondNote ? [inventoryCandidate(secondNoteId)] : []),
  ];
  const artifactPayload = {
    schemaVersion: "xhs.media-inventory/v2",
    candidates,
  };
  const artifactBytes = Buffer.from(canonicalJsonString(artifactPayload), "utf8");
  const artifactChecksum = sha256(artifactBytes);
  const contractHash = computeContractHash(XHS_NOTE_DETAIL);
  const header: CapturePackagePayloadV2["header"] = {
    protocolVersion: "capture-submission/v2",
    captureId: `capture-domain-${label}`,
    platform: "xhs",
    target: { expectedTargetKey: records[0].targetKey ?? "", observedTargetKey: records[0].targetKey },
    observedAt,
    collectorVersion: "domain-proof-v1",
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
    sourceSummary: "B3 media domain proof",
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records,
    artifacts: [{
      kind: "media_inventory",
      encoding: "base64",
      artifactPayload: artifactBytes.toString("base64"),
      artifactChecksum,
      contentLength: artifactBytes.length,
      restricted: false,
    }],
  };
  const packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  const packageChecksum = sha256(packageBytes);

  await db.$executeRawUnsafe(
    `INSERT INTO "CapturePackage" ("id","workspaceId","packagePayload","checksumAlgorithm","checksumValue","contentLength") VALUES ($1,$2,$3::bytea,'sha256',$4,$5)`,
    capturePackageId, workspaceId, packageBytes, packageChecksum, packageBytes.length,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "RawSnapshot" ("id","workspaceId","captureId","platform","targetKey","observedAt","capturePackageId","checksumAlgorithm","checksumValue","contentLength","integrityStatus","contractId","contractVersion","contractHash") VALUES ($1,$2,$3,'xhs',$4,$5,$6,'sha256',$7,$8,'verified',$9,$10,$11)`,
    rawSnapshotId, workspaceId, header.captureId, records[0].targetKey, new Date(observedAt), capturePackageId,
    packageChecksum, packageBytes.length, XHS_NOTE_DETAIL.id, XHS_NOTE_DETAIL.version, contractHash,
  );
  for (const [index, record] of records.entries()) {
    const currentRecordId = index === 0 ? recordId : `${recordId}-${index}`;
    const currentRunId = index === 0 ? runId : `${runId}-${index}`;
    const currentObservationId = index === 0 ? observationId : secondObservationId;
    await db.$executeRawUnsafe(
      `INSERT INTO "RawRecord" ("id","workspaceId","rawSnapshotId","recordKind","platform","targetKey","externalRecordId","sequence","payload","payloadHash","observedAt","idempotencyKey") VALUES ($1,$2,$3,'note','xhs',$4,$5,$6,$7::jsonb,$8,$9,$10)`,
      currentRecordId, workspaceId, rawSnapshotId, record.targetKey, record.externalRecordId, record.sequence,
      JSON.stringify(record.payload), sha256(canonicalJsonString(record.payload)), new Date(observedAt), record.idempotencyKey,
    );
    await db.$executeRawUnsafe(
      `INSERT INTO "NormalizationRun" ("id","workspaceId","rawSnapshotId","rawRecordId","adapterId","adapterVersion","canonicalSchemaVersion","status","inputPayloadHash","outputPayloadHash") VALUES ($1,$2,$3,$4,'xhs.note','proof-v1','proof-v1','normalized',$5,$6)`,
      currentRunId, workspaceId, rawSnapshotId, currentRecordId,
      sha256(canonicalJsonString(record.payload)), sha256(canonicalJsonString({ payload: record.payload })),
    );
    await db.$executeRawUnsafe(
      `INSERT INTO "CanonicalObservation" ("id","workspaceId","normalizationRunId","rawSnapshotId","rawRecordId","observationKind","subjectKey","observedAt","schemaVersion","payload","payloadHash","qualityStatus") VALUES ($1,$2,$3,$4,$5,'note',$6,$7,'proof-v1',$8::jsonb,$9,'complete')`,
      currentObservationId, workspaceId, currentRunId, rawSnapshotId, currentRecordId,
      `xhs:note:${encodeURIComponent(record.externalRecordId ?? "")}`, new Date(observedAt),
      JSON.stringify({ payload: record.payload }), sha256(canonicalJsonString({ payload: record.payload })),
    );
  }
  await db.$executeRawUnsafe(
    `INSERT INTO "CaptureArtifact" ("id","workspaceId","rawSnapshotId","kind","artifactChecksum","restricted") VALUES ($1,$2,$3,'media_inventory',$4,false)`,
    artifactId, workspaceId, rawSnapshotId, artifactChecksum,
  );
  return {
    label,
    workspaceId,
    rawSnapshotId,
    capturePackageId,
    canonicalObservationId: observationId,
    secondCanonicalObservationId: options.secondNote ? secondObservationId : null,
    noteId,
  };
}

function auditSource(client: PrismaClient, fixture: SeededFixture): EvidenceArtifactAuditSource {
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
              id: true, workspaceId: true, rawSnapshotId: true, recordKind: true, platform: true,
              targetKey: true, externalRecordId: true, sequence: true, payload: true,
              payloadHash: true, observedAt: true, idempotencyKey: true,
            },
          },
          captureArtifacts: {
            select: {
              id: true, workspaceId: true, rawSnapshotId: true, kind: true,
              artifactChecksum: true, restricted: true,
            },
          },
        },
      });
      if (!snapshot.capturePackageId) throw new Error("Proof snapshot has no package.");
      const [packaged] = await client.$queryRawUnsafe<Array<{
        packagePayload: unknown;
        checksumAlgorithm: string;
        checksumValue: string;
        contentLength: number;
      }>>(
        `SELECT "packagePayload","checksumAlgorithm","checksumValue","contentLength" FROM "CapturePackage" WHERE "workspaceId"=$1 AND id=$2`,
        input.workspaceId,
        snapshot.capturePackageId,
      );
      if (!packaged) throw new Error("Proof package is missing.");
      const receipt: EvidenceArtifactAuditReceipt = {
        workspaceId: snapshot.workspaceId,
        rawSnapshotId: snapshot.id,
        capturePackageId: snapshot.capturePackageId,
        accessAuditId: `audit-domain-${fixture.label}`,
        lifecycleStatus: "ACTIVE",
        integrityStatus: snapshot.integrityStatus ?? "",
        packageBytes: Buffer.isBuffer(packaged.packagePayload)
          ? packaged.packagePayload
          : Buffer.from(packaged.packagePayload as string, "base64"),
        packageChecksumAlgorithm: packaged.checksumAlgorithm,
        packageChecksumValue: packaged.checksumValue,
        packageContentLength: packaged.contentLength,
        snapshot,
        artifacts: snapshot.captureArtifacts,
      };
      return receipt;
    },
  };
}

async function verifiedFor(fixture: SeededFixture, client: PrismaClient = db) {
  return new VerifiedArtifactReader(auditSource(client, fixture)).readAndVerify({
    workspaceId: fixture.workspaceId,
    rawSnapshotId: fixture.rawSnapshotId,
  });
}

async function writeAndRegister(fixture: SeededFixture, client = db) {
  const verified = await verifiedFor(fixture, client);
  if (!verified.ok) throw new Error(`${verified.reason}: ${verified.detail}`);
  await new CanonicalMediaSlotWriter(client).writeSlots({
    canonicalObservationId: fixture.canonicalObservationId,
    verified: verified.verified,
  });
  const result = await new CanonicalMediaAdapter(client).registerObservedMedia({
    canonicalObservationId: fixture.canonicalObservationId,
    verified: verified.verified,
  });
  return { verified: verified.verified, result };
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
}, 180_000);

afterAll(async () => {
  if (db) await db.$disconnect();
  if (db2) await db2.$disconnect();
  if (existsSync(FIXED_SCHEMA_PATH)) unlinkSync(FIXED_SCHEMA_PATH);
  if (OPTED_IN) {
    console.log(`[b3-domain] Proof DB preserved: ${PROOF_DB}`);
    console.log(`[b3-domain] Manual cleanup: DROP DATABASE IF EXISTS "${PROOF_DB}";`);
  }
});

const describeIntegration = OPTED_IN ? describe : describe.skip;

describeIntegration("B3 Canonical Media Domain isolated proof", () => {
  it("01 asserts the protected proof database before schema writes", () => {
    assertProofDatabase();
  });

  it("02 registers an observed image as MediaItem, MediaOrigin, and Outbox", async () => {
    const fixture = await seedFixture("image");
    const { result } = await writeAndRegister(fixture);
    expect(result).toMatchObject({ ok: true, enqueued: 1, replayed: 0 });
  });

  it("03 registers an observed video with the governed video role", async () => {
    const noteId = "note-domain-video";
    const fixture = await seedFixture("video", {
      noteId,
      candidates: [inventoryCandidate(noteId, { purpose: "video", kind: "video" })],
    });
    const { result } = await writeAndRegister(fixture);
    expect(result).toMatchObject({ ok: true, enqueued: 1 });
    const event = await db.outboxEvent.findFirstOrThrow({ where: { workspaceId: fixture.workspaceId } });
    expect(event.payload).toMatchObject({ role: "video", kind: "video" });
  });

  it("04 merges same-source cover and image:0 identity but emits one receipt per slot", async () => {
    const noteId = "note-domain-cover-body";
    const address = "https://example.invalid/shared-cover-body";
    const fixture = await seedFixture("cover-body", {
      noteId,
      candidates: [
        inventoryCandidate(noteId, { purpose: "cover", observedAddress: address }),
        inventoryCandidate(noteId, { purpose: "body", ordinal: 0, observedAddress: address }),
      ],
    });
    const { result } = await writeAndRegister(fixture);
    expect(result).toMatchObject({ ok: true, enqueued: 2 });
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
    expect(await db.mediaOrigin.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(2);
  });

  it("05 writes and strictly parses all five Canonical ledger fields", async () => {
    const fixture = await seedFixture("five-fields");
    await writeAndRegister(fixture);
    const event = await db.outboxEvent.findFirstOrThrow({ where: { workspaceId: fixture.workspaceId } });
    expect(parseCanonicalMediaProcessingPayload(event.payload)).toMatchObject({
      ledgerOrigin: {
        canonicalObservationId: fixture.canonicalObservationId,
        slotId: `note:${fixture.noteId}:cover:image:0`,
        originId: expect.any(String),
        mediaItemId: expect.any(String),
        generation: 1,
      },
    });
  });

  it("06 keeps the stable locator free of URL material", async () => {
    const fixture = await seedFixture("locator", {
      candidates: [inventoryCandidate("note-domain-locator", {
        observedAddress: "https://example.invalid/a.jpg?token=secret&sign=secret",
      })],
    });
    await writeAndRegister(fixture);
    const origin = await db.mediaOrigin.findFirstOrThrow({ where: { workspaceId: fixture.workspaceId } });
    expect(origin.stableLocator).not.toMatch(/https|token|secret|sign/);
  });

  it("07 replays a pending Canonical event without inserting another", async () => {
    const fixture = await seedFixture("replay-pending");
    const first = await writeAndRegister(fixture);
    const adapter = new CanonicalMediaAdapter(db);
    const second = await adapter.registerObservedMedia({
      canonicalObservationId: fixture.canonicalObservationId,
      verified: first.verified,
    });
    expect(second).toMatchObject({ ok: true, enqueued: 0, replayed: 1 });
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
  });

  it("08 replays a processed Canonical event without re-enqueueing", async () => {
    const fixture = await seedFixture("replay-processed");
    const first = await writeAndRegister(fixture);
    await db.outboxEvent.updateMany({ where: { workspaceId: fixture.workspaceId }, data: { status: "processed" } });
    const second = await new CanonicalMediaAdapter(db).registerObservedMedia({
      canonicalObservationId: fixture.canonicalObservationId,
      verified: first.verified,
    });
    expect(second).toMatchObject({ ok: true, enqueued: 0, replayed: 1 });
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
  });

  it("09 converges concurrent identical registrations", async () => {
    const fixture = await seedFixture("concurrent");
    const verified = await verifiedFor(fixture);
    if (!verified.ok) throw new Error(verified.detail);
    await new CanonicalMediaSlotWriter(db).writeSlots({
      canonicalObservationId: fixture.canonicalObservationId,
      verified: verified.verified,
    });
    const [left, right] = await Promise.all([
      new CanonicalMediaAdapter(db).registerObservedMedia({ verified: verified.verified, canonicalObservationId: fixture.canonicalObservationId }),
      new CanonicalMediaAdapter(db2).registerObservedMedia({ verified: verified.verified, canonicalObservationId: fixture.canonicalObservationId }),
    ]);
    expect(left.ok && right.ok).toBe(true);
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
    expect(await db.mediaOrigin.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(1);
  });

  it("10 advances an existing stable origin generation for a newer address", async () => {
    const workspaceId = "ws-domain-generation";
    const noteId = "note-domain-generation";
    const first = await seedFixture("generation-old", { workspaceId, noteId, observedAt: "2026-08-11T08:00:00Z" });
    await writeAndRegister(first);
    const second = await seedFixture("generation-new", {
      workspaceId,
      noteId,
      observedAt: "2026-08-11T09:00:00Z",
      candidates: [inventoryCandidate(noteId, { observedAddress: "https://example.invalid/new-address" })],
    });
    await writeAndRegister(second);
    const origins = await db.mediaOrigin.findMany({ where: { workspaceId } });
    expect(origins).toHaveLength(1);
    expect(origins[0]).toMatchObject({ generation: 2, fetchUrl: "https://example.invalid/new-address" });
  });

  it("11 rejects an older address without overwriting the newer generation", async () => {
    const workspaceId = "ws-domain-stale";
    const noteId = "note-domain-stale";
    const newer = await seedFixture("stale-new", {
      workspaceId, noteId, observedAt: "2026-08-11T10:00:00Z",
      candidates: [inventoryCandidate(noteId, { observedAddress: "https://example.invalid/newer" })],
    });
    await writeAndRegister(newer);
    const older = await seedFixture("stale-old", {
      workspaceId, noteId, observedAt: "2026-08-11T09:00:00Z",
      candidates: [inventoryCandidate(noteId, { observedAddress: "https://example.invalid/older" })],
    });
    const beforeEvents = await db.outboxEvent.count({ where: { workspaceId } });
    const { result } = await writeAndRegister(older);
    expect(result).toMatchObject({ ok: false, reason: "canonical_media_source_superseded" });
    const origin = await db.mediaOrigin.findFirstOrThrow({ where: { workspaceId } });
    expect(origin.fetchUrl).toBe("https://example.invalid/newer");
    expect(await db.outboxEvent.count({ where: { workspaceId } })).toBe(beforeEvents);
  });

  it("12 rejects cross-workspace Observation binding with zero writes", async () => {
    const left = await seedFixture("cross-left");
    const right = await seedFixture("cross-right");
    const verified = await verifiedFor(left);
    if (!verified.ok) throw new Error(verified.detail);
    const result = await new CanonicalMediaAdapter(db).registerObservedMedia({
      verified: verified.verified,
      canonicalObservationId: right.canonicalObservationId,
    });
    expect(result.ok).toBe(false);
    expect(await db.mediaItem.count({ where: { workspaceId: right.workspaceId } })).toBe(0);
  });

  it("13 rejects a forged capability before database access", async () => {
    const forged = Object.create(VerifiedMediaArtifacts.prototype) as VerifiedMediaArtifacts;
    await expect(new CanonicalMediaAdapter(db).registerObservedMedia({
      verified: forged,
      canonicalObservationId: "irrelevant",
    })).rejects.toThrow(MediaArtifactInvariantError);
  });

  it("14 rejects a slot/candidate immutable-field mismatch", async () => {
    const fixture = await seedFixture("slot-mismatch");
    const verified = await verifiedFor(fixture);
    if (!verified.ok) throw new Error(verified.detail);
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
    const result = await new CanonicalMediaAdapter(db).registerObservedMedia({
      verified: verified.verified,
      canonicalObservationId: fixture.canonicalObservationId,
    });
    expect(result).toMatchObject({ ok: false, reason: "canonical_media_slot_binding_mismatch" });
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("15 rejects a missing CanonicalMediaSlot rather than inferring absence", async () => {
    const fixture = await seedFixture("missing-slot");
    const verified = await verifiedFor(fixture);
    if (!verified.ok) throw new Error(verified.detail);
    const result = await new CanonicalMediaAdapter(db).registerObservedMedia({
      verified: verified.verified,
      canonicalObservationId: fixture.canonicalObservationId,
    });
    expect(result).toMatchObject({ ok: false, reason: "canonical_media_slot_binding_mismatch" });
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("16 rolls back Media identity when Outbox insertion fails", async () => {
    const fixture = await seedFixture("rollback");
    const verified = await verifiedFor(fixture);
    if (!verified.ok) throw new Error(verified.detail);
    await new CanonicalMediaSlotWriter(db).writeSlots({
      verified: verified.verified,
      canonicalObservationId: fixture.canonicalObservationId,
    });
    psqlSql(PROOF_DB, `CREATE OR REPLACE FUNCTION _b3_domain_fail() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'injected' USING ERRCODE='55000'; END; $$ LANGUAGE plpgsql; CREATE TRIGGER _b3_domain_fail_trigger BEFORE INSERT ON "OutboxEvent" FOR EACH ROW EXECUTE FUNCTION _b3_domain_fail();`);
    try {
      await expect(new CanonicalMediaAdapter(db).registerObservedMedia({
        verified: verified.verified,
        canonicalObservationId: fixture.canonicalObservationId,
      })).rejects.toThrow(/injected/);
    } finally {
      psqlSql(PROOF_DB, `DROP TRIGGER IF EXISTS _b3_domain_fail_trigger ON "OutboxEvent"; DROP FUNCTION IF EXISTS _b3_domain_fail();`);
    }
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.mediaOrigin.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("17 isolates two note subjects in the same CapturePackage", async () => {
    const fixture = await seedFixture("multi-note", { secondNote: true });
    const verified = await verifiedFor(fixture);
    if (!verified.ok || !fixture.secondCanonicalObservationId) throw new Error("multi-note proof invalid");
    const writer = new CanonicalMediaSlotWriter(db);
    await writer.writeSlots({ verified: verified.verified, canonicalObservationId: fixture.canonicalObservationId });
    await writer.writeSlots({ verified: verified.verified, canonicalObservationId: fixture.secondCanonicalObservationId });
    const adapter = new CanonicalMediaAdapter(db);
    const first = await adapter.registerObservedMedia({ verified: verified.verified, canonicalObservationId: fixture.canonicalObservationId });
    const second = await adapter.registerObservedMedia({ verified: verified.verified, canonicalObservationId: fixture.secondCanonicalObservationId });
    expect(first).toMatchObject({ ok: true, enqueued: 1 });
    expect(second).toMatchObject({ ok: true, enqueued: 1 });
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(2);
  });

  it("18 fails closed for live_photo without partially registering valid slots", async () => {
    const noteId = "note-domain-live";
    const fixture = await seedFixture("live", {
      noteId,
      candidates: [
        inventoryCandidate(noteId),
        inventoryCandidate(noteId, { purpose: "live_photo", kind: "live_photo", ordinal: 1 }),
      ],
    });
    const { result } = await writeAndRegister(fixture);
    expect(result).toMatchObject({ ok: false, reason: "canonical_media_kind_source_incomplete" });
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.mediaOrigin.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("19 rejects an ambiguous identity plan without partially writing another valid slot", async () => {
    const noteId = "note-domain-ambiguous";
    const sharedAddress = "https://example.invalid/ambiguous-body";
    const fixture = await seedFixture("ambiguous", {
      noteId,
      candidates: [
        inventoryCandidate(noteId, { purpose: "body", ordinal: 0, observedAddress: sharedAddress }),
        inventoryCandidate(noteId, { purpose: "body", ordinal: 1, observedAddress: sharedAddress }),
        inventoryCandidate(noteId, {
          purpose: "video",
          kind: "video",
          ordinal: 0,
          observedAddress: "https://example.invalid/otherwise-valid-video",
        }),
      ],
    });
    const { result } = await writeAndRegister(fixture);
    expect(result).toMatchObject({ ok: false, reason: "canonical_media_identity_rejected" });
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.mediaOrigin.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
    expect(await db.outboxEvent.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("20 rejects absent/unavailable slot state with zero Media Domain writes", async () => {
    const fixture = await seedFixture("non-observed");
    const verified = await verifiedFor(fixture);
    if (!verified.ok) throw new Error(verified.detail);
    await db.canonicalMediaSlot.create({
      data: {
        workspaceId: fixture.workspaceId,
        canonicalObservationId: fixture.canonicalObservationId,
        slotId: `note:${fixture.noteId}:cover:image:0`,
        status: "absent",
        kind: "image",
        ordinal: 0,
      },
    });
    const result = await new CanonicalMediaAdapter(db).registerObservedMedia({
      verified: verified.verified,
      canonicalObservationId: fixture.canonicalObservationId,
    });
    expect(result.ok).toBe(false);
    expect(await db.mediaItem.count({ where: { workspaceId: fixture.workspaceId } })).toBe(0);
  });

  it("21 leaves all business media relations and B3 Projection models untouched", async () => {
    expect(await db.contentMediaUsage.count()).toBe(0);
    expect(await db.authorMedia.count()).toBe(0);
    expect(await db.publicationMedia.count()).toBe(0);
    expect(await db.topicMediaSelection.count()).toBe(0);
    expect(psqlQuery(PROOF_DB, `SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('ContentCurrentProjection','AuthorCurrentProjection','CommentCurrentProjection');`)).toBe("0");
  });

  it("22 satisfies the frozen observed-slot Media receipt SQL", async () => {
    const fixture = await seedFixture("freeze-sql");
    await writeAndRegister(fixture);
    const denominator = psqlQuery(PROOF_DB, `SELECT count(*) FROM "CanonicalMediaSlot" WHERE "workspaceId"='${fixture.workspaceId}' AND status='observed';`);
    const numerator = psqlQuery(PROOF_DB, `SELECT count(*) FROM "CanonicalMediaSlot" cms WHERE cms."workspaceId"='${fixture.workspaceId}' AND cms.status='observed' AND EXISTS (SELECT 1 FROM "OutboxEvent" o JOIN "MediaOrigin" mo ON mo.id=(o.payload#>>'{ledgerOrigin,originId}')::text AND mo."workspaceId"=o."workspaceId" JOIN "MediaItem" mi ON mi.id=mo."mediaItemId" AND mi."workspaceId"=o."workspaceId" WHERE o."workspaceId"=cms."workspaceId" AND o."eventType"='media.processing_requested' AND o.payload#>>'{ledgerOrigin,canonicalObservationId}'=cms."canonicalObservationId" AND o.payload#>>'{ledgerOrigin,slotId}'=cms."slotId" AND mo."mediaItemId"=(o.payload#>>'{ledgerOrigin,mediaItemId}')::text AND mi.id=mo."mediaItemId" AND mo.generation=(o.payload#>>'{ledgerOrigin,generation}')::int);`);
    const unmatched = psqlQuery(PROOF_DB, `SELECT count(*) FROM "CanonicalMediaSlot" cms WHERE cms."workspaceId"='${fixture.workspaceId}' AND cms.status='observed' AND NOT EXISTS (SELECT 1 FROM "OutboxEvent" o JOIN "MediaOrigin" mo ON mo.id=(o.payload#>>'{ledgerOrigin,originId}')::text AND mo."workspaceId"=o."workspaceId" WHERE o."workspaceId"=cms."workspaceId" AND o.payload#>>'{ledgerOrigin,canonicalObservationId}'=cms."canonicalObservationId" AND o.payload#>>'{ledgerOrigin,slotId}'=cms."slotId" AND mo."mediaItemId"=(o.payload#>>'{ledgerOrigin,mediaItemId}')::text);`);
    console.log(JSON.stringify({ database: PROOF_DB, denominator, numerator, unmatched }));
    expect(numerator).toBe(denominator);
    expect(unmatched).toBe("0");
  });
});
