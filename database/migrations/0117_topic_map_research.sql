-- TOPIC-MAP-V41-001: bounded research extends the generic invocation ledger.
-- Material text remains canonical; durable requests retain identities and hashes only.
CREATE TABLE linggan_topic_map_research_policy (
 domain_ref uuid PRIMARY KEY REFERENCES observation_domain,
 model_config_ref uuid NOT NULL REFERENCES linggan_model_config,
 daily_token_limit bigint NOT NULL CHECK(daily_token_limit BETWEEN 1024 AND 10000000),
 run_token_limit bigint NOT NULL CHECK(run_token_limit BETWEEN 1024 AND 10000000),
 automatic_enabled boolean NOT NULL DEFAULT false,
 collection_enabled boolean NOT NULL DEFAULT false,
 status text NOT NULL DEFAULT 'active' CHECK(status IN ('active','paused','stopped')),
 scan_after uuid,
 revision bigint NOT NULL DEFAULT 1,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_research_command (
 request_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
 receipt jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_research_run (
 run_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 request_ref uuid NOT NULL UNIQUE,
 trigger text NOT NULL CHECK(trigger IN ('historical','incremental','on_demand')),
 topic_ref uuid REFERENCES linggan_topic_workspace(topic_ref),
 config_ref uuid NOT NULL REFERENCES linggan_model_config,
 method_version text NOT NULL,
 token_limit bigint NOT NULL CHECK(token_limit BETWEEN 1024 AND 10000000),
 state text NOT NULL CHECK(state IN ('queued','running','daily_budget_paused','paused','stopped','run_budget_exhausted','completed','failed')),
 last_reason text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_research_task (
 task_ref uuid PRIMARY KEY,
 run_ref uuid NOT NULL REFERENCES linggan_topic_map_research_run,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
 input_refs jsonb NOT NULL,
 state text NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','running','succeeded','no_signal','insufficient','stale','failed','stopped','unknown_dispatch')),
 attempt_count integer NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 3),
 lease_token uuid,
 lease_expires_at timestamptz,
 last_reason text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(domain_ref,work_public_ref,input_hash)
);
CREATE TABLE linggan_topic_map_research_request (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation,
 task_ref uuid NOT NULL REFERENCES linggan_topic_map_research_task,
 run_ref uuid NOT NULL REFERENCES linggan_topic_map_research_run,
 attempt_ordinal integer NOT NULL CHECK(attempt_ordinal BETWEEN 1 AND 3),
 request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 request_manifest jsonb NOT NULL,
 budget_day date NOT NULL,
 dispatch_started_at timestamptz,
 deadline_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(task_ref,attempt_ordinal)
);
CREATE TABLE linggan_topic_map_research_result (
 result_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
 output_json jsonb NOT NULL,
 method_version text NOT NULL,
 invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX topic_map_research_queue_idx ON linggan_topic_map_research_task(state,created_at);
CREATE INDEX topic_map_research_budget_idx ON linggan_topic_map_research_request(budget_day,run_ref);
CREATE TRIGGER topic_map_research_result_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_research_result FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
-- Temporary research rounds reference the existing Collection Request/WorkOrder chain.
CREATE TABLE linggan_topic_map_collection_round (
 round_ref uuid PRIMARY KEY,domain_ref uuid NOT NULL REFERENCES observation_domain,
 topic_ref uuid REFERENCES linggan_topic_workspace,scope_hash text NOT NULL,
 purpose text NOT NULL,kind text NOT NULL CHECK(kind IN ('details','comments','search')),
 state text NOT NULL CHECK(state IN ('active','partial','completed','stopped','blocked')),
 request_ref uuid NOT NULL UNIQUE,detail_limit integer NOT NULL DEFAULT 10 CHECK(detail_limit BETWEEN 1 AND 10),
 last_reason text,created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE UNIQUE INDEX topic_map_collection_active_scope ON linggan_topic_map_collection_round(domain_ref,scope_hash,kind) WHERE state='active';
CREATE TABLE linggan_topic_map_collection_slot (
 round_ref uuid NOT NULL REFERENCES linggan_topic_map_collection_round,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 collection_request_ref uuid,work_order_ref uuid REFERENCES collection_work_order,
 state text NOT NULL CHECK(state IN ('reserved','admitted','blocked')),
 PRIMARY KEY(round_ref,work_public_ref)
);
CREATE TABLE linggan_topic_map_comment_budget (
 work_order_ref uuid NOT NULL REFERENCES collection_work_order,
 content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 known_comment_ids jsonb NOT NULL CHECK(jsonb_typeof(known_comment_ids)='array' AND jsonb_array_length(known_comment_ids)<=5000),
 new_unique_limit integer NOT NULL DEFAULT 30 CHECK(new_unique_limit BETWEEN 1 AND 30),
 max_scroll_rounds integer NOT NULL CHECK(max_scroll_rounds BETWEEN 1 AND 50),
 max_duration_seconds integer NOT NULL CHECK(max_duration_seconds BETWEEN 1 AND 600),
 PRIMARY KEY(work_order_ref,content_public_ref)
);
CREATE TRIGGER topic_map_comment_budget_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_comment_budget FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
