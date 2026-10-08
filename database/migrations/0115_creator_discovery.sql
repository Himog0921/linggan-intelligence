-- CREATOR-DISCOVERY-001. Derived analysis only; no duplicate author/work facts.
-- The accepted, as-of attribution owner is shared by Work Current and the historical view.
-- A Target is deliberately absent: deleting a monitoring decision cannot erase authorship.
CREATE FUNCTION linggan_material_content_author_at(p_content_public_ref uuid, p_as_of timestamptz)
RETURNS TABLE(author_external_id text, attribution_source text, observed_at timestamptz)
LANGUAGE sql STABLE AS $$
 SELECT candidate.author_external_id,candidate.attribution_source,candidate.observed_at
 FROM (
   SELECT detail.author_external_id,'content_detail'::text AS attribution_source,
          detail.observed_at::timestamptz AS observed_at,detail.created_at AS recorded_at,
          detail.package_ref,detail.material_ref,2 AS source_priority
   FROM linggan_material_content_detail detail
   JOIN linggan_runtime_capture_package package USING(package_ref)
   WHERE detail.content_public_ref=p_content_public_ref
     AND package.accepted_at<=p_as_of
     AND NULLIF(btrim(detail.author_external_id),'') IS NOT NULL
   UNION ALL
   SELECT task.task_spec #>> '{target,authorExternalId}','profile_discovery'::text,
          finding.observed_at::timestamptz,finding.created_at,
          finding.package_ref,finding.material_ref,1
   FROM linggan_material_discovery_finding finding
   JOIN linggan_runtime_capture_package package USING(package_ref)
   JOIN linggan_runtime_task task USING(task_id)
   WHERE finding.content_public_ref=p_content_public_ref
     AND package.package_kind='profile_discovery'
     AND package.accepted_at<=p_as_of
     AND NULLIF(btrim(task.task_spec #>> '{target,authorExternalId}'),'') IS NOT NULL
 ) candidate
 ORDER BY candidate.source_priority DESC,candidate.observed_at DESC,
          candidate.recorded_at DESC,candidate.package_ref DESC,candidate.material_ref DESC
 LIMIT 1
$$;

CREATE OR REPLACE VIEW linggan_material_content_author AS
 SELECT content.public_ref AS content_public_ref,content.platform,
        attribution.author_external_id,attribution.attribution_source,attribution.observed_at
 FROM linggan_material_content content
 JOIN LATERAL linggan_material_content_author_at(content.public_ref,scope_001_now()) attribution ON true;

CREATE TABLE linggan_creator_discovery_policy (
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 platform text NOT NULL CHECK(platform='xhs'),
 like_threshold bigint CHECK(like_threshold>0),
 revision bigint NOT NULL DEFAULT 1,
 analysis_enabled boolean NOT NULL DEFAULT false,
 config_ref uuid REFERENCES linggan_model_config,
 daily_token_limit bigint CHECK(daily_token_limit BETWEEN 1024 AND 10000000),
 scan_after uuid,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(domain_ref,platform),
 CHECK(NOT analysis_enabled OR (config_ref IS NOT NULL AND daily_token_limit IS NOT NULL))
);
CREATE TABLE linggan_creator_discovery_work_analysis (
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 requested_fingerprint text NOT NULL,
 result_fingerprint text,
 result_json jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(result_json)='object'),
 result_rule_version text,
 result_model_config_ref uuid REFERENCES linggan_model_config,
 result_invocation_ref uuid REFERENCES linggan_model_invocation,
 result_at timestamptz,
 manual_overrides jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(manual_overrides)='object'),
 job_state text NOT NULL DEFAULT 'queued' CHECK(job_state IN('idle','queued','running','failed','paused')),
 attempt_count integer NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 3),
 next_attempt_at timestamptz NOT NULL DEFAULT scope_001_now(),
 lease_token uuid,
 lease_expires_at timestamptz,
 last_error_code text,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(domain_ref,work_public_ref),
 CHECK((job_state='running')=(lease_token IS NOT NULL AND lease_expires_at IS NOT NULL))
);
CREATE INDEX creator_discovery_work_queue ON linggan_creator_discovery_work_analysis(next_attempt_at,domain_ref,work_public_ref) WHERE job_state IN('queued','running');
CREATE TABLE linggan_creator_discovery_author_analysis (
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 platform text NOT NULL,
 author_external_id text NOT NULL,
 requested_fingerprint text NOT NULL,
 result_fingerprint text,
 result_json jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(result_json)='object'),
 result_rule_version text,
 result_model_config_ref uuid REFERENCES linggan_model_config,
 result_invocation_ref uuid REFERENCES linggan_model_invocation,
 result_at timestamptz,
 manual_overrides jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(manual_overrides)='object'),
 job_state text NOT NULL DEFAULT 'queued' CHECK(job_state IN('idle','queued','running','failed','paused')),
 attempt_count integer NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 3),
 next_attempt_at timestamptz NOT NULL DEFAULT scope_001_now(),
 lease_token uuid,
 lease_expires_at timestamptz,
 last_error_code text,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(domain_ref,platform,author_external_id),
 CHECK((job_state='running')=(lease_token IS NOT NULL AND lease_expires_at IS NOT NULL))
);
