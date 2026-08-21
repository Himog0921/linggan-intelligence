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

-- ---------------------------------------------------------------------------
-- 2. Record processing lease, epoch and closed business outcome
-- ---------------------------------------------------------------------------

-- The business outcome belongs to the work, not to an attempt: an attempt records a run, while
-- the work records the one decided result. Runtime level (ready/leased/finalized) is derived on
-- read from this column plus the current attempt's lease, so there is no second state machine.
ALTER TABLE record_processing_work
    ADD COLUMN business_outcome text
        CHECK (business_outcome IN (
            'observation_recorded',
            'source_identity_unresolved',
            'source_identity_conflict',
            'record_contract_invalid'
        ));

CREATE TABLE record_processing_attempt (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    processing_attempt_ref uuid NOT NULL UNIQUE,
    processing_work_id bigint NOT NULL REFERENCES record_processing_work(id),
    epoch integer NOT NULL CHECK (epoch > 0),
    claimed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    lease_expires_at timestamptz NOT NULL,
    finalized_at timestamptz,
    run_error text,
    UNIQUE (processing_work_id, epoch)
);

-- One work is finalized by exactly one attempt; a late epoch can never claim the same result.
CREATE UNIQUE INDEX record_processing_work_has_one_finalizing_attempt
    ON record_processing_attempt (processing_work_id)
    WHERE finalized_at IS NOT NULL;

-- ---------------------------------------------------------------------------
-- 3. Source Identity, Content, Observation, Current and field sources
-- ---------------------------------------------------------------------------

CREATE TABLE source_identity (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    source_identity_ref uuid NOT NULL UNIQUE,
    source_system text NOT NULL CHECK (source_system = 'synthetic'),
    namespace text NOT NULL CHECK (namespace = 'scope-001'),
    object_type text NOT NULL CHECK (object_type = 'content'),
    external_id text NOT NULL CHECK (length(external_id) > 0),
    UNIQUE (source_system, namespace, object_type, external_id)
);

CREATE TABLE source_content (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    content_ref uuid NOT NULL UNIQUE,
    source_identity_id bigint NOT NULL UNIQUE REFERENCES source_identity(id),
    -- Nullable while a content exists without a published revision. The composite foreign key
    -- added after content_current_revision exists forbids pointing at another content's revision.
    current_revision_id bigint
);

CREATE TABLE content_observation (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    observation_ref uuid NOT NULL UNIQUE,
    source_content_id bigint NOT NULL REFERENCES source_content(id),
    -- One accepted record forms at most one observation in this slice.
    capture_record_id bigint NOT NULL UNIQUE REFERENCES capture_record(id),
    observed_at timestamptz NOT NULL,
    observed_at_precision text NOT NULL CHECK (observed_at_precision = 'exact'),
    parser_version text NOT NULL CHECK (parser_version = 'content-detail-processor-v1'),
    title_observed boolean NOT NULL,
    title_value text,
    body_observed boolean NOT NULL,
    body_value text,
    CHECK (title_observed = (title_value IS NOT NULL)),
    CHECK (body_observed = (body_value IS NOT NULL)),
    UNIQUE (id, source_content_id)
);

CREATE TABLE content_current_revision (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    revision_ref uuid NOT NULL UNIQUE,
    source_content_id bigint NOT NULL REFERENCES source_content(id),
    policy_version text NOT NULL CHECK (policy_version = 'content-current-policy-v1'),
    title_state text NOT NULL CHECK (title_state IN ('selected', 'unknown', 'unresolved')),
    title_value text,
    body_state text NOT NULL CHECK (body_state IN ('selected', 'unknown', 'unresolved')),
    body_value text,
    -- The exact inputs this recomputation locked, as fixed public refs. Verified by a deferred
    -- constraint trigger below so it can never drift from the rows it claims to summarize.
    watermark jsonb NOT NULL,
    -- Only `selected` carries a value; an empty string is still an observed value.
    CHECK (title_state = 'selected' OR title_value IS NULL),
    CHECK (title_state <> 'selected' OR title_value IS NOT NULL),
    CHECK (body_state = 'selected' OR body_value IS NULL),
    CHECK (body_state <> 'selected' OR body_value IS NOT NULL),
    UNIQUE (id, source_content_id)
);

ALTER TABLE source_content
    ADD CONSTRAINT source_content_points_at_its_own_revision
    FOREIGN KEY (current_revision_id, id)
    REFERENCES content_current_revision(id, source_content_id);

CREATE TABLE content_current_revision_field_source (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    field_source_ref uuid NOT NULL UNIQUE,
    revision_id bigint NOT NULL,
    source_content_id bigint NOT NULL,
    field_kind text NOT NULL CHECK (field_kind IN ('title', 'body')),
    observation_id bigint NOT NULL,
    role text NOT NULL CHECK (role IN ('selected_support', 'conflicting_candidate')),
    UNIQUE (revision_id, field_kind, observation_id),
    -- Revision, observation and content must all belong together.
    FOREIGN KEY (revision_id, source_content_id) REFERENCES content_current_revision(id, source_content_id),
    FOREIGN KEY (observation_id, source_content_id) REFERENCES content_observation(id, source_content_id)
);

-- A published field state must match the provenance it claims: `selected` needs at least one
-- same-value support, `unresolved` needs at least two conflicting candidates, and `unknown` must
-- not invent any field support at all.
CREATE FUNCTION scope_001_verify_current_field_provenance() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    field text;
    state text;
    supports integer;
    conflicts integer;
BEGIN
    FOREACH field IN ARRAY ARRAY['title', 'body'] LOOP
        state := CASE field WHEN 'title' THEN NEW.title_state ELSE NEW.body_state END;
        SELECT
            count(*) FILTER (WHERE role = 'selected_support'),
            count(*) FILTER (WHERE role = 'conflicting_candidate')
        INTO supports, conflicts
        FROM content_current_revision_field_source
        WHERE revision_id = NEW.id AND field_kind = field;

        IF state = 'selected' AND (supports < 1 OR conflicts > 0) THEN
            RAISE EXCEPTION 'selected % must have at least one selected_support and no conflicting candidate', field;
        END IF;
        IF state = 'unresolved' AND (conflicts < 2 OR supports > 0) THEN
            RAISE EXCEPTION 'unresolved % must have at least two conflicting candidates and no support', field;
        END IF;
        IF state = 'unknown' AND (supports > 0 OR conflicts > 0) THEN
            RAISE EXCEPTION 'unknown % must not claim any field source', field;
        END IF;
    END LOOP;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER content_current_revision_field_provenance_matches_state
    AFTER INSERT ON content_current_revision
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION scope_001_verify_current_field_provenance();

-- The watermark must be exactly the inputs this content actually holds: every input package with
-- the delivery and receipt that created it, its records and the observations they formed.
CREATE FUNCTION scope_001_verify_current_revision_watermark() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    expected jsonb;
BEGIN
    SELECT coalesce(jsonb_agg(entry ORDER BY entry ->> 'packageRef'), '[]'::jsonb)
    INTO expected
    FROM (
        SELECT jsonb_build_object(
            'packageRef', p.package_ref,
            'originalAcceptedDeliveryRef', d.delivery_ref,
            'acceptedReceiptRef', p.accepted_receipt_ref,
            'recordRefs', jsonb_agg(DISTINCT to_jsonb(r.record_ref::text)),
            'observationRefs', jsonb_agg(DISTINCT to_jsonb(o.observation_ref::text))
        ) AS entry
        FROM content_observation o
        JOIN capture_record r ON r.id = o.capture_record_id
        JOIN capture_package p ON p.id = r.package_id
        JOIN capture_ingress_delivery d ON d.id = p.accepted_delivery_id
        WHERE o.source_content_id = NEW.source_content_id
        GROUP BY p.package_ref, d.delivery_ref, p.accepted_receipt_ref
    ) grouped;

    IF NEW.watermark IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'current revision watermark does not match the inputs locked for this content';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER content_current_revision_watermark_matches_inputs
    AFTER INSERT ON content_current_revision
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION scope_001_verify_current_revision_watermark();
