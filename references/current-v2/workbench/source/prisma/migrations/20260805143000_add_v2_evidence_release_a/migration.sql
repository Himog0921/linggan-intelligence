-- V2 B1 / Release-A (expand only)
--
-- This migration is deliberately additive.  It creates the immutable capture
-- package/receipt/artifact structures and adds nullable V2 ingress metadata to
-- existing raw tables.  No runtime path is switched by this migration and no
-- historical row is inferred, rewritten, or deleted.

CREATE TABLE "CapturePackage" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "packagePayload" BYTEA NOT NULL,
    "checksumAlgorithm" TEXT NOT NULL DEFAULT 'sha256',
    "checksumValue" TEXT NOT NULL,
    "contentLength" INTEGER NOT NULL,
    "restricted" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "CapturePackage_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "CapturePackage_workspaceId_id_key"
  ON "CapturePackage"("workspaceId", "id");

-- V2 submission kinds other than execution must never invent a job or attempt.
-- Existing V1 writers still provide both values; this only relaxes the old
-- execution-only representation before the Release-B hard cutover.
ALTER TABLE "RawSnapshot"
  DROP CONSTRAINT "RawSnapshot_jobId_fkey",
  ALTER COLUMN "jobId" DROP NOT NULL,
  ALTER COLUMN "attemptId" DROP NOT NULL;

ALTER TABLE "RawSnapshot"
  ADD COLUMN "ingressKind" TEXT,
  ADD COLUMN "capturePackageId" TEXT,
  ADD COLUMN "protocolVersion" TEXT,
  ADD COLUMN "collectorVersion" TEXT,
  ADD COLUMN "sourcePrincipal" TEXT,
  ADD COLUMN "sourceSummary" TEXT,
  ADD COLUMN "importerIdentity" TEXT,
  ADD COLUMN "migrationAuthorization" TEXT,
  ADD COLUMN "recoveryCaptureId" TEXT,
  ADD COLUMN "recoveryAuthorizedBy" TEXT,
  ADD COLUMN "leaseEpoch" INTEGER,
  ADD COLUMN "executionPlanVersion" TEXT,
  ADD COLUMN "integrityStatus" TEXT,
  ADD COLUMN "integrityReason" TEXT,
  ADD COLUMN "contractId" TEXT,
  ADD COLUMN "contractVersion" INTEGER,
  ADD COLUMN "contractHash" TEXT;

-- Preserve the V1 identity contract for rows that have no CapturePackage,
-- while allowing V2 to retain conflicting package variants.  The V2 variant
-- key makes identical package replays converge, and the verified key prevents
-- more than one variant for the same capture identity from becoming eligible
-- for normalization/projection.
DROP INDEX "RawSnapshot_workspaceId_captureId_key";

CREATE UNIQUE INDEX "RawSnapshot_workspaceId_captureId_key"
  ON "RawSnapshot"("workspaceId", "captureId")
  WHERE "capturePackageId" IS NULL;

CREATE UNIQUE INDEX "RawSnapshot_v2_capture_variant_key"
  ON "RawSnapshot"("workspaceId", "captureId", "checksumAlgorithm", "checksumValue")
  WHERE "capturePackageId" IS NOT NULL;

CREATE UNIQUE INDEX "RawSnapshot_v2_verified_capture_key"
  ON "RawSnapshot"("workspaceId", "captureId")
  WHERE "capturePackageId" IS NOT NULL AND "integrityStatus" = 'verified';

CREATE UNIQUE INDEX "RawSnapshot_workspaceId_id_key"
  ON "RawSnapshot"("workspaceId", "id");

ALTER TABLE "RawSnapshot"
  ADD CONSTRAINT "RawSnapshot_workspace_capturePackage_fkey"
  FOREIGN KEY ("workspaceId", "capturePackageId")
  REFERENCES "CapturePackage"("workspaceId", "id")
  ON DELETE RESTRICT
  ON UPDATE CASCADE;

ALTER TABLE "RawRecord"
  ALTER COLUMN "jobId" DROP NOT NULL,
  ADD COLUMN "recordKind" TEXT;

CREATE UNIQUE INDEX "RawRecord_workspaceId_rawSnapshotId_id_key"
  ON "RawRecord"("workspaceId", "rawSnapshotId", "id");

CREATE TABLE "EvidenceIngressReceipt" (
    "id" TEXT NOT NULL DEFAULT gen_random_uuid()::text,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "captureId" TEXT NOT NULL,
    "ingressKind" TEXT NOT NULL,
    "collectorVersion" TEXT NOT NULL,
    "receivedAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "EvidenceIngressReceipt_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "EvidenceIngressReceipt_workspaceId_rawSnapshotId_key"
  ON "EvidenceIngressReceipt"("workspaceId", "rawSnapshotId");

CREATE UNIQUE INDEX "EvidenceIngressReceipt_workspaceId_id_key"
  ON "EvidenceIngressReceipt"("workspaceId", "id");

CREATE INDEX "EvidenceIngressReceipt_workspaceId_receivedAt_idx"
  ON "EvidenceIngressReceipt"("workspaceId", "receivedAt");

ALTER TABLE "EvidenceIngressReceipt"
  ADD CONSTRAINT "EvidenceIngressReceipt_workspace_snapshot_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId")
  REFERENCES "RawSnapshot"("workspaceId", "id")
  ON DELETE RESTRICT
  ON UPDATE CASCADE;

CREATE TABLE "CaptureArtifact" (
    "id" TEXT NOT NULL,
    "workspaceId" TEXT NOT NULL,
    "rawSnapshotId" TEXT NOT NULL,
    "kind" TEXT NOT NULL,
    "artifactChecksum" TEXT NOT NULL,
    "restricted" BOOLEAN NOT NULL DEFAULT false,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "CaptureArtifact_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "CaptureArtifact_workspaceId_id_key"
  ON "CaptureArtifact"("workspaceId", "id");

CREATE UNIQUE INDEX "CaptureArtifact_workspaceId_rawSnapshotId_id_key"
  ON "CaptureArtifact"("workspaceId", "rawSnapshotId", "id");

ALTER TABLE "CaptureArtifact"
  ADD CONSTRAINT "CaptureArtifact_workspace_snapshot_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId")
  REFERENCES "RawSnapshot"("workspaceId", "id")
  ON DELETE RESTRICT
  ON UPDATE CASCADE;

-- New Evidence structures are append-only from their first deployed row.
-- RawSnapshot and RawRecord are intentionally not protected here: their V1
-- writers are still live until the B1 hard cutover removes update/delete paths.
CREATE OR REPLACE FUNCTION "reject_v2_evidence_mutation"()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'V2 Evidence rows are append-only: %.% is not allowed', TG_TABLE_NAME, TG_OP
    USING ERRCODE = '55000';
END;
$$;

CREATE TRIGGER "CapturePackage_append_only"
  BEFORE UPDATE OR DELETE ON "CapturePackage"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_evidence_mutation"();

CREATE TRIGGER "EvidenceIngressReceipt_append_only"
  BEFORE UPDATE OR DELETE ON "EvidenceIngressReceipt"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_evidence_mutation"();

CREATE TRIGGER "CaptureArtifact_append_only"
  BEFORE UPDATE OR DELETE ON "CaptureArtifact"
  FOR EACH ROW EXECUTE FUNCTION "reject_v2_evidence_mutation"();
