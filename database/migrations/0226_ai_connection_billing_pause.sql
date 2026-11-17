-- An HTTP 402 is transport evidence, not an egress revocation or measured token use.
CREATE TABLE linggan_ai_connection_billing_pause (
 connection_ref uuid PRIMARY KEY REFERENCES linggan_model_connection,
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation,
 opened_at timestamptz NOT NULL
);

-- An explicit successful model probe on the current connection version can
-- resume dispatch. Older/in-flight probes cannot clear a newer billing failure.
CREATE FUNCTION linggan_ai_billing_paused(connection_id uuid) RETURNS boolean
 LANGUAGE sql STABLE AS $$
 SELECT EXISTS (
   SELECT 1 FROM linggan_ai_connection_billing_pause pause
   WHERE pause.connection_ref=connection_id AND NOT EXISTS (
     SELECT 1 FROM linggan_model_invocation probe
     WHERE probe.task_key='model_probe' AND probe.operation='probe' AND probe.state='succeeded'
       AND probe.connection_version_ref=(
         SELECT v.version_ref FROM linggan_model_connection_version v
         WHERE v.connection_ref=connection_id ORDER BY v.revision DESC LIMIT 1
       )
       AND probe.created_at>pause.opened_at
       AND probe.dispatch_state='started'
       AND probe.result->'modelCallable'='true'::jsonb
       AND probe.result#>>'{diagnostic,httpStatus}'='200'
       AND probe.result#>>'{diagnostic,terminalReceived}'='true'
   )
 );
$$;
