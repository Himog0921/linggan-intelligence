-- CI-AUTO-004 P3: isolated shadow replay receipts.
-- These tables deliberately retain references and hashes, never a second copy of comment text or
-- a production analysis/atom/problem/projection receipt. Provider accounting remains in
-- linggan_model_invocation so replay cannot create an unmetered model-call path.

CREATE TABLE linggan_comment_replay_sample_set (
    sample_set_ref uuid PRIMARY KEY,
    selector_version text NOT NULL CHECK (selector_version='comment-replay.sample-selector.v1'),
    selector_hash text NOT NULL CHECK (selector_hash ~ '^[0-9a-f]{64}$'),
    member_hash text NOT NULL CHECK (member_hash ~ '^[0-9a-f]{64}$'),
    seed bigint NOT NULL,
    frozen_count integer NOT NULL CHECK (frozen_count BETWEEN 1 AND 120),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    state text NOT NULL DEFAULT 'frozen' CHECK (state IN ('frozen','expired')),
    CHECK (expires_at > created_at)
);

CREATE TABLE linggan_comment_replay_member (
    sample_set_ref uuid NOT NULL REFERENCES linggan_comment_replay_sample_set(sample_set_ref),
    ordinal smallint NOT NULL CHECK (ordinal BETWEEN 1 AND 120),
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    work_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    source_hash text NOT NULL CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    context_refs uuid[] NOT NULL DEFAULT '{}',
    context_hash text NOT NULL CHECK (context_hash ~ '^[0-9a-f]{64}$'),
    strata jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(strata)='object'),
    PRIMARY KEY(sample_set_ref,ordinal),
    UNIQUE(sample_set_ref,source_ref)
);
CREATE INDEX linggan_comment_replay_member_work_idx
    ON linggan_comment_replay_member(sample_set_ref,work_ref,ordinal);
CREATE TRIGGER linggan_comment_replay_member_is_immutable
    BEFORE UPDATE OR DELETE ON linggan_comment_replay_member
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_auto_upgrade_policy (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision>=0),
    enabled boolean NOT NULL DEFAULT false,
    selected_candidate_rule_revision_ref uuid REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    engineering_gate_version text NOT NULL DEFAULT 'comment-replay.engineering.v1',
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((enabled AND selected_candidate_rule_revision_ref IS NOT NULL) OR NOT enabled)
);
INSERT INTO linggan_comment_auto_upgrade_policy(singleton) VALUES(true);

CREATE TABLE linggan_comment_replay_run (
    run_ref uuid PRIMARY KEY,
    request_hash text NOT NULL UNIQUE CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    sample_set_ref uuid NOT NULL REFERENCES linggan_comment_replay_sample_set(sample_set_ref),
    baseline_rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    candidate_rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    baseline_snapshot jsonb NOT NULL CHECK (jsonb_typeof(baseline_snapshot)='object'),
    candidate_snapshot jsonb NOT NULL CHECK (jsonb_typeof(candidate_snapshot)='object'),
    config_ref uuid NOT NULL REFERENCES linggan_model_config(config_ref),
    model_snapshot jsonb NOT NULL CHECK (jsonb_typeof(model_snapshot)='object'),
    context_policy jsonb NOT NULL CHECK (jsonb_typeof(context_policy)='object'),
    context_policy_hash text NOT NULL CHECK (context_policy_hash ~ '^[0-9a-f]{64}$'),
    thresholds jsonb NOT NULL CHECK (jsonb_typeof(thresholds)='object'),
    seed bigint NOT NULL,
    repeat_member_limit smallint NOT NULL DEFAULT 0 CHECK (repeat_member_limit BETWEEN 0 AND 20),
    authorized_token_budget bigint NOT NULL CHECK (authorized_token_budget>=0),
    reserved_tokens bigint NOT NULL DEFAULT 0 CHECK (reserved_tokens>=0),
    execution_day text,
    policy_revision bigint NOT NULL,
    state text NOT NULL DEFAULT 'draft' CHECK (state IN ('draft','queued','running','pass','degraded','insufficient_evidence','waiting_daily_budget','failed')),
    failure_code text,
    comparison jsonb,
    adoption_receipt jsonb,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    started_at timestamptz,
    finished_at timestamptz,
    CHECK (baseline_rule_revision_ref<>candidate_rule_revision_ref),
    CHECK (reserved_tokens<=authorized_token_budget)
);
CREATE INDEX linggan_comment_replay_run_state_idx
    ON linggan_comment_replay_run(state,created_at,run_ref);

CREATE TABLE linggan_comment_replay_item (
    item_ref uuid PRIMARY KEY,
    run_ref uuid NOT NULL REFERENCES linggan_comment_replay_run(run_ref),
    member_ordinal smallint NOT NULL CHECK (member_ordinal BETWEEN 1 AND 120),
    side text NOT NULL CHECK (side IN ('baseline','candidate')),
    repeat_ordinal smallint NOT NULL DEFAULT 0 CHECK (repeat_ordinal BETWEEN 0 AND 20),
    attempt_ordinal smallint NOT NULL DEFAULT 0 CHECK (attempt_ordinal BETWEEN 0 AND 2),
    idempotency_hash text NOT NULL UNIQUE CHECK (idempotency_hash ~ '^[0-9a-f]{64}$'),
    invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    execution_day text,
    state text NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','running','succeeded','failed','excluded','waiting_daily_budget')),
    structure_accepted boolean,
    no_signal boolean,
    safe_summary jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(safe_summary)='object'),
    input_tokens bigint CHECK (input_tokens IS NULL OR input_tokens>=0),
    output_tokens bigint CHECK (output_tokens IS NULL OR output_tokens>=0),
    elapsed_ms bigint CHECK (elapsed_ms IS NULL OR elapsed_ms>=0),
    failure_code text,
    exclusion_reason text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    started_at timestamptz,
    finished_at timestamptz,
    UNIQUE(run_ref,member_ordinal,side,repeat_ordinal,attempt_ordinal)
);
-- PostgreSQL cannot express a foreign key through replay_run.sample_set_ref. This trigger
-- validates that every item ordinal belongs to the run's frozen sample set at write time.
CREATE OR REPLACE FUNCTION linggan_comment_replay_item_matches_run()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NOT EXISTS(
    SELECT 1 FROM linggan_comment_replay_run r
    JOIN linggan_comment_replay_member m ON m.sample_set_ref=r.sample_set_ref
      AND m.ordinal=NEW.member_ordinal
    WHERE r.run_ref=NEW.run_ref
  ) THEN
    RAISE EXCEPTION 'replay item member does not belong to run sample set';
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER linggan_comment_replay_item_member_guard
    BEFORE INSERT OR UPDATE OF run_ref,member_ordinal ON linggan_comment_replay_item
    FOR EACH ROW EXECUTE FUNCTION linggan_comment_replay_item_matches_run();
CREATE INDEX linggan_comment_replay_item_run_idx
    ON linggan_comment_replay_item(run_ref,state,member_ordinal,side,repeat_ordinal,attempt_ordinal);

-- A replay trace is intentionally metadata-only. The ordinary request trace has a daily packet
-- foreign key and can persist a short-lived prompt; replay does neither, so no source body can
-- survive in this domain while the invocation/diagnostic ledger remains directly inspectable.
CREATE TABLE linggan_comment_replay_trace (
    invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation(invocation_ref),
    item_ref uuid NOT NULL UNIQUE REFERENCES linggan_comment_replay_item(item_ref),
    request_hash text NOT NULL CHECK (request_hash ~ '^[0-9a-f]{64}$'),
    source_hash text NOT NULL CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    context_hash text NOT NULL CHECK (context_hash ~ '^[0-9a-f]{64}$'),
    rule_hash text NOT NULL CHECK (rule_hash ~ '^[0-9a-f]{64}$'),
    policy jsonb NOT NULL CHECK (jsonb_typeof(policy)='object'),
    events jsonb NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(events)='array'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

COMMENT ON TABLE linggan_comment_replay_sample_set IS
    'Frozen bounded shadow-sample references and hashes; never comment body text.';
COMMENT ON TABLE linggan_comment_replay_item IS
    'Independent baseline/candidate/repeat model receipts; no production analysis result is written here.';
COMMENT ON TABLE linggan_comment_replay_trace IS
    'Metadata-only replay trace paired with the shared model invocation ledger.';
