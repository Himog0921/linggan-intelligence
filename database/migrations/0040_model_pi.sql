-- MODEL-PI-001: local workspace settings and a bounded adapter invocation ledger.
CREATE TABLE linggan_model_workspace (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    workspace_ref uuid NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    default_config_ref uuid,
    active_auto_plan_ref uuid,
    worker_last_seen_at timestamptz,
    worker_state text CHECK(worker_state IN ('idle','running','error')),
    worker_last_error text
);
INSERT INTO linggan_model_workspace(singleton) VALUES(true);
CREATE TABLE linggan_model_connection (
    connection_ref uuid PRIMARY KEY,
    enabled boolean NOT NULL DEFAULT true,
    revision integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_model_connection_version (
    version_ref uuid PRIMARY KEY,
    connection_ref uuid NOT NULL REFERENCES linggan_model_connection,
    revision integer NOT NULL CHECK(revision>0),
    name text NOT NULL CHECK(char_length(name) BETWEEN 1 AND 100),
    api text NOT NULL CHECK(api IN ('openai-completions','openai-responses','anthropic-messages')),
    base_url text NOT NULL CHECK(char_length(base_url)<=1000),
    local_endpoint boolean NOT NULL,
    secret_ref uuid NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(connection_ref,revision)
);
CREATE TABLE linggan_model_entry (
    model_ref uuid PRIMARY KEY,
    connection_version_ref uuid NOT NULL REFERENCES linggan_model_connection_version,
    model_id text NOT NULL CHECK(char_length(model_id) BETWEEN 1 AND 200),
    origin text NOT NULL CHECK(origin IN ('manual','account_endpoint')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(connection_version_ref,model_id)
);
CREATE TABLE linggan_model_config (
    config_ref uuid PRIMARY KEY,
    model_ref uuid NOT NULL REFERENCES linggan_model_entry,
    input_token_limit integer NOT NULL CHECK(input_token_limit BETWEEN 1024 AND 32768),
    output_token_limit integer NOT NULL CHECK(output_token_limit BETWEEN 128 AND 8192),
    timeout_seconds integer NOT NULL CHECK(timeout_seconds BETWEEN 1 AND 60),
    max_attempts integer NOT NULL CHECK(max_attempts BETWEEN 1 AND 3),
    auto_source_limit integer NOT NULL CHECK(auto_source_limit BETWEEN 1 AND 1000),
    auto_token_limit bigint NOT NULL CHECK(auto_token_limit BETWEEN 1024 AND 10000000),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_model_plan (
    plan_ref uuid PRIMARY KEY,
    config_ref uuid NOT NULL REFERENCES linggan_model_config,
    kind text NOT NULL CHECK(kind IN ('trial','automatic','backfill')),
    revision integer NOT NULL DEFAULT 0 CHECK(revision>=0),
    enabled boolean NOT NULL,
    source_limit integer NOT NULL CHECK(source_limit BETWEEN 1 AND 1000),
    token_limit bigint NOT NULL CHECK(token_limit BETWEEN 1024 AND 10000000),
    request jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_comment_model_work (
    work_ref uuid PRIMARY KEY REFERENCES linggan_comment_analysis_work,
    plan_ref uuid NOT NULL REFERENCES linggan_model_plan,
    config_ref uuid NOT NULL REFERENCES linggan_model_config,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_model_invocation (
    invocation_ref uuid PRIMARY KEY,
    connection_version_ref uuid NOT NULL REFERENCES linggan_model_connection_version,
    model_ref uuid REFERENCES linggan_model_entry,
    work_ref uuid REFERENCES linggan_comment_analysis_work,
    plan_ref uuid REFERENCES linggan_model_plan,
    config_ref uuid REFERENCES linggan_model_config,
    operation text NOT NULL CHECK(operation IN ('connect','discover','probe','analyze')),
    request_hash text NOT NULL,
    state text NOT NULL CHECK(state IN ('running','succeeded','failed')),
    reserved_tokens bigint NOT NULL CHECK(reserved_tokens>=0),
    charged_tokens bigint NOT NULL CHECK(charged_tokens>=0),
    input_tokens bigint CHECK(input_tokens>=0),
    output_tokens bigint CHECK(output_tokens>=0),
    elapsed_ms bigint,
    failure_code text,
    result jsonb,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz
);
ALTER TABLE linggan_model_workspace ADD FOREIGN KEY(default_config_ref) REFERENCES linggan_model_config;
ALTER TABLE linggan_model_workspace ADD FOREIGN KEY(active_auto_plan_ref) REFERENCES linggan_model_plan;
CREATE TRIGGER linggan_model_connection_version_immutable BEFORE UPDATE OR DELETE ON linggan_model_connection_version
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_model_entry_immutable BEFORE UPDATE OR DELETE ON linggan_model_entry
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_model_config_immutable BEFORE UPDATE OR DELETE ON linggan_model_config
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE INDEX linggan_model_invocation_plan_idx ON linggan_model_invocation(plan_ref,created_at);
CREATE UNIQUE INDEX linggan_model_one_running_call_per_work ON linggan_model_invocation(work_ref)
WHERE work_ref IS NOT NULL AND state='running';
