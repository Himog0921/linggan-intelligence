\set ON_ERROR_STOP on

-- Release-A positive proof: all rows use synthetic proof-only workspaces.
INSERT INTO "CapturePackage"
  (id, "workspaceId", "packagePayload", "checksumValue", "contentLength")
VALUES
  ('pkg_exec_a', 'proof_ws_a', decode('65786563', 'hex'), 'hash_exec_a', 4),
  ('pkg_manual_a', 'proof_ws_a', decode('6d616e75616c', 'hex'), 'hash_manual_a', 6),
  ('pkg_recovery_a', 'proof_ws_a', decode('7265636f76657279', 'hex'), 'hash_recovery_a', 8),
  ('pkg_migration_a', 'proof_ws_a', decode('6d6967726174696f6e', 'hex'), 'hash_migration_a', 9),
  ('pkg_conflict_a', 'proof_ws_a', decode('6669727374', 'hex'), 'hash_first_a', 5),
  ('pkg_conflict_b', 'proof_ws_a', decode('7365636f6e64', 'hex'), 'hash_second_a', 6);

-- V1 execution-shaped write remains legal and keeps its legacy identity key.
INSERT INTO "RawSnapshot"
  (id, "workspaceId", "jobId", "attemptId", "captureId", platform,
   "targetKey", "observedAt", "schemaVersion")
VALUES
  ('snap_v1_exec', 'proof_ws_v1', 'job_v1', 'attempt_v1', 'capture_v1',
   'xiaohongshu', 'note:v1', now(), 'v3');

INSERT INTO "RawRecord"
  (id, "workspaceId", "rawSnapshotId", "jobId", "recordType", platform,
   payload, "observedAt", "idempotencyKey")
VALUES
  ('record_v1_exec', 'proof_ws_v1', 'snap_v1_exec', 'job_v1', 'note',
   'xiaohongshu', '{"kind":"v1"}'::jsonb, now(), 'idem_v1_exec');

-- Four legal V2 ingress kinds. Non-execution sources intentionally omit all
-- execution relationships.
INSERT INTO "RawSnapshot"
  (id, "workspaceId", "jobId", "attemptId", "captureId", platform,
   "targetKey", "observedAt", "schemaVersion", "checksumAlgorithm",
   "checksumValue", "contentLength", "ingressKind", "capturePackageId",
   "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
   "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
   "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion",
   "integrityStatus", "contractId", "contractVersion", "contractHash",
   "stationId")
VALUES
  ('snap_exec_a', 'proof_ws_a', 'job_exec_a', 'attempt_exec_a', 'capture_exec_a',
   'xiaohongshu', 'note:exec', now(), 'v3', 'sha256', 'hash_exec_a', 4,
   'execution', 'pkg_exec_a', '2', 'collector-proof', 'station:proof', NULL,
   NULL, NULL, NULL, NULL, 1,
   'plan-proof', 'verified', 'content-detail', 1, 'contract-hash', 'station_a'),
  ('snap_manual_a', 'proof_ws_a', NULL, NULL, 'capture_manual_a',
   'xiaohongshu', 'note:manual', now(), 'v3', 'sha256', 'hash_manual_a', 6,
   'manual_import', 'pkg_manual_a', '2', 'importer-proof', 'user:proof',
   'manual proof', 'operator:proof', NULL, NULL, NULL, NULL,
   NULL, 'verified', 'content-detail', 1, 'contract-hash', NULL),
  ('snap_recovery_a', 'proof_ws_a', NULL, NULL, 'capture_recovery_a',
   'xiaohongshu', 'note:recovery', now(), 'v3', 'sha256', 'hash_recovery_a', 8,
   'recovery', 'pkg_recovery_a', '2', 'recovery-proof', 'user:proof', NULL,
   NULL, NULL, 'recovery-source-proof', 'operator:proof', NULL,
   NULL, 'verified', 'content-detail', 1, 'contract-hash', NULL),
  ('snap_migration_a', 'proof_ws_a', NULL, NULL, 'capture_migration_a',
   'xiaohongshu', 'note:migration', now(), 'v3', 'sha256', 'hash_migration_a', 9,
   'migration', 'pkg_migration_a', '2', 'migration-proof', 'tool:proof',
   'migration proof', NULL, 'migration-approval-proof', NULL, NULL, NULL,
   NULL, 'verified', 'content-detail', 1, 'contract-hash', NULL),
  ('snap_conflict_first', 'proof_ws_a', NULL, NULL, 'capture_conflict_a',
   'xiaohongshu', 'note:conflict', now(), 'v3', 'sha256', 'hash_first_a', 5,
   'manual_import', 'pkg_conflict_a', '2', 'importer-proof', 'user:proof',
   'manual proof', 'operator:proof', NULL, NULL, NULL, NULL,
   NULL, 'verified', 'content-detail', 1, 'contract-hash', NULL),
  ('snap_conflict_second', 'proof_ws_a', NULL, NULL, 'capture_conflict_a',
   'xiaohongshu', 'note:conflict', now(), 'v3', 'sha256', 'hash_second_a', 6,
   'manual_import', 'pkg_conflict_b', '2', 'importer-proof', 'user:proof',
   'manual proof', 'operator:proof', NULL, NULL, NULL, NULL,
   NULL, 'capture_identity_conflict', 'content-detail', 1, 'contract-hash', NULL);

INSERT INTO "RawRecord"
  (id, "workspaceId", "rawSnapshotId", "jobId", "recordType", "recordKind",
   platform, payload, "payloadHash", "observedAt", "idempotencyKey")
VALUES
  ('record_exec_a', 'proof_ws_a', 'snap_exec_a', 'job_exec_a', 'note', 'note',
   'xiaohongshu', '{"kind":"execution"}'::jsonb, 'record_hash_exec', now(), 'idem_exec_a'),
  ('record_manual_a', 'proof_ws_a', 'snap_manual_a', NULL, 'note', 'note',
   'xiaohongshu', '{"kind":"manual"}'::jsonb, 'record_hash_manual', now(), 'idem_manual_a'),
  ('record_recovery_a', 'proof_ws_a', 'snap_recovery_a', NULL, 'note', 'note',
   'xiaohongshu', '{"kind":"recovery"}'::jsonb, 'record_hash_recovery', now(), 'idem_recovery_a'),
  ('record_migration_a', 'proof_ws_a', 'snap_migration_a', NULL, 'note', 'note',
   'xiaohongshu', '{"kind":"migration"}'::jsonb, 'record_hash_migration', now(), 'idem_migration_a'),
  ('record_conflict_first', 'proof_ws_a', 'snap_conflict_first', NULL, 'note', 'note',
   'xiaohongshu', '{"variant":1}'::jsonb, 'record_hash_first', now(), 'idem_conflict_first'),
  ('record_conflict_second', 'proof_ws_a', 'snap_conflict_second', NULL, 'note', 'note',
   'xiaohongshu', '{"variant":2}'::jsonb, 'record_hash_second', now(), 'idem_conflict_second');

INSERT INTO "EvidenceIngressReceipt"
  ("workspaceId", "rawSnapshotId", "captureId", "ingressKind", "collectorVersion")
SELECT "workspaceId", id, "captureId", "ingressKind", "collectorVersion"
FROM "RawSnapshot"
WHERE id IN (
  'snap_exec_a', 'snap_manual_a', 'snap_recovery_a', 'snap_migration_a',
  'snap_conflict_first', 'snap_conflict_second'
);

INSERT INTO "CaptureArtifact"
  (id, "workspaceId", "rawSnapshotId", kind, "artifactChecksum")
VALUES
  ('artifact_exec_a', 'proof_ws_a', 'snap_exec_a', 'platform_response', 'artifact_hash_exec');

-- Same identity + same complete package hash converges on the existing
-- observation. ON CONFLICT models the Release-B replay branch without writing
-- a second package/snapshot/record/receipt.
INSERT INTO "RawSnapshot"
  (id, "workspaceId", "captureId", platform, "targetKey", "observedAt",
   "schemaVersion", "checksumAlgorithm", "checksumValue", "contentLength",
   "ingressKind", "capturePackageId", "protocolVersion", "collectorVersion",
   "sourcePrincipal", "integrityStatus")
VALUES
  ('snap_conflict_replay_attempt', 'proof_ws_a', 'capture_conflict_a',
   'xiaohongshu', 'note:conflict', now(), 'v3', 'sha256', 'hash_first_a', 5,
   'manual_import', 'pkg_conflict_a', '2', 'importer-proof', 'user:proof',
   'verified')
ON CONFLICT DO NOTHING;

DO $$
DECLARE
  v_count integer;
BEGIN
  SELECT count(*) INTO v_count FROM "RawSnapshot"
  WHERE "workspaceId" = 'proof_ws_a' AND "captureId" = 'capture_conflict_a';
  IF v_count <> 2 THEN
    RAISE EXCEPTION 'expected two retained variants, got %', v_count;
  END IF;

  SELECT count(*) INTO v_count FROM "RawSnapshot"
  WHERE "workspaceId" = 'proof_ws_a'
    AND "captureId" = 'capture_conflict_a'
    AND "integrityStatus" = 'verified';
  IF v_count <> 1 THEN
    RAISE EXCEPTION 'expected exactly one projection-eligible variant, got %', v_count;
  END IF;

  SELECT count(*) INTO v_count FROM "RawSnapshot"
  WHERE "workspaceId" = 'proof_ws_a'
    AND "captureId" = 'capture_conflict_a'
    AND "integrityStatus" = 'capture_identity_conflict';
  IF v_count <> 1 THEN
    RAISE EXCEPTION 'expected one retained conflict variant, got %', v_count;
  END IF;

  SELECT count(*) INTO v_count FROM "RawSnapshot"
  WHERE "workspaceId" = 'proof_ws_a'
    AND "ingressKind" IN ('manual_import', 'recovery', 'migration')
    AND ("jobId" IS NOT NULL OR "attemptId" IS NOT NULL OR "stationId" IS NOT NULL);
  IF v_count <> 0 THEN
    RAISE EXCEPTION 'non-execution Evidence forged execution relationships';
  END IF;

  SELECT count(*) INTO v_count FROM "EvidenceIngressReceipt"
  WHERE id !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$';
  IF v_count <> 0 THEN
    RAISE EXCEPTION 'EvidenceIngressReceipt database UUID default was not used';
  END IF;

  SELECT count(*) INTO v_count FROM pg_trigger
  WHERE tgrelid IN ('"RawSnapshot"'::regclass, '"RawRecord"'::regclass)
    AND NOT tgisinternal;
  IF v_count <> 0 THEN
    RAISE EXCEPTION 'Release-A must not install RawSnapshot/RawRecord append-only triggers';
  END IF;
END
$$;

SELECT 'RELEASE_A_POSITIVE_PASS' AS result;
