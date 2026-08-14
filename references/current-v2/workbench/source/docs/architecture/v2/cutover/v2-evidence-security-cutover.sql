-- ============================================================================
-- V2 Evidence Security Hard-Cut Migration (R1-02)
-- OPERATOR ONLY — independent PostgreSQL cluster. NEVER on 127.0.0.1:54329.
-- ============================================================================
BEGIN;

-- 1. NOLOGIN owner roles
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_schema_owner') THEN
    CREATE ROLE v2_schema_owner NOLOGIN; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_evidence_reader_owner') THEN
    CREATE ROLE v2_evidence_reader_owner NOLOGIN; END IF;
END $$;

-- 2. LOGIN roles
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_evidence_writer') THEN
    CREATE ROLE v2_evidence_writer LOGIN PASSWORD NULL; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_adapter_reader') THEN
    CREATE ROLE v2_adapter_reader LOGIN PASSWORD NULL; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_contract_reader') THEN
    CREATE ROLE v2_contract_reader LOGIN PASSWORD NULL; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_canonical_writer') THEN
    CREATE ROLE v2_canonical_writer LOGIN PASSWORD NULL; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_default_app') THEN
    CREATE ROLE v2_default_app LOGIN PASSWORD NULL; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_preflight_reader') THEN
    CREATE ROLE v2_preflight_reader LOGIN PASSWORD NULL; END IF;
END $$;
ALTER ROLE v2_preflight_reader NOINHERIT NOCREATEDB NOCREATEROLE
  NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_preflight_reader SET default_transaction_read_only = on;
ALTER ROLE v2_evidence_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_adapter_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_contract_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_canonical_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_default_app NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;

-- 3. Transfer Evidence table ownership
ALTER TABLE "CapturePackage" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceIngressReceipt" OWNER TO v2_schema_owner;
ALTER TABLE "CaptureArtifact" OWNER TO v2_schema_owner;
ALTER TABLE "RawSnapshot" OWNER TO v2_schema_owner;
ALTER TABLE "RawRecord" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceAccessAudit" OWNER TO v2_schema_owner;
ALTER TABLE "EvidenceReaderWorkspaceGrant" OWNER TO v2_schema_owner;
ALTER TABLE "V2DurableWork" OWNER TO v2_schema_owner;

-- 4. evidence_writer: INSERT + metadata SELECT (for replay). Payload bytes
-- remain available only through the audited SECURITY DEFINER function.
GRANT INSERT ON "CapturePackage" TO v2_evidence_writer;
GRANT SELECT ("id", "workspaceId", "checksumAlgorithm", "checksumValue", "contentLength", "restricted", "createdAt") ON "CapturePackage" TO v2_evidence_writer;
GRANT INSERT, SELECT ON "EvidenceIngressReceipt" TO v2_evidence_writer;
GRANT INSERT, SELECT ON "CaptureArtifact" TO v2_evidence_writer;
GRANT INSERT ON "RawSnapshot" TO v2_evidence_writer;
GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId", "platform",
  "targetKey", "expectedTargetKey", "observedTargetKey", "observedAt", "receivedAt",
  "clientObservedAt", "clockSkewSeconds", "observedAtSource", "checksumAlgorithm",
  "checksumValue", "contentLength", "schemaVersion", "pluginVersion", "stationId",
  "qualityStatus", "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion", "integrityStatus",
  "integrityReason", "lifecycleStatus", "contractId", "contractVersion",
  "contractHash", "createdAt") ON "RawSnapshot" TO v2_evidence_writer;
GRANT INSERT ON "RawRecord" TO v2_evidence_writer;
GRANT SELECT ("id", "workspaceId", "rawSnapshotId", "jobId", "recordType", "recordKind", "platform", "targetKey", "externalRecordId", "sequence", "payloadHash", "observedAt", "collectedAt", "idempotencyKey", "dedupeKey", "createdAt") ON "RawRecord" TO v2_evidence_writer;
GRANT INSERT, SELECT ON "V2DurableWork" TO v2_evidence_writer;

-- Authority readers can verify only execution/control metadata; package bytes
-- remain reachable exclusively through the audited function below.
GRANT SELECT ON "ExecutionStation", "PluginAuthorization", "ExecutionJob",
  "TaskAttempt", "ExecutionQueueEntry" TO v2_adapter_reader;
GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId", "platform",
  "targetKey", "expectedTargetKey", "observedTargetKey", "observedAt", "receivedAt",
  "clientObservedAt", "clockSkewSeconds", "observedAtSource", "checksumAlgorithm",
  "checksumValue", "contentLength", "schemaVersion", "pluginVersion", "stationId",
  "qualityStatus", "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion", "integrityStatus",
  "integrityReason", "lifecycleStatus", "contractId", "contractVersion",
  "contractHash", "createdAt") ON "RawSnapshot" TO v2_adapter_reader;
-- Strict HMAC verification persists a one-use nonce and clears expired rows.
-- The adapter has no SELECT/UPDATE permission on this ledger.
GRANT INSERT, DELETE ON "ExecutionStationRequestNonce" TO v2_adapter_reader;
GRANT SELECT ("expiresAt") ON "ExecutionStationRequestNonce" TO v2_adapter_reader;

-- The execution-control finalizer runs as default_app after Evidence commits.
GRANT SELECT, UPDATE ON "ExecutionJob", "TaskAttempt", "ExecutionQueueEntry",
  "ExecutionTaskRuntime", "TaskStatusProjection" TO v2_default_app;

-- 5. Revoke default_app from writing Evidence
REVOKE INSERT, UPDATE, DELETE ON "CapturePackage" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "EvidenceIngressReceipt" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "CaptureArtifact" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "RawSnapshot" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "RawRecord" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "EvidenceAccessAudit" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "EvidenceReaderWorkspaceGrant" FROM PUBLIC;
REVOKE INSERT, UPDATE, DELETE ON "V2DurableWork" FROM PUBLIC;

-- 6. Grant function owner the table access it needs (SECURITY DEFINER runs as owner)
GRANT SELECT, INSERT ON "EvidenceAccessAudit" TO v2_evidence_reader_owner;
GRANT SELECT ON "RawSnapshot", "RawRecord", "CaptureArtifact" TO v2_evidence_reader_owner;
GRANT SELECT ON "CapturePackage" TO v2_evidence_reader_owner;
GRANT SELECT ON "EvidenceReaderWorkspaceGrant" TO v2_evidence_reader_owner;
CREATE SCHEMA IF NOT EXISTS evidence_private;
ALTER SCHEMA evidence_private OWNER TO v2_evidence_reader_owner;
REVOKE ALL ON SCHEMA evidence_private FROM PUBLIC;
GRANT USAGE ON SCHEMA evidence_private TO v2_adapter_reader, v2_contract_reader;

CREATE OR REPLACE FUNCTION evidence_private.read_capture_package_and_audit(
  p_workspace_id TEXT, p_raw_snapshot_id TEXT,
  p_access_reason TEXT DEFAULT 'b2_normalization', p_request_trace_id TEXT DEFAULT NULL
) RETURNS TABLE (
  workspace_id TEXT, raw_snapshot_id TEXT, access_audit_id TEXT,
  lifecycle_status TEXT, integrity_status TEXT,
  package_bytes BYTEA, checksum_algorithm TEXT, checksum_value TEXT,
  content_length INTEGER, restricted BOOLEAN,
  capture_package_id TEXT, snapshot_metadata JSONB, artifact_descriptors JSONB
) LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, evidence_private
AS $$
DECLARE
  v_pkg_id TEXT; v_audit_id TEXT; v_integrity TEXT; v_lifecycle TEXT; v_restricted BOOLEAN;
BEGIN
  IF session_user NOT IN ('v2_adapter_reader','v2_contract_reader') THEN
    RAISE EXCEPTION 'Unauthorized reader: session_user=%', session_user USING ERRCODE = '42501'; END IF;
  IF current_user != 'v2_evidence_reader_owner' THEN
    RAISE EXCEPTION 'Function owner mismatch: current_user=%', current_user USING ERRCODE = '42501'; END IF;
  IF p_access_reason NOT IN ('b2_normalization','artifact_verification','projection_replay') THEN
    RAISE EXCEPTION 'Invalid evidence access reason' USING ERRCODE='22023';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM public."EvidenceReaderWorkspaceGrant"
    WHERE "workspaceId"=p_workspace_id
      AND "readerRole"=session_user
      AND "revokedAt" IS NULL
  ) THEN
    RAISE EXCEPTION 'Workspace grant denied for reader %', session_user USING ERRCODE='42501';
  END IF;

  SELECT "capturePackageId", "integrityStatus", "lifecycleStatus"
    INTO v_pkg_id, v_integrity, v_lifecycle
    FROM public."RawSnapshot" WHERE "workspaceId"=p_workspace_id AND "id"=p_raw_snapshot_id;
  IF NOT FOUND THEN RAISE EXCEPTION 'RawSnapshot not found' USING ERRCODE='P0002'; END IF;
  IF v_pkg_id IS NULL THEN RAISE EXCEPTION 'No CapturePackage' USING ERRCODE='P0001'; END IF;
  IF v_lifecycle IS NULL OR v_lifecycle NOT IN ('ACTIVE','ARCHIVED','REDACTED','PURGED') THEN
    RAISE EXCEPTION 'Unproved Evidence lifecycle' USING ERRCODE='55000'; END IF;
  IF v_lifecycle IN ('REDACTED','PURGED') THEN
    RAISE EXCEPTION 'Evidence lifecycle denies access' USING ERRCODE='42501'; END IF;

  SELECT package."restricted" INTO v_restricted FROM public."CapturePackage" AS package
    WHERE package."workspaceId"=p_workspace_id AND package."id"=v_pkg_id;
  IF NOT FOUND THEN RAISE EXCEPTION 'CapturePackage not found' USING ERRCODE='P0002'; END IF;
  IF v_restricted THEN
    RAISE EXCEPTION 'Restricted CapturePackage access denied' USING ERRCODE='42501'; END IF;
  IF EXISTS (SELECT 1 FROM public."CaptureArtifact" artifact_guard
    WHERE artifact_guard."workspaceId"=p_workspace_id
      AND artifact_guard."rawSnapshotId"=p_raw_snapshot_id
      AND artifact_guard."restricted"=true) THEN
    RAISE EXCEPTION 'Restricted CaptureArtifact access denied' USING ERRCODE='42501'; END IF;

  INSERT INTO public."EvidenceAccessAudit" ("workspaceId","capturePackageId","accessedBy","accessReason","requestTraceId","restricted")
    VALUES (p_workspace_id, v_pkg_id, session_user, p_access_reason, p_request_trace_id, v_restricted)
    RETURNING "id" INTO v_audit_id;

  RETURN QUERY
    SELECT p_workspace_id, p_raw_snapshot_id, v_audit_id, v_lifecycle, v_integrity,
      cp."packagePayload", cp."checksumAlgorithm", cp."checksumValue",
      cp."contentLength", cp."restricted", v_pkg_id,
      jsonb_build_object(
        'id', rs.id, 'workspaceId', rs."workspaceId", 'captureId', rs."captureId",
        'platform', rs.platform, 'integrityStatus', rs."integrityStatus",
        'checksumAlgorithm', rs."checksumAlgorithm", 'checksumValue', rs."checksumValue",
        'contentLength', rs."contentLength", 'contractId', rs."contractId",
        'contractVersion', rs."contractVersion", 'contractHash', rs."contractHash",
        'capturePackageId', rs."capturePackageId",
        'records', COALESCE((SELECT jsonb_agg(jsonb_build_object(
          'id', rr.id, 'workspaceId', rr."workspaceId", 'rawSnapshotId', rr."rawSnapshotId",
          'recordKind', rr."recordKind", 'platform', rr.platform, 'targetKey', rr."targetKey",
          'externalRecordId', rr."externalRecordId", 'sequence', rr.sequence,
          'payloadHash', rr."payloadHash", 'observedAt', rr."observedAt",
          'idempotencyKey', rr."idempotencyKey") ORDER BY rr.sequence NULLS LAST, rr.id)
          FROM public."RawRecord" rr WHERE rr."workspaceId"=p_workspace_id
            AND rr."rawSnapshotId"=p_raw_snapshot_id), '[]'::jsonb)),
      COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'id', ca.id, 'workspaceId', ca."workspaceId", 'rawSnapshotId', ca."rawSnapshotId",
        'kind', ca.kind, 'artifactChecksum', ca."artifactChecksum", 'restricted', ca.restricted)
        ORDER BY ca.id) FROM public."CaptureArtifact" ca
        WHERE ca."workspaceId"=p_workspace_id AND ca."rawSnapshotId"=p_raw_snapshot_id), '[]'::jsonb)
    FROM public."CapturePackage" cp
    JOIN public."RawSnapshot" rs ON rs."workspaceId"=p_workspace_id AND rs.id=p_raw_snapshot_id
    WHERE cp."workspaceId"=p_workspace_id AND cp."id"=v_pkg_id;
END;
$$;

ALTER FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT)
  OWNER TO v2_evidence_reader_owner;
REVOKE ALL ON FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION evidence_private.read_capture_package_and_audit(TEXT,TEXT,TEXT,TEXT)
  TO v2_adapter_reader, v2_contract_reader;

-- 7. Append-only: revoke UPDATE/DELETE on Evidence+Audit from all roles
REVOKE UPDATE, DELETE ON "CapturePackage" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "EvidenceIngressReceipt" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "CaptureArtifact" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "RawSnapshot" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "RawRecord" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "EvidenceAccessAudit" FROM v2_evidence_writer, v2_canonical_writer, v2_adapter_reader, v2_contract_reader;
REVOKE UPDATE, DELETE ON "V2DurableWork" FROM v2_evidence_writer, v2_adapter_reader, v2_contract_reader, v2_default_app;

-- 8. canonical_writer: minimum SELECT
GRANT SELECT ("id", "workspaceId", "jobId", "attemptId", "captureId", "platform",
  "targetKey", "expectedTargetKey", "observedTargetKey", "observedAt", "receivedAt",
  "clientObservedAt", "clockSkewSeconds", "observedAtSource", "checksumAlgorithm",
  "checksumValue", "contentLength", "schemaVersion", "pluginVersion", "stationId",
  "qualityStatus", "qualityReason", "source", "ingressKind", "capturePackageId",
  "protocolVersion", "collectorVersion", "sourcePrincipal", "sourceSummary",
  "importerIdentity", "migrationAuthorization", "recoveryCaptureId",
  "recoveryAuthorizedBy", "leaseEpoch", "executionPlanVersion", "integrityStatus",
  "integrityReason", "lifecycleStatus", "contractId", "contractVersion",
  "contractHash", "createdAt") ON "RawSnapshot" TO v2_canonical_writer;
GRANT SELECT ("id", "workspaceId", "rawSnapshotId", "jobId", "recordType", "recordKind", "platform", "targetKey", "externalRecordId", "sequence", "payloadHash", "observedAt", "collectedAt", "idempotencyKey", "dedupeKey", "createdAt") ON "RawRecord" TO v2_canonical_writer;
-- B2 revocation coordination reads ContentAsset identities and B3 creates the
-- stable identity on first acceptance. Display columns remain projection-owned.
GRANT SELECT, INSERT ON "ContentAsset" TO v2_canonical_writer;
GRANT SELECT, UPDATE ON "V2DurableWork" TO v2_canonical_writer;
GRANT SELECT, INSERT ON "NormalizationRun", "ContractEvaluation",
  "ContractEvaluationNormalizationRun", "ContractEvaluationInput",
  "CanonicalObservation", "CanonicalMediaSlot", "ContentObservation",
  "OutboxEvent" TO v2_canonical_writer;
GRANT SELECT, INSERT, UPDATE ON "NormalizationRunCurrent",
  "ContractEvaluationCurrent", "ContentCurrentProjection",
  "ContentMediaUsage", "MediaItem", "MediaOrigin",
  "ContentProjectionSubjectFence" TO v2_canonical_writer;
-- canonical_writer does NOT get SELECT on CapturePackage directly.

-- Material K-01 uses the default_app read identity. It can read only the
-- V2 projection/media facts and the stable ContentAsset identity; no Raw or
-- CapturePackage access is granted.
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
