-- Claim a snapshot of source events. Writers append independently while this transaction
-- resolves the affected domains; later events remain for the next worker tick.
WITH claimed AS MATERIALIZED (
    SELECT event_id,event_kind,event_key,changed_at
    FROM linggan_opportunity_card_source_event
    ORDER BY event_id FOR UPDATE SKIP LOCKED
), keys AS MATERIALIZED (
    SELECT event_kind,event_key,max(changed_at) AS changed_at
    FROM claimed GROUP BY event_kind,event_key
), uuid_keys AS MATERIALIZED (
    -- Non-creator keys are written from UUID columns by the source triggers.
    SELECT event_kind,event_key::uuid AS event_ref,changed_at
    FROM keys WHERE event_kind<>'creator'
), affected AS (
    SELECT problem.problem_ref,uuid_keys.changed_at
    FROM uuid_keys JOIN linggan_opportunity_problem problem ON problem.domain_ref=uuid_keys.event_ref
    WHERE uuid_keys.event_kind='domain'
    UNION ALL
    SELECT problem.problem_ref,uuid_keys.changed_at
    FROM uuid_keys JOIN linggan_material_domain_usage usage ON usage.content_public_ref=uuid_keys.event_ref
    JOIN linggan_opportunity_problem problem ON problem.domain_ref=usage.domain_ref
    WHERE uuid_keys.event_kind='content' AND usage.use_kind='domain_research' AND usage.role='primary'
    UNION ALL
    SELECT density.problem_ref,keys.changed_at
    FROM keys JOIN linggan_opportunity_creator_problem_density density ON density.creator_id=keys.event_key
    WHERE keys.event_kind='creator'
    UNION ALL
    SELECT problem.problem_ref,uuid_keys.changed_at
    FROM uuid_keys JOIN observation_domain_target relation ON relation.target_ref=uuid_keys.event_ref
    JOIN linggan_opportunity_problem problem ON problem.domain_ref=relation.domain_ref
    WHERE uuid_keys.event_kind='target'
), latest AS MATERIALIZED (
    SELECT problem_ref,max(changed_at) AS changed_at FROM affected GROUP BY problem_ref
), enqueued AS (
    INSERT INTO linggan_opportunity_card_change(problem_ref,changed_at)
    SELECT problem_ref,changed_at FROM latest ORDER BY problem_ref
    ON CONFLICT(problem_ref) DO UPDATE SET
        changed_at=GREATEST(linggan_opportunity_card_change.changed_at,EXCLUDED.changed_at),
        revision=linggan_opportunity_card_change.revision+1
    RETURNING problem_ref
)
DELETE FROM linggan_opportunity_card_source_event event
USING claimed
WHERE event.event_id=claimed.event_id;
