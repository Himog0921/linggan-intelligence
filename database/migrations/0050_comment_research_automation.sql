-- CI-AUTO-003: new cleaning and decision versions, historical evidence retained.
ALTER TABLE linggan_comment_clean DROP CONSTRAINT linggan_comment_clean_state_check;
ALTER TABLE linggan_comment_clean ADD CONSTRAINT linggan_comment_clean_state_check CHECK(state IN ('direct','context','low_information','anomaly','dropped'));
ALTER TABLE linggan_comment_daily_item DROP CONSTRAINT linggan_comment_daily_item_state_check;
ALTER TABLE linggan_comment_daily_item ADD CONSTRAINT linggan_comment_daily_item_state_check CHECK(state IN ('pending','running','succeeded','no_signal','failed','low_information','anomaly','context_missing','restricted','source_limit','dropped','carried_forward'));
CREATE TABLE cross_industry_comment_clean (
 source_ref uuid NOT NULL REFERENCES cross_industry_comment(comment_ref),
 source_sha256 text NOT NULL,cleaner_version text NOT NULL,
 state text NOT NULL CHECK(state IN ('direct','context','anomaly','dropped')),
 result jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(source_ref,cleaner_version,source_sha256)
);
CREATE TRIGGER cross_comment_clean_immutable BEFORE UPDATE OR DELETE ON cross_industry_comment_clean FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
ALTER TABLE linggan_ci_prepare ADD COLUMN preflight jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE linggan_ci_prepare ADD COLUMN config_ref uuid REFERENCES linggan_model_config(config_ref);
ALTER TABLE linggan_comment_daily_batch DROP CONSTRAINT linggan_comment_daily_batch_source_limit_check;
ALTER TABLE linggan_comment_daily_batch ADD CONSTRAINT linggan_comment_daily_batch_source_limit_check CHECK(source_limit BETWEEN 1 AND 3000);
ALTER TABLE linggan_comment_daily_schedule DROP CONSTRAINT linggan_comment_daily_schedule_source_limit_check;
ALTER TABLE linggan_comment_daily_schedule ADD CONSTRAINT linggan_comment_daily_schedule_source_limit_check CHECK(source_limit BETWEEN 1 AND 3000);
ALTER TABLE linggan_ci_problem_candidate DROP CONSTRAINT linggan_ci_problem_candidate_state_check;
ALTER TABLE linggan_ci_problem_candidate ADD CONSTRAINT linggan_ci_problem_candidate_state_check CHECK(state IN ('unmerged','assigned','resolved','superseded'));
CREATE TABLE linggan_ci_problem_boundary_decision (
 decision_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 candidate_ref uuid NOT NULL REFERENCES linggan_ci_problem_candidate(candidate_ref),
 definition_key text NOT NULL,target_ref uuid NOT NULL REFERENCES linggan_ci_problem(problem_ref),
 target_definition_revision bigint NOT NULL CHECK(target_definition_revision>0),
 relation text NOT NULL CHECK(relation IN ('same','related','different','uncertain')),
 reason text NOT NULL CHECK(length(reason) BETWEEN 1 AND 1000),
 origin text NOT NULL CHECK(origin IN ('manual','task_b','boundary_reuse')),command_ref uuid,
 contract_version text NOT NULL,created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX ci_boundary_lookup ON linggan_ci_problem_boundary_decision(domain_ref,definition_key,target_ref,target_definition_revision,created_at DESC);
CREATE TRIGGER ci_boundary_immutable BEFORE UPDATE OR DELETE ON linggan_ci_problem_boundary_decision FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
-- Existing grants retain their versions. They never execute with the new prompt implicitly.
UPDATE linggan_comment_daily_batch SET enabled=false WHERE enabled AND request->>'ruleVersion'<>'comment-research.v4';
UPDATE linggan_comment_daily_schedule SET enabled=false,revision=revision+1 WHERE enabled;
ALTER TABLE linggan_comment_daily_item ADD COLUMN origin_batch_ref uuid REFERENCES linggan_comment_daily_batch(batch_ref);
ALTER TABLE linggan_comment_daily_packet ADD COLUMN purpose text NOT NULL DEFAULT 'extraction' CHECK(purpose IN ('extraction','problem_relation','problem_embedding'));
CREATE TABLE linggan_ci_problem_task (
 task_ref uuid PRIMARY KEY,candidate_ref uuid NOT NULL REFERENCES linggan_ci_problem_candidate(candidate_ref),
 batch_ref uuid NOT NULL REFERENCES linggan_comment_daily_batch(batch_ref),
 config_ref uuid NOT NULL REFERENCES linggan_model_config(config_ref),contract_version text NOT NULL,
 input_fingerprint text NOT NULL,
 state text NOT NULL CHECK(state IN ('blocked_retrieval','pending','running','succeeded','needs_judgment','failed','superseded')),
 task_snapshot jsonb,snapshot_hash text,invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
 packet_ref uuid REFERENCES linggan_comment_daily_packet(packet_ref),failure_code text,lease_until timestamptz,
 expires_at timestamptz,purged_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(candidate_ref,contract_version,input_fingerprint)
);
CREATE INDEX ci_problem_task_queue ON linggan_ci_problem_task(state,created_at);

ALTER TABLE linggan_model_invocation DROP CONSTRAINT linggan_model_invocation_operation_check;
ALTER TABLE linggan_model_invocation ADD CONSTRAINT linggan_model_invocation_operation_check CHECK(operation IN ('connect','discover','probe','analyze','embed'));
CREATE TABLE linggan_embedding_config (
 config_ref uuid PRIMARY KEY,model_ref uuid NOT NULL REFERENCES linggan_model_entry(model_ref),
 dimensions integer CHECK(dimensions BETWEEN 1 AND 8192),qualified boolean NOT NULL DEFAULT false,enabled boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK(NOT enabled OR qualified AND dimensions IS NOT NULL)
);
CREATE TABLE linggan_embedding_settings (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),revision bigint NOT NULL DEFAULT 0,
 config_ref uuid REFERENCES linggan_embedding_config(config_ref)
);
INSERT INTO linggan_embedding_settings(singleton) VALUES(true);

ALTER TABLE linggan_ci_problem DROP CONSTRAINT linggan_ci_problem_origin_check;
ALTER TABLE linggan_ci_problem ADD CONSTRAINT linggan_ci_problem_origin_check CHECK(origin IN ('manual','exact_definition','model_expression'));
ALTER TABLE linggan_ci_problem_member DROP CONSTRAINT linggan_ci_problem_member_origin_check;
ALTER TABLE linggan_ci_problem_member ADD CONSTRAINT linggan_ci_problem_member_origin_check CHECK(origin IN ('manual','exact_definition','model_equivalence','model_expression'));
