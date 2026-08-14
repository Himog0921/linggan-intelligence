import { createHash } from "node:crypto";

import { buildContentAssetKey, buildContentCode } from "@/lib/data-foundation/content-identity";
import type { PrismaClient } from "@/lib/prisma-client";

import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "../contracts/xhs-collection-contracts";
import { canonicalJsonString } from "../ingress/canonical-json";
import type { CapturePackagePayloadV2 } from "../ingress/types";
import { CanonicalMediaSlotWriter } from "../media/canonical-media-slot-writer";
import {
  type EvidenceArtifactAuditReceipt,
  type EvidenceArtifactAuditSource,
  VerifiedArtifactReader,
  type VerifiedMediaArtifacts,
} from "../media/verified-artifact-reader";

export type ProofCandidate = {
  subject: { kind: "note"; noteId: string; platformContentId: string };
  slotId: string;
  purpose: "cover" | "body" | "video" | "live_photo";
  kind: "image" | "video" | "live_photo";
  ordinal: number;
  observedAddress: string;
  coverProvenance: "platform_explicit" | "first_observed_image" | "not_cover";
};

export function proofCandidate(
  noteId: string,
  input: Partial<Omit<ProofCandidate, "subject" | "slotId">> = {},
): ProofCandidate {
  const purpose = input.purpose ?? "cover";
  const kind = input.kind ?? "image";
  const ordinal = input.ordinal ?? 0;
  return {
    subject: { kind: "note", noteId, platformContentId: noteId },
    slotId: `note:${noteId}:${purpose}:${kind}:${ordinal}`,
    purpose,
    kind,
    ordinal,
    observedAddress: input.observedAddress ?? `https://media.invalid/${noteId}/${purpose}-${ordinal}`,
    coverProvenance: input.coverProvenance ?? (purpose === "cover" ? "platform_explicit" : "not_cover"),
  };
}

export type ProofFixture = {
  label: string;
  workspaceId: string;
  rawSnapshotId: string;
  rawRecordId: string;
  canonicalObservationId: string;
  evaluationId: string;
  noteId: string;
  candidates: ProofCandidate[];
  capturePackageId: string;
};

type SeedOptions = {
  workspaceId?: string;
  noteId?: string;
  observedAt?: string;
  url?: string | null;
  contentType?: "normal" | "video" | string;
  title?: string;
  bodyText?: string;
  platformContentId?: string;
  decision?: "accepted" | "rejected";
  completeness?: "full" | "partial" | "not_applicable";
  candidates?: ProofCandidate[];
  installCurrent?: boolean;
};

export async function seedProofEvaluation(
  db: PrismaClient,
  label: string,
  options: SeedOptions = {},
): Promise<ProofFixture> {
  const workspaceId = options.workspaceId ?? `ws-${label}`;
  const noteId = options.noteId ?? `note-${label}`;
  const observedAt = options.observedAt ?? "2026-08-11T08:00:00.000Z";
  const rawSnapshotId = `snapshot-${label}`;
  const rawRecordId = `record-${label}`;
  const normalizationRunId = `run-${label}`;
  const canonicalObservationId = `observation-${label}`;
  const evaluationId = `evaluation-${label}`;
  const capturePackageId = `package-${label}`;
  const artifactId = `artifact-${label}`;
  const candidates = options.candidates ?? [];
  const sourcePayload: Record<string, string> = {
    noteId,
    platformContentId: options.platformContentId ?? noteId,
    type: options.contentType ?? "normal",
    title: options.title ?? `title-${label}`,
    content: options.bodyText ?? `body-${label}`,
  };
  if (options.url !== null) {
    sourcePayload.url = options.url ?? `https://www.xiaohongshu.com/explore/${noteId}`;
  }
  const recordPayload = { ...sourcePayload };
  const artifactPayload = { schemaVersion: "xhs.media-inventory/v2", candidates };
  const artifactBytes = Buffer.from(canonicalJsonString(artifactPayload), "utf8");
  const artifactChecksum = sha256(artifactBytes);
  const contractHash = computeContractHash(XHS_NOTE_DETAIL);
  const record: CapturePackagePayloadV2["records"][number] = {
    idempotencyKey: `key-${label}`,
    recordKind: "note",
    platform: "xhs",
    targetKey: `xhs:note/${noteId}`,
    externalRecordId: noteId,
    sequence: 0,
    payload: recordPayload,
    observedAt,
  };
  const header: CapturePackagePayloadV2["header"] = {
    protocolVersion: "capture-submission/v2",
    captureId: `capture-${label}`,
    platform: "xhs",
    target: { expectedTargetKey: record.targetKey ?? "", observedTargetKey: record.targetKey },
    observedAt,
    collectorVersion: "b3-projection-proof-v1",
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
    sourceSummary: "B3 Projection isolated proof",
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records: [record],
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
    rawSnapshotId, workspaceId, header.captureId, record.targetKey, new Date(observedAt),
    capturePackageId, packageChecksum, packageBytes.length, XHS_NOTE_DETAIL.id,
    XHS_NOTE_DETAIL.version, contractHash,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "RawRecord" ("id","workspaceId","rawSnapshotId","recordKind","platform","targetKey","externalRecordId","sequence","payload","payloadHash","observedAt","idempotencyKey") VALUES ($1,$2,$3,'note','xhs',$4,$5,0,$6::jsonb,$7,$8,$9)`,
    rawRecordId, workspaceId, rawSnapshotId, record.targetKey, record.externalRecordId,
    JSON.stringify(recordPayload), sha256(canonicalJsonString(recordPayload)), new Date(observedAt), record.idempotencyKey,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "NormalizationRun" ("id","workspaceId","rawSnapshotId","rawRecordId","adapterId","adapterVersion","canonicalSchemaVersion","status","inputPayloadHash","outputPayloadHash") VALUES ($1,$2,$3,$4,'xhs.note','2.0.0','xhs.canonical/2','normalized',$5,$6)`,
    normalizationRunId, workspaceId, rawSnapshotId, rawRecordId,
    sha256(canonicalJsonString(recordPayload)), sha256(canonicalJsonString(sourcePayload)),
  );
  const canonicalPayload = {
    platform: "xhs",
    recordKind: "note",
    subjectKey: `xhs:note:${noteId}`,
    sourcePayload,
  };
  await db.$executeRawUnsafe(
    `INSERT INTO "CanonicalObservation" ("id","workspaceId","normalizationRunId","rawSnapshotId","rawRecordId","observationKind","subjectKey","observedAt","schemaVersion","payload","payloadHash","qualityStatus","fieldPresence") VALUES ($1,$2,$3,$4,$5,'note',$6,$7,'xhs.canonical/2',$8::jsonb,$9,'complete',$10::jsonb)`,
    canonicalObservationId, workspaceId, normalizationRunId, rawSnapshotId, rawRecordId,
    `xhs:note:${encodeURIComponent(noteId)}`, new Date(observedAt), JSON.stringify(canonicalPayload),
    sha256(canonicalJsonString(canonicalPayload)), JSON.stringify({ title: true, content: true, type: true, url: options.url !== null }),
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "CaptureArtifact" ("id","workspaceId","rawSnapshotId","kind","artifactChecksum","restricted") VALUES ($1,$2,$3,'media_inventory',$4,false)`,
    artifactId, workspaceId, rawSnapshotId, artifactChecksum,
  );
  const decision = options.decision ?? "accepted";
  const completeness = options.completeness ?? (decision === "accepted" ? "full" : "not_applicable");
  await db.$executeRawUnsafe(
    `INSERT INTO "ContractEvaluation" ("id","workspaceId","rawSnapshotId","contractId","contractVersion","evaluatorVersion","canonicalSchemaVersion","evaluationInputHash","decision","completeness") VALUES ($1,$2,$3,$4,$5,'proof-v1','xhs.canonical/2',$6,$7,$8)`,
    evaluationId, workspaceId, rawSnapshotId, XHS_NOTE_DETAIL.id, XHS_NOTE_DETAIL.version,
    sha256(label), decision, completeness,
  );
  if (decision === "accepted") {
    await db.$executeRawUnsafe(
      `INSERT INTO "ContractEvaluationInput" ("id","workspaceId","rawSnapshotId","contractEvaluationId","canonicalObservationId","ordinal","canonicalOutputHash") VALUES ($1,$2,$3,$4,$5,0,$6)`,
      `input-${label}`, workspaceId, rawSnapshotId, evaluationId, canonicalObservationId,
      sha256(canonicalJsonString(canonicalPayload)),
    );
  }
  if (options.installCurrent !== false) {
    await db.contractEvaluationCurrent.create({
      data: {
        workspaceId,
        rawSnapshotId,
        contractId: XHS_NOTE_DETAIL.id,
        contractVersion: XHS_NOTE_DETAIL.version,
        contractEvaluationId: evaluationId,
        revision: 1,
      },
    });
  }
  return {
    label, workspaceId, rawSnapshotId, rawRecordId, canonicalObservationId,
    evaluationId, noteId, candidates, capturePackageId,
  };
}

export async function installCurrentWithRevocation(
  db: PrismaClient,
  fixture: ProofFixture,
  previousEvaluationId: string | null,
): Promise<void> {
  await db.$transaction(async (tx) => {
    await tx.$queryRaw`
      SELECT "revoke_content_projection_for_evaluation"(
        ${fixture.workspaceId}, ${previousEvaluationId}, ${fixture.evaluationId}
      )
    `;
    await tx.contractEvaluationCurrent.create({
      data: {
        workspaceId: fixture.workspaceId,
        rawSnapshotId: fixture.rawSnapshotId,
        contractId: XHS_NOTE_DETAIL.id,
        contractVersion: XHS_NOTE_DETAIL.version,
        contractEvaluationId: fixture.evaluationId,
        revision: 1,
      },
    });
  });
}

export async function addNoteInputToEvaluation(
  db: PrismaClient,
  fixture: ProofFixture,
  suffix: string,
  noteId: string,
  observedAt: string,
): Promise<string> {
  const rawRecordId = `record-${suffix}`;
  const normalizationRunId = `run-${suffix}`;
  const canonicalObservationId = `observation-${suffix}`;
  const recordPayload = {
    noteId,
    platformContentId: noteId,
    type: "normal",
    title: `title-${suffix}`,
    content: `body-${suffix}`,
    url: `https://www.xiaohongshu.com/explore/${noteId}`,
  };
  const canonicalPayload = {
    platform: "xhs",
    recordKind: "note",
    subjectKey: `xhs:note:${noteId}`,
    sourcePayload: recordPayload,
  };
  const inputHash = sha256(canonicalJsonString(recordPayload));
  const outputHash = sha256(canonicalJsonString(canonicalPayload));
  await db.$executeRawUnsafe(
    `INSERT INTO "RawRecord" ("id","workspaceId","rawSnapshotId","recordKind","platform","targetKey","externalRecordId","sequence","payload","payloadHash","observedAt","idempotencyKey") VALUES ($1,$2,$3,'note','xhs',$4,$5,1,$6::jsonb,$7,$8,$9)`,
    rawRecordId, fixture.workspaceId, fixture.rawSnapshotId, `xhs:note/${noteId}`, noteId,
    JSON.stringify(recordPayload), inputHash, new Date(observedAt), `key-${suffix}`,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "NormalizationRun" ("id","workspaceId","rawSnapshotId","rawRecordId","adapterId","adapterVersion","canonicalSchemaVersion","status","inputPayloadHash","outputPayloadHash") VALUES ($1,$2,$3,$4,'xhs.note','2.0.0','xhs.canonical/2','normalized',$5,$6)`,
    normalizationRunId, fixture.workspaceId, fixture.rawSnapshotId, rawRecordId, inputHash, outputHash,
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "CanonicalObservation" ("id","workspaceId","normalizationRunId","rawSnapshotId","rawRecordId","observationKind","subjectKey","observedAt","schemaVersion","payload","payloadHash","qualityStatus","fieldPresence") VALUES ($1,$2,$3,$4,$5,'note',$6,$7,'xhs.canonical/2',$8::jsonb,$9,'complete',$10::jsonb)`,
    canonicalObservationId, fixture.workspaceId, normalizationRunId, fixture.rawSnapshotId,
    rawRecordId, `xhs:note:${encodeURIComponent(noteId)}`, new Date(observedAt),
    JSON.stringify(canonicalPayload), outputHash,
    JSON.stringify({ title: true, content: true, type: true, url: true }),
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "ContractEvaluationInput" ("id","workspaceId","rawSnapshotId","contractEvaluationId","canonicalObservationId","ordinal","canonicalOutputHash") VALUES ($1,$2,$3,$4,$5,1,$6)`,
    `input-${suffix}`, fixture.workspaceId, fixture.rawSnapshotId, fixture.evaluationId,
    canonicalObservationId, outputHash,
  );
  return canonicalObservationId;
}

export async function seedProjectionObservationWithoutCurrent(
  db: PrismaClient,
  fixture: ProofFixture,
  existingContentAssetId?: string,
): Promise<{ contentAssetId: string; observationId: string }> {
  const contentAssetId = existingContentAssetId ?? `asset-${fixture.label}`;
  const observationId = `content-observation-${fixture.label}`;
  const assetKey = buildContentAssetKey({
    workspaceId: fixture.workspaceId,
    platform: "xhs",
    platformContentId: fixture.noteId,
  });
  const contentCode = buildContentCode({ platform: "xhs", platformContentId: fixture.noteId });
  if (!assetKey || !contentCode) throw new Error("Proof identity could not produce ContentAsset keys.");
  if (!existingContentAssetId) {
    await db.contentAsset.create({
      data: {
        id: contentAssetId,
        workspaceId: fixture.workspaceId,
        platform: "xhs",
        platformContentId: fixture.noteId,
        assetKey,
        contentCode,
        contentType: "normal",
        contentKind: "image_text",
      },
    });
  }
  await db.$executeRawUnsafe(
    `INSERT INTO "ContentObservation" (
      "id","workspaceId","contentAssetId","canonicalObservationId","rawSnapshotId","rawRecordId",
      "observedAt","contentType","title","bodyText","publishedAt","authorId","originalUrl",
      "fieldPresence","qualityStatus","adapterVersion"
    )
    SELECT $1,co."workspaceId",$2,co."id",co."rawSnapshotId",co."rawRecordId",co."observedAt",
      co."payload"#>>'{sourcePayload,type}',co."payload"#>>'{sourcePayload,title}',
      co."payload"#>>'{sourcePayload,content}',NULL,NULL,co."payload"#>>'{sourcePayload,url}',
      co."fieldPresence",co."qualityStatus",nr."adapterVersion"
    FROM "CanonicalObservation" co
    JOIN "NormalizationRun" nr
      ON nr."workspaceId"=co."workspaceId" AND nr."rawSnapshotId"=co."rawSnapshotId"
     AND nr."id"=co."normalizationRunId"
    WHERE co."workspaceId"=$3 AND co."id"=$4`,
    observationId, contentAssetId, fixture.workspaceId, fixture.canonicalObservationId,
  );
  return { contentAssetId, observationId };
}

export async function seedAcceptedReevaluation(
  db: PrismaClient,
  fixture: ProofFixture,
  suffix: string,
): Promise<ProofFixture> {
  const evaluationId = `${fixture.evaluationId}-${suffix}`;
  await db.$executeRawUnsafe(
    `INSERT INTO "ContractEvaluation" ("id","workspaceId","rawSnapshotId","contractId","contractVersion","evaluatorVersion","canonicalSchemaVersion","evaluationInputHash","decision","completeness") VALUES ($1,$2,$3,$4,$5,$6,'xhs.canonical/2',$7,'accepted','full')`,
    evaluationId, fixture.workspaceId, fixture.rawSnapshotId, XHS_NOTE_DETAIL.id,
    XHS_NOTE_DETAIL.version, `proof-${suffix}`, sha256(evaluationId),
  );
  await db.$executeRawUnsafe(
    `INSERT INTO "ContractEvaluationInput" ("id","workspaceId","rawSnapshotId","contractEvaluationId","canonicalObservationId","ordinal","canonicalOutputHash") VALUES ($1,$2,$3,$4,$5,0,$6)`,
    `input-${fixture.label}-${suffix}`, fixture.workspaceId, fixture.rawSnapshotId,
    evaluationId, fixture.canonicalObservationId, sha256(fixture.canonicalObservationId),
  );
  await db.$transaction(async (tx) => {
    await tx.$queryRaw`
      SELECT "revoke_content_projection_for_evaluation"(
        ${fixture.workspaceId}, ${fixture.evaluationId}, ${evaluationId}
      )
    `;
    await tx.contractEvaluationCurrent.update({
      where: {
        workspaceId_rawSnapshotId_contractId_contractVersion: {
          workspaceId: fixture.workspaceId,
          rawSnapshotId: fixture.rawSnapshotId,
          contractId: XHS_NOTE_DETAIL.id,
          contractVersion: XHS_NOTE_DETAIL.version,
        },
      },
      data: { contractEvaluationId: evaluationId, revision: { increment: 1 } },
    });
  });
  return { ...fixture, evaluationId };
}

export async function seedRejectedReevaluation(
  db: PrismaClient,
  fixture: ProofFixture,
  suffix: string,
): Promise<ProofFixture> {
  const evaluationId = `${fixture.evaluationId}-${suffix}`;
  await db.$executeRawUnsafe(
    `INSERT INTO "ContractEvaluation" ("id","workspaceId","rawSnapshotId","contractId","contractVersion","evaluatorVersion","canonicalSchemaVersion","evaluationInputHash","decision","completeness") VALUES ($1,$2,$3,$4,$5,$6,'xhs.canonical/2',$7,'rejected','not_applicable')`,
    evaluationId, fixture.workspaceId, fixture.rawSnapshotId, XHS_NOTE_DETAIL.id,
    XHS_NOTE_DETAIL.version, `proof-${suffix}`, sha256(evaluationId),
  );
  await db.$transaction(async (tx) => {
    await tx.$queryRaw`
      SELECT "revoke_content_projection_for_evaluation"(
        ${fixture.workspaceId}, ${fixture.evaluationId}, ${evaluationId}
      )
    `;
    await tx.contractEvaluationCurrent.update({
      where: {
        workspaceId_rawSnapshotId_contractId_contractVersion: {
          workspaceId: fixture.workspaceId,
          rawSnapshotId: fixture.rawSnapshotId,
          contractId: XHS_NOTE_DETAIL.id,
          contractVersion: XHS_NOTE_DETAIL.version,
        },
      },
      data: { contractEvaluationId: evaluationId, revision: { increment: 1 } },
    });
  });
  return { ...fixture, evaluationId };
}

export async function prepareVerifiedMedia(
  db: PrismaClient,
  fixture: ProofFixture,
): Promise<VerifiedMediaArtifacts> {
  const result = await readProofVerifiedMedia(db, fixture);
  await new CanonicalMediaSlotWriter(db).writeSlots({
    canonicalObservationId: fixture.canonicalObservationId,
    verified: result,
  });
  return result;
}

export async function readProofVerifiedMedia(
  db: PrismaClient,
  fixture: ProofFixture,
): Promise<VerifiedMediaArtifacts> {
  const result = await new VerifiedArtifactReader(auditSource(db, fixture)).readAndVerify({
    workspaceId: fixture.workspaceId,
    rawSnapshotId: fixture.rawSnapshotId,
  });
  if (!result.ok) throw new Error(`${result.reason}: ${result.detail}`);
  return result.verified;
}

function auditSource(db: PrismaClient, fixture: ProofFixture): EvidenceArtifactAuditSource {
  return {
    async readCaptureArtifactsAndAudit(input) {
      const snapshot = await db.rawSnapshot.findFirstOrThrow({
        where: { id: input.rawSnapshotId, workspaceId: input.workspaceId },
        select: {
          id: true, workspaceId: true, captureId: true, platform: true,
          integrityStatus: true, checksumAlgorithm: true, checksumValue: true,
          contentLength: true, contractId: true, contractVersion: true,
          contractHash: true, capturePackageId: true,
          records: { select: {
            id: true, workspaceId: true, rawSnapshotId: true, recordKind: true,
            platform: true, targetKey: true, externalRecordId: true, sequence: true,
            payload: true, payloadHash: true, observedAt: true, idempotencyKey: true,
          } },
          captureArtifacts: { select: {
            id: true, workspaceId: true, rawSnapshotId: true, kind: true,
            artifactChecksum: true, restricted: true,
          } },
        },
      });
      if (!snapshot.capturePackageId) throw new Error("Proof snapshot has no package.");
      const [packaged] = await db.$queryRaw<Array<{
        packagePayload: Uint8Array;
        checksumAlgorithm: string;
        checksumValue: string;
        contentLength: number;
      }>>`
        SELECT "packagePayload", "checksumAlgorithm", "checksumValue", "contentLength"
        FROM "CapturePackage"
        WHERE "workspaceId" = ${input.workspaceId} AND "id" = ${snapshot.capturePackageId}
      `;
      if (!packaged) throw new Error("Proof package is missing.");
      const receipt: EvidenceArtifactAuditReceipt = {
        workspaceId: snapshot.workspaceId,
        rawSnapshotId: snapshot.id,
        capturePackageId: snapshot.capturePackageId,
        accessAuditId: `audit-${fixture.label}`,
        lifecycleStatus: "ACTIVE",
        integrityStatus: snapshot.integrityStatus ?? "",
        packageBytes: Uint8Array.from(packaged.packagePayload),
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

function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}
