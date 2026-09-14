-- COMMENT-RESEARCH-EXECUTION-RECOVERY-001: an embedding row is owned by one bounded lease.
-- The existing embedding checkpoint remains authoritative; this adds recovery and fencing.

ALTER TABLE linggan_comment_research_atom_embedding
  DROP CONSTRAINT linggan_comment_research_atom_embedding_state_check,
  ADD CONSTRAINT linggan_comment_research_atom_embedding_state_check
    CHECK(state IN ('pending','running','retryable','succeeded','failed','incompatible')),
  ADD COLUMN attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0),
  ADD COLUMN lease_until timestamptz,
  ADD COLUMN next_attempt_at timestamptz;

-- `lease_until` arrives NULL on every existing row. The recovery pass reads it as a deadline
-- (`lease_until<=scope_001_now()`), so NULL never matches and such a row is never recovered;
-- the claim pass takes only `pending`/`retryable` rows, so it skips them as well. A row already
-- `running` when this migration lands therefore falls between the two: never reclaimed, never
-- recovered, and the run it belongs to never completes — with no error to explain it.
-- Expire those rows here instead, so the first recovery pass returns them to `retryable` under
-- whatever attempt count they already carry. From here on the invariant holds by construction:
-- the claim path is the only writer of `state='running'`, and it always sets a lease with it.
UPDATE linggan_comment_research_atom_embedding
   SET lease_until=scope_001_now()
 WHERE state='running';

CREATE INDEX linggan_comment_research_atom_embedding_claim_idx
  ON linggan_comment_research_atom_embedding(space_ref,state,next_attempt_at,created_at,atom_ref);

COMMENT ON COLUMN linggan_comment_research_atom_embedding.attempts IS
  'Monotonic claim generation for an Atom embedding; every owner write is fenced by it.';
COMMENT ON COLUMN linggan_comment_research_atom_embedding.lease_until IS
  'Bounded running ownership. Expiry recovers to retryable or terminal failed.';
