-- COMMENT-RESEARCH-CUMULATIVE-STATE-001
--
-- A problem-resolution row belongs to one long-lived Atom, but its provider work may be resumed
-- by a later explicitly started Run. Keep the source Run immutable for statistics while storing
-- the currently accountable execution Run separately. This prevents retry-only executions from
-- rewriting source evidence or pretending to be a new frozen statistical sample.

ALTER TABLE linggan_comment_research_problem_resolution
  ADD COLUMN execution_run_ref uuid REFERENCES linggan_comment_research_run(run_ref),
  ADD COLUMN last_attempt_at timestamptz;

UPDATE linggan_comment_research_problem_resolution resolution
SET execution_run_ref=atom.run_ref,
    last_attempt_at=CASE WHEN resolution.attempts>0 THEN resolution.updated_at ELSE NULL END
FROM linggan_comment_research_atom atom
WHERE atom.atom_ref=resolution.atom_ref;

ALTER TABLE linggan_comment_research_problem_resolution
  ALTER COLUMN execution_run_ref SET NOT NULL,
  DROP CONSTRAINT linggan_comment_research_problem_resolution_attempts_check,
  ADD CONSTRAINT linggan_comment_research_problem_resolution_attempts_nonnegative
    CHECK(attempts >= 0);

CREATE INDEX linggan_comment_research_problem_resolution_execution_queue_idx
  ON linggan_comment_research_problem_resolution(execution_run_ref,state,next_attempt_at,created_at)
  WHERE state IN ('pending','retryable','running');

-- `execution_run_ref` is the live queue pointer. Keep a separate per-activation record so a
-- later Run taking over the same Atom cannot erase what an earlier Run actually attempted.
CREATE TABLE linggan_comment_research_problem_resolution_execution (
    atom_ref uuid NOT NULL REFERENCES linggan_comment_research_problem_resolution(atom_ref),
    run_ref uuid NOT NULL REFERENCES linggan_comment_research_run(run_ref),
    state text NOT NULL CHECK(state IN ('pending','running','retryable','succeeded','model_failed','incompatible')),
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0),
    last_attempt_at timestamptz,
    next_attempt_at timestamptz,
    failure_code text,
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    PRIMARY KEY(atom_ref,run_ref),
    CHECK((state IN ('succeeded','model_failed','incompatible')) = (finished_at IS NOT NULL)),
    CHECK((state='retryable') = (next_attempt_at IS NOT NULL))
);

CREATE INDEX linggan_comment_research_problem_resolution_execution_run_idx
  ON linggan_comment_research_problem_resolution_execution(run_ref,state,created_at);

INSERT INTO linggan_comment_research_problem_resolution_execution(
    atom_ref,run_ref,state,attempts,last_attempt_at,next_attempt_at,failure_code,invocation_ref,created_at,updated_at,finished_at
)
SELECT atom_ref,execution_run_ref,state,attempts,last_attempt_at,next_attempt_at,failure_code,invocation_ref,created_at,updated_at,finished_at
FROM linggan_comment_research_problem_resolution;

COMMENT ON COLUMN linggan_comment_research_problem_resolution.execution_run_ref IS
  'The latest explicitly started Run accountable for executing this resolution. atom.run_ref remains the immutable source/statistical Run.';
COMMENT ON COLUMN linggan_comment_research_problem_resolution.last_attempt_at IS
  'The most recent provider-attempt start for this resolution; attempts are bounded within each execution activation and historical invocations remain in the ledger.';
COMMENT ON TABLE linggan_comment_research_problem_resolution_execution IS
  'One durable execution history row per Atom and explicitly started Run. Later cross-Run retry activation never rewrites an earlier Run history.';
