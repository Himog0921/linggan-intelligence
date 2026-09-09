-- CI-AUTO-004 A10: a repair is a bounded, one-shot supplement to an immutable analysis.
-- The original result is never updated.  A successful repair appends one complete merged
-- analysis version and repoints the existing semantic ownership to that version.
CREATE TABLE linggan_comment_field_repair (
    repair_ref uuid PRIMARY KEY,
    base_analysis_ref uuid NOT NULL REFERENCES linggan_comment_analysis_work(work_ref),
    supplement_analysis_ref uuid UNIQUE REFERENCES linggan_comment_analysis_work(work_ref),
    semantic_ref uuid NOT NULL REFERENCES linggan_comment_semantic_work(semantic_ref),
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    rule_hash text NOT NULL CHECK(char_length(rule_hash)=64),
    schema_version text NOT NULL CHECK(char_length(schema_version) BETWEEN 1 AND 100),
    selector_version text NOT NULL CHECK(char_length(selector_version) BETWEEN 1 AND 100),
    semantic_fingerprint text NOT NULL CHECK(char_length(semantic_fingerprint)=64),
    source_sha256 text NOT NULL CHECK(char_length(source_sha256)=64),
    context_fingerprint text NOT NULL CHECK(char_length(context_fingerprint)=64),
    requested_fields jsonb NOT NULL CHECK(jsonb_typeof(requested_fields)='array' AND jsonb_array_length(requested_fields) BETWEEN 1 AND 8),
    requested_fields_hash text NOT NULL CHECK(char_length(requested_fields_hash)=64),
    selected_field jsonb NOT NULL CHECK(jsonb_typeof(selected_field)='object'),
    base_accepted_field_hashes jsonb NOT NULL CHECK(jsonb_typeof(base_accepted_field_hashes)='array'),
    request_hash text NOT NULL UNIQUE CHECK(char_length(request_hash)=64),
    invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    state text NOT NULL CHECK(state IN ('queued','running','succeeded','succeeded_partial','failed','expired','source_unavailable')),
    attempt_count smallint NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 1),
    execution_day date,
    failure_code text,
    result_manifest jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(result_manifest)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    started_at timestamptz,
    lease_until timestamptz,
    finished_at timestamptz,
    trace_expires_at timestamptz NOT NULL DEFAULT scope_001_now()+interval '24 hours',
    CHECK((state='running')=(invocation_ref IS NOT NULL AND lease_until IS NOT NULL)),
    UNIQUE(base_analysis_ref,requested_fields_hash)
);
CREATE INDEX linggan_comment_field_repair_dispatch_idx
 ON linggan_comment_field_repair(state,created_at) WHERE state='queued';
CREATE INDEX linggan_comment_field_repair_expiry_idx
 ON linggan_comment_field_repair(trace_expires_at) WHERE state IN ('queued','running');

-- This is deliberately not an immutable receipt: expiry/restriction purge must erase the
-- retained body while keeping hashes and the non-content repair receipt auditable.
CREATE TABLE linggan_comment_field_repair_trace (
    repair_ref uuid PRIMARY KEY REFERENCES linggan_comment_field_repair(repair_ref),
    invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    input_content jsonb,
    output_content text,
    input_hash text NOT NULL CHECK(char_length(input_hash)=64),
    output_hash text,
    context_guard jsonb NOT NULL CHECK(jsonb_typeof(context_guard)='object'),
    validation jsonb NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(validation)='array'),
    receipt jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(receipt)='object'),
    expires_at timestamptz NOT NULL DEFAULT scope_001_now()+interval '24 hours',
    purged_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK(input_content IS NULL OR octet_length(input_content::text)<=65536),
    CHECK(output_content IS NULL OR octet_length(output_content)<=65536)
);
CREATE INDEX linggan_comment_field_repair_trace_expiry_idx
 ON linggan_comment_field_repair_trace(expires_at) WHERE purged_at IS NULL;
