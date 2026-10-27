-- The source catalog remains; retired schedulers and their derived records are removed.
-- Collection authorization is independent of AI model/method configuration.
CREATE TABLE linggan_topic_collection_policy (
 domain_ref uuid PRIMARY KEY REFERENCES observation_domain,
 collection_enabled boolean NOT NULL DEFAULT false,
 status text NOT NULL DEFAULT 'active' CHECK(status IN('active','paused','stopped')),
 revision bigint NOT NULL DEFAULT 1,updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_topic_collection_policy SELECT domain_ref,collection_enabled,status,revision,updated_at FROM linggan_topic_map_research_policy;
ALTER TABLE linggan_topic_map_research_command RENAME TO linggan_topic_collection_command;
CREATE FUNCTION comment_clean_reasons_are_strings(value jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE STRICT AS $$ SELECT CASE WHEN jsonb_typeof(value)='array' THEN
 NOT EXISTS(SELECT 1 FROM jsonb_array_elements(value) e WHERE jsonb_typeof(e)<>'string') ELSE false END $$;
CREATE TABLE linggan_comment_clean_cache (
 source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
 cleaner_version text NOT NULL CHECK(char_length(cleaner_version) BETWEEN 1 AND 100),
 raw_sha256 text NOT NULL CHECK(raw_sha256 ~ '^[0-9a-f]{64}$'),
 research_text text NOT NULL CHECK(char_length(research_text)<=16000),
 clean_state text NOT NULL CHECK(clean_state IN('direct','context','dropped','anomaly')),
 clean_reasons jsonb NOT NULL CHECK(comment_clean_reasons_are_strings(clean_reasons)),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),PRIMARY KEY(source_ref,cleaner_version)
);
DO $$ BEGIN IF to_regclass('linggan_comment_study_clean_cache') IS NOT NULL THEN
 EXECUTE 'INSERT INTO linggan_comment_clean_cache SELECT * FROM linggan_comment_study_clean_cache';
END IF; END $$;
CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;
CREATE INDEX comment_clean_search_idx ON linggan_comment_clean_cache USING gin(research_text public.gin_trgm_ops) WHERE clean_state IN('direct','context');
CREATE INDEX comment_clean_version_state_idx ON linggan_comment_clean_cache(cleaner_version,clean_state,source_ref);
DROP VIEW IF EXISTS linggan_comment_study_current_problem,linggan_comment_study_effective_signal,linggan_comment_study_current_comment,linggan_comment_study_effective_target RESTRICT;
DROP TABLE IF EXISTS
 linggan_comment_study_reset_receipt,linggan_comment_study_model_request,
 linggan_comment_study_pool_cursor,linggan_comment_study_comparison,
 linggan_comment_study_problem_membership,linggan_comment_study_problem_pair,
 linggan_comment_study_resolution,linggan_comment_study_problem_revision,linggan_comment_study_problem,
 linggan_comment_study_signal,linggan_comment_study_semantic_attempt,
 linggan_comment_study_batch_target,linggan_comment_study_target,linggan_comment_study_batch,
 linggan_comment_study_work,linggan_comment_study_start_request,linggan_comment_study_run,
 linggan_comment_study_active_policy,linggan_comment_study_policy,
 linggan_comment_study_clean_cache,linggan_comment_study_embedding_cache,
 linggan_topic_map_embedding,linggan_comment_study_embedding_profile,
 linggan_topic_map_backfill_queue,linggan_topic_map_definition_scan,
 linggan_topic_map_research_result,linggan_topic_map_research_request,
 linggan_topic_map_research_task,linggan_topic_map_research_run,linggan_topic_map_research_policy RESTRICT;
DROP FUNCTION IF EXISTS cs_clean_reasons_are_strings(jsonb),cs_policy_reject_mutation(),cs_policy_validate_insert(),cs_start_reject_mutation(),cs_run_validate_insert(),cs_run_protect_frozen(),cs_target_validate_input(),cs_model_request_protect(),cs_model_request_no_truncate(),cs_membership_revision_validate() RESTRICT;
