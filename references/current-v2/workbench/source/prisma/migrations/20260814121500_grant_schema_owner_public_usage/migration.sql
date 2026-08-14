-- Foreign-key checks on V2 Evidence tables execute as the table owner.
-- The hard cut revoked PUBLIC schema access, so the NOLOGIN schema owner
-- needs explicit USAGE to resolve referenced public tables during FK checks.
GRANT USAGE ON SCHEMA public TO v2_schema_owner;
