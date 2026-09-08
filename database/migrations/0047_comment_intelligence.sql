-- CI-20260907-V1: derived research state. Raw evidence is never rewritten.
CREATE INDEX linggan_comment_identity_history_idx ON linggan_material_comment(content_public_ref,comment_external_id,created_at,material_ref);
-- A persisted identity cannot change when observations share a timestamp or arrive late.
CREATE TABLE linggan_ci_comment_identity (
 canonical_ref uuid PRIMARY KEY REFERENCES linggan_material_comment(material_ref),
 work_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),comment_external_id text NOT NULL,
 first_observed_at timestamptz NOT NULL,UNIQUE(work_ref,comment_external_id)
);
INSERT INTO linggan_ci_comment_identity(canonical_ref,work_ref,comment_external_id,first_observed_at)
 SELECT DISTINCT ON(content_public_ref,comment_external_id) material_ref,content_public_ref,comment_external_id,created_at
 FROM linggan_material_comment ORDER BY content_public_ref,comment_external_id,created_at,material_ref;
CREATE FUNCTION linggan_ci_remember_comment_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO linggan_ci_comment_identity(canonical_ref,work_ref,comment_external_id,first_observed_at)
 VALUES(NEW.material_ref,NEW.content_public_ref,NEW.comment_external_id,NEW.created_at) ON CONFLICT(work_ref,comment_external_id) DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER linggan_ci_comment_identity_insert AFTER INSERT ON linggan_material_comment FOR EACH ROW EXECUTE FUNCTION linggan_ci_remember_comment_identity();
CREATE TRIGGER linggan_ci_comment_identity_immutable BEFORE UPDATE OR DELETE ON linggan_ci_comment_identity FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Cross-industry legacy storage has only a last observation. Keep its migration baseline
-- explicitly marked; never pretend it is a recovered historical first observation.
ALTER TABLE cross_industry_comment ADD COLUMN first_observed_at timestamptz;
ALTER TABLE cross_industry_comment ADD COLUMN first_observed_known boolean NOT NULL DEFAULT false;
CREATE FUNCTION linggan_ci_cross_first_observed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN NEW.first_observed_at:=NEW.observed_at; NEW.first_observed_known:=true;
 ELSE NEW.first_observed_at:=COALESCE(OLD.first_observed_at,OLD.observed_at); NEW.first_observed_known:=OLD.first_observed_known;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER linggan_ci_cross_first_observed BEFORE INSERT OR UPDATE ON cross_industry_comment FOR EACH ROW EXECUTE FUNCTION linggan_ci_cross_first_observed();

CREATE VIEW linggan_ci_source AS
SELECT c.material_ref AS source_ref,identity.canonical_ref AS canonical_ref,w.domain_ref,
 c.content_public_ref AS work_ref,c.body_text AS body,c.is_reply,c.parent_comment_external_id AS parent_external_id,
 c.comment_external_id,c.author_external_id,
 CASE WHEN c.author_external_id IS NOT NULL AND c.author_external_id=(SELECT d.author_external_id FROM linggan_material_content_detail d WHERE d.content_public_ref=c.content_public_ref AND d.author_external_id IS NOT NULL ORDER BY d.observed_at::timestamptz DESC,d.created_at DESC LIMIT 1) THEN 'author' WHEN payload.data->>'authorRole' IN ('author','platform_system','user') THEN payload.data->>'authorRole' ELSE 'unknown' END AS role,
 CASE WHEN (payload.data->>'likeCount') ~ '^[0-9]{1,18}$' THEN (payload.data->>'likeCount')::bigint ELSE NULL END AS likes,
 CASE WHEN (payload.data->>'publishedAt') ~ '^[0-9]{12,15}$' AND (payload.data->>'publishedAt')::numeric < 253402300800000
 THEN to_timestamp((payload.data->>'publishedAt')::double precision/1000) ELSE NULL END AS published_at,
 left(payload.data->>'publishedAtText',100) AS published_at_text,
 identity.first_observed_at AS first_observed_at,true AS first_observed_known,c.observed_at::timestamptz AS last_observed_at,
 NULL::text AS work_title,NULL::text AS creator_display_name,
 encode(sha256(convert_to(COALESCE(c.body_text,''),'UTF8')),'hex') AS source_sha256
FROM linggan_comment_research_current(scope_001_now()) c
JOIN linggan_material_content w ON w.public_ref=c.content_public_ref
JOIN linggan_runtime_capture_package p USING(package_ref)
JOIN linggan_ci_comment_identity identity ON identity.work_ref=c.content_public_ref AND identity.comment_external_id=c.comment_external_id
CROSS JOIN LATERAL (SELECT p.payload->'records'->c.record_ordinal->'payload' AS data) payload
WHERE NOT EXISTS(SELECT 1 FROM linggan_comment_research_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)
UNION ALL
SELECT c.comment_ref,c.comment_ref,c.domain_ref,c.sample_ref,c.body_text,c.is_reply,c.parent_comment_external_id,
 c.comment_external_id,c.author_external_id,CASE WHEN c.author_external_id=s.author_external_id AND c.author_external_id IS NOT NULL THEN 'author' ELSE 'unknown' END,
 c.like_count,NULL::timestamptz,NULL::text,COALESCE(c.first_observed_at,c.observed_at),c.first_observed_known,c.observed_at,
 s.title,s.author_name,encode(sha256(convert_to(COALESCE(c.body_text,''),'UTF8')),'hex')
FROM cross_industry_comment c JOIN cross_industry_sample s ON s.sample_ref=c.sample_ref AND s.domain_ref=c.domain_ref;

CREATE TABLE linggan_comment_semantic_work (
 semantic_ref uuid PRIMARY KEY,workspace_ref uuid NOT NULL REFERENCES linggan_model_workspace(workspace_ref),
 identity_key text NOT NULL,fingerprint text NOT NULL,source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
 config_ref uuid NOT NULL REFERENCES linggan_model_config(config_ref),state text NOT NULL CHECK(state IN('running','succeeded','no_signal','failed')),
 analysis_ref uuid REFERENCES linggan_comment_analysis_work(work_ref),invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(workspace_ref,identity_key,fingerprint)
);
ALTER TABLE linggan_comment_daily_item ADD COLUMN semantic_ref uuid REFERENCES linggan_comment_semantic_work(semantic_ref);
CREATE TABLE linggan_comment_daily_adjustment (
 command_ref uuid PRIMARY KEY,batch_ref uuid NOT NULL REFERENCES linggan_comment_daily_batch(batch_ref),
 kind text NOT NULL CHECK(kind IN('continue','usage_review')),request jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER linggan_comment_daily_adjustment_immutable BEFORE UPDATE OR DELETE ON linggan_comment_daily_adjustment FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_prepare (
 prepare_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),scope jsonb NOT NULL,
 source_refs uuid[] NOT NULL,source_hashes jsonb NOT NULL,reanalyze boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),expires_at timestamptz NOT NULL DEFAULT scope_001_now()+interval '30 minutes'
);
CREATE TRIGGER linggan_ci_prepare_immutable BEFORE UPDATE OR DELETE ON linggan_ci_prepare FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- CI-20260907-V1: mutable projections are backed by append-only commands/versions.
CREATE TABLE linggan_ci_command (
 command_ref uuid PRIMARY KEY, domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 request jsonb NOT NULL CHECK(jsonb_typeof(request)='object'),
 result jsonb NOT NULL CHECK(jsonb_typeof(result)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER ci_command_immutable BEFORE UPDATE OR DELETE ON linggan_ci_command FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_source_research (
 canonical_ref uuid PRIMARY KEY, domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 revision bigint NOT NULL DEFAULT 0 CHECK(revision >= 0),
 labels jsonb NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(labels)='array'),
 needs_context boolean NOT NULL DEFAULT false, locked boolean NOT NULL DEFAULT false,
 bookmarked boolean NOT NULL DEFAULT false, bookmark_note text NOT NULL DEFAULT '',
 reason text NOT NULL DEFAULT '', updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX ci_source_research_domain ON linggan_ci_source_research(domain_ref,canonical_ref);
CREATE TABLE linggan_ci_problem (
 problem_ref uuid PRIMARY KEY, domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 120),
 meaning text NOT NULL CHECK(length(meaning) BETWEEN 1 AND 1000),
 definition jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(definition)='object'),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision > 0),
 redirect_ref uuid REFERENCES linggan_ci_problem(problem_ref),
 bookmarked boolean NOT NULL DEFAULT false,
 origin text NOT NULL CHECK(origin IN ('manual','exact_definition')),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK(redirect_ref IS NULL OR redirect_ref <> problem_ref)
);
CREATE INDEX ci_problem_domain ON linggan_ci_problem(domain_ref,problem_ref);
CREATE TABLE linggan_ci_problem_member (
 problem_ref uuid NOT NULL REFERENCES linggan_ci_problem(problem_ref), canonical_ref uuid NOT NULL,
 origin text NOT NULL CHECK(origin IN ('manual','exact_definition')),
 evidence jsonb NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(evidence)='array'),
 analysis_ref uuid REFERENCES linggan_comment_analysis_work(work_ref),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(problem_ref,canonical_ref)
);
CREATE INDEX ci_problem_member_source ON linggan_ci_problem_member(canonical_ref,problem_ref);
CREATE TABLE linggan_ci_problem_version (
 problem_ref uuid NOT NULL REFERENCES linggan_ci_problem(problem_ref), revision bigint NOT NULL,
 command_ref uuid, kind text NOT NULL, before_value jsonb NOT NULL,
 after_value jsonb NOT NULL, reason text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),PRIMARY KEY(problem_ref,revision)
);
CREATE TRIGGER ci_problem_version_immutable BEFORE UPDATE OR DELETE ON linggan_ci_problem_version FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_source_revision (
 canonical_ref uuid NOT NULL, revision bigint NOT NULL, command_ref uuid NOT NULL,
 kind text NOT NULL, before_value jsonb NOT NULL, after_value jsonb NOT NULL,
 reason text NOT NULL, created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(canonical_ref,revision)
);
CREATE TRIGGER ci_source_revision_immutable BEFORE UPDATE OR DELETE ON linggan_ci_source_revision FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ci_term_setting (
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref), term text NOT NULL CHECK(length(term) BETWEEN 2 AND 48),
 hidden boolean NOT NULL, revision bigint NOT NULL CHECK(revision > 0),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(), PRIMARY KEY(domain_ref,term)
);
CREATE TABLE linggan_ci_term_index (
 canonical_ref uuid NOT NULL, domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 term text NOT NULL CHECK(length(term) BETWEEN 2 AND 48),
 source_sha256 text NOT NULL, dictionary_version text NOT NULL,
 PRIMARY KEY(canonical_ref,term)
);
CREATE INDEX ci_term_index_scope ON linggan_ci_term_index(domain_ref,term,canonical_ref);
CREATE TABLE linggan_ci_problem_candidate (
 candidate_ref uuid PRIMARY KEY, canonical_ref uuid NOT NULL, domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 analysis_ref uuid NOT NULL REFERENCES linggan_comment_analysis_work(work_ref),
 ordinal integer NOT NULL CHECK(ordinal >= 0), name text NOT NULL, meaning text NOT NULL,
 definition_key text NOT NULL, evidence jsonb NOT NULL,
 state text NOT NULL CHECK(state IN ('unmerged','assigned','superseded')),
 problem_ref uuid REFERENCES linggan_ci_problem(problem_ref),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(), UNIQUE(analysis_ref,ordinal)
);
CREATE INDEX ci_candidate_definition ON linggan_ci_problem_candidate(domain_ref,definition_key,state);
CREATE INDEX ci_candidate_source ON linggan_ci_problem_candidate(canonical_ref,analysis_ref);
CREATE TABLE linggan_ci_projection_cursor (
 canonical_ref uuid PRIMARY KEY, source_sha256 text NOT NULL, dictionary_version text NOT NULL,
 analysis_ref uuid, updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

ALTER TABLE linggan_comment_daily_item ADD COLUMN context_candidate_hash text, ADD COLUMN context_candidate_at timestamptz;

ALTER TABLE linggan_ci_source_research ADD COLUMN labels_locked boolean NOT NULL DEFAULT false;
-- Definition identity changes separately from incremental member-count revisions.
ALTER TABLE linggan_ci_problem ADD COLUMN definition_revision bigint NOT NULL DEFAULT 1 CHECK(definition_revision > 0);
ALTER TABLE linggan_ci_problem_candidate ADD COLUMN proposal jsonb CHECK(proposal IS NULL OR jsonb_typeof(proposal)='object');
ALTER TABLE linggan_ci_problem_member DROP CONSTRAINT linggan_ci_problem_member_origin_check;
ALTER TABLE linggan_ci_problem_member ADD CONSTRAINT linggan_ci_problem_member_origin_check CHECK(origin IN ('manual','exact_definition','model_equivalence'));
-- A human decision remains recorded when source text changes, but is no longer active.
ALTER TABLE linggan_ci_source_research ADD COLUMN source_sha256 text CHECK(source_sha256 IS NULL OR source_sha256 ~ '^[0-9a-f]{64}$');
-- Automatic text revisions preserve their original daily grant; likes alone never create supplements.
ALTER TABLE linggan_comment_daily_batch DROP CONSTRAINT linggan_comment_daily_batch_kind_check;
ALTER TABLE linggan_comment_daily_batch ADD CONSTRAINT linggan_comment_daily_batch_kind_check CHECK(kind IN('daily','selected','supplement'));
CREATE UNIQUE INDEX linggan_comment_daily_supplement_identity ON linggan_comment_daily_batch((request->>'originBatchRef'),(request->>'sourceRef')) WHERE kind='supplement';
-- The new semantic contract does not silently upgrade previously frozen v2 authorizations.
UPDATE linggan_comment_daily_batch SET enabled=false WHERE enabled AND COALESCE(request->>'ruleVersion','')<>'comment-research.v3';
UPDATE linggan_comment_daily_schedule SET enabled=false,revision=revision+1 WHERE enabled;
-- Retry limits belong to semantic work, shared by every plan that references it.
ALTER TABLE linggan_comment_semantic_work ADD COLUMN attempts integer NOT NULL DEFAULT 0 CHECK(attempts>=0);

-- Derived research cannot outlive any comment or acquired media dependency.
-- Mirrors Evidence's material_media_read ACQUIRED gate; raw comments stay browsable.
CREATE FUNCTION linggan_ci_analysis_context_readable(result jsonb) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT result IS NOT NULL
 AND (result#>'{contextRefs,researchSourceRefs}' IS NULL OR (jsonb_typeof(result#>'{contextRefs,researchSourceRefs}')='array' AND jsonb_array_length(result#>'{contextRefs,researchSourceRefs}')>0))
 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements_text(COALESCE(result#>'{contextRefs,researchSourceRefs}','[]')) ref WHERE NOT EXISTS(SELECT 1 FROM linggan_comment_research_readable c WHERE c.material_ref::text=ref))
 AND (result#>>'{contextRefs,parentSourceRef}' IS NULL OR EXISTS(SELECT 1 FROM linggan_comment_research_readable c WHERE c.material_ref::text=result#>>'{contextRefs,parentSourceRef}'))
 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements_text(COALESCE(result#>'{contextRefs,mediaJobs}','[]')) ref WHERE NOT EXISTS(
 SELECT 1 FROM linggan_media_processing_job job JOIN linggan_media_derivative d USING(job_ref)
 JOIN linggan_material_derived_text text ON text.derivative_ref=d.derivative_ref
 WHERE job.job_ref::text=ref AND job.created_at<=scope_001_now() AND d.created_at<=scope_001_now() AND text.created_at<=scope_001_now()
 AND EXISTS(SELECT 1 FROM linggan_material_media_origin o WHERE o.slot_key=job.slot_key AND o.content_public_ref::text=result#>>'{contextRefs,workRef}')
 AND (SELECT e.state FROM linggan_media_processing_job_event e WHERE e.job_ref=job.job_ref AND e.occurred_at<=scope_001_now() ORDER BY e.occurred_at DESC LIMIT 1)='succeeded'
 AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition p WHERE p.state='WITHDRAWN_OR_RESTRICTED' AND(p.derivative_ref=d.derivative_ref OR p.blob_sha256=job.blob_sha256 OR p.slot_key=job.slot_key))
 ));
$$;
-- Advanced automatic publication stays off until a real, held-out calibration is accepted.
CREATE TABLE linggan_ci_rule_release (
 domain_ref uuid PRIMARY KEY REFERENCES observation_domain(domain_ref),
 revision bigint NOT NULL DEFAULT 0,advanced_release_enabled boolean NOT NULL DEFAULT false,
 evaluation_report jsonb,approved_at timestamptz,
 CHECK(NOT advanced_release_enabled OR (evaluation_report IS NOT NULL AND approved_at IS NOT NULL))
);

-- Superseded v1 grants stay auditable; only the unified v3 daily/selected path dispatches.
UPDATE linggan_model_plan SET enabled=false,revision=revision+1 WHERE enabled;
UPDATE linggan_model_workspace SET active_auto_plan_ref=NULL WHERE singleton;
