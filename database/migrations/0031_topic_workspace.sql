-- TOPIC-WORKSPACE-REAL-001
--
-- A provisional Topic is an explicit human research decision over already admitted Work
-- Resources. The workspace stores identities, versioned definitions, adjudication and exact
-- Work Resource references only. It never copies source text, engagement fields or Evidence.

CREATE TABLE linggan_topic_workspace (
    topic_ref uuid PRIMARY KEY,
    domain_key text NOT NULL CHECK (domain_key ~ '^[a-z0-9][a-z0-9_-]{1,63}$'),
    canonical_key text NOT NULL UNIQUE CHECK (canonical_key ~ '^[a-z0-9][a-z0-9_-]{1,95}$'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_topic_definition (
    definition_ref uuid PRIMARY KEY,
    topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace(topic_ref),
    version integer NOT NULL CHECK (version > 0),
    display_name text NOT NULL CHECK (length(btrim(display_name)) BETWEEN 1 AND 120),
    definition_text text NOT NULL CHECK (length(btrim(definition_text)) BETWEEN 1 AND 2000),
    lifecycle_state text NOT NULL CHECK (lifecycle_state = 'provisional'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (topic_ref, version)
);

CREATE TABLE linggan_topic_classification_run (
    classification_run_ref uuid PRIMARY KEY,
    definition_ref uuid NOT NULL UNIQUE REFERENCES linggan_topic_definition(definition_ref),
    run_kind text NOT NULL CHECK (run_kind = 'human_adjudicated'),
    run_state text NOT NULL CHECK (run_state = 'completed'),
    adjudication_note text NOT NULL CHECK (length(btrim(adjudication_note)) BETWEEN 1 AND 2000),
    completed_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_topic_material_pack (
    material_pack_ref uuid PRIMARY KEY,
    classification_run_ref uuid NOT NULL UNIQUE
        REFERENCES linggan_topic_classification_run(classification_run_ref),
    source_boundary text NOT NULL CHECK (length(btrim(source_boundary)) BETWEEN 1 AND 2000),
    frozen_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_topic_material_member (
    classification_run_ref uuid NOT NULL
        REFERENCES linggan_topic_classification_run(classification_run_ref),
    work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    role text NOT NULL CHECK (role IN ('support', 'challenge', 'boundary')),
    rationale text NOT NULL CHECK (length(btrim(rationale)) BETWEEN 1 AND 1000),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    PRIMARY KEY (classification_run_ref, work_public_ref),
    UNIQUE (classification_run_ref, ordinal)
);

CREATE TABLE linggan_topic_import_receipt (
    receipt_ref uuid PRIMARY KEY,
    idempotency_key text NOT NULL UNIQUE
        CHECK (idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,127}$'),
    request_sha256 text NOT NULL CHECK (request_sha256 ~ '^[0-9a-f]{64}$'),
    topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace(topic_ref),
    definition_ref uuid NOT NULL UNIQUE REFERENCES linggan_topic_definition(definition_ref),
    classification_run_ref uuid NOT NULL UNIQUE
        REFERENCES linggan_topic_classification_run(classification_run_ref),
    material_pack_ref uuid NOT NULL UNIQUE
        REFERENCES linggan_topic_material_pack(material_pack_ref),
    imported_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX linggan_topic_definition_current_idx
    ON linggan_topic_definition(topic_ref, version DESC);

CREATE INDEX linggan_topic_material_member_role_idx
    ON linggan_topic_material_member(classification_run_ref, role, ordinal);

CREATE TRIGGER linggan_topic_workspace_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_workspace
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_topic_definition_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_definition
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_topic_classification_run_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_classification_run
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_topic_material_pack_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_material_pack
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_topic_material_member_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_material_member
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_topic_import_receipt_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_topic_import_receipt
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
