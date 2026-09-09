-- COMMENT-RESEARCH-RESET-001: a V1 worker claim is recoverable execution state, not a
-- permanent lock.  A stale caller keeps its ModelInvocation receipt (unknown usage remains
-- charged); the input item becomes retryable or terminal according to its bounded attempt count.

ALTER TABLE linggan_comment_research_run_item
  ADD COLUMN lease_until timestamptz;

CREATE INDEX linggan_comment_research_run_item_lease_idx
  ON linggan_comment_research_run_item(state,lease_until)
  WHERE state='running';

COMMENT ON COLUMN linggan_comment_research_run_item.lease_until IS
  'Bounded V1 semantic-extraction lease. Expiry is recovered to a truthful retry/failure state; it never blocks later healthy items.';
