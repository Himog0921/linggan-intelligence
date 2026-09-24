-- COMMENT-STUDY-PRODUCTIZATION-001 / schema phase.
-- CANDIDATE: not registered in local-runtime.sh until constraints/views and dispatch gates land.
-- The authorized migration runner must apply this file in one transaction after worker drain.
-- Existing installations: delta only. Never bootstrap/reset an existing clean-study schema.

DO $$
DECLARE
    relation_name text;
BEGIN
    FOREACH relation_name IN ARRAY ARRAY[
        'observation_domain', 'linggan_material_comment', 'linggan_model_invocation',
        'linggan_comment_study_policy', 'linggan_comment_study_run',
        'linggan_comment_study_target', 'linggan_comment_study_batch',
        'linggan_comment_study_resolution', 'linggan_comment_study_problem_pair',
        'linggan_comment_study_problem_revision', 'linggan_comment_study_problem_membership'
    ] LOOP
        IF to_regclass(format('%I.%I', current_schema(), relation_name)) IS NULL THEN
            RAISE EXCEPTION 'comment_study_schema_prerequisite_missing: %', relation_name;
        END IF;
    END LOOP;
    IF to_regprocedure('scope_001_now()') IS NULL THEN
        RAISE EXCEPTION 'comment_study_schema_prerequisite_missing: scope_001_now';
    END IF;
    FOREACH relation_name IN ARRAY ARRAY[
        'linggan_comment_study_clean_cache', 'linggan_comment_study_start_request',
        'linggan_comment_study_model_request'
    ] LOOP
        IF to_regclass(format('%I.%I', current_schema(), relation_name)) IS NOT NULL THEN
            RAISE EXCEPTION 'comment_study_delta_already_present_or_partial: %', relation_name;
        END IF;
    END LOOP;
END
$$;

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;

ALTER TABLE linggan_comment_study_policy
    ADD COLUMN method_name text,
    ADD COLUMN parent_policy_ref uuid REFERENCES linggan_comment_study_policy(policy_ref),
    ADD COLUMN method_manifest jsonb,
    ADD COLUMN method_hash text,
    ADD CONSTRAINT cs_policy_method_fields_ck CHECK (
        (method_name IS NULL AND method_manifest IS NULL AND method_hash IS NULL)
        OR (method_name IS NOT NULL AND method_manifest IS NOT NULL AND method_hash IS NOT NULL
            AND char_length(btrim(method_name)) BETWEEN 1 AND 100
            AND jsonb_typeof(method_manifest) = 'object'
            AND method_hash ~ '^[0-9a-f]{64}$')
    ),
    ADD CONSTRAINT cs_policy_parent_self_ck CHECK (parent_policy_ref IS DISTINCT FROM policy_ref);

ALTER TABLE linggan_comment_study_run
    ADD COLUMN comment_budget integer CHECK (comment_budget BETWEEN 1 AND 3000),
    ADD COLUMN context_character_budget integer CHECK (context_character_budget BETWEEN 1 AND 20000),
    ADD COLUMN token_limit bigint CHECK (token_limit BETWEEN 1024 AND 10000000),
    ADD COLUMN execution_manifest jsonb NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(execution_manifest) = 'object'),
    ADD COLUMN dispatch_state text NOT NULL DEFAULT 'stopped'
        CHECK (dispatch_state IN ('enabled', 'paused', 'stopped')),
    ADD COLUMN dispatch_reason text DEFAULT 'legacy_unrecorded',
    ADD COLUMN control_version bigint NOT NULL DEFAULT 0 CHECK (control_version >= 0),
    ADD CONSTRAINT cs_run_dispatch_reason_ck CHECK (
        (dispatch_state = 'enabled' AND dispatch_reason IS NULL)
        OR (dispatch_state IN ('paused', 'stopped') AND dispatch_reason IS NOT NULL
            AND dispatch_reason IN ('user_paused', 'user_stopped', 'budget_exhausted',
                                    'legacy_unrecorded', 'upgrade_guard', 'method_unavailable'))
    );

-- The constraints phase will backfill identities and add new-write/fencing constraints.
-- Do not invent a historical input fingerprint, completion time or matching revision.
ALTER TABLE linggan_comment_study_target
    ADD COLUMN comment_external_id text,
    ADD COLUMN input_fingerprint text CHECK (input_fingerprint ~ '^[0-9a-f]{64}$'),
    ADD COLUMN finished_at timestamptz,
    ADD COLUMN terminal_reason text CHECK (terminal_reason IN (
        'user_stopped', 'budget_exhausted', 'source_unavailable', 'input_limit_exceeded',
        'attempts_exhausted', 'provider_failed', 'legacy_execution_stopped'
    ));

ALTER TABLE linggan_comment_study_problem_membership
    ADD COLUMN problem_revision_ref uuid REFERENCES linggan_comment_study_problem_revision(revision_ref);
-- The constraints phase strengthens this to the documented (revision_ref, problem_ref) FK.

CREATE FUNCTION cs_clean_reasons_are_strings(value jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT AS $$
    SELECT CASE WHEN jsonb_typeof(value) = 'array' THEN
        NOT EXISTS (SELECT 1 FROM jsonb_array_elements(value) AS element(item)
                    WHERE jsonb_typeof(element.item) <> 'string')
        ELSE false END
$$;

CREATE TABLE linggan_comment_study_clean_cache (
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    cleaner_version text NOT NULL CHECK (char_length(cleaner_version) BETWEEN 1 AND 100),
    raw_sha256 text NOT NULL CHECK (raw_sha256 ~ '^[0-9a-f]{64}$'),
    research_text text NOT NULL CHECK (char_length(research_text) <= 16000),
    clean_state text NOT NULL CHECK (clean_state IN ('direct', 'context', 'dropped', 'anomaly')),
    clean_reasons jsonb NOT NULL CHECK (cs_clean_reasons_are_strings(clean_reasons)),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (source_ref, cleaner_version)
);
CREATE INDEX cs_clean_search_trgm_idx
    ON linggan_comment_study_clean_cache USING gin (research_text public.gin_trgm_ops)
    WHERE clean_state IN ('direct', 'context');
CREATE INDEX cs_clean_version_state_idx
    ON linggan_comment_study_clean_cache (cleaner_version, clean_state, source_ref);

CREATE TABLE linggan_comment_study_start_request (
    request_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    policy_ref uuid NOT NULL REFERENCES linggan_comment_study_policy(policy_ref),
    request_hash text NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    origin text NOT NULL CHECK (origin IN ('manual', 'scheduled')),
    origin_ref uuid,
    scheduled_for timestamptz,
    command_manifest jsonb NOT NULL CHECK (jsonb_typeof(command_manifest) = 'object'),
    outcome text NOT NULL CHECK (outcome IN ('created', 'no_work', 'index_pending')),
    run_ref uuid UNIQUE REFERENCES linggan_comment_study_run(run_ref),
    result_manifest jsonb NOT NULL CHECK (jsonb_typeof(result_manifest) = 'object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT cs_start_origin_ck CHECK (
        (origin = 'manual' AND origin_ref IS NULL AND scheduled_for IS NULL)
        OR (origin = 'scheduled' AND origin_ref IS NOT NULL AND scheduled_for IS NOT NULL)
    ),
    CONSTRAINT cs_start_run_outcome_ck CHECK ((outcome = 'created') = (run_ref IS NOT NULL))
);
CREATE INDEX cs_start_domain_created_idx
    ON linggan_comment_study_start_request (domain_ref, created_at DESC, request_ref);
CREATE UNIQUE INDEX cs_start_schedule_slot_uq
    ON linggan_comment_study_start_request (origin_ref, scheduled_for) WHERE origin = 'scheduled';

CREATE TABLE linggan_comment_study_model_request (
    invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation(invocation_ref),
    run_ref uuid NOT NULL REFERENCES linggan_comment_study_run(run_ref),
    policy_ref uuid NOT NULL REFERENCES linggan_comment_study_policy(policy_ref),
    stage text NOT NULL CHECK (stage IN ('semantic', 'resolution', 'pair')),
    batch_ref uuid REFERENCES linggan_comment_study_batch(batch_ref),
    resolution_ref uuid REFERENCES linggan_comment_study_resolution(resolution_ref),
    pair_ref uuid REFERENCES linggan_comment_study_problem_pair(pair_ref),
    attempt_ordinal integer NOT NULL CHECK (attempt_ordinal BETWEEN 1 AND 3),
    input_context_hash text NOT NULL CHECK (input_context_hash ~ '^[0-9a-f]{64}$'),
    request_manifest jsonb NOT NULL CHECK (jsonb_typeof(request_manifest) = 'object'),
    request_hash text NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    dispatch_started_at timestamptz,
    deadline_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT cs_request_subject_ck CHECK (
        (stage = 'semantic' AND batch_ref IS NOT NULL AND resolution_ref IS NULL AND pair_ref IS NULL
            AND attempt_ordinal = 1)
        OR (stage = 'resolution' AND batch_ref IS NULL AND resolution_ref IS NOT NULL AND pair_ref IS NULL)
        OR (stage = 'pair' AND batch_ref IS NULL AND resolution_ref IS NULL AND pair_ref IS NOT NULL)
    )
);
CREATE UNIQUE INDEX cs_request_batch_uq
    ON linggan_comment_study_model_request (batch_ref) WHERE stage = 'semantic';
CREATE UNIQUE INDEX cs_request_resolution_attempt_uq
    ON linggan_comment_study_model_request (resolution_ref, input_context_hash, attempt_ordinal)
    WHERE stage = 'resolution';
CREATE UNIQUE INDEX cs_request_pair_attempt_uq
    ON linggan_comment_study_model_request (pair_ref, input_context_hash, attempt_ordinal)
    WHERE stage = 'pair';
CREATE INDEX cs_request_run_idx ON linggan_comment_study_model_request (run_ref, created_at, invocation_ref);
CREATE INDEX cs_request_deadline_idx ON linggan_comment_study_model_request (deadline_at, invocation_ref);
