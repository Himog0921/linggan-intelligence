CREATE TABLE linggan_content_understanding_deep (
 analysis_ref uuid PRIMARY KEY REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 core_ref uuid NOT NULL REFERENCES linggan_content_understanding_result(request_ref),
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 revision bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
 method_version text NOT NULL CHECK(char_length(method_version) BETWEEN 1 AND 100),
 question text NOT NULL CHECK(char_length(question) BETWEEN 1 AND 300),
 findings jsonb NOT NULL CHECK(jsonb_typeof(findings)='array'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX content_understanding_deep_latest ON linggan_content_understanding_deep(core_ref, domain_ref, revision DESC);
CREATE TRIGGER content_understanding_deep_immutable BEFORE UPDATE OR DELETE ON linggan_content_understanding_deep
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
