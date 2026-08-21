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
    -- A revision is assembled before it becomes visible. `published_at` records that one-way
    -- transition so a later revision can never make an earlier published source set writable
    -- again merely by moving the Current pointer forward.
    published_at timestamptz,
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

-- A published field state must be true of the observations it names: the support must actually
-- observe the published value at the latest qualified instant, an unresolved field must really
-- have competing values at that instant, and unknown must have had nothing qualified to use.
-- Counting rows by role is not enough; a bypassing writer could satisfy counts with any rows.
CREATE FUNCTION scope_001_verify_current_field(
    revision_id bigint,
    content_id bigint,
    field text,
    state text,
    published_value text
) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
    latest timestamptz;
    qualified_at_latest integer;
    supports integer;
    conflicts integer;
    unobserved integer;
    stale integer;
    wrong_value integer;
    distinct_values integer;
BEGIN
    SELECT max(o.observed_at) INTO latest
    FROM content_observation o
    WHERE o.source_content_id = content_id
      AND CASE field WHEN 'title' THEN o.title_observed ELSE o.body_observed END;

    SELECT count(*) INTO qualified_at_latest
    FROM content_observation o
    WHERE o.source_content_id = content_id
      AND CASE field WHEN 'title' THEN o.title_observed ELSE o.body_observed END
      AND o.observed_at = latest;

    SELECT
        count(*) FILTER (WHERE fs.role = 'selected_support'),
        count(*) FILTER (WHERE fs.role = 'conflicting_candidate'),
        count(*) FILTER (WHERE NOT (CASE field WHEN 'title' THEN o.title_observed ELSE o.body_observed END)),
        count(*) FILTER (WHERE o.observed_at IS DISTINCT FROM latest),
        count(*) FILTER (WHERE (CASE field WHEN 'title' THEN o.title_value ELSE o.body_value END) IS DISTINCT FROM published_value),
        count(DISTINCT CASE field WHEN 'title' THEN o.title_value ELSE o.body_value END)
    INTO supports, conflicts, unobserved, stale, wrong_value, distinct_values
    FROM content_current_revision_field_source fs
    JOIN content_observation o ON o.id = fs.observation_id
    WHERE fs.revision_id = scope_001_verify_current_field.revision_id
      AND fs.field_kind = field;

    IF unobserved > 0 THEN
        RAISE EXCEPTION 'a % field source names an observation that does not observe %', field, field;
    END IF;
    IF stale > 0 THEN
        RAISE EXCEPTION 'a % field source is not from the latest qualified observation instant', field;
    END IF;

    IF state = 'selected' THEN
        IF supports < 1 OR conflicts > 0 THEN
            RAISE EXCEPTION 'selected % must have at least one selected_support and no conflicting candidate', field;
        END IF;
        IF wrong_value > 0 THEN
            RAISE EXCEPTION 'a selected % source does not observe the published value', field;
        END IF;
        IF supports <> qualified_at_latest THEN
            RAISE EXCEPTION 'selected % must fix every observation at the latest qualified instant', field;
        END IF;
    ELSIF state = 'unresolved' THEN
        IF conflicts < 2 OR supports > 0 THEN
            RAISE EXCEPTION 'unresolved % must have at least two conflicting candidates and no support', field;
        END IF;
        IF distinct_values < 2 THEN
            RAISE EXCEPTION 'unresolved % must name candidates with at least two distinct values', field;
        END IF;
        IF conflicts <> qualified_at_latest THEN
            RAISE EXCEPTION 'unresolved % must fix every observation at the latest qualified instant', field;
        END IF;
    ELSE
        IF supports > 0 OR conflicts > 0 THEN
            RAISE EXCEPTION 'unknown % must not claim any field source', field;
        END IF;
        IF latest IS NOT NULL THEN
            RAISE EXCEPTION 'unknown % cannot ignore a qualified observation of %', field, field;
        END IF;
    END IF;
END;
$$;

CREATE FUNCTION scope_001_verify_current_field_provenance() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    PERFORM scope_001_verify_current_field(
        NEW.id, NEW.source_content_id, 'title', NEW.title_state, NEW.title_value);
    PERFORM scope_001_verify_current_field(
        NEW.id, NEW.source_content_id, 'body', NEW.body_state, NEW.body_value);
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER content_current_revision_field_provenance_matches_state
    AFTER INSERT ON content_current_revision
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION scope_001_verify_current_field_provenance();

-- The watermark must be exactly the inputs this content actually holds: every input package with
-- the delivery and receipt that created it, its records and the observations they formed.
CREATE FUNCTION scope_001_verify_current_revision_watermark_values(
    revision_id bigint,
    content_id bigint,
    supplied_watermark jsonb
) RETURNS void LANGUAGE plpgsql AS $$
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
            'recordRefs', jsonb_agg(DISTINCT to_jsonb(r.record_ref::text) ORDER BY to_jsonb(r.record_ref::text)),
            'observationRefs', jsonb_agg(DISTINCT to_jsonb(o.observation_ref::text) ORDER BY to_jsonb(o.observation_ref::text))
        ) AS entry
        FROM content_observation o
        JOIN capture_record r ON r.id = o.capture_record_id
        JOIN capture_package p ON p.id = r.package_id
        JOIN capture_ingress_delivery d ON d.id = p.accepted_delivery_id
        WHERE o.source_content_id = content_id
        GROUP BY p.package_ref, d.delivery_ref, p.accepted_receipt_ref
    ) grouped;

    IF supplied_watermark IS DISTINCT FROM expected THEN
        RAISE EXCEPTION 'current revision watermark does not match the inputs locked for this content';
    END IF;
END;
$$;

CREATE FUNCTION scope_001_verify_current_revision_watermark() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    PERFORM scope_001_verify_current_revision_watermark_values(
        NEW.id, NEW.source_content_id, NEW.watermark);
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER content_current_revision_watermark_matches_inputs
    AFTER INSERT ON content_current_revision
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION scope_001_verify_current_revision_watermark();

-- ---------------------------------------------------------------------------
-- 4. What the database refuses regardless of which client is writing
-- ---------------------------------------------------------------------------

-- Observations and published current values are append-only. Correcting a value means
-- recomputing a new revision from qualified evidence, never rewriting history in place.
CREATE FUNCTION scope_001_forbid_rewriting_history() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% is append-only; recompute instead of rewriting or deleting a row', TG_TABLE_NAME;
END;
$$;

CREATE TRIGGER content_observation_is_append_only
    BEFORE UPDATE OR DELETE ON content_observation
    FOR EACH ROW EXECUTE FUNCTION scope_001_forbid_rewriting_history();

CREATE FUNCTION scope_001_forbid_rewriting_revision_history() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    -- Publication is the one allowed transition: its business content is identical, and it only
    -- records that the already-validated revision has become eligible to be Current.
    IF TG_OP = 'UPDATE'
       AND OLD.published_at IS NULL
       AND NEW.published_at IS NOT NULL
       AND NEW.revision_ref IS NOT DISTINCT FROM OLD.revision_ref
       AND NEW.source_content_id IS NOT DISTINCT FROM OLD.source_content_id
       AND NEW.policy_version IS NOT DISTINCT FROM OLD.policy_version
       AND NEW.title_state IS NOT DISTINCT FROM OLD.title_state
       AND NEW.title_value IS NOT DISTINCT FROM OLD.title_value
       AND NEW.body_state IS NOT DISTINCT FROM OLD.body_state
       AND NEW.body_value IS NOT DISTINCT FROM OLD.body_value
       AND NEW.watermark IS NOT DISTINCT FROM OLD.watermark THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'content_current_revision is append-only; recompute instead of rewriting or deleting a row';
END;
$$;

CREATE TRIGGER content_current_revision_is_append_only
    BEFORE UPDATE OR DELETE ON content_current_revision
    FOR EACH ROW EXECUTE FUNCTION scope_001_forbid_rewriting_revision_history();

CREATE TRIGGER content_current_revision_field_source_is_append_only
    BEFORE UPDATE OR DELETE ON content_current_revision_field_source
    FOR EACH ROW EXECUTE FUNCTION scope_001_forbid_rewriting_history();

-- A finalized business outcome must be true of the rows that exist. `observation_recorded` in
-- particular may not be claimed without a finalized attempt, the observation this record formed,
-- and a published current revision for the content that observation belongs to. Every other
-- outcome forms no observation at all.
CREATE FUNCTION scope_001_verify_business_outcome_facts() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    observed integer;
    published integer;
BEGIN
    IF NEW.business_outcome IS NULL THEN
        RETURN NULL;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM record_processing_attempt a
        WHERE a.processing_work_id = NEW.id AND a.finalized_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'a finalized work must have a finalized processing attempt';
    END IF;

    SELECT count(*) INTO observed
    FROM content_observation o WHERE o.capture_record_id = NEW.capture_record_id;

    IF NEW.business_outcome = 'observation_recorded' THEN
        IF observed <> 1 THEN
            RAISE EXCEPTION 'observation_recorded requires exactly one observation for this record, found %', observed;
        END IF;
        -- Not merely "some current exists": the published revision must have this record's own
        -- observation in its watermark, otherwise the work would be borrowing an older current
        -- that never absorbed it.
        SELECT count(*) INTO published
        FROM content_observation o
        JOIN source_content c ON c.id = o.source_content_id
        JOIN content_current_revision rev ON rev.id = c.current_revision_id
        WHERE o.capture_record_id = NEW.capture_record_id
          AND EXISTS (
              SELECT 1
              FROM jsonb_array_elements(rev.watermark) AS package_entry,
                   jsonb_array_elements_text(package_entry -> 'observationRefs') AS watermark_ref
              WHERE watermark_ref.value = o.observation_ref::text
          );
        IF published <> 1 THEN
            RAISE EXCEPTION 'observation_recorded requires the published current revision to include this record''s observation';
        END IF;
    ELSIF observed <> 0 THEN
        RAISE EXCEPTION 'business outcome % must not leave an observation behind', NEW.business_outcome;
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER record_processing_work_outcome_matches_facts
    AFTER INSERT OR UPDATE ON record_processing_work
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION scope_001_verify_business_outcome_facts();

-- A finalized business outcome is a fact, not a temporary status. In particular it cannot be
-- nulled first to evade the reverse guards on its attempt or Current support.
CREATE FUNCTION scope_001_forbid_rewriting_business_outcome() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.business_outcome IS NOT NULL
       AND NEW.business_outcome IS DISTINCT FROM OLD.business_outcome THEN
        RAISE EXCEPTION 'a fixed business outcome cannot be cleared or changed';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER record_processing_work_business_outcome_is_fixed
    BEFORE UPDATE OF business_outcome ON record_processing_work
    FOR EACH ROW EXECUTE FUNCTION scope_001_forbid_rewriting_business_outcome();

-- ---------------------------------------------------------------------------
-- 5. Published facts stay closed, and a finished outcome keeps what it depends on
-- ---------------------------------------------------------------------------

-- Once a revision is the published current value, the set of observations it fixes is closed.
-- New evidence produces a new revision; it never re-opens the meaning of an old one.
CREATE FUNCTION scope_001_forbid_appending_to_published_revision() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM content_current_revision
        WHERE id = NEW.revision_id AND published_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'the source set of a published current revision is closed; new evidence must produce a new revision';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER content_current_revision_field_source_closes_on_publication
    BEFORE INSERT ON content_current_revision_field_source
    FOR EACH ROW EXECUTE FUNCTION scope_001_forbid_appending_to_published_revision();

-- A business outcome may not outlive the facts that justify it. Finalizing an attempt is legal;
-- deleting or reopening one that already supports an outcome is not.
CREATE FUNCTION scope_001_protect_finalized_attempt() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.finalized_at IS NOT NULL AND EXISTS (
        SELECT 1 FROM record_processing_work w
        WHERE w.id = OLD.processing_work_id AND w.business_outcome IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'a finalized attempt supporting a business outcome cannot be removed or reopened';
    END IF;
    RETURN CASE TG_OP WHEN 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER record_processing_attempt_keeps_supporting_a_finished_outcome
    BEFORE UPDATE OR DELETE ON record_processing_attempt
    FOR EACH ROW EXECUTE FUNCTION scope_001_protect_finalized_attempt();

-- A published Current is one-way: it can move only to a complete, newer, already-marked
-- revision. The publication transition repeats the exact field-source and watermark checks, so
-- staging a valid revision, contaminating it later, and only then publishing it cannot bypass
-- the insert-time deferred checks.
CREATE FUNCTION scope_001_protect_published_current_pointer() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    revision content_current_revision%ROWTYPE;
BEGIN
    IF OLD.current_revision_id IS NOT NULL AND NEW.current_revision_id IS NULL THEN
        RAISE EXCEPTION 'a published current pointer cannot be cleared; recompute a new revision instead';
    END IF;
    IF OLD.current_revision_id IS NOT NULL
       AND NEW.current_revision_id IS NOT NULL
       AND NEW.current_revision_id < OLD.current_revision_id THEN
        RAISE EXCEPTION 'a published current pointer cannot roll back to an older revision';
    END IF;
    IF NEW.current_revision_id IS NOT NULL
       AND NEW.current_revision_id IS DISTINCT FROM OLD.current_revision_id THEN
        SELECT * INTO revision FROM content_current_revision WHERE id = NEW.current_revision_id;
        IF NOT FOUND OR revision.source_content_id <> NEW.id THEN
            RAISE EXCEPTION 'a current pointer must publish a revision for the same content';
        END IF;
        IF revision.published_at IS NULL THEN
            RAISE EXCEPTION 'a current pointer may publish only a revision marked for publication';
        END IF;
        PERFORM scope_001_verify_current_field(
            revision.id, NEW.id, 'title', revision.title_state, revision.title_value);
        PERFORM scope_001_verify_current_field(
            revision.id, NEW.id, 'body', revision.body_state, revision.body_value);
        PERFORM scope_001_verify_current_revision_watermark_values(
            revision.id, NEW.id, revision.watermark);
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER source_content_keeps_its_published_current
    BEFORE UPDATE ON source_content
    FOR EACH ROW EXECUTE FUNCTION scope_001_protect_published_current_pointer();

-- Identity and content anchors are allowed to be assembled before they have any observational
-- use. Once a Source/Content has an Observation or published Current, rebinding it would
-- silently reinterpret retained history and is therefore forbidden.
CREATE FUNCTION scope_001_protect_source_identity_anchor() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.external_id IS DISTINCT FROM OLD.external_id
       AND EXISTS (
           SELECT 1
           FROM source_content c
           LEFT JOIN content_observation o ON o.source_content_id = c.id
           WHERE c.source_identity_id = OLD.id
           GROUP BY c.id, c.current_revision_id
           HAVING count(o.id) > 0 OR c.current_revision_id IS NOT NULL
       ) THEN
        RAISE EXCEPTION 'a source identity used by an observation or current cannot be rebound';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER source_identity_anchor_is_fixed_after_use
    BEFORE UPDATE OF external_id ON source_identity
    FOR EACH ROW EXECUTE FUNCTION scope_001_protect_source_identity_anchor();

CREATE FUNCTION scope_001_protect_source_content_anchor() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.source_identity_id IS DISTINCT FROM OLD.source_identity_id
        OR NEW.content_ref IS DISTINCT FROM OLD.content_ref)
       AND (
           OLD.current_revision_id IS NOT NULL
           OR EXISTS (SELECT 1 FROM content_observation WHERE source_content_id = OLD.id)
       ) THEN
        RAISE EXCEPTION 'a source content anchor used by an observation or current cannot be rebound';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER source_content_anchor_is_fixed_after_use
    BEFORE UPDATE OF source_identity_id, content_ref ON source_content
    FOR EACH ROW EXECUTE FUNCTION scope_001_protect_source_content_anchor();

-- The runtime role may create Source/Content rows only while processing an accepted Record. A
-- direct writer cannot invent a brand-new external identity and then build a parallel fact chain
-- around it: every identity must already be stated by at least one accepted typed envelope.
CREATE FUNCTION scope_001_require_identity_from_accepted_record() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM capture_record r
        WHERE r.source_system = NEW.source_system
          AND r.source_namespace = NEW.namespace
          AND r.source_object_type = NEW.object_type
          AND r.source_external_id = NEW.external_id
    ) THEN
        RAISE EXCEPTION 'a source identity must be rooted in an accepted record envelope';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER source_identity_requires_accepted_record
    BEFORE INSERT ON source_identity
    FOR EACH ROW EXECUTE FUNCTION scope_001_require_identity_from_accepted_record();

-- An Observation cannot bind an accepted record to some different Content. This turns the typed
-- envelope's source statement into a database-enforced boundary even for callers bypassing Rust.
CREATE FUNCTION scope_001_require_observation_anchor_match() RETURNS trigger
    LANGUAGE plpgsql AS $$
DECLARE
    record_external_id text;
    content_external_id text;
BEGIN
    SELECT r.source_external_id INTO record_external_id
    FROM capture_record r WHERE r.id = NEW.capture_record_id;
    SELECT i.external_id INTO content_external_id
    FROM source_content c JOIN source_identity i ON i.id = c.source_identity_id
    WHERE c.id = NEW.source_content_id;
    IF record_external_id IS NULL OR content_external_id IS DISTINCT FROM record_external_id THEN
        RAISE EXCEPTION 'an observation must bind a record to its matching source content anchor';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER content_observation_requires_matching_anchor
    BEFORE INSERT ON content_observation
    FOR EACH ROW EXECUTE FUNCTION scope_001_require_observation_anchor_match();
