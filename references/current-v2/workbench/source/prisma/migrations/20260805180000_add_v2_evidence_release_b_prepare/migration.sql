-- V2 B1 / Release-B prepare (expand only)
--
-- This migration prepares RawSnapshot and RawRecord for the V2 EvidenceIngress
-- dark-core without switching any runtime path.  It only relaxes legacy columns
-- to nullable and tightens the RawRecord FK / idempotency scope so that V2
-- writes can coexist with V1 rows until the final hard cutover.
--
-- No append-only triggers, no NOT NULL constraints, no role revokes, and no
-- historical data changes are introduced by this migration.

DO $$
DECLARE
  cross_ws_count integer;
BEGIN
  -- ── Preflight: cross-workspace RawRecord → RawSnapshot references ──────
  --
  -- The new compound FK (workspaceId, rawSnapshotId) → RawSnapshot(workspaceId, id)
  -- will fail if any RawRecord.workspaceId differs from its parent
  -- RawSnapshot.workspaceId.  Check for this anomaly before the DDL.
  --
  -- This migration does NOT fix or rewrite historical data; it only reports
  -- the violation and aborts.

  SELECT count(*) INTO cross_ws_count
  FROM "RawRecord" rr
  LEFT JOIN "RawSnapshot" rs
    ON rs."id" = rr."rawSnapshotId"
  WHERE rs."workspaceId" IS DISTINCT FROM rr."workspaceId";

  IF cross_ws_count > 0 THEN
    RAISE EXCEPTION
      'Release-B prepare blocked: % RawRecord row(s) have a workspaceId '
      'that differs from their parent RawSnapshot.workspaceId. '
      'These rows must be manually investigated and resolved before the '
      'compound FK can be applied. This migration does NOT modify data.',
      cross_ws_count;
  END IF;
END $$;

-- ── RawSnapshot: relax legacy columns to nullable ──────────────────────────

ALTER TABLE "RawSnapshot" ALTER COLUMN "schemaVersion" DROP NOT NULL;
ALTER TABLE "RawSnapshot" ALTER COLUMN "qualityStatus" DROP NOT NULL;
ALTER TABLE "RawSnapshot" ALTER COLUMN "source" DROP NOT NULL;

-- ── RawRecord: relax recordType to nullable ────────────────────────────────

ALTER TABLE "RawRecord" ALTER COLUMN "recordType" DROP NOT NULL;

-- ── RawRecord: replace single-column FK with compound workspace-scoped FK ──

ALTER TABLE "RawRecord" DROP CONSTRAINT "RawRecord_rawSnapshotId_fkey";

ALTER TABLE "RawRecord"
  ADD CONSTRAINT "RawRecord_workspace_snapshot_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId")
  REFERENCES "RawSnapshot"("workspaceId", "id")
  ON DELETE RESTRICT
  ON UPDATE CASCADE;

-- ── RawRecord: scope idempotency to (workspaceId, rawSnapshotId, key) ──────

DROP INDEX "RawRecord_workspaceId_idempotencyKey_key";

CREATE UNIQUE INDEX "RawRecord_workspaceId_rawSnapshotId_idempotencyKey_key"
  ON "RawRecord"("workspaceId", "rawSnapshotId", "idempotencyKey");
