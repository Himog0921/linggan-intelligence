-- Existing installations revoke PUBLIC access to the public schema. V2 runtime
-- roles need schema traversal in addition to their existing table privileges.
BEGIN;

GRANT USAGE ON SCHEMA public TO
  v2_evidence_writer,
  v2_adapter_reader,
  v2_contract_reader,
  v2_canonical_writer,
  v2_default_app,
  v2_preflight_reader;

COMMIT;
