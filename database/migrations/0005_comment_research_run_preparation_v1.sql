-- Comment Research Run Preparation V1.
--
-- 0004 deliberately had no creator and therefore no initial event. A frozen
-- input is neither queued nor running when no executor exists, so preparation
-- records an explicit append-only `prepared` lifecycle observation.

ALTER TABLE comment_research_run_item_event_v1
  DROP CONSTRAINT comment_research_run_item_event_v1_execution_state_check;

ALTER TABLE comment_research_run_item_event_v1
  ADD CONSTRAINT comment_research_run_item_event_v1_execution_state_check
  CHECK (execution_state IN (
    'prepared', 'queued', 'running', 'completed', 'failed', 'timed_out',
    'interrupted', 'awaiting_recovery', 'cancelled', 'blocked'
  ));
