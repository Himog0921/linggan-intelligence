-- CI-AUTO-004 P1: policy is explicit; older grants and accepted analyses remain historical.
-- The JSON shape is constrained here as well as by the Rust DTO so direct SQL cannot silently
-- create a new automatic outbound authorization.
CREATE FUNCTION linggan_comment_auto_policy_valid(policy jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE WHEN jsonb_typeof(policy)='object'
  AND policy ?& ARRAY['continuousNew','historicalEnabled','historyStart','outdatedPolicy','unknownRetryMaxAttempts','unknownRetryTokenLimit','dayTokenLimit','replayTokenLimit','semanticTokenLimit']
  AND (policy - ARRAY['continuousNew','historicalEnabled','historyStart','outdatedPolicy','unknownRetryMaxAttempts','unknownRetryTokenLimit','dayTokenLimit','replayTokenLimit','semanticTokenLimit'])='{}'::jsonb
  AND jsonb_typeof(policy->'continuousNew')='boolean'
  AND jsonb_typeof(policy->'historicalEnabled')='boolean'
  AND jsonb_typeof(policy->'historyStart') IN ('string','null')
  AND (policy->>'historyStart' IS NULL OR length(policy->>'historyStart') BETWEEN 1 AND 64)
  AND policy->>'outdatedPolicy' IN ('disabled','current_only','historical')
  AND jsonb_typeof(policy->'unknownRetryMaxAttempts')='number'
  AND jsonb_typeof(policy->'unknownRetryTokenLimit')='number'
  AND jsonb_typeof(policy->'dayTokenLimit')='number'
  AND jsonb_typeof(policy->'replayTokenLimit')='number'
  AND jsonb_typeof(policy->'semanticTokenLimit')='number'
  AND COALESCE(policy->>'unknownRetryMaxAttempts','') ~ '^[0-9]+$'
  AND COALESCE(policy->>'unknownRetryTokenLimit','') ~ '^[0-9]+$'
  AND COALESCE(policy->>'dayTokenLimit','') ~ '^[0-9]+$'
  AND COALESCE(policy->>'replayTokenLimit','') ~ '^[0-9]+$'
  AND COALESCE(policy->>'semanticTokenLimit','') ~ '^[0-9]+$'
 THEN (policy->>'unknownRetryMaxAttempts')::bigint BETWEEN 0 AND 1
  AND (policy->>'dayTokenLimit')::bigint BETWEEN 1024 AND 10000000
  AND (policy->>'unknownRetryTokenLimit')::bigint BETWEEN 0 AND (policy->>'dayTokenLimit')::bigint
  AND (policy->>'replayTokenLimit')::bigint BETWEEN 0 AND (policy->>'dayTokenLimit')::bigint
  AND (policy->>'semanticTokenLimit')::bigint BETWEEN 0 AND (policy->>'dayTokenLimit')::bigint
  AND (policy->>'unknownRetryTokenLimit')::bigint+(policy->>'replayTokenLimit')::bigint+(policy->>'semanticTokenLimit')::bigint <= (policy->>'dayTokenLimit')::bigint
 ELSE false END
$$;

ALTER TABLE linggan_comment_daily_schedule
 ADD COLUMN auto_policy jsonb NOT NULL DEFAULT '{"continuousNew":false,"historicalEnabled":false,"historyStart":null,"outdatedPolicy":"disabled","unknownRetryMaxAttempts":0,"unknownRetryTokenLimit":0,"dayTokenLimit":100000,"replayTokenLimit":0,"semanticTokenLimit":0}'::jsonb,
 ADD COLUMN auto_enabled_at timestamptz,
 ADD COLUMN legacy_activation_unknown boolean NOT NULL DEFAULT false,
 ADD CONSTRAINT linggan_comment_daily_auto_policy_check CHECK(linggan_comment_auto_policy_valid(auto_policy));

-- A currently enabled legacy schedule has no durable first-enable record. Its migration-time
-- boundary is retained only for diagnosis; historic processing stays disabled until a new policy
-- explicitly grants it. Paused schedules establish a trustworthy boundary on their next enable.
UPDATE linggan_comment_daily_schedule
 SET auto_policy=jsonb_build_object(
   'continuousNew',enabled,
   'historicalEnabled',false,
   'historyStart',NULL,
   'outdatedPolicy','disabled',
   'unknownRetryMaxAttempts',0,
   'unknownRetryTokenLimit',0,
   'dayTokenLimit',token_limit,
   'replayTokenLimit',0,
   'semanticTokenLimit',CASE WHEN enabled THEN token_limit ELSE 0 END
 ),
 auto_enabled_at=CASE WHEN enabled THEN scope_001_now() ELSE auto_enabled_at END,
 legacy_activation_unknown=enabled
 WHERE auto_enabled_at IS NULL;

ALTER TABLE linggan_comment_daily_item
 ADD COLUMN queue_class text CHECK(queue_class IS NULL OR queue_class IN ('new_intake','recovery','historical')),
 ADD COLUMN execution_day date,
 ADD CONSTRAINT linggan_comment_daily_item_queue_owner_check CHECK((queue_class IS NULL)=(execution_day IS NULL));
CREATE INDEX linggan_comment_daily_item_execution_queue
 ON linggan_comment_daily_item(execution_day,queue_class,state,source_ref)
 WHERE queue_class IS NOT NULL;

-- A semantic work row remains the sole cross-batch execution owner. New work retains only a
-- proof manifest of source/context hashes and the effective rule, never another raw body copy.
ALTER TABLE linggan_comment_semantic_work
 ADD COLUMN input_manifest jsonb,
 ADD COLUMN unknown_retry_attempts integer NOT NULL DEFAULT 0 CHECK(unknown_retry_attempts>=0),
 ADD CONSTRAINT linggan_comment_semantic_work_input_manifest_check
 CHECK(input_manifest IS NULL OR jsonb_typeof(input_manifest)='object');
CREATE INDEX linggan_comment_semantic_work_identity_current
 ON linggan_comment_semantic_work(workspace_ref,identity_key,updated_at DESC,semantic_ref DESC);

-- Compatibility aliases are additive evidence. They never rewrite an old frozen fingerprint or
-- assert reuse without a retained accepted analysis and a proof hash.
CREATE TABLE linggan_comment_legacy_fingerprint_alias (
 source_identity text NOT NULL CHECK(source_identity ~ '^[0-9a-f]{64}$'),
 old_fingerprint text NOT NULL CHECK(old_fingerprint ~ '^[0-9a-f]{64}$'),
 new_fingerprint text NOT NULL CHECK(new_fingerprint ~ '^[0-9a-f]{64}$'),
 analysis_ref uuid NOT NULL REFERENCES linggan_comment_analysis_work(work_ref),
 proof_hash text NOT NULL CHECK(proof_hash ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(source_identity,old_fingerprint,new_fingerprint)
);
CREATE INDEX linggan_comment_legacy_fingerprint_alias_new
 ON linggan_comment_legacy_fingerprint_alias(source_identity,new_fingerprint,analysis_ref);
CREATE TRIGGER linggan_comment_legacy_fingerprint_alias_immutable
 BEFORE UPDATE OR DELETE ON linggan_comment_legacy_fingerprint_alias
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- This conservative dependency stamp is deliberately broader than the lexical selector. A
-- changed detail, parent, OCR or ASR fact first makes the cached eligibility stale; local
-- reconciliation then rebuilds the exact selected fragments and may prove that reuse remains
-- valid. Engagement/re-observation facts are absent, so they never invalidate a semantic key.
CREATE FUNCTION linggan_comment_research_context_revision(source_ref uuid) RETURNS text
LANGUAGE sql STABLE AS $$
 WITH source AS (
   SELECT comment.content_public_ref,comment.parent_comment_external_id
   FROM linggan_material_comment comment WHERE comment.material_ref=source_ref
 ), detail AS (
   SELECT COALESCE(jsonb_agg(jsonb_build_object(
     'detailRef',row.material_ref,
     'titleHash',encode(sha256(convert_to(COALESCE(row.title,''),'UTF8')),'hex'),
     'bodyHash',encode(sha256(convert_to(COALESCE(row.body_text,''),'UTF8')),'hex'),
     'observedAt',row.observed_at,
     'createdAt',row.created_at
   ) ORDER BY row.created_at,row.material_ref),'[]'::jsonb) AS value
   FROM linggan_material_content_detail row JOIN source ON source.content_public_ref=row.content_public_ref
 ), parent AS (
   SELECT COALESCE(jsonb_agg(jsonb_build_object(
     'sourceRef',row.material_ref,
     'bodyHash',encode(sha256(convert_to(COALESCE(row.body_text,''),'UTF8')),'hex'),
     'observedAt',row.observed_at,
     'createdAt',row.created_at
   ) ORDER BY row.material_ref),'[]'::jsonb) AS value
   FROM source
   JOIN linggan_material_comment_current row
     ON row.content_public_ref=source.content_public_ref
    AND row.comment_external_id=source.parent_comment_external_id
   JOIN linggan_comment_research_readable readable ON readable.material_ref=row.material_ref
 ), derived AS (
   SELECT COALESCE(jsonb_agg(jsonb_build_object(
     'derivativeRef',text.derivative_ref,
     'jobRef',derivative.job_ref,
     'kind',text.kind,
     'textHash',encode(sha256(convert_to(text.display_text,'UTF8')),'hex'),
     'createdAt',text.created_at
   ) ORDER BY text.derivative_ref),'[]'::jsonb) AS value
   FROM linggan_material_derived_text text
   JOIN linggan_media_derivative derivative USING(derivative_ref)
   JOIN source ON source.content_public_ref=text.content_public_ref
   WHERE derivative.derivative_kind IN ('ocr_text','asr_text','frame_ocr_text')
     AND (SELECT event.state FROM linggan_media_processing_job_event event
          WHERE event.job_ref=derivative.job_ref
          ORDER BY event.occurred_at DESC,event.event_ref DESC LIMIT 1)='succeeded'
 )
 SELECT encode(sha256(convert_to(jsonb_build_object(
   'detail',(SELECT value FROM detail),
   'parent',(SELECT value FROM parent),
   'derived',(SELECT value FROM derived)
 )::text,'UTF8')),'hex')
$$;

-- The UI consumes the same locally reconciled eligibility that controls reservation. It stores
-- only hashes, references and state; current text is still read through the source boundary.
CREATE TABLE linggan_comment_research_eligibility_current (
 source_identity text PRIMARY KEY CHECK(source_identity ~ '^[0-9a-f]{64}$'),
 source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
 source_sha256 text NOT NULL CHECK(source_sha256 ~ '^[0-9a-f]{64}$'),
 fingerprint text NOT NULL CHECK(fingerprint ~ '^[0-9a-f]{64}$'),
 rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
 rule_hash text NOT NULL CHECK(rule_hash ~ '^[0-9a-f]{64}$'),
 schema_version text NOT NULL CHECK(char_length(schema_version) BETWEEN 1 AND 100),
 selector_version text NOT NULL CHECK(char_length(selector_version) BETWEEN 1 AND 100),
 source_context_revision text NOT NULL CHECK(source_context_revision ~ '^[0-9a-f]{64}$'),
 current_analysis_ref uuid REFERENCES linggan_comment_analysis_work(work_ref),
 last_accepted_analysis_ref uuid REFERENCES linggan_comment_analysis_work(work_ref),
 current_attempt_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
 semantic_ref uuid REFERENCES linggan_comment_semantic_work(semantic_ref),
 result_state text NOT NULL CHECK(result_state IN ('unstudied','studied','no_signal','outdated')),
 execution_state text NOT NULL CHECK(execution_state IN ('idle','queued','running','failed','waiting_recovery')),
 reason_code text NOT NULL CHECK(char_length(reason_code) BETWEEN 1 AND 100),
 eligible_to_dispatch boolean NOT NULL DEFAULT false,
 input_manifest jsonb NOT NULL CHECK(jsonb_typeof(input_manifest)='object'),
 checked_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX linggan_comment_research_eligibility_current_source
 ON linggan_comment_research_eligibility_current(source_ref,checked_at DESC);

-- Internal automatic backlog manifests remain auxiliary run records, not a new user object.
ALTER TABLE linggan_comment_daily_batch DROP CONSTRAINT linggan_comment_daily_batch_kind_check;
ALTER TABLE linggan_comment_daily_batch ADD CONSTRAINT linggan_comment_daily_batch_kind_check
 CHECK(kind IN('daily','selected','supplement','backlog'));

-- Daily comment allowance is shared by all automatic manifests. Retries of an already
-- admitted identity do not consume another comment slot; their tokens still count again.
CREATE FUNCTION linggan_comment_execution_day_sources()
RETURNS TABLE(work_ref uuid,comment_external_id text) LANGUAGE sql STABLE AS $$
 SELECT DISTINCT source.content_public_ref,source.comment_external_id
 FROM linggan_comment_daily_packet packet
 JOIN linggan_model_invocation invocation USING(invocation_ref)
 CROSS JOIN LATERAL unnest(packet.source_refs) AS member(source_ref)
 JOIN linggan_material_comment source ON source.material_ref=member.source_ref
 WHERE packet.purpose='extraction' AND invocation.result->>'callStarted' IS DISTINCT FROM 'false'
   AND invocation.created_at>=date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'
   AND invocation.created_at<(date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai')+interval '1 day') AT TIME ZONE 'Asia/Shanghai'
$$;
CREATE FUNCTION linggan_comment_day_source_allowed(work uuid,external_id text)
RETURNS boolean LANGUAGE sql STABLE AS $$
 WITH used AS MATERIALIZED(SELECT * FROM linggan_comment_execution_day_sources())
 SELECT EXISTS(SELECT 1 FROM used WHERE work_ref=work AND comment_external_id=external_id)
 OR (SELECT count(*) FROM used)<(SELECT source_limit FROM linggan_comment_daily_schedule WHERE singleton)
$$;

-- Bounded local reconciliation must advance past an incompatible/blocked head of queue.
ALTER TABLE linggan_comment_semantic_work ADD COLUMN compatibility_checked_at timestamptz;
ALTER TABLE linggan_comment_daily_item ADD COLUMN eligibility_checked_at timestamptz;
