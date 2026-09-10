-- COMMENT-RESEARCH-RESET-001: bounded, auditable admission decisions after vector recall.
--
-- A vector is only a candidate generator.  This queue freezes the exact candidates shown to the
-- semantic model, records the invocation that made the same/new decision, and gives permanent
-- model failures a terminal state instead of repeatedly spending calls on the same Atom.

CREATE TABLE linggan_comment_research_problem_resolution (
    atom_ref uuid PRIMARY KEY REFERENCES linggan_comment_research_atom(atom_ref),
    space_ref uuid NOT NULL REFERENCES linggan_comment_research_embedding_space(space_ref),
    candidate_set jsonb NOT NULL CHECK(jsonb_typeof(candidate_set)='array'),
    candidate_hash text NOT NULL CHECK(candidate_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('pending','running','retryable','succeeded','model_failed','incompatible')),
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0 AND attempts <= 3),
    next_attempt_at timestamptz,
    failure_code text,
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    lease_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    CHECK((state IN ('succeeded','model_failed','incompatible')) = (finished_at IS NOT NULL)),
    CHECK((state='retryable') = (next_attempt_at IS NOT NULL))
);

CREATE INDEX linggan_comment_research_problem_resolution_queue_idx
  ON linggan_comment_research_problem_resolution(state,next_attempt_at,created_at)
  WHERE state IN ('pending','retryable','running');

COMMENT ON TABLE linggan_comment_research_problem_resolution IS
  'A frozen Top-K candidate set and bounded model decision for one problem-bearing Atom. Candidate recall never writes membership by itself.';
