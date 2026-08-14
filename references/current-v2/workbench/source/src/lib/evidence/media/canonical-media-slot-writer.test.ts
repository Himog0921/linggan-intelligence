// @vitest-environment node

import { createHash } from "node:crypto";

import { describe, expect, it, vi } from "vitest";

import type { Prisma, PrismaClient } from "@/lib/prisma-client";

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

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function buildReceipt(): EvidenceArtifactAuditReceipt {
  const workspaceId = "workspace-unit";
  const rawSnapshotId = "snapshot-unit";
  const capturePackageId = "package-unit";
  const noteId = "note-unit";
  const observedAt = "2026-08-11T08:00:00.000Z";
  const payload = { noteId, platformContentId: noteId, type: "normal", title: "unit" };
  const record = {
    idempotencyKey: "record-key-unit",
    recordKind: "note" as const,
    platform: "xhs" as const,
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 0,
    payload,
    observedAt,
  };
  const inventoryBytes = Buffer.from(canonicalJsonString({
    schemaVersion: "xhs.media-inventory/v2",
    candidates: [{
      subject: { kind: "note", noteId, platformContentId: noteId },
      slotId: `note:${noteId}:cover:image:0`,
      purpose: "cover",
      kind: "image",
      ordinal: 0,
      observedAddress: "https://example.invalid/unit.jpg",
      coverProvenance: "platform_explicit",
    }],
  }), "utf8");
  const artifactChecksum = sha256(inventoryBytes);
  const contractHash = computeContractHash(XHS_NOTE_DETAIL);
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header: {
      protocolVersion: "capture-submission/v2",
      captureId: "capture-unit",
      platform: "xhs",
      target: { expectedTargetKey: record.targetKey, observedTargetKey: record.targetKey },
      observedAt,
      collectorVersion: "unit-v1",
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
        counters: { requested: 1, discovered: 1, emitted: 1, deduplicated: 0, failed: 0 },
        diagnostics: {},
      },
      ingressKind: "manual_import",
      sourceSummary: "unit",
    },
    records: [record],
    artifacts: [{
      kind: "media_inventory",
      encoding: "base64",
      artifactPayload: inventoryBytes.toString("base64"),
      artifactChecksum,
      contentLength: inventoryBytes.length,
      restricted: false,
    }],
  };
  const packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  const packageChecksum = sha256(packageBytes);
  return {
    workspaceId,
    rawSnapshotId,
    capturePackageId,
    accessAuditId: "audit-unit",
    lifecycleStatus: "ACTIVE",
    integrityStatus: "verified",
    packageBytes,
    packageChecksumAlgorithm: "sha256",
    packageChecksumValue: packageChecksum,
    packageContentLength: packageBytes.length,
    snapshot: {
      id: rawSnapshotId,
      workspaceId,
      captureId: capturePackage.header.captureId,
      platform: "xhs",
      integrityStatus: "verified",
      checksumAlgorithm: "sha256",
      checksumValue: packageChecksum,
      contentLength: packageBytes.length,
      contractId: XHS_NOTE_DETAIL.id,
      contractVersion: XHS_NOTE_DETAIL.version,
      contractHash,
      capturePackageId,
      records: [{
        id: "record-unit",
        workspaceId,
        rawSnapshotId,
        recordKind: "note",
        platform: "xhs",
        targetKey: record.targetKey,
        externalRecordId: noteId,
        sequence: 0,
        payloadHash: sha256(canonicalJsonString(payload)),
        observedAt: new Date(observedAt),
        idempotencyKey: record.idempotencyKey,
      }],
    },
    artifacts: [{
      id: "artifact-unit",
      workspaceId,
      rawSnapshotId,
      kind: "media_inventory",
      artifactChecksum,
      restricted: false,
    }],
  };
}

function addSecondNote(receipt: EvidenceArtifactAuditReceipt): EvidenceArtifactAuditReceipt {
  const next = structuredClone(receipt);
  const capturePackage = JSON.parse(Buffer.from(next.packageBytes).toString("utf8")) as CapturePackagePayloadV2;
  const noteId = "note-unit-2";
  const observedAt = "2026-08-11T08:01:00.000Z";
  const payload = { noteId, platformContentId: noteId, type: "normal", title: "unit-2" };
  capturePackage.records.push({
    idempotencyKey: "record-key-unit-2",
    recordKind: "note",
    platform: "xhs",
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 1,
    payload,
    observedAt,
  });
  capturePackage.header.report.counters.emitted = 2;
  const inventory = JSON.parse(
    Buffer.from(capturePackage.artifacts[0].artifactPayload, "base64").toString("utf8"),
  ) as { schemaVersion: string; candidates: Array<Record<string, unknown>> };
  inventory.candidates.push({
    subject: { kind: "note", noteId, platformContentId: noteId },
    slotId: `note:${noteId}:cover:image:0`,
    purpose: "cover",
    kind: "image",
    ordinal: 0,
    observedAddress: "https://example.invalid/unit-2.jpg",
    coverProvenance: "platform_explicit",
  });
  const artifactBytes = Buffer.from(canonicalJsonString(inventory), "utf8");
  const artifactChecksum = sha256(artifactBytes);
  capturePackage.artifacts[0] = {
    ...capturePackage.artifacts[0],
    artifactPayload: artifactBytes.toString("base64"),
    artifactChecksum,
    contentLength: artifactBytes.length,
  };
  next.artifacts[0].artifactChecksum = artifactChecksum;
  next.snapshot.records.push({
    id: "record-unit-2",
    workspaceId: next.workspaceId,
    rawSnapshotId: next.rawSnapshotId,
    recordKind: "note",
    platform: "xhs",
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 1,
    payloadHash: sha256(canonicalJsonString(payload)),
    observedAt: new Date(observedAt),
    idempotencyKey: "record-key-unit-2",
  });
  next.packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  next.packageChecksumValue = sha256(next.packageBytes);
  next.packageContentLength = next.packageBytes.length;
  next.snapshot.checksumValue = next.packageChecksumValue;
  next.snapshot.contentLength = next.packageContentLength;
  return next;
}

function sourceFor(
  receipt: EvidenceArtifactAuditReceipt,
): EvidenceArtifactAuditSource {
  return { readCaptureArtifactsAndAudit: vi.fn().mockResolvedValue(receipt) };
}

async function validCapability(): Promise<VerifiedMediaArtifacts> {
  const receipt = buildReceipt();
  const result = await new VerifiedArtifactReader(sourceFor(receipt)).readAndVerify({
    workspaceId: receipt.workspaceId,
    rawSnapshotId: receipt.rawSnapshotId,
  });
  if (!result.ok) throw new Error(result.detail);
  return result.verified;
}

function database(overrides: {
  observation?: { id: string; observationKind: string; subjectKey: string } | null;
  createdCount?: number;
  row?: { id: string; status: string; kind: string; ordinal: number };
} = {}) {
  const transaction = vi.fn(async (callback: (tx: Prisma.TransactionClient) => unknown) => callback({
    canonicalObservation: {
      findFirst: vi.fn().mockResolvedValue(overrides.observation === undefined
        ? { id: "observation-unit", observationKind: "note", subjectKey: "xhs:note:note-unit" }
        : overrides.observation),
    },
    canonicalMediaSlot: {
      createMany: vi.fn().mockResolvedValue({ count: overrides.createdCount ?? 1 }),
      findUniqueOrThrow: vi.fn().mockResolvedValue(overrides.row ?? {
        id: "slot-unit",
        status: "observed",
        kind: "image",
        ordinal: 0,
      }),
    },
  } as unknown as Prisma.TransactionClient));
  return { $transaction: transaction } as unknown as Pick<PrismaClient, "$transaction">;
}

describe("B3 verified media capability", () => {
  it("accepts a controlled receipt and writes only its bound observation", async () => {
    const verified = await validCapability();
    await expect(new CanonicalMediaSlotWriter(database()).writeSlots({
      canonicalObservationId: "observation-unit",
      verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
  });

  it("rejects a same-shaped forged capability without opening a transaction", async () => {
    const db = database();
    const forged = Object.create(VerifiedMediaArtifacts.prototype) as VerifiedMediaArtifacts;
    await expect(new CanonicalMediaSlotWriter(db).writeSlots({
      canonicalObservationId: "observation-unit",
      verified: forged,
    })).rejects.toThrow(MediaArtifactInvariantError);
    expect(db.$transaction).not.toHaveBeenCalled();
  });

  it("rejects a receipt whose package is not bound to the snapshot", async () => {
    const receipt = buildReceipt();
    receipt.capturePackageId = "different-package";
    await expect(new VerifiedArtifactReader(sourceFor(receipt)).readAndVerify({
      workspaceId: receipt.workspaceId,
      rawSnapshotId: receipt.rawSnapshotId,
    })).resolves.toMatchObject({ ok: false, reason: "evidence_ineligible" });
  });

  it("rejects changed immutable fields instead of calling it replay", async () => {
    const verified = await validCapability();
    await expect(new CanonicalMediaSlotWriter(database({
      createdCount: 0,
      row: { id: "slot-unit", status: "observed", kind: "video", ordinal: 0 },
    })).writeSlots({ canonicalObservationId: "observation-unit", verified }))
      .rejects.toThrow(/replay fields changed/);
  });

  it("does not convert a controlled-reader infrastructure failure into validation", async () => {
    const source: EvidenceArtifactAuditSource = {
      readCaptureArtifactsAndAudit: vi.fn().mockRejectedValue(new Error("audit database unavailable")),
    };
    await expect(new VerifiedArtifactReader(source).readAndVerify({
      workspaceId: "workspace-unit",
      rawSnapshotId: "snapshot-unit",
    })).rejects.toThrow("audit database unavailable");
  });

  it("selects only the matching note when one package contains multiple subjects", async () => {
    const receipt = addSecondNote(buildReceipt());
    const result = await new VerifiedArtifactReader(sourceFor(receipt)).readAndVerify({
      workspaceId: receipt.workspaceId,
      rawSnapshotId: receipt.rawSnapshotId,
    });
    if (!result.ok) throw new Error(result.detail);

    await expect(new CanonicalMediaSlotWriter(database()).writeSlots({
      canonicalObservationId: "observation-unit",
      verified: result.verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
    await expect(new CanonicalMediaSlotWriter(database({
      observation: { id: "observation-unit-2", observationKind: "note", subjectKey: "xhs:note:note-unit-2" },
    })).writeSlots({
      canonicalObservationId: "observation-unit-2",
      verified: result.verified,
    })).resolves.toMatchObject({ inserted: 1, replayed: 0 });
  });
});
