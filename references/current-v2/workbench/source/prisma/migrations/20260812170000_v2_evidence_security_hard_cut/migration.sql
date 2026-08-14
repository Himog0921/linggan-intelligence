-- V2 XHS Content hard cut. This migration is intentionally atomic: failure
-- at any statement rolls back catalog, ACL and function changes together.
BEGIN;

DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_schema_owner') THEN
    CREATE ROLE v2_schema_owner NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_evidence_reader_owner') THEN
    CREATE ROLE v2_evidence_reader_owner NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_evidence_writer') THEN
    CREATE ROLE v2_evidence_writer LOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_adapter_reader') THEN
    CREATE ROLE v2_adapter_reader LOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_contract_reader') THEN
    CREATE ROLE v2_contract_reader LOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_canonical_writer') THEN
    CREATE ROLE v2_canonical_writer LOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_default_app') THEN
    CREATE ROLE v2_default_app LOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_preflight_reader') THEN
    CREATE ROLE v2_preflight_reader LOGIN;
  END IF;
END $$;
-- Reassert the expand-phase preflight role's non-escalating attributes. The
-- role intentionally already exists before this migration so D3 can run.
ALTER ROLE v2_preflight_reader NOINHERIT NOCREATEDB NOCREATEROLE
  NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_preflight_reader SET default_transaction_read_only = on;
ALTER ROLE v2_evidence_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_adapter_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_contract_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_canonical_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_default_app NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;

ALTER TABLE "CapturePackage" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceIngressReceipt" OWNER TO v2_schema_owner;
ALTER TABLE "CaptureArtifact" OWNER TO v2_schema_owner;
ALTER TABLE "RawSnapshot" OWNER TO v2_schema_owner;
ALTER TABLE "RawRecord" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceAccessAudit" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceReaderWorkspaceGrant" OWNER TO v2_schema_owner;
ALTER TABLE "V2DurableWork" OWNER TO v2_schema_owner;

REVOKE ALL ON "CapturePackage", "EvidenceIngressReceipt", "CaptureArtifact",
  "RawSnapshot", "RawRecord", "EvidenceAccessAudit",
  "EvidenceReaderWorkspaceGrant", "V2DurableWork" FROM PUBLIC;

GRANT INSERT ON "CapturePackage", "EvidenceIngressReceipt", "CaptureArtifact",
  "RawSnapshot", "RawRecord", "V2DurableWork" TO v2_evidence_writer;
-- Runtime logins may inspect identities/checksums for idempotency, never the
-- source bytes themselves. packagePayload/RawRecord.payload remain reachable
-- only by the NOLOGIN SECURITY DEFINER owner through the audited package read.
GRANT SELECT ("id", "workspaceId", "checksumAlgorithm", "checksumValue",
  "contentLength", "restricted", "createdAt") ON "CapturePackage" TO v2_evidence_writer;
GRANT SELECT ON "EvidenceIngressReceipt", "CaptureArtifact", "V2DurableWork"
  TO v2_evidence_writer;
GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId",
  "platform", "targetKey", "expectedTargetKey", "observedTargetKey",
  "observedAt", "receivedAt", "clientObservedAt", "clockSkewSeconds",
  "observedAtSource", "checksumAlgorithm", "checksumValue", "contentLength",
  "schemaVersion", "pluginVersion", "stationId", "qualityStatus",
  "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion",
  "integrityStatus", "integrityReason", "lifecycleStatus", "contractId",
  "contractVersion", "contractHash", "createdAt") ON "RawSnapshot"
  TO v2_evidence_writer;
GRANT SELECT ("id", "workspaceId", "rawSnapshotId", "jobId", "recordType",
  "recordKind", "platform", "targetKey", "externalRecordId", "sequence",
  "payloadHash", "observedAt", "collectedAt", "idempotencyKey", "dedupeKey",
  "createdAt") ON "RawRecord" TO v2_evidence_writer;
GRANT SELECT ON "ExecutionStation", "PluginAuthorization", "ExecutionJob",
  "TaskAttempt", "ExecutionQueueEntry" TO v2_adapter_reader;
GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId",
  "platform", "targetKey", "expectedTargetKey", "observedTargetKey",
  "observedAt", "receivedAt", "clientObservedAt", "clockSkewSeconds",
  "observedAtSource", "checksumAlgorithm", "checksumValue", "contentLength",
  "schemaVersion", "pluginVersion", "stationId", "qualityStatus",
  "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion",
  "integrityStatus", "integrityReason", "lifecycleStatus", "contractId",
  "contractVersion", "contractHash", "createdAt") ON "RawSnapshot"
  TO v2_adapter_reader;
-- Strict HMAC verification persists a one-use nonce and clears expired rows.
-- The adapter has no SELECT/UPDATE permission on this ledger.
GRANT INSERT, DELETE ON "ExecutionStationRequestNonce" TO v2_adapter_reader;
GRANT SELECT ("expiresAt") ON "ExecutionStationRequestNonce" TO v2_adapter_reader;
GRANT SELECT, UPDATE ON "ExecutionJob", "TaskAttempt", "ExecutionQueueEntry",
  "ExecutionTaskRuntime", "TaskStatusProjection" TO v2_default_app;

GRANT SELECT, INSERT ON "EvidenceAccessAudit" TO v2_evidence_reader_owner;
GRANT SELECT ON "RawSnapshot", "RawRecord", "CapturePackage", "CaptureArtifact",
  "EvidenceReaderWorkspaceGrant"
  TO v2_evidence_reader_owner;

CREATE SCHEMA IF NOT EXISTS evidence_private;
ALTER SCHEMA evidence_private OWNER TO v2_evidence_reader_owner;
REVOKE ALL ON SCHEMA evidence_private FROM PUBLIC;
GRANT USAGE ON SCHEMA evidence_private TO v2_adapter_reader, v2_contract_reader;

CREATE OR REPLACE FUNCTION evidence_private.read_capture_package_and_audit(
  p_workspace_id TEXT,
  p_raw_snapshot_id TEXT,
  p_access_reason TEXT DEFAULT 'b2_normalization',
  p_request_trace_id TEXT DEFAULT NULL
) RETURNS TABLE (
  workspace_id TEXT,
  raw_snapshot_id TEXT,
  access_audit_id TEXT,
  lifecycle_status TEXT,
  integrity_status TEXT,
  package_bytes BYTEA,
  checksum_algorithm TEXT,
  checksum_value TEXT,
  content_length INTEGER,
  restricted BOOLEAN,
  capture_package_id TEXT,
  snapshot_metadata JSONB,
  artifact_descriptors JSONB
) LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, evidence_private
AS $$
DECLARE
  v_package_id TEXT;
  v_audit_id TEXT;
  v_integrity TEXT;
  v_lifecycle TEXT;
  v_restricted BOOLEAN;
BEGIN
  IF session_user NOT IN ('v2_adapter_reader','v2_contract_reader') THEN
    RAISE EXCEPTION 'Unauthorized evidence reader' USING ERRCODE='42501';
  END IF;
  IF current_user <> 'v2_evidence_reader_owner' THEN
    RAISE EXCEPTION 'Evidence reader owner mismatch' USING ERRCODE='42501';
  END IF;
  IF p_access_reason NOT IN ('b2_normalization','artifact_verification','projection_replay') THEN
    RAISE EXCEPTION 'Invalid evidence access reason' USING ERRCODE='22023';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM public."EvidenceReaderWorkspaceGrant"
    WHERE "workspaceId"=p_workspace_id
      AND "readerRole"=session_user
      AND "revokedAt" IS NULL
  ) THEN
    RAISE EXCEPTION 'Evidence workspace grant denied' USING ERRCODE='42501';
  END IF;

  SELECT "capturePackageId", "integrityStatus", "lifecycleStatus"
    INTO v_package_id, v_integrity, v_lifecycle
  FROM public."RawSnapshot"
  WHERE "workspaceId"=p_workspace_id AND "id"=p_raw_snapshot_id;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'RawSnapshot not found' USING ERRCODE='P0002';
  END IF;
  IF v_package_id IS NULL THEN
    RAISE EXCEPTION 'CapturePackage missing' USING ERRCODE='P0001';
  END IF;
  IF v_lifecycle IS NULL OR v_lifecycle NOT IN ('ACTIVE','ARCHIVED','REDACTED','PURGED') THEN
    RAISE EXCEPTION 'Unproved Evidence lifecycle' USING ERRCODE='55000';
  END IF;
  IF v_lifecycle IN ('REDACTED','PURGED') THEN
    RAISE EXCEPTION 'Evidence lifecycle denies access' USING ERRCODE='42501';
  END IF;

  SELECT package."restricted" INTO v_restricted
  FROM public."CapturePackage" AS package
  WHERE package."workspaceId"=p_workspace_id AND package."id"=v_package_id;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'CapturePackage not found' USING ERRCODE='P0002';
  END IF;
  IF v_restricted THEN
    RAISE EXCEPTION 'Restricted CapturePackage access denied' USING ERRCODE='42501';
  END IF;
  IF EXISTS (
    SELECT 1 FROM public."CaptureArtifact" AS artifact_guard
    WHERE artifact_guard."workspaceId"=p_workspace_id
      AND artifact_guard."rawSnapshotId"=p_raw_snapshot_id
      AND artifact_guard."restricted"=true
  ) THEN
    RAISE EXCEPTION 'Restricted CaptureArtifact access denied' USING ERRCODE='42501';
  END IF;

  INSERT INTO public."EvidenceAccessAudit"(
    "workspaceId","capturePackageId","accessedBy","accessReason","requestTraceId","restricted"
  ) VALUES (
    p_workspace_id,v_package_id,session_user,p_access_reason,p_request_trace_id,v_restricted
  ) RETURNING "id" INTO v_audit_id;

  RETURN QUERY
  SELECT p_workspace_id,p_raw_snapshot_id,v_audit_id,v_lifecycle,v_integrity,
    package."packagePayload",package."checksumAlgorithm",package."checksumValue",
    package."contentLength",package."restricted",v_package_id,
    jsonb_build_object(
      'id', snapshot.id, 'workspaceId', snapshot."workspaceId",
      'captureId', snapshot."captureId", 'platform', snapshot.platform,
      'integrityStatus', snapshot."integrityStatus",
      'checksumAlgorithm', snapshot."checksumAlgorithm",
      'checksumValue', snapshot."checksumValue", 'contentLength', snapshot."contentLength",
      'contractId', snapshot."contractId", 'contractVersion', snapshot."contractVersion",
      'contractHash', snapshot."contractHash", 'capturePackageId', snapshot."capturePackageId",
      'records', COALESCE((
        SELECT jsonb_agg(jsonb_build_object(
          'id', record.id, 'workspaceId', record."workspaceId",
          'rawSnapshotId', record."rawSnapshotId", 'recordKind', record."recordKind",
          'platform', record.platform, 'targetKey', record."targetKey",
          'externalRecordId', record."externalRecordId", 'sequence', record.sequence,
          'payloadHash', record."payloadHash", 'observedAt', record."observedAt",
          'idempotencyKey', record."idempotencyKey"
        ) ORDER BY record.sequence NULLS LAST, record.id)
        FROM public."RawRecord" record
        WHERE record."workspaceId"=p_workspace_id
          AND record."rawSnapshotId"=p_raw_snapshot_id
      ), '[]'::jsonb)
    ),
    COALESCE((
      SELECT jsonb_agg(jsonb_build_object(
        'id', artifact.id, 'workspaceId', artifact."workspaceId",
        'rawSnapshotId', artifact."rawSnapshotId", 'kind', artifact.kind,
        'artifactChecksum', artifact."artifactChecksum", 'restricted', artifact.restricted
      ) ORDER BY artifact.id)
      FROM public."CaptureArtifact" artifact
      WHERE artifact."workspaceId"=p_workspace_id
        AND artifact."rawSnapshotId"=p_raw_snapshot_id
    ), '[]'::jsonb)
  FROM public."CapturePackage" AS package
  JOIN public."RawSnapshot" AS snapshot
    ON snapshot."workspaceId"=p_workspace_id AND snapshot.id=p_raw_snapshot_id
  WHERE package."workspaceId"=p_workspace_id AND package."id"=v_package_id;
END;
$$;

ALTER FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT)
  OWNER TO v2_evidence_reader_owner;
REVOKE ALL ON FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT)
  FROM PUBLIC;
GRANT EXECUTE ON FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT)
  TO v2_adapter_reader, v2_contract_reader;

REVOKE UPDATE, DELETE ON "CapturePackage", "EvidenceIngressReceipt",
  "CaptureArtifact", "RawSnapshot", "RawRecord", "EvidenceAccessAudit",
  "V2DurableWork" FROM v2_evidence_writer, v2_canonical_writer,
  v2_adapter_reader, v2_contract_reader, v2_default_app;

GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId",
  "platform", "targetKey", "expectedTargetKey", "observedTargetKey",
  "observedAt", "receivedAt", "clientObservedAt", "clockSkewSeconds",
  "observedAtSource", "checksumAlgorithm", "checksumValue", "contentLength",
  "schemaVersion", "pluginVersion", "stationId", "qualityStatus",
  "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion",
  "integrityStatus", "integrityReason", "lifecycleStatus", "contractId",
  "contractVersion", "contractHash", "createdAt") ON "RawSnapshot"
  TO v2_canonical_writer;
GRANT SELECT ("id", "workspaceId", "rawSnapshotId", "jobId", "recordType",
  "recordKind", "platform", "targetKey", "externalRecordId", "sequence",
  "payloadHash", "observedAt", "collectedAt", "idempotencyKey", "dedupeKey",
  "createdAt") ON "RawRecord" TO v2_canonical_writer;
-- B2 revocation coordination reads ContentAsset identities and B3 creates the
-- stable identity on first acceptance. Display columns remain projection-owned.
GRANT SELECT, INSERT ON "ContentAsset" TO v2_canonical_writer;
GRANT SELECT, UPDATE ON "V2DurableWork" TO v2_canonical_writer;
GRANT SELECT, INSERT ON "NormalizationRun", "ContractEvaluation",
  "ContractEvaluationNormalizationRun", "ContractEvaluationInput",
  "CanonicalObservation", "CanonicalMediaSlot", "ContentObservation",
  "OutboxEvent" TO v2_canonical_writer;
GRANT SELECT, INSERT, UPDATE ON "NormalizationRunCurrent",
  "ContractEvaluationCurrent", "ContentCurrentProjection", "ContentMediaUsage",
  "MediaItem", "MediaOrigin", "ContentProjectionSubjectFence"
  TO v2_canonical_writer;

GRANT SELECT ON "ContentAsset", "ContentCurrentProjection", "ContentObservation",
  "ContractEvaluation", "ContractEvaluationCurrent", "ContractEvaluationInput",
  "ContentMediaUsage", "CanonicalMediaSlot", "MediaItem", "MediaOrigin",
  "MediaMaterialization", "MediaBlob", "MediaReplica", "OutboxEvent"
  TO v2_default_app;

GRANT SELECT ON "ExecutionStation", "ExecutionQueueEntry", "ExecutionTaskRuntime",
  "V2DurableWork", "EvidenceAccessAudit", "TaskStatusProjection",
  "ExecutionJob" TO v2_preflight_reader;
GRANT SELECT ("workspaceId", "contentAssetId", "currentObservationId",
  "visibilityState") ON "ContentCurrentProjection" TO v2_preflight_reader;
GRANT SELECT ("id", "workspaceId", "rawSnapshotId") ON "ContentObservation"
  TO v2_preflight_reader;
DO $$ BEGIN
  IF to_regclass('public._prisma_migrations') IS NOT NULL THEN
    GRANT SELECT ON public."_prisma_migrations" TO v2_preflight_reader;
  END IF;
END $$;

COMMIT;
