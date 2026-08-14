BEGIN;

INSERT INTO "RawSnapshot"
  ("id", "workspaceId", "captureId", "platform", "targetKey", "observedAt")
VALUES
  ('snapshot-negative', 'workspace-a', 'capture-negative', 'xhs', 'xhs:note:negative', now());

INSERT INTO "RawRecord"
  ("id", "workspaceId", "rawSnapshotId", "recordKind", "platform", "payload",
   "payloadHash", "observedAt", "idempotencyKey")
VALUES
  ('record-negative', 'workspace-a', 'snapshot-negative', 'note', 'xhs', '{}'::jsonb,
   'raw-hash-negative', now(), 'record-key-negative');

INSERT INTO "NormalizationRun"
  ("id", "workspaceId", "rawSnapshotId", "rawRecordId", "adapterId", "adapterVersion",
   "canonicalSchemaVersion", "status", "inputPayloadHash", "outputPayloadHash")
VALUES
  ('run-secondary', 'workspace-a', 'snapshot-a', 'record-a', 'xhs-note-secondary', '1',
   '1', 'normalized', 'raw-hash-a', 'canonical-hash-secondary');

DO $proof$
DECLARE
  rejected_constraint TEXT;
BEGIN
  BEGIN
    INSERT INTO "NormalizationRunCurrent"
      ("workspaceId", "rawRecordId", "rawSnapshotId", "normalizationRunId", "adapterId",
       "adapterVersion", "canonicalSchemaVersion")
    VALUES
      ('workspace-a', 'record-negative', 'snapshot-negative', 'run-secondary', 'xhs-note', '1', '1');
    RAISE EXCEPTION 'ASSERT_FAIL: cross-snapshot run was accepted';
  EXCEPTION WHEN foreign_key_violation THEN
    GET STACKED DIAGNOSTICS rejected_constraint = CONSTRAINT_NAME;
    IF rejected_constraint <> 'NormalizationRunCurrent_workspace_snapshot_run_fkey' THEN
      RAISE EXCEPTION 'ASSERT_FAIL: wrong run constraint %', rejected_constraint;
    END IF;
    RAISE NOTICE 'EXPECTED_REJECTION SQLSTATE=23503 constraint=%', rejected_constraint;
  END;

  BEGIN
    INSERT INTO "NormalizationRunCurrent"
      ("workspaceId", "rawRecordId", "rawSnapshotId", "normalizationRunId", "adapterId",
       "adapterVersion", "canonicalSchemaVersion")
    VALUES
      ('workspace-a', 'record-negative', 'snapshot-a', 'run-secondary', 'xhs-note', '1', '1');
    RAISE EXCEPTION 'ASSERT_FAIL: cross-snapshot record was accepted';
  EXCEPTION WHEN foreign_key_violation THEN
    GET STACKED DIAGNOSTICS rejected_constraint = CONSTRAINT_NAME;
    IF rejected_constraint <> 'NormalizationRunCurrent_workspace_snapshot_record_fkey' THEN
      RAISE EXCEPTION 'ASSERT_FAIL: wrong record constraint %', rejected_constraint;
    END IF;
    RAISE NOTICE 'EXPECTED_REJECTION SQLSTATE=23503 constraint=%', rejected_constraint;
  END;
END
$proof$;

DO $proof$
BEGIN
  RAISE EXCEPTION 'EXPECTED_NEGATIVE_SUITE_COMPLETE' USING ERRCODE = 'P0001';
END
$proof$;
