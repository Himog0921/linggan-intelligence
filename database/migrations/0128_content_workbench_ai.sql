-- Explicit creation suggestions; no scheduler, research target or automatic dispatch.
CREATE TABLE linggan_creation_ai_preview (
 preview_ref uuid PRIMARY KEY,
 work_ref uuid NOT NULL REFERENCES linggan_creation_work(work_ref),
 input_revision uuid NOT NULL REFERENCES linggan_creation_revision(revision_ref),
 config_ref uuid NOT NULL REFERENCES linggan_model_config(config_ref),
 request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 method_version text NOT NULL CHECK(method_version='content-workbench-writing.v1'),
 action text NOT NULL CHECK(action IN ('draft','rewrite_selection','page_copy')),
 input_manifest jsonb NOT NULL CHECK(jsonb_typeof(input_manifest)='object'),
 reserved_tokens bigint NOT NULL CHECK(reserved_tokens>0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_creation_ai_request (
 request_ref uuid PRIMARY KEY,
 command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
 preview_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_ai_preview(preview_ref),
 invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
 state text NOT NULL CHECK(state IN ('running','succeeded','failed','cancel_requested','cancelled')),
 dispatch_started boolean NOT NULL DEFAULT false,
 suggestion jsonb,
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 finished_at timestamptz,
 CHECK(state<>'succeeded' OR suggestion IS NOT NULL)
);
CREATE TABLE linggan_creation_ai_suggestion (
 suggestion_ref uuid PRIMARY KEY,
 work_ref uuid NOT NULL REFERENCES linggan_creation_work(work_ref),
 request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_ai_request(request_ref),
 UNIQUE(work_ref,suggestion_ref)
);
ALTER TABLE linggan_creation_adoption ADD FOREIGN KEY(work_ref,suggestion_ref) REFERENCES linggan_creation_ai_suggestion(work_ref,suggestion_ref);
CREATE TABLE linggan_creation_ai_adoption_request (
 request_ref uuid PRIMARY KEY REFERENCES linggan_creation_request(request_ref),
 suggestion_ref uuid NOT NULL REFERENCES linggan_creation_ai_suggestion(suggestion_ref),
 command_hash text NOT NULL CHECK(command_hash ~ '^[0-9a-f]{64}$'),
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object')
);
CREATE TRIGGER linggan_creation_ai_adoption_request_immutable BEFORE UPDATE OR DELETE ON linggan_creation_ai_adoption_request FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_creation_writing_qualification (
 model_ref uuid NOT NULL REFERENCES linggan_model_entry(model_ref),
 connection_version_ref uuid NOT NULL REFERENCES linggan_model_connection_version(version_ref),
 method_version text NOT NULL CHECK(method_version='content-workbench-writing.v1'),
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation(invocation_ref),
 qualified boolean NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(model_ref,connection_version_ref,invocation_ref)
);
CREATE TRIGGER linggan_creation_ai_preview_immutable BEFORE UPDATE OR DELETE ON linggan_creation_ai_preview FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_creation_writing_qualification_immutable BEFORE UPDATE OR DELETE ON linggan_creation_writing_qualification FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
