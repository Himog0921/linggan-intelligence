-- P3 continuation: explanations, follow-up runs and future-only automatic rollback remain in
-- the shadow domain.  None of these tables reference formal analysis or atom projections.
ALTER TABLE linggan_comment_replay_run
 ADD COLUMN authorization_root_run_ref uuid REFERENCES linggan_comment_replay_run(run_ref);
ALTER TABLE linggan_comment_replay_run
 ADD COLUMN run_kind text NOT NULL DEFAULT 'candidate'
 CHECK(run_kind IN ('candidate','continuity','active_health'));
CREATE INDEX linggan_comment_replay_authorization_root_idx
 ON linggan_comment_replay_run(COALESCE(authorization_root_run_ref,run_ref));

-- Reused shadow observations are a new receipt, never a mutation of the source run or a
-- second provider invocation.  The source/context hashes are checked by the Rust transaction
-- before this reference can be written.
ALTER TABLE linggan_comment_replay_item
 ADD COLUMN reused_from_item_ref uuid REFERENCES linggan_comment_replay_item(item_ref);
CREATE INDEX linggan_comment_replay_item_reuse_idx
 ON linggan_comment_replay_item(reused_from_item_ref)
 WHERE reused_from_item_ref IS NOT NULL;

CREATE TABLE linggan_comment_replay_follow_up (
 follow_up_ref uuid PRIMARY KEY,
 prior_run_ref uuid NOT NULL REFERENCES linggan_comment_replay_run(run_ref),
 authorization_root_run_ref uuid NOT NULL REFERENCES linggan_comment_replay_run(run_ref),
 reason text NOT NULL CHECK(reason IN ('required_new_samples','daily_budget_wait')),
 selector_hash text NOT NULL CHECK(char_length(selector_hash)=64),
 policy_revision bigint NOT NULL,
 candidate_rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
 candidate_rule_hash text NOT NULL CHECK(char_length(candidate_rule_hash)=64),
 request_hash text NOT NULL UNIQUE CHECK(char_length(request_hash)=64),
 state text NOT NULL CHECK(state IN ('queued','waiting_daily_budget','created','no_new_samples','failed','expired')),
 successor_run_ref uuid UNIQUE REFERENCES linggan_comment_replay_run(run_ref),
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(prior_run_ref,reason)
);
CREATE INDEX linggan_comment_replay_follow_up_queue_idx
 ON linggan_comment_replay_follow_up(state,created_at) WHERE state IN ('queued','waiting_daily_budget');

CREATE TABLE linggan_comment_replay_explanation (
 explanation_ref uuid PRIMARY KEY,
 run_ref uuid NOT NULL REFERENCES linggan_comment_replay_run(run_ref),
 pair_hash text NOT NULL CHECK(char_length(pair_hash)=64),
 baseline_item_ref uuid NOT NULL REFERENCES linggan_comment_replay_item(item_ref),
 candidate_item_ref uuid NOT NULL REFERENCES linggan_comment_replay_item(item_ref),
 request_hash text NOT NULL UNIQUE CHECK(char_length(request_hash)=64),
 invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
 state text NOT NULL CHECK(state IN ('queued','running','succeeded','failed','expired')),
 reserved_tokens bigint NOT NULL DEFAULT 0 CHECK(reserved_tokens>=0),
 safe_input jsonb NOT NULL CHECK(jsonb_typeof(safe_input)='object'),
 result jsonb,
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 finished_at timestamptz,
 expires_at timestamptz NOT NULL DEFAULT scope_001_now()+interval '24 hours',
 UNIQUE(run_ref,pair_hash)
);
CREATE INDEX linggan_comment_replay_explanation_queue_idx
 ON linggan_comment_replay_explanation(state,created_at) WHERE state='queued';
CREATE INDEX linggan_comment_replay_explanation_run_idx
 ON linggan_comment_replay_explanation(run_ref,state,created_at);

CREATE TABLE linggan_comment_rule_rollback_receipt (
 receipt_ref uuid PRIMARY KEY,
 triggering_run_ref uuid NOT NULL REFERENCES linggan_comment_replay_run(run_ref),
 policy_revision bigint NOT NULL,
 candidate_rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
 restored_rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
 active_revision_before integer NOT NULL,
 active_revision_after integer NOT NULL,
 health_evidence jsonb NOT NULL CHECK(jsonb_typeof(health_evidence)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(triggering_run_ref)
);
CREATE TRIGGER linggan_comment_rule_rollback_receipt_immutable
 BEFORE UPDATE OR DELETE ON linggan_comment_rule_rollback_receipt
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
