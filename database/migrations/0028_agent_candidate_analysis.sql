CREATE TABLE linggan_agent_invocation (
    invocation_ref uuid PRIMARY KEY,
    idempotency_key text NOT NULL UNIQUE
        CHECK (idempotency_key ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{7,127}$'),
    request_sha256 text NOT NULL CHECK (request_sha256 ~ '^[0-9a-f]{64}$'),
    actor_ref uuid NOT NULL,
    delegation_revision text NOT NULL CHECK (length(delegation_revision) BETWEEN 1 AND 160),
    purpose text NOT NULL CHECK (length(purpose) BETWEEN 1 AND 1000),
    profile_key text NOT NULL CHECK (profile_key ~ '^[a-z0-9][a-z0-9-]{0,63}$'),
    profile_version integer NOT NULL CHECK (profile_version > 0),
    topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace(topic_ref),
    definition_ref uuid NOT NULL REFERENCES linggan_topic_definition(definition_ref),
    definition_version integer NOT NULL CHECK (definition_version > 0),
    classification_run_ref uuid NOT NULL REFERENCES linggan_topic_classification_run(classification_run_ref),
    material_pack_ref uuid NOT NULL REFERENCES linggan_topic_material_pack(material_pack_ref),
    frozen_request jsonb NOT NULL CHECK (jsonb_typeof(frozen_request) = 'object'),
    budget jsonb NOT NULL CHECK (jsonb_typeof(budget) = 'object'),
    adapter_identity text NOT NULL CHECK (length(adapter_identity) BETWEEN 1 AND 160),
    state text NOT NULL CHECK (state IN ('accepted','running','succeeded','cancelled','rejected','failed')),
    candidate_output jsonb,
    failure_code text,
    submitted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    started_at timestamptz,
    finalized_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((state = 'succeeded') = (candidate_output IS NOT NULL)),
    CHECK (candidate_output IS NULL OR jsonb_typeof(candidate_output) = 'object'),
    CHECK (state NOT IN ('cancelled','rejected','failed') OR failure_code IS NOT NULL)
);

CREATE INDEX linggan_agent_invocation_state_idx
    ON linggan_agent_invocation(state, submitted_at, invocation_ref);

CREATE TABLE linggan_agent_tool_receipt (
    receipt_ref uuid PRIMARY KEY,
    invocation_ref uuid NOT NULL REFERENCES linggan_agent_invocation(invocation_ref),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    tool_name text NOT NULL CHECK (tool_name = 'read_topic_material_pack'),
    resource_ref uuid NOT NULL,
    outcome text NOT NULL CHECK (outcome IN ('succeeded','failed')),
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (invocation_ref, ordinal)
);
