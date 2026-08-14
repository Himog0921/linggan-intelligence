\set ON_ERROR_STOP on

-- Every block catches only the expected PostgreSQL error. If an illegal write
-- succeeds, ASSERT_FAIL escapes with P0001 and the proof runner rejects it.
DO $$
BEGIN
  BEGIN
    UPDATE "CapturePackage" SET restricted = true WHERE id = 'pkg_exec_a';
    RAISE EXCEPTION 'ASSERT_FAIL CapturePackage UPDATE was accepted';
  EXCEPTION WHEN SQLSTATE '55000' THEN
    RAISE NOTICE 'EXPECTED_REJECTION CapturePackage UPDATE';
  END;

  BEGIN
    DELETE FROM "EvidenceIngressReceipt" WHERE "rawSnapshotId" = 'snap_exec_a';
    RAISE EXCEPTION 'ASSERT_FAIL EvidenceIngressReceipt DELETE was accepted';
  EXCEPTION WHEN SQLSTATE '55000' THEN
    RAISE NOTICE 'EXPECTED_REJECTION EvidenceIngressReceipt DELETE';
  END;

  BEGIN
    UPDATE "CaptureArtifact" SET restricted = true WHERE id = 'artifact_exec_a';
    RAISE EXCEPTION 'ASSERT_FAIL CaptureArtifact UPDATE was accepted';
  EXCEPTION WHEN SQLSTATE '55000' THEN
    RAISE NOTICE 'EXPECTED_REJECTION CaptureArtifact UPDATE';
  END;

  BEGIN
    INSERT INTO "CaptureArtifact"
      (id, "workspaceId", "rawSnapshotId", kind, "artifactChecksum")
    VALUES
      ('artifact_cross_workspace', 'proof_ws_b', 'snap_exec_a',
       'platform_response', 'artifact_hash_cross');
    RAISE EXCEPTION 'ASSERT_FAIL cross-workspace artifact FK was accepted';
  EXCEPTION WHEN foreign_key_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION cross-workspace artifact FK';
  END;

  BEGIN
    INSERT INTO "EvidenceIngressReceipt"
      ("workspaceId", "rawSnapshotId", "captureId", "ingressKind", "collectorVersion")
    VALUES
      ('proof_ws_b', 'snap_manual_a', 'capture_cross_workspace',
       'manual_import', 'collector-proof');
    RAISE EXCEPTION 'ASSERT_FAIL cross-workspace receipt FK was accepted';
  EXCEPTION WHEN foreign_key_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION cross-workspace receipt FK';
  END;

  BEGIN
    INSERT INTO "RawSnapshot"
      (id, "workspaceId", "captureId", platform, "targetKey", "observedAt",
       "schemaVersion", "checksumAlgorithm", "checksumValue", "contentLength",
       "ingressKind", "capturePackageId", "protocolVersion", "collectorVersion",
       "sourcePrincipal", "integrityStatus")
    VALUES
      ('snap_cross_workspace_package', 'proof_ws_b', 'capture_cross_workspace_package',
       'xiaohongshu', 'note:cross-workspace-package', now(), 'v3', 'sha256',
       'hash_exec_a', 4, 'manual_import', 'pkg_exec_a', '2', 'importer-proof',
       'user:proof', 'verified');
    RAISE EXCEPTION 'ASSERT_FAIL cross-workspace CapturePackage FK was accepted';
  EXCEPTION WHEN foreign_key_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION cross-workspace CapturePackage FK';
  END;

  BEGIN
    INSERT INTO "RawSnapshot"
      (id, "workspaceId", "captureId", platform, "targetKey", "observedAt",
       "schemaVersion", "checksumAlgorithm", "checksumValue", "contentLength",
       "ingressKind", "capturePackageId", "protocolVersion", "collectorVersion",
       "sourcePrincipal", "integrityStatus")
    VALUES
      ('snap_duplicate_variant', 'proof_ws_a', 'capture_conflict_a',
       'xiaohongshu', 'note:conflict', now(), 'v3', 'sha256', 'hash_first_a', 5,
       'manual_import', 'pkg_conflict_a', '2', 'importer-proof', 'user:proof',
       'capture_identity_conflict');
    RAISE EXCEPTION 'ASSERT_FAIL duplicate identity+hash variant was accepted';
  EXCEPTION WHEN unique_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION duplicate identity+hash variant';
  END;

  BEGIN
    INSERT INTO "CapturePackage"
      (id, "workspaceId", "packagePayload", "checksumValue", "contentLength")
    VALUES
      ('pkg_third_verified', 'proof_ws_a', decode('7468697264', 'hex'), 'hash_third_a', 5);

    INSERT INTO "RawSnapshot"
      (id, "workspaceId", "captureId", platform, "targetKey", "observedAt",
       "schemaVersion", "checksumAlgorithm", "checksumValue", "contentLength",
       "ingressKind", "capturePackageId", "protocolVersion", "collectorVersion",
       "sourcePrincipal", "integrityStatus")
    VALUES
      ('snap_second_verified', 'proof_ws_a', 'capture_conflict_a',
       'xiaohongshu', 'note:conflict', now(), 'v3', 'sha256', 'hash_third_a', 5,
       'manual_import', 'pkg_third_verified', '2', 'importer-proof', 'user:proof',
       'verified');
    RAISE EXCEPTION 'ASSERT_FAIL second verified variant was accepted';
  EXCEPTION WHEN unique_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION second verified variant';
  END;

  BEGIN
    INSERT INTO "RawSnapshot"
      (id, "workspaceId", "jobId", "attemptId", "captureId", platform,
       "targetKey", "observedAt", "schemaVersion")
    VALUES
      ('snap_v1_duplicate', 'proof_ws_v1', 'job_v1_b', 'attempt_v1_b',
       'capture_v1', 'xiaohongshu', 'note:v1-duplicate', now(), 'v3');
    RAISE EXCEPTION 'ASSERT_FAIL duplicate V1 capture identity was accepted';
  EXCEPTION WHEN unique_violation THEN
    RAISE NOTICE 'EXPECTED_REJECTION duplicate V1 capture identity';
  END;
END
$$;

-- The validation contract requires the negative script itself to fail. The
-- runner accepts only this final marker; any earlier ASSERT_FAIL is a failure.
DO $$
BEGIN
  RAISE EXCEPTION 'EXPECTED_NEGATIVE_SUITE_COMPLETE';
END
$$;
