-- COMMENT-RESEARCH-EXECUTION-RECOVERY-001: an embedding row is owned by one bounded lease.
-- The existing embedding checkpoint remains authoritative; this adds recovery and fencing.

ALTER TABLE linggan_comment_research_atom_embedding
  DROP CONSTRAINT linggan_comment_research_atom_embedding_state_check,
  ADD CONSTRAINT linggan_comment_research_atom_embedding_state_check
    CHECK(state IN ('pending','running','retryable','succeeded','failed','incompatible')),
  ADD COLUMN attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  ADD COLUMN lease_until timestamptz,
  ADD COLUMN next_attempt_at timestamptz;

CREATE INDEX linggan_comment_research_atom_embedding_claim_idx
  ON linggan_comment_research_atom_embedding(space_ref,state,next_attempt_at,created_at,atom_ref);

COMMENT ON COLUMN linggan_comment_research_atom_embedding.attempts IS
  'Monotonic claim generation for an Atom embedding; every owner write is fenced by it.';
COMMENT ON COLUMN linggan_comment_research_atom_embedding.lease_until IS
  'Bounded running ownership. Expiry recovers to retryable or terminal failed.';
