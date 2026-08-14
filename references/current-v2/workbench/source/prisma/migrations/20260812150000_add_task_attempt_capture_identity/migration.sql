ALTER TABLE "TaskAttempt"
ADD COLUMN "captureId" TEXT;

UPDATE "TaskAttempt"
SET "captureId" = 'legacy-attempt:' || "id"
WHERE "captureId" IS NULL;

ALTER TABLE "TaskAttempt"
ALTER COLUMN "captureId" SET NOT NULL;

CREATE UNIQUE INDEX "TaskAttempt_captureId_key" ON "TaskAttempt"("captureId");

-- Nullable expand only. Historical V1 rows have no proved V2 lifecycle and
-- must not be backfilled with a fabricated ACTIVE fact.
ALTER TABLE "RawSnapshot"
ADD COLUMN "lifecycleStatus" TEXT;

-- Expand-phase preflight identity. The operator provisions its password out
-- of band; no credential is stored in migration SQL. This role must be able
-- to execute D3 before the hard-cut migration is applied, but receives only
-- the SELECTs used by cutover-preflight.ts and no mutation privilege.
DO $$ BEGIN
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
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='v2_preflight_proof_owner') THEN
    CREATE ROLE v2_preflight_proof_owner NOLOGIN;
  END IF;
END $$;
ALTER ROLE v2_evidence_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_adapter_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_contract_reader NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_canonical_writer NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_default_app NOINHERIT NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_preflight_reader NOINHERIT NOCREATEDB NOCREATEROLE
  NOREPLICATION NOBYPASSRLS;
ALTER ROLE v2_preflight_reader SET default_transaction_read_only = on;

-- The pre-cut restore proof may compare sensitive tables, but the runtime
-- preflight login receives only aggregate count/hash facts.  A NOLOGIN owner
-- is the sole principal allowed to read the closed table set below.
CREATE SCHEMA IF NOT EXISTS evidence_private;
REVOKE ALL ON SCHEMA evidence_private FROM PUBLIC;
GRANT USAGE ON SCHEMA public TO v2_preflight_proof_owner;
GRANT SELECT ON "CapturePackage", "EvidenceIngressReceipt", "CaptureArtifact",
  "RawSnapshot", "RawRecord", "V2DurableWork", "ContentAsset",
  "ContentCurrentProjection", "ContentObservation", "MediaItem", "MediaOrigin"
  TO v2_preflight_proof_owner;
DO $$ BEGIN
  IF to_regclass('public._prisma_migrations') IS NOT NULL THEN
    GRANT SELECT ON public."_prisma_migrations" TO v2_preflight_proof_owner;
  END IF;
END $$;

CREATE OR REPLACE FUNCTION evidence_private.read_cutover_restore_fingerprints()
RETURNS TABLE (table_name TEXT, row_count BIGINT, fingerprint TEXT)
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, evidence_private
AS $$
DECLARE
  v_table TEXT;
BEGIN
  IF session_user <> 'v2_preflight_reader' THEN
    RAISE EXCEPTION 'Unauthorized restore proof reader' USING ERRCODE='42501';
  END IF;
  IF current_user <> 'v2_preflight_proof_owner' THEN
    RAISE EXCEPTION 'Restore proof owner mismatch' USING ERRCODE='42501';
  END IF;
  FOREACH v_table IN ARRAY ARRAY[
    '_prisma_migrations', 'CapturePackage', 'EvidenceIngressReceipt',
    'CaptureArtifact', 'RawSnapshot', 'RawRecord', 'V2DurableWork',
    'ContentAsset', 'ContentCurrentProjection', 'ContentObservation',
    'MediaItem', 'MediaOrigin'
  ] LOOP
    RETURN QUERY EXECUTE format(
      'SELECT %L::text, count(*)::bigint, md5(coalesce(string_agg(md5(to_jsonb(row_value)::text), '''' ORDER BY md5(to_jsonb(row_value)::text)), '''')) FROM public.%I row_value',
      v_table, v_table
    );
  END LOOP;
END;
$$;
ALTER FUNCTION evidence_private.read_cutover_restore_fingerprints()
  OWNER TO v2_preflight_proof_owner;
REVOKE ALL ON FUNCTION evidence_private.read_cutover_restore_fingerprints() FROM PUBLIC;
GRANT USAGE ON SCHEMA evidence_private TO v2_preflight_reader;
GRANT EXECUTE ON FUNCTION evidence_private.read_cutover_restore_fingerprints()
  TO v2_preflight_reader;

GRANT SELECT ON "ExecutionStation", "ExecutionQueueEntry", "ExecutionTaskRuntime",
  "V2DurableWork", "EvidenceIngressReceipt" TO v2_preflight_reader;
DO $$ BEGIN
  IF to_regclass('public._prisma_migrations') IS NOT NULL THEN
    GRANT SELECT ON public."_prisma_migrations" TO v2_preflight_reader;
  END IF;
END $$;
