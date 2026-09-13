-- Comment Research Execution Foundation V1.
--
-- This migration records immutable run-input snapshots and future terminal
-- analysis conclusions. It creates neither a queue nor a worker contract: a
-- `prepared` item is only eligible for a separately authorized executor.

CREATE TABLE comment_research_run_v1 (
  id UUID PRIMARY KEY,
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  -- Snapshot formation is the only V1 state this package can create. Later
  -- execution state transitions belong in append-only event records.
  execution_state TEXT NOT NULL
    CHECK (execution_state IN ('prepared', 'blocked'))
);

CREATE TABLE comment_research_run_item_v1 (
  id UUID PRIMARY KEY,
  run_id UUID NOT NULL REFERENCES comment_research_run_v1(id) ON DELETE RESTRICT,
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),

  -- Frozen locator and identity are operator/audit data only. They must not be
  -- projected through the existing User Voices DTO.
  source_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  source_record_index INTEGER NOT NULL CHECK (source_record_index >= 0),
  comment_observation_id UUID NOT NULL REFERENCES comment_observation_v0(id) ON DELETE RESTRICT,
  comment_derivation_id UUID NOT NULL REFERENCES comment_derivation_v1(id) ON DELETE RESTRICT,

  cleaned_research_text TEXT NOT NULL CHECK (btrim(cleaned_research_text) <> ''),
  cleaning_contract TEXT NOT NULL
    CHECK (cleaning_contract ~ '^comment-cleaning\.v[1-9][0-9]*$'),
  research_contract TEXT NOT NULL
    CHECK (research_contract = 'comment-research-execution.v1'),
  output_schema TEXT NOT NULL
    CHECK (output_schema = 'comment-analysis-structured-output.v1'),
  model_strategy_id TEXT NOT NULL CHECK (btrim(model_strategy_id) <> ''),
  model_strategy_version TEXT NOT NULL CHECK (btrim(model_strategy_version) <> ''),
  research_fingerprint CHAR(64) NOT NULL
    CHECK (research_fingerprint ~ '^[0-9a-f]{64}$'),

  context_pack_version TEXT NOT NULL
    CHECK (context_pack_version = 'comment-research-input-snapshot.v1'),
  context_pack_integrity_sha256 CHAR(64) NOT NULL
    CHECK (context_pack_integrity_sha256 ~ '^[0-9a-f]{64}$'),
  frozen_context_pack_text TEXT NOT NULL CHECK (btrim(frozen_context_pack_text) <> ''),
  context_sufficiency_state TEXT NOT NULL
    CHECK (context_sufficiency_state IN ('sufficient', 'insufficient_needs_context')),

  -- Execution is operational state. It never doubles as a user-facing
  -- conclusion such as success/no_signal; those live only in
  -- comment_analysis_v1 below.
  execution_state TEXT NOT NULL CHECK (execution_state IN ('prepared', 'blocked')),
  initial_failure_code TEXT,
  output_validation_state TEXT NOT NULL DEFAULT 'not_submitted'
    CHECK (output_validation_state = 'not_submitted'),

  CONSTRAINT comment_research_run_item_v1_workspace_matches_run CHECK (
    workspace_id <> ''
  ),
  CONSTRAINT comment_research_run_item_v1_context_gate CHECK (
    (context_sufficiency_state = 'sufficient'
      AND execution_state = 'prepared'
      AND initial_failure_code IS NULL)
    OR
    (context_sufficiency_state = 'insufficient_needs_context'
      AND execution_state = 'blocked'
      AND initial_failure_code = 'needs_context_insufficient')
  ),
  CONSTRAINT comment_research_run_item_v1_run_derivation_fingerprint_key
    UNIQUE (run_id, comment_derivation_id, research_fingerprint)
);

CREATE INDEX comment_research_run_item_v1_workspace_created_idx
  ON comment_research_run_item_v1(workspace_id, created_at DESC);

CREATE INDEX comment_research_run_item_v1_reuse_lookup_idx
  ON comment_research_run_item_v1(workspace_id, comment_derivation_id, research_fingerprint);

-- Later executors append state observations rather than overwriting the frozen
-- item. `result_recorded` describes an accepted or rejected contract outcome,
-- never which conclusion the commenter produced.
CREATE TABLE comment_research_run_item_event_v1 (
  id UUID PRIMARY KEY,
  run_item_id UUID NOT NULL REFERENCES comment_research_run_item_v1(id) ON DELETE RESTRICT,
  occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  execution_state TEXT NOT NULL CHECK (execution_state IN (
    'queued', 'running', 'completed', 'failed', 'timed_out', 'interrupted',
    'awaiting_recovery', 'cancelled', 'blocked'
  )),
  failure_code TEXT,
  output_validation_state TEXT NOT NULL CHECK (output_validation_state IN (
    'not_submitted', 'accepted', 'rejected'
  )),
  CONSTRAINT comment_research_run_item_event_v1_state_fields CHECK (
    (execution_state IN ('failed', 'timed_out', 'interrupted', 'blocked') AND failure_code IS NOT NULL)
    OR
    (execution_state NOT IN ('failed', 'timed_out', 'interrupted', 'blocked') AND failure_code IS NULL)
  )
);

CREATE INDEX comment_research_run_item_event_v1_item_time_idx
  ON comment_research_run_item_event_v1(run_item_id, occurred_at DESC, id DESC);

-- A conclusion is independent from execution mechanics. It can only be
-- success or no_signal; errors are represented by run-item events. The strict
-- JSON/typed validator lives in the contracts crate and must run before a
-- future writer inserts this immutable record.
CREATE TABLE comment_analysis_v1 (
  id UUID PRIMARY KEY,
  run_item_id UUID NOT NULL UNIQUE
    REFERENCES comment_research_run_item_v1(id) ON DELETE RESTRICT,
  comment_observation_id UUID NOT NULL REFERENCES comment_observation_v0(id) ON DELETE RESTRICT,
  comment_derivation_id UUID NOT NULL REFERENCES comment_derivation_v1(id) ON DELETE RESTRICT,
  research_fingerprint CHAR(64) NOT NULL
    CHECK (research_fingerprint ~ '^[0-9a-f]{64}$'),
  conclusion_state TEXT NOT NULL CHECK (conclusion_state IN ('success', 'no_signal')),
  structured_output JSONB NOT NULL,
  validated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT comment_analysis_v1_derivation_fingerprint_key
    UNIQUE (comment_derivation_id, research_fingerprint)
);

CREATE INDEX comment_analysis_v1_fingerprint_lookup_idx
  ON comment_analysis_v1(comment_derivation_id, research_fingerprint);

CREATE TRIGGER comment_research_run_v1_append_only
  BEFORE UPDATE OR DELETE ON comment_research_run_v1
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER comment_research_run_item_v1_append_only
  BEFORE UPDATE OR DELETE ON comment_research_run_item_v1
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER comment_research_run_item_event_v1_append_only
  BEFORE UPDATE OR DELETE ON comment_research_run_item_event_v1
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER comment_analysis_v1_append_only
  BEFORE UPDATE OR DELETE ON comment_analysis_v1
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();
