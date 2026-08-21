-- SCOPE-001 second migration.
--
-- Order matters and is fixed by the Issue #3 DECISION_RESOLVED ruling:
--   1. complete the typed Record envelope on already-accepted capture records;
--   2. add the closed business outcome and lease/epoch expression record processing needs;
--   3. create Source Identity, Source Content, Observation, Current and field sources.
--
-- 0001 is never edited. This migration adds to it.

-- ---------------------------------------------------------------------------
-- 1. Typed Record envelope
-- ---------------------------------------------------------------------------

-- Existing rows were written before the envelope columns existed, so their record kind, source
-- statement and observation instant cannot be reconstructed from any qualified source. Guessing
-- them from the target, the payload or a receive time is forbidden, so this fails closed instead.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM capture_record) THEN
        RAISE EXCEPTION 'capture_record holds rows predating the typed envelope; migrate them from a qualified source before applying 0002 rather than back-filling guesses';
    END IF;
END $$;

ALTER TABLE capture_record
    ADD COLUMN record_kind text NOT NULL,
    ADD COLUMN source_system text NOT NULL,
    ADD COLUMN source_namespace text NOT NULL,
    ADD COLUMN source_object_type text NOT NULL,
    -- The contract field is mandatory but its value may be an explicit null, so the column is
    -- nullable. A nullable column is not permission for the field to be absent: a missing field
    -- fails the package schema before it ever reaches here.
    ADD COLUMN source_external_id text,
    ADD COLUMN source_channel text NOT NULL,
    -- The producer's observation instant. It is never a receive, accept, insert or process time.
    ADD COLUMN observed_at timestamptz NOT NULL,
    ADD COLUMN observed_at_precision text NOT NULL,
    ADD COLUMN observed_at_basis text NOT NULL;

ALTER TABLE capture_record
    ADD CONSTRAINT capture_record_v1_record_kind CHECK (record_kind = 'content_detail'),
    ADD CONSTRAINT capture_record_v1_source_system CHECK (source_system = 'synthetic'),
    ADD CONSTRAINT capture_record_v1_source_namespace CHECK (source_namespace = 'scope-001'),
    ADD CONSTRAINT capture_record_v1_source_object_type CHECK (source_object_type = 'content'),
    ADD CONSTRAINT capture_record_v1_source_channel CHECK (source_channel = 'synthetic_page'),
    ADD CONSTRAINT capture_record_v1_source_external_id CHECK (source_external_id IS NULL OR length(source_external_id) > 0),
    ADD CONSTRAINT capture_record_v1_observed_at_precision CHECK (observed_at_precision = 'exact'),
    ADD CONSTRAINT capture_record_v1_observed_at_basis CHECK (observed_at_basis = 'fixture');
