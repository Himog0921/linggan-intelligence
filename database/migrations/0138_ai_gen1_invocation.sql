-- AI-GEN1: typed reservations and immutable terminal receipts. This migration does not
-- dispatch, manufacture old completion times, or infer retry permission from a code name.
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM linggan_model_invocation WHERE
   (state='running') IS DISTINCT FROM (finished_at IS NULL)) THEN
   RAISE EXCEPTION 'ai_invocation_preflight_required: running/finished_at conflict; inspect invocation_ref,state,created_at,finished_at before migration';
 END IF;
END $$;

CREATE TABLE linggan_ai_failure_code (
 code text PRIMARY KEY,
 retryable boolean,
 counts_as_failure boolean,
 description_zh text NOT NULL CHECK(length(description_zh)>0),
 classification_source text NOT NULL CHECK(classification_source IN ('executor_v1','legacy_unclassified','runtime_unclassified'))
);
INSERT INTO linggan_ai_failure_code VALUES
 ('ai_control_disabled',false,false,'工作已暂停，尚未派发','executor_v1'),
 ('ai_source_unavailable',false,false,'当前材料不具备读取资格，尚未派发','executor_v1'),
 ('ai_egress_denied',false,false,'当前外发许可不足，尚未派发','executor_v1'),
 ('ai_budget_exhausted',false,false,'额度不足，尚未派发','executor_v1'),
 ('ai_configuration_stale',false,false,'冻结配置与当前连接或能力不符，尚未派发','executor_v1'),
 ('ai_cancelled_before_dispatch',false,false,'在派发前取消','executor_v1'),
 ('ai_provider_failed',NULL,true,'供应商调用失败，是否可重试须核对派发与用量','executor_v1'),
 ('ai_output_invalid',true,true,'已返回内容未通过工作输出校验','executor_v1'),
 ('ai_dispatch_unknown',false,true,'调用可能已发出，不能自动重复发送','executor_v1');
INSERT INTO linggan_ai_failure_code
 SELECT DISTINCT failure_code,NULL::boolean,NULL::boolean,'历史失败码；重试与刹车语义未核实','legacy_unclassified'
 FROM linggan_model_invocation WHERE failure_code IS NOT NULL
 ON CONFLICT DO NOTHING;
CREATE TRIGGER ai_failure_code_immutable BEFORE UPDATE OR DELETE ON linggan_ai_failure_code
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

ALTER TABLE linggan_model_invocation
 ADD COLUMN task_key text REFERENCES linggan_ai_task,
 ADD COLUMN task_config_ref uuid REFERENCES linggan_ai_task_config,
 ADD COLUMN batch_ref uuid REFERENCES linggan_ai_batch,
 ADD COLUMN round_no integer CHECK(round_no>0),
 ADD COLUMN quota_bucket text NOT NULL DEFAULT 'legacy_unclassified'
   CHECK(quota_bucket IN ('legacy_unclassified','task_day','domain_day','run','request','probe')),
 ADD COLUMN budget_domain_ref uuid REFERENCES observation_domain,
 ADD COLUMN budget_run_ref uuid,
 ADD COLUMN budget_day date,
 ADD COLUMN usage_state text CHECK(usage_state IN ('measured','unknown_reserved','not_dispatched')),
 ADD COLUMN dispatch_state text NOT NULL DEFAULT 'unknown'
   CHECK(dispatch_state IN ('unknown','started','not_dispatched')),
 ADD COLUMN cached_input_tokens bigint CHECK(cached_input_tokens>=0),
 ADD COLUMN cached_write_tokens bigint CHECK(cached_write_tokens>=0),
 ADD COLUMN reasoning_tokens bigint CHECK(reasoning_tokens>=0),
 ADD COLUMN measured_tokens bigint CHECK(measured_tokens>=0),
 ADD COLUMN legacy_charged_tokens bigint CHECK(legacy_charged_tokens>=0),
 ADD COLUMN cost_pico_usd numeric(40,0) CHECK(cost_pico_usd>=0),
 ADD COLUMN price_ref uuid REFERENCES linggan_model_price,
 ADD COLUMN dispatch_admitted_at timestamptz,
 ADD COLUMN deadline_at timestamptz,
 ADD COLUMN egress_event_ref uuid REFERENCES linggan_model_egress_event,
 ADD COLUMN classification_source text NOT NULL DEFAULT 'legacy_unclassified'
   CHECK(classification_source IN ('legacy_unclassified','legacy_migration','producer_v1'));

-- Casting untrusted historic JSON must not abort classification or manufacture UUIDs.
CREATE FUNCTION linggan_ai_optional_uuid(value text) RETURNS uuid
 LANGUAGE plpgsql IMMUTABLE STRICT AS $$ BEGIN RETURN value::uuid;
 EXCEPTION WHEN invalid_text_representation THEN RETURN NULL; END $$;

-- Authoritative consumer relationships and exact old producer markers. Conflicting
-- owners remain unclassified; no cross-task budget can be charged by an arbitrary winner.
CREATE TEMP TABLE ai_invocation_claim(invocation_ref uuid,task_key text,run_ref uuid,domain_ref uuid) ON COMMIT DROP;
INSERT INTO ai_invocation_claim
 SELECT invocation_ref,CASE result->>'purpose'
   WHEN 'creator_discovery' THEN 'legacy_creator_discovery'
   WHEN 'topic_map' THEN 'legacy_topic_map' WHEN 'content_workbench' THEN 'writing_legacy' END,
   linggan_ai_optional_uuid(result->>'runRef'),d.domain_ref
 FROM linggan_model_invocation i LEFT JOIN observation_domain d
   ON d.domain_ref=linggan_ai_optional_uuid(i.result->>'domainRef')
 WHERE result->>'purpose' IN ('creator_discovery','topic_map','content_workbench')

 UNION ALL
 SELECT invocation_ref,'model_probe',NULL::uuid,NULL::uuid FROM linggan_model_invocation
 WHERE operation IN ('connect','discover','probe');
-- Consumer bootstrap schemas may be absent in a fresh installation. Only relationships
-- that actually exist are evidence; adding a model ledger must not create those consumers.
DO $$ BEGIN
 IF to_regclass('linggan_comment_study_model_request') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT q.invocation_ref,'comment_study',q.run_ref,policy.domain_ref
   FROM linggan_comment_study_model_request q JOIN linggan_comment_study_run r USING(run_ref)
   JOIN linggan_comment_study_policy policy ON policy.policy_ref=r.policy_ref$q$;
 END IF;
 IF to_regclass('linggan_comment_study_batch') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT b.model_invocation_ref,'comment_study',b.run_ref,policy.domain_ref
   FROM linggan_comment_study_batch b JOIN linggan_comment_study_run r USING(run_ref)
   JOIN linggan_comment_study_policy policy ON policy.policy_ref=r.policy_ref
   WHERE b.model_invocation_ref IS NOT NULL$q$;
 END IF;
 IF to_regclass('linggan_comment_study_resolution') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT s.model_invocation_ref,'comment_study',t.run_ref,policy.domain_ref
   FROM linggan_comment_study_resolution s JOIN linggan_comment_study_signal g USING(signal_ref)
   JOIN linggan_comment_study_target t USING(target_ref) JOIN linggan_comment_study_run r USING(run_ref)
   JOIN linggan_comment_study_policy policy ON policy.policy_ref=r.policy_ref
   WHERE s.model_invocation_ref IS NOT NULL$q$;
 END IF;
 IF to_regclass('linggan_comment_study_problem_pair') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT p.model_invocation_ref,'comment_study',t.run_ref,policy.domain_ref
   FROM linggan_comment_study_problem_pair p JOIN linggan_comment_study_signal g ON g.signal_ref=p.first_signal_ref
   JOIN linggan_comment_study_target t USING(target_ref) JOIN linggan_comment_study_run r USING(run_ref)
   JOIN linggan_comment_study_policy policy ON policy.policy_ref=r.policy_ref
   WHERE p.model_invocation_ref IS NOT NULL$q$;
 END IF;
 IF to_regclass('linggan_comment_study_run') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT i.invocation_ref,'comment_study',r.run_ref,policy.domain_ref
   FROM linggan_model_invocation i JOIN linggan_comment_study_run r
    ON r.run_ref=linggan_ai_optional_uuid(i.result->>'legacyRunRef')
   JOIN linggan_comment_study_policy policy ON policy.policy_ref=r.policy_ref
   WHERE i.result->>'legacyRequestLedgerMissing'='true'$q$;
 END IF;
 IF to_regclass('linggan_topic_map_research_request') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_claim SELECT q.invocation_ref,'legacy_topic_map',q.run_ref,r.domain_ref
   FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_run r USING(run_ref)$q$;
 END IF;
 IF to_regclass('linggan_creation_ai_request') IS NOT NULL THEN
  INSERT INTO ai_invocation_claim SELECT invocation_ref,'writing_legacy',NULL::uuid,NULL::uuid FROM linggan_creation_ai_request;
 END IF;
END $$;
CREATE TEMP TABLE ai_invocation_owner ON COMMIT DROP AS
 SELECT invocation_ref,min(task_key) AS task_key,
 CASE WHEN count(DISTINCT run_ref)=1 THEN min(run_ref::text)::uuid END AS run_ref,
 CASE WHEN count(DISTINCT domain_ref)=1 THEN min(domain_ref::text)::uuid END AS domain_ref
 FROM ai_invocation_claim GROUP BY invocation_ref HAVING count(DISTINCT task_key)=1;
UPDATE linggan_model_invocation i SET task_key=o.task_key,budget_run_ref=o.run_ref,budget_domain_ref=o.domain_ref,
 classification_source='legacy_migration' FROM ai_invocation_owner o WHERE i.invocation_ref=o.invocation_ref;
UPDATE linggan_model_invocation SET task_key='legacy_unclassified' WHERE task_key IS NULL;
UPDATE linggan_model_invocation SET legacy_charged_tokens=charged_tokens,quota_bucket=CASE task_key
 WHEN 'model_probe' THEN 'probe' WHEN 'writing_legacy' THEN 'request'
 WHEN 'legacy_creator_discovery' THEN 'domain_day'
 WHEN 'comment_study' THEN 'run' WHEN 'legacy_topic_map' THEN 'run' ELSE 'legacy_unclassified' END,
 budget_day=linggan_ai_budget_day(created_at);
CREATE TEMP TABLE ai_invocation_dispatch(invocation_ref uuid,started_at timestamptz,deadline_at timestamptz,not_dispatched boolean) ON COMMIT DROP;
DO $$ BEGIN
 IF to_regclass('linggan_comment_study_model_request') IS NOT NULL THEN
  EXECUTE $q$INSERT INTO ai_invocation_dispatch SELECT invocation_ref,dispatch_started_at,deadline_at,dispatch_started_at IS NULL FROM linggan_comment_study_model_request$q$;
 END IF;
 IF to_regclass('linggan_topic_map_research_request') IS NOT NULL THEN
  INSERT INTO ai_invocation_dispatch SELECT invocation_ref,dispatch_started_at,deadline_at,dispatch_started_at IS NULL FROM linggan_topic_map_research_request;
 END IF;
 IF to_regclass('linggan_creation_ai_request') IS NOT NULL THEN
  INSERT INTO ai_invocation_dispatch SELECT invocation_ref,NULL,NULL,NOT dispatch_started FROM linggan_creation_ai_request;
 END IF;
END $$;
UPDATE linggan_model_invocation i SET deadline_at=d.deadline_at,dispatch_admitted_at=d.started_at
 FROM (SELECT invocation_ref,max(started_at) AS started_at,min(deadline_at) AS deadline_at
       FROM ai_invocation_dispatch GROUP BY invocation_ref)d WHERE i.invocation_ref=d.invocation_ref;
-- A completed request with NULL dispatch fence, or an explicit producer false marker,
-- proves no dispatch only when no positive usage/positive marker contradicts it.
UPDATE linggan_model_invocation i SET dispatch_state=CASE
 WHEN result->>'callStarted'='true' OR result->>'dispatchStarted'='true' OR dispatch_admitted_at IS NOT NULL
   OR COALESCE(input_tokens,0)>0 OR COALESCE(output_tokens,0)>0 THEN 'started'
 WHEN state<>'running' AND (
   result->>'callStarted'='false' OR result->>'dispatchStarted'='false'
   OR EXISTS(SELECT 1 FROM ai_invocation_dispatch q WHERE q.invocation_ref=i.invocation_ref AND q.not_dispatched)
 ) THEN 'not_dispatched' ELSE 'unknown' END;
UPDATE linggan_model_invocation SET
 usage_state=CASE WHEN dispatch_state='not_dispatched' THEN 'not_dispatched'
   WHEN input_tokens IS NOT NULL AND output_tokens IS NOT NULL THEN 'measured' ELSE 'unknown_reserved' END,
 measured_tokens=CASE WHEN dispatch_state='not_dispatched' THEN 0
   WHEN input_tokens IS NOT NULL AND output_tokens IS NOT NULL THEN input_tokens+output_tokens END,
 charged_tokens=CASE WHEN dispatch_state='not_dispatched' THEN 0
   WHEN input_tokens IS NOT NULL AND output_tokens IS NOT NULL THEN input_tokens+output_tokens
   ELSE GREATEST(reserved_tokens,charged_tokens,COALESCE(input_tokens,0)+COALESCE(output_tokens,0)) END;

ALTER TABLE linggan_model_invocation
 ALTER COLUMN task_key SET NOT NULL,
 ALTER COLUMN task_key SET DEFAULT 'legacy_unclassified',
 ALTER COLUMN budget_day SET NOT NULL,
 ALTER COLUMN usage_state SET NOT NULL,
 ADD CONSTRAINT ai_invocation_terminal_time_ck CHECK((state='running')=(finished_at IS NULL)),
 ADD CONSTRAINT ai_invocation_failure_fk FOREIGN KEY(failure_code) REFERENCES linggan_ai_failure_code,
 ADD CONSTRAINT ai_invocation_cache_ck CHECK(input_tokens IS NULL OR
   COALESCE(cached_input_tokens,0)::numeric+COALESCE(cached_write_tokens,0)<=input_tokens),
 ADD CONSTRAINT ai_invocation_reasoning_ck CHECK(output_tokens IS NULL OR reasoning_tokens<=output_tokens),
 ADD CONSTRAINT ai_invocation_usage_ck CHECK(
   (usage_state='not_dispatched' AND dispatch_state='not_dispatched' AND measured_tokens=0 AND charged_tokens=0)
   OR (usage_state='measured' AND dispatch_state<>'not_dispatched' AND input_tokens IS NOT NULL
     AND output_tokens IS NOT NULL AND measured_tokens=input_tokens+output_tokens AND charged_tokens=measured_tokens)
   OR (usage_state='unknown_reserved' AND dispatch_state<>'not_dispatched' AND measured_tokens IS NULL
     AND (input_tokens IS NULL OR output_tokens IS NULL) AND charged_tokens>=reserved_tokens)
 );

CREATE FUNCTION linggan_ai_invocation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE known_input bigint; known_output bigint; price linggan_model_price%ROWTYPE;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ai_invocation_delete_forbidden'; END IF;
 IF TG_OP='UPDATE' THEN
   IF OLD.state<>'running' THEN RAISE EXCEPTION 'ai_invocation_terminal_immutable'; END IF;
   IF ROW(NEW.invocation_ref,NEW.connection_version_ref,NEW.model_ref,NEW.config_ref,
      NEW.operation,NEW.request_hash,NEW.reserved_tokens,NEW.created_at,NEW.task_key,NEW.task_config_ref,
      NEW.batch_ref,NEW.round_no,NEW.quota_bucket,NEW.budget_domain_ref,NEW.budget_run_ref,NEW.budget_day,NEW.classification_source,NEW.legacy_charged_tokens)
     IS DISTINCT FROM
      ROW(OLD.invocation_ref,OLD.connection_version_ref,OLD.model_ref,OLD.config_ref,
      OLD.operation,OLD.request_hash,OLD.reserved_tokens,OLD.created_at,OLD.task_key,OLD.task_config_ref,
      OLD.batch_ref,OLD.round_no,OLD.quota_bucket,OLD.budget_domain_ref,OLD.budget_run_ref,OLD.budget_day,OLD.classification_source,OLD.legacy_charged_tokens)
   THEN RAISE EXCEPTION 'ai_invocation_identity_immutable'; END IF;
   IF (OLD.dispatch_state='started' AND NEW.dispatch_state<>'started')
     OR (OLD.price_ref IS NOT NULL AND NEW.price_ref IS DISTINCT FROM OLD.price_ref)
     OR (OLD.egress_event_ref IS NOT NULL AND NEW.egress_event_ref IS DISTINCT FROM OLD.egress_event_ref)
     OR (OLD.dispatch_admitted_at IS NOT NULL AND NEW.dispatch_admitted_at IS DISTINCT FROM OLD.dispatch_admitted_at)
     OR (OLD.deadline_at IS NOT NULL AND NEW.deadline_at IS DISTINCT FROM OLD.deadline_at)
     OR (OLD.input_tokens IS NOT NULL AND NEW.input_tokens IS DISTINCT FROM OLD.input_tokens)
     OR (OLD.output_tokens IS NOT NULL AND NEW.output_tokens IS DISTINCT FROM OLD.output_tokens)
     OR (OLD.cached_input_tokens IS NOT NULL AND NEW.cached_input_tokens IS DISTINCT FROM OLD.cached_input_tokens)
     OR (OLD.cached_write_tokens IS NOT NULL AND NEW.cached_write_tokens IS DISTINCT FROM OLD.cached_write_tokens)
     OR (OLD.reasoning_tokens IS NOT NULL AND NEW.reasoning_tokens IS DISTINCT FROM OLD.reasoning_tokens)
   THEN RAISE EXCEPTION 'ai_invocation_evidence_conflict'; END IF;
 ELSE
   NEW.budget_day=linggan_ai_budget_day(NEW.created_at);
 END IF;
 -- Frozen legacy consumers still supply explicit markers. New executor writes the typed
 -- dispatch column at its fence; absence and charged_tokens=0 never prove no dispatch.
 IF NEW.result->>'callStarted'='true' OR NEW.result->>'dispatchStarted'='true' THEN
   NEW.dispatch_state='started';
 ELSIF NEW.state<>'running' AND NEW.dispatch_state='unknown'
   AND (NEW.result->>'callStarted'='false' OR NEW.result->>'dispatchStarted'='false') THEN
   NEW.dispatch_state='not_dispatched';
 END IF;
 known_input=COALESCE(NEW.input_tokens,COALESCE(NEW.cached_input_tokens,0)+COALESCE(NEW.cached_write_tokens,0));
 known_output=COALESCE(NEW.output_tokens,NEW.reasoning_tokens,0);
 IF NEW.dispatch_state='not_dispatched' THEN
   IF known_input>0 OR known_output>0 THEN RAISE EXCEPTION 'ai_not_dispatched_positive_usage'; END IF;
   NEW.usage_state='not_dispatched'; NEW.measured_tokens=0; NEW.charged_tokens=0;
 ELSIF NEW.input_tokens IS NOT NULL AND NEW.output_tokens IS NOT NULL THEN
   NEW.usage_state='measured'; NEW.measured_tokens=NEW.input_tokens+NEW.output_tokens;
   NEW.charged_tokens=NEW.measured_tokens;
 ELSE
   NEW.usage_state='unknown_reserved'; NEW.measured_tokens=NULL;
   NEW.charged_tokens=GREATEST(NEW.reserved_tokens,NEW.charged_tokens,known_input+known_output);
 END IF;
 -- The quote is derived from the frozen price receipt, never an SDK default zero.
 NEW.cost_pico_usd=NULL;
 IF NEW.price_ref IS NOT NULL THEN
   SELECT p.* INTO price FROM linggan_model_price p
    JOIN linggan_ai_model_identity identity USING(identity_ref)
    JOIN linggan_model_entry model ON model.model_ref=NEW.model_ref AND model.model_id=identity.model_id
    JOIN linggan_model_connection_version model_version ON model_version.version_ref=model.connection_version_ref
      AND model_version.connection_ref=identity.connection_ref
    JOIN linggan_ai_connection_endpoint endpoint ON endpoint.version_ref=NEW.connection_version_ref
      AND endpoint.connection_ref=identity.connection_ref AND endpoint.endpoint_ref=p.endpoint_ref
    WHERE p.price_ref=NEW.price_ref;
   IF NOT FOUND THEN RAISE EXCEPTION 'ai_invocation_price_mismatch'; END IF;
   IF NEW.usage_state='measured' AND NEW.cached_input_tokens IS NOT NULL AND NEW.cached_write_tokens IS NOT NULL THEN
     NEW.cost_pico_usd=
       CASE WHEN NEW.input_tokens-NEW.cached_input_tokens-NEW.cached_write_tokens=0 THEN 0
         ELSE (NEW.input_tokens::numeric-NEW.cached_input_tokens-NEW.cached_write_tokens)*price.input_micro_usd_per_million END
       +CASE WHEN NEW.output_tokens=0 THEN 0 ELSE NEW.output_tokens::numeric*price.output_micro_usd_per_million END
       +CASE WHEN NEW.cached_input_tokens=0 THEN 0 ELSE NEW.cached_input_tokens::numeric*price.cached_read_micro_usd_per_million END
       +CASE WHEN NEW.cached_write_tokens=0 THEN 0 ELSE NEW.cached_write_tokens::numeric*price.cached_write_micro_usd_per_million END;
   END IF;
 END IF;
 IF NEW.dispatch_state='not_dispatched' THEN NEW.cost_pico_usd=0; END IF;
 IF NEW.task_config_ref IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_ai_task_config c
   WHERE c.task_config_ref=NEW.task_config_ref AND c.task_key=NEW.task_key AND c.model_config_ref IS NOT DISTINCT FROM NEW.config_ref)
 THEN RAISE EXCEPTION 'ai_invocation_config_mismatch'; END IF;
 IF NEW.batch_ref IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch b
   WHERE b.batch_ref=NEW.batch_ref AND b.task_key=NEW.task_key AND b.task_config_ref=NEW.task_config_ref)
 THEN RAISE EXCEPTION 'ai_invocation_batch_mismatch'; END IF;
 IF NEW.egress_event_ref IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_model_egress_event e
   JOIN linggan_ai_connection_endpoint route ON route.endpoint_ref=e.endpoint_ref
   WHERE e.event_ref=NEW.egress_event_ref AND e.allowed AND e.task_key=NEW.task_key AND route.version_ref=NEW.connection_version_ref)
 THEN RAISE EXCEPTION 'ai_invocation_egress_mismatch'; END IF;
 IF NEW.failure_code IS NOT NULL THEN
   INSERT INTO linggan_ai_failure_code VALUES
    (NEW.failure_code,NULL,NULL,'既有消费者失败码；重试与刹车语义未核实','runtime_unclassified') ON CONFLICT DO NOTHING;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_invocation_guard BEFORE INSERT OR UPDATE OR DELETE ON linggan_model_invocation
 FOR EACH ROW EXECUTE FUNCTION linggan_ai_invocation_guard();
CREATE INDEX ai_invocation_task_recent_idx ON linggan_model_invocation(task_key,created_at DESC);
CREATE INDEX ai_invocation_task_day_idx ON linggan_model_invocation(task_key,budget_day)
 INCLUDE(budget_domain_ref,budget_run_ref,charged_tokens,usage_state);
CREATE INDEX ai_invocation_run_idx ON linggan_model_invocation(task_key,budget_run_ref) INCLUDE(charged_tokens,state);
CREATE INDEX ai_invocation_domain_day_idx ON linggan_model_invocation(task_key,budget_domain_ref,budget_day) INCLUDE(charged_tokens);
CREATE INDEX ai_invocation_batch_idx ON linggan_model_invocation(batch_ref) WHERE batch_ref IS NOT NULL;
CREATE INDEX ai_invocation_running_deadline_idx ON linggan_model_invocation(state,deadline_at) WHERE state='running';
CREATE INDEX ai_invocation_config_idx ON linggan_model_invocation(config_ref);
