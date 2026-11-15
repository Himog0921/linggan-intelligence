-- A reviewed L1 Core extends a published Core without updating either source receipt.
CREATE TABLE linggan_content_understanding_core_revision (
    revision_ref uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    base_request_ref uuid NOT NULL REFERENCES linggan_content_understanding_request(request_ref),
    source_request_ref uuid NOT NULL UNIQUE,
    source_raw_output text NOT NULL,
    source_failure jsonb NOT NULL CHECK (jsonb_typeof(source_failure) = 'object'),
    reviewed_draft text NOT NULL,
    validator_revision text NOT NULL CHECK (validator_revision ~ '^[0-9a-f]{40}$'),
    result jsonb NOT NULL CHECK (jsonb_typeof(result) = 'object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (source_request_ref <> base_request_ref)
);
CREATE INDEX content_understanding_core_revision_latest
    ON linggan_content_understanding_core_revision (base_request_ref, created_at DESC, revision_ref DESC);
CREATE TRIGGER content_understanding_core_revision_immutable
    BEFORE UPDATE OR DELETE ON linggan_content_understanding_core_revision
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
