-- COLLECTION-CONTROL-CLOSURE-001 / Issue #149 scope amendment
--
-- A station without an installation cannot be handed work.  Once a current
-- Browser Producer installation is claimed, the normal state is therefore
-- automatic acceptance.  A later person pause remains a durable override and
-- is never silently reversed by a reinstall or a heartbeat.

ALTER TABLE execution_station_acceptance_transition
    DROP CONSTRAINT IF EXISTS execution_station_acceptance_transition_reason_code_check;

ALTER TABLE execution_station_acceptance_transition
    ADD CONSTRAINT execution_station_acceptance_transition_reason_code_check
    CHECK (reason_code IN (
        'registered_closed',
        'registered_awaiting_claim',
        'person_enabled',
        'person_disabled',
        'migration_closed',
        'migration_auto_enabled',
        'installation_claimed_auto_enabled',
        'station_retired'
    ));

-- Carry forward only the old default states. A person-disabled station is an
-- explicit safety decision, so it stays paused. The deterministic id
-- makes an accidental isolated replay fail rather than duplicate the audit
-- trail.
WITH auto_enabled AS (
    UPDATE execution_station station
       SET accepting_tasks = true
     WHERE station.accepting_tasks = false
       AND station.retired_at IS NULL
       AND EXISTS (
           SELECT 1
             FROM plugin_installation installation
            WHERE installation.station_ref = station.station_ref
              AND installation.superseded_at IS NULL
       )
       AND (
           SELECT transition.reason_code
             FROM execution_station_acceptance_transition transition
            WHERE transition.station_ref = station.station_ref
            ORDER BY transition.occurred_at DESC, transition.transition_ref DESC
            LIMIT 1
       ) IN ('migration_closed','registered_closed')
    RETURNING station.station_ref
)
INSERT INTO execution_station_acceptance_transition (
    transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code
)
SELECT md5(station_ref::text || ':0035:migration_auto_enabled')::uuid,
       station_ref,false,true,'system','migration_auto_enabled'
FROM auto_enabled;

COMMENT ON COLUMN execution_station.accepting_tasks IS
    'Current automatic-acceptance mode. A claimed current installation enables it by default; a person may explicitly pause it, and later heartbeats or reinstalls must preserve that pause.';

COMMENT ON TABLE execution_station_acceptance_transition IS
    'Append-only automatic-acceptance and explicit-pause audit trail. A current claimed installation starts accepting by default; a person pause is never silently undone.';
