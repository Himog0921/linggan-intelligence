-- KEYWORD-MONITORING-LIFECYCLE-001
--
-- Keyword observation has a monitor lifecycle, not a creator archive lifecycle.
-- Repair historical rows with an append-only transition before enforcing that
-- invariant, so a repaired target has an auditable reason instead of a silent
-- state rewrite.

WITH invalid_keyword_state AS (
    SELECT target.target_ref,
           target.lifecycle_state AS from_state,
           CASE
               WHEN target.monitoring_enabled
                AND COALESCE(rule.automatic_enabled, false)
                AND COALESCE(rule.mode, '') = 'fixed'
                   THEN 'monitoring'
               ELSE 'paused'
           END AS to_state
    FROM collection_observation_target target
    LEFT JOIN collection_monitor_rule_revision rule
      ON rule.rule_revision_ref = target.active_monitor_rule_revision_ref
    WHERE target.target_kind = 'keyword'
      AND target.lifecycle_state IN ('archiving', 'archived')
), repaired_transition AS (
    INSERT INTO collection_observation_target_transition
        (transition_ref,target_ref,from_state,to_state,actor,reason_code,reason)
    SELECT gen_random_uuid(),target_ref,from_state,to_state,'system',
           'keyword_lifecycle_repaired_0042',
           '0042 repaired a creator-only lifecycle state on a keyword observation target'
    FROM invalid_keyword_state
)
UPDATE collection_observation_target target
SET lifecycle_state = invalid.to_state,
    lifecycle_changed_at = scope_001_now(),
    monitoring_enabled = (invalid.to_state = 'monitoring'),
    monitor_schedule_anchor_at = CASE WHEN invalid.to_state = 'monitoring'
                                      THEN target.monitor_schedule_anchor_at ELSE NULL END,
    monitor_next_run_at = CASE WHEN invalid.to_state = 'monitoring'
                               THEN target.monitor_next_run_at ELSE NULL END,
    monitor_schedule_slot_seconds = CASE WHEN invalid.to_state = 'monitoring'
                                         THEN target.monitor_schedule_slot_seconds ELSE 0 END
FROM invalid_keyword_state invalid
WHERE target.target_ref = invalid.target_ref;

ALTER TABLE collection_observation_target
    ADD CONSTRAINT collection_observation_target_keyword_never_archives
    CHECK (target_kind <> 'keyword' OR lifecycle_state NOT IN ('archiving', 'archived'));
