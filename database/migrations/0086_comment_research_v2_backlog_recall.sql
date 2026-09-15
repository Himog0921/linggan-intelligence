-- COMMENT-RESEARCH-V2-BACKLOG-RECALL-001
--
-- A V2 resolution whose policy or catalog changed must refresh its closed-world candidate
-- snapshot before another comparison. `pending` alone cannot carry that distinction: a worker
-- may only claim a pending resolution after `catalog_revision_at_recall` is frozen.

ALTER TABLE linggan_comment_research_problem_resolution
  ADD COLUMN candidate_recall_required boolean NOT NULL DEFAULT false;

CREATE INDEX linggan_comment_research_problem_resolution_recall_queue_idx
  ON linggan_comment_research_problem_resolution(execution_run_ref,created_at,atom_ref)
  WHERE candidate_recall_required
    AND state IN ('pending','retryable');

COMMENT ON COLUMN linggan_comment_research_problem_resolution.candidate_recall_required IS
  'True only for a V2 cross-Run re-evaluation awaiting a fresh candidate snapshot and catalog revision; it is never a provider-call state.';
