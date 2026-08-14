// @vitest-environment node

/**
 * Minimal production-seam proof for the V2 XHS Content-only pipeline.
 *
 * The test deliberately starts at the public manual-import orchestrator seam:
 * route authentication remains covered by route tests, while every persisted
 * success fact below is produced by real services against one isolated
 * PostgreSQL database. No mock returns success and no test code writes a
 * Derived, Canonical, Projection, or worker terminal row.
 */
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { createIndependentPrismaClient } from "@/lib/db";
import { PrismaClient } from "@/lib/prisma-client";

import { computeContractHash } from "./contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "./contracts/xhs-collection-contracts";
import {
  B2DerivedService,
  createEvidenceAnalysisReader,
} from "./derived/b2-derived-service";
import { sha256Hex } from "./ingress/base64";
import { canonicalJsonString } from "./ingress/canonical-json";
import { submitManualImportEvidence } from "./ingress/evidence-ingress-orchestrator";
import type {
  CaptureHeaderV2,
  CapturePackagePayloadV2,
  CaptureSubmissionBodyV2,
} from "./ingress/types";
import { CanonicalMediaAdapter } from "./media/canonical-media-adapter";
import { VerifiedArtifactReader } from "./media/verified-artifact-reader";
import { ContentProjectionReadService } from "./projection/content-projection-read-service";
import { ContentProjectionService } from "./projection/content-projection-service";
import { ControlledEvidenceReader } from "./security/controlled-evidence-reader";
import { validateV2DatabaseIdentity } from "./security/v2-database-roles";
import { V2DurableWorker } from "./worker/v2-durable-worker";

const enabled = process.env.V2_CONTENT_PIPELINE_INTEGRATION_DB === "1";
const describeIntegration = enabled ? describe : describe.skip;

describeIntegration("V2 Content pipeline real service and database proof", () => {
  const admin = client(requiredEnv("V2_PIPELINE_ADMIN_DSN"));
  const evidenceWriter = client(requiredEnv("V2_EVIDENCE_WRITER_DSN"));
  const contractReader = client(requiredEnv("V2_CONTRACT_READER_DSN"));
  const canonicalWriter = client(requiredEnv("V2_CANONICAL_WRITER_DSN"));
  const defaultApp = client(requiredEnv("V2_DEFAULT_APP_DSN"));
  const suffix = process.env.V2_PIPELINE_FIXED_SUFFIX?.trim() || `${Date.now()}`;
  const workspaceId = process.env.V2_PIPELINE_FIXED_WORKSPACE_ID?.trim() || `ws-v2-pipeline-${suffix}`;
  const noteId = `note-v2-pipeline-${suffix}`;
  let database = "";

  beforeAll(async () => {
    const rows = await admin.$queryRaw<Array<{ name: string }>>`
      SELECT current_database() AS name
    `;
    database = rows[0]?.name ?? "";
    if (!database.startsWith("content_workbench_v2_rc_e2e_")) {
      throw new Error(`Refusing non-E2E database ${database || "unknown"}`);
    }
    await Promise.all([
      validateV2DatabaseIdentity(evidenceWriter, "evidence_writer"),
      validateV2DatabaseIdentity(contractReader, "contract_reader"),
      validateV2DatabaseIdentity(canonicalWriter, "canonical_writer"),
      validateV2DatabaseIdentity(defaultApp, "default_app"),
    ]);

    // Test fixture authority only. No business or terminal state is seeded.
    if (process.env.V2_PIPELINE_EXPECT_PROVISIONED_GRANT !== "1") {
      await admin.evidenceReaderWorkspaceGrant.create({
        data: {
          workspaceId,
          readerRole: "v2_contract_reader",
          grantedBy: "v2-pipeline-integration",
        },
      });
    }
  });

  afterAll(async () => {
    await Promise.all([
      admin.$disconnect(),
      evidenceWriter.$disconnect(),
      contractReader.$disconnect(),
      canonicalWriter.$disconnect(),
      defaultApp.$disconnect(),
    ]);
    if (database) {
      console.log(`[v2-pipeline] Preserving proof database ${database}.`);
    }
  });

  it("persists one auditable fact chain from ingress through the Projection DTO", async () => {
    const submission = buildSubmission(noteId, suffix);
    const before = await counts(admin, workspaceId);

    const ingress = await submitManualImportEvidence({
      body: submission,
      context: {
        workspaceId,
        userId: `user-${suffix}`,
        pluginAuthorization: {
          id: `plugin-authorization-${suffix}`,
          workspaceId,
        },
      },
    }, { db: evidenceWriter });

    expect(ingress.status).toBe("committed");
    if (ingress.status !== "committed") {
      throw new Error(`Expected committed ingress, received ${ingress.status}`);
    }

    const afterIngress = await counts(admin, workspaceId);
    expect(delta(afterIngress, before)).toMatchObject({
      capturePackages: 1,
      rawSnapshots: 1,
      rawRecords: 1,
      captureArtifacts: 1,
      ingressReceipts: 1,
      durableWork: 1,
      accessAudits: 0,
      normalizationRuns: 0,
      evaluations: 0,
      canonicalObservations: 0,
      contentObservations: 0,
      currentProjections: 0,
    });

    const controlledEvidenceReader = new ControlledEvidenceReader(contractReader);
    const evidenceReader = createEvidenceAnalysisReader(controlledEvidenceReader);
    const b2 = new B2DerivedService(canonicalWriter, evidenceReader);
    const b3 = new ContentProjectionService(
      canonicalWriter,
      new CanonicalMediaAdapter(canonicalWriter),
    );
    const worker = new V2DurableWorker({
      db: canonicalWriter,
      b2Service: b2,
      artifactReader: new VerifiedArtifactReader(controlledEvidenceReader),
      projectionService: b3,
      workerId: `v2-pipeline-proof-${suffix}`,
    });

    await expect(worker.tickReceipt({
      workspaceId,
      receiptId: ingress.receiptId,
    })).resolves.toMatchObject({
      processed: 1,
      errors: 0,
    });

    const work = await admin.v2DurableWork.findFirstOrThrow({
      where: { workspaceId, rawSnapshotId: ingress.rawSnapshotId },
    });
    expect(work).toMatchObject({
      status: "b3_completed",
      attemptCount: 0,
      lockedBy: null,
      lockedAt: null,
      leaseExpiresAt: null,
    });
    expect(work.b2Result).toMatchObject({
      status: "committed",
      decision: "accepted",
    });
    expect(work.b3Result).toMatchObject({ ok: true, replayed: false });

    const afterWorker = await counts(admin, workspaceId);
    expect(delta(afterWorker, before)).toMatchObject({
      capturePackages: 1,
      rawSnapshots: 1,
      rawRecords: 1,
      ingressReceipts: 1,
      durableWork: 1,
      accessAudits: 2,
      normalizationRuns: 1,
      evaluations: 1,
      canonicalObservations: 1,
      contentAssets: 1,
      contentObservations: 1,
      currentProjections: 1,
      canonicalMediaSlots: 1,
      mediaItems: 1,
      mediaOrigins: 1,
      mediaProcessingEvents: 1,
      contentMediaUsages: 1,
    });

    const audits = await admin.evidenceAccessAudit.findMany({
      where: { workspaceId },
      orderBy: { accessReason: "asc" },
    });
    expect(audits).toHaveLength(2);
    expect(audits).toEqual(expect.arrayContaining([
      expect.objectContaining({ accessedBy: "v2_contract_reader", accessReason: "b2_normalization", restricted: false }),
      expect.objectContaining({ accessedBy: "v2_contract_reader", accessReason: "artifact_verification", restricted: false }),
    ]));

    const read = await new ContentProjectionReadService(defaultApp).read({
      workspaceId,
      platform: "xhs",
      platformContentId: noteId,
    });
    expect(read).toMatchObject({
      status: "available",
      projection: {
        schemaVersion: "content-projection/v2",
        workspaceId,
        platform: "xhs",
        platformContentId: noteId,
        title: "V2 pipeline proof",
        bodyText: "One immutable fact traversed the real V2 pipeline.",
        originalUrl: `https://www.xiaohongshu.com/explore/${noteId}`,
        source: "v2",
        cover: { status: "unavailable", reason: "no_deliverable_cover" },
        mediaUsages: [expect.objectContaining({
          purpose: "source_cover",
          canonicalSlotId: `note:${noteId}:cover:image:0`,
          deliveryState: "processing",
        })],
      },
    });

    console.log(`[v2-pipeline] database=${database}`);
    console.log(`[v2-pipeline] workspace=${workspaceId}`);
    console.log(`[v2-pipeline] counts=${JSON.stringify(afterWorker)}`);
  }, 60_000);
});

function client(dsn: string): PrismaClient {
  return createIndependentPrismaClient(dsn);
}

function delta(
  after: Record<string, number>,
  before: Record<string, number>,
): Record<string, number> {
  return Object.fromEntries(Object.entries(after).map(([key, value]) => [
    key,
    value - (before[key] ?? 0),
  ]));
}

function requiredEnv(name: string): string {
  if (!enabled) return "postgresql://disabled";
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required when V2_CONTENT_PIPELINE_INTEGRATION_DB=1`);
  return value;
}

function buildSubmission(noteId: string, suffix: string): CaptureSubmissionBodyV2 {
  const observedAt = "2026-08-12T08:00:00.000Z";
  const targetKey = `xhs:note/${noteId}`;
  const header: CaptureHeaderV2 = {
    protocolVersion: "capture-submission/v2",
    ingressKind: "manual_import",
    captureId: `capture-v2-pipeline-${suffix}`,
    platform: "xhs",
    target: {
      expectedTargetKey: targetKey,
      observedTargetKey: targetKey,
    },
    observedAt,
    collectorVersion: "v2-pipeline-proof/1",
    contractId: XHS_NOTE_DETAIL.id,
    contractVersion: XHS_NOTE_DETAIL.version,
    contractHash: computeContractHash(XHS_NOTE_DETAIL),
    sourceSummary: "V2 isolated pipeline proof",
    report: {
      startedAt: "2026-08-12T07:59:00.000Z",
      completedAt: observedAt,
      terminal: {
        state: "completed",
        reason: "source_exhausted",
        retryable: false,
      },
      slots: [
        { slotId: "note", status: "observed", reason: null },
        { slotId: "comments", status: "not_applicable", reason: "not_requested" },
      ],
      counters: {
        requested: 1,
        discovered: 1,
        emitted: 1,
        deduplicated: 0,
        failed: 0,
      },
      diagnostics: {},
    },
  };
  const record: CapturePackagePayloadV2["records"][number] = {
    idempotencyKey: `record-v2-pipeline-${suffix}`,
    recordKind: "note",
    platform: "xhs",
    targetKey,
    externalRecordId: noteId,
    sequence: 0,
    payload: {
      noteId,
      platformContentId: noteId,
      type: "normal",
      title: "V2 pipeline proof",
      content: "One immutable fact traversed the real V2 pipeline.",
      url: `https://www.xiaohongshu.com/explore/${noteId}`,
    },
    observedAt,
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records: [record],
    artifacts: [producerMediaInventoryArtifact(noteId)],
  };
  const packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  return {
    header,
    capturePackage: {
      encoding: "base64",
      packagePayload: packageBytes.toString("base64"),
      checksumAlgorithm: "sha256",
      checksumValue: sha256Hex(packageBytes),
      contentLength: packageBytes.length,
      restricted: false,
    },
  };
}

/** Mirrors the real XHS terminal producer's media_inventory artifact contract. */
function producerMediaInventoryArtifact(noteId: string): CapturePackagePayloadV2["artifacts"][number] {
  const inventory = {
    schemaVersion: "xhs.media-inventory/v2",
    candidates: [{
      subject: { kind: "note", noteId, platformContentId: noteId },
      slotId: `note:${noteId}:cover:image:0`,
      purpose: "cover",
      kind: "image",
      ordinal: 0,
      observedAddress: `https://sns-webpic-qc.xhscdn.com/${noteId}.jpg`,
      coverProvenance: "platform_explicit",
    }],
  };
  const bytes = Buffer.from(canonicalJsonString(inventory), "utf8");
  return {
    kind: "media_inventory",
    encoding: "base64",
    artifactPayload: bytes.toString("base64"),
    artifactChecksum: sha256Hex(bytes),
    contentLength: bytes.length,
    restricted: false,
  };
}

async function counts(db: PrismaClient, workspaceId: string) {
  const [
    capturePackages,
    rawSnapshots,
    rawRecords,
    captureArtifacts,
    ingressReceipts,
    durableWork,
    accessAudits,
    normalizationRuns,
    evaluations,
    canonicalObservations,
    contentAssets,
    contentObservations,
    currentProjections,
    canonicalMediaSlots,
    mediaItems,
    mediaOrigins,
    mediaProcessingEvents,
    contentMediaUsages,
  ] = await Promise.all([
    db.capturePackage.count({ where: { workspaceId } }),
    db.rawSnapshot.count({ where: { workspaceId } }),
    db.rawRecord.count({ where: { workspaceId } }),
    db.captureArtifact.count({ where: { workspaceId } }),
    db.evidenceIngressReceipt.count({ where: { workspaceId } }),
    db.v2DurableWork.count({ where: { workspaceId } }),
    db.evidenceAccessAudit.count({ where: { workspaceId } }),
    db.normalizationRun.count({ where: { workspaceId } }),
    db.contractEvaluation.count({ where: { workspaceId } }),
    db.canonicalObservation.count({ where: { workspaceId } }),
    db.contentAsset.count({ where: { workspaceId } }),
    db.contentObservation.count({ where: { workspaceId } }),
    db.contentCurrentProjection.count({ where: { workspaceId } }),
    db.canonicalMediaSlot.count({ where: { workspaceId } }),
    db.mediaItem.count({ where: { workspaceId } }),
    db.mediaOrigin.count({ where: { workspaceId } }),
    db.outboxEvent.count({ where: { workspaceId, eventType: "media.process.requested" } }),
    db.contentMediaUsage.count({ where: { workspaceId } }),
  ]);
  return {
    capturePackages,
    rawSnapshots,
    rawRecords,
    captureArtifacts,
    ingressReceipts,
    durableWork,
    accessAudits,
    normalizationRuns,
    evaluations,
    canonicalObservations,
    contentAssets,
    contentObservations,
    currentProjections,
    canonicalMediaSlots,
    mediaItems,
    mediaOrigins,
    mediaProcessingEvents,
    contentMediaUsages,
  };
}
