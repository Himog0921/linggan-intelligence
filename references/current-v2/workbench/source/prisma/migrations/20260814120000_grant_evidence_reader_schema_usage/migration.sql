-- The audited SECURITY DEFINER reader owns no tables, but PostgreSQL still
-- requires its NOLOGIN owner to have schema USAGE before fully-qualified
-- public table references can be resolved. Table privileges remain unchanged.
GRANT USAGE ON SCHEMA public TO v2_evidence_reader_owner;
