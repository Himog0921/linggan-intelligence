BEGIN;

ALTER TABLE "RawRecord"
  ADD CONSTRAINT "RawRecord_mismatch_probe_key" UNIQUE ("workspaceId", "id");

ALTER TABLE "NormalizationRunCurrent"
  ADD CONSTRAINT "NormalizationRunCurrent_mismatch_probe_fkey"
  FOREIGN KEY ("workspaceId", "rawSnapshotId", "rawRecordId")
  REFERENCES "RawRecord"("workspaceId", "id");

ROLLBACK;
