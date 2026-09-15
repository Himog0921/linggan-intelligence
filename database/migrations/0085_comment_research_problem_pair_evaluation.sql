-- COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 · durable pair evaluation
--
-- This is a worker checkpoint, not a user-facing Problem or a second backlog product. It records
-- one bounded semantic comparison of two independently eligible deferred signals, so a rejected
-- pair is not sent to the model again until its frozen catalog revision changes.

CREATE TABLE linggan_comment_research_problem_pair_evaluation (
    pair_evaluation_ref uuid PRIMARY KEY,
    first_atom_ref uuid NOT NULL REFERENCES linggan_comment_research_problem_resolution(atom_ref),
    second_atom_ref uuid NOT NULL REFERENCES linggan_comment_research_problem_resolution(atom_ref),
    execution_run_ref uuid NOT NULL REFERENCES linggan_comment_research_run(run_ref),
    scope_domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    catalog_revision_at_recall bigint NOT NULL CHECK(catalog_revision_at_recall >= 0),
    pair_input jsonb NOT NULL CHECK(jsonb_typeof(pair_input)='object'),
    pair_input_hash text NOT NULL CHECK(pair_input_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('pending','running','retryable','succeeded','model_failed','incompatible')),
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0),
    next_attempt_at timestamptz,
    lease_until timestamptz,
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    failure_code text,
    decision_kind text,
    decision_payload jsonb,
    recheck_conditions jsonb,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    CHECK(first_atom_ref < second_atom_ref),
    CHECK((state IN ('succeeded','model_failed','incompatible')) = (finished_at IS NOT NULL)),
    CHECK((state='retryable') = (next_attempt_at IS NOT NULL)),
    CHECK(
      (decision_kind IS NULL AND decision_payload IS NULL AND recheck_conditions IS NULL)
      OR (
        decision_kind IN ('created_problem','deferred_novel','deferred_ambiguous','protocol_failure','reevaluate_catalog')
        AND jsonb_typeof(decision_payload)='object'
        AND jsonb_typeof(recheck_conditions)='array'
      )
    ),
    UNIQUE(first_atom_ref,second_atom_ref,catalog_revision_at_recall,pair_input_hash)
);

CREATE INDEX linggan_comment_research_problem_pair_evaluation_queue_idx
  ON linggan_comment_research_problem_pair_evaluation(execution_run_ref,state,next_attempt_at,created_at)
  WHERE state IN ('pending','retryable','running');

COMMENT ON TABLE linggan_comment_research_problem_pair_evaluation IS
  'Internal V2 pair-comparison checkpoint. Its rows are never shown as Problems or user actions.';
