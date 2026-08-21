CREATE FUNCTION scope_001_now() RETURNS timestamptz
    LANGUAGE sql
    VOLATILE
    AS $$ SELECT clock_timestamp() $$;

CREATE TABLE capture_work_order (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    work_order_ref uuid NOT NULL UNIQUE,
    contract_version text NOT NULL CHECK (contract_version = 'content-detail.synthetic.v1'),
    target_basis text NOT NULL CHECK (target_basis IN ('known_set', 'maximum_quota')),
    target_unit text NOT NULL CHECK (target_unit = 'content_detail'),
    target_manifest_hash text,
    known_target_count integer,
    quota_limit integer,
    CHECK ((target_basis = 'known_set' AND target_manifest_hash IS NOT NULL AND known_target_count IS NOT NULL AND known_target_count >= 0 AND quota_limit IS NULL) OR (target_basis = 'maximum_quota' AND target_manifest_hash IS NULL AND known_target_count IS NULL AND quota_limit IS NOT NULL AND quota_limit > 0))
);

CREATE TABLE capture_work_order_target (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    work_order_id bigint NOT NULL REFERENCES capture_work_order(id),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    external_id text NOT NULL CHECK (length(external_id) > 0),
    UNIQUE (id, work_order_id),
    UNIQUE (work_order_id, ordinal),
    UNIQUE (work_order_id, external_id)
);

CREATE TABLE capture_attempt (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    attempt_ref uuid NOT NULL UNIQUE,
    capture_identity uuid NOT NULL UNIQUE,
    work_order_id bigint NOT NULL UNIQUE REFERENCES capture_work_order(id),
    lease_epoch integer NOT NULL CHECK (lease_epoch > 0),
    authority_valid_until timestamptz NOT NULL,
    authority_revoked_at timestamptz,
    terminal_reason text CHECK (terminal_reason IN ('target_reached', 'risk_control')),
    UNIQUE (id, work_order_id),
    UNIQUE (id, capture_identity)
);

CREATE TABLE capture_ingress_delivery (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    audit_kind text NOT NULL CHECK (audit_kind IN ('pre_routing_error', 'public_delivery')),
    delivery_ref uuid UNIQUE,
    work_order_id bigint,
    attempt_id bigint,
    capture_identity uuid,
    outcome text CHECK (outcome IN ('accepted', 'replay', 'conflict', 'rejected')),
    external_code text,
    received_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (id, attempt_id, capture_identity, outcome),
    FOREIGN KEY (attempt_id, work_order_id) REFERENCES capture_attempt(id, work_order_id),
    FOREIGN KEY (attempt_id, capture_identity) REFERENCES capture_attempt(id, capture_identity),
    CHECK (
        (audit_kind = 'pre_routing_error' AND delivery_ref IS NULL AND work_order_id IS NULL AND attempt_id IS NULL AND capture_identity IS NULL AND outcome IS NULL AND external_code IN ('unauthenticated', 'forbidden', 'body_limit_exceeded', 'malformed_json', 'canonicalization_invalid', 'package_schema_invalid', 'package_hash_invalid', 'record_hash_invalid', 'routing_reference_not_found', 'work_attempt_mismatch', 'attempt_capture_mismatch'))
        OR (audit_kind = 'public_delivery' AND delivery_ref IS NOT NULL AND work_order_id IS NOT NULL AND attempt_id IS NOT NULL AND capture_identity IS NOT NULL AND outcome IS NOT NULL AND ((outcome = 'rejected' AND external_code IN ('lease_epoch_mismatch', 'target_mismatch', 'authority_revoked', 'authority_expired')) OR (outcome IN ('accepted', 'replay', 'conflict') AND external_code IS NULL)))
    )
);

CREATE TABLE capture_package (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    package_ref uuid NOT NULL UNIQUE,
    work_order_id bigint NOT NULL,
    attempt_id bigint NOT NULL UNIQUE,
    capture_identity uuid NOT NULL UNIQUE,
    package_hash text NOT NULL UNIQUE,
    accepted_delivery_id bigint NOT NULL UNIQUE,
    accepted_receipt_ref uuid NOT NULL UNIQUE,
    accepted_outcome text NOT NULL DEFAULT 'accepted' CHECK (accepted_outcome = 'accepted'),
    accepted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (id, work_order_id),
    FOREIGN KEY (attempt_id, work_order_id) REFERENCES capture_attempt(id, work_order_id),
    FOREIGN KEY (attempt_id, capture_identity) REFERENCES capture_attempt(id, capture_identity),
    FOREIGN KEY (accepted_delivery_id, attempt_id, capture_identity, accepted_outcome) REFERENCES capture_ingress_delivery(id, attempt_id, capture_identity, outcome)
);

CREATE TABLE capture_record (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    record_ref uuid NOT NULL UNIQUE,
    package_id bigint NOT NULL REFERENCES capture_package(id),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    target_external_id text NOT NULL CHECK (length(target_external_id) > 0),
    record_hash text NOT NULL,
    payload jsonb NOT NULL,
    UNIQUE (id, package_id),
    UNIQUE (package_id, ordinal)
);

CREATE TABLE capture_package_target_result (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    package_id bigint NOT NULL,
    work_order_id bigint NOT NULL,
    target_id bigint NOT NULL,
    outcome text NOT NULL CHECK (outcome IN ('emitted', 'failed', 'not_attempted')),
    record_id bigint,
    reason text,
    FOREIGN KEY (package_id, work_order_id) REFERENCES capture_package(id, work_order_id),
    FOREIGN KEY (target_id, work_order_id) REFERENCES capture_work_order_target(id, work_order_id),
    FOREIGN KEY (record_id, package_id) REFERENCES capture_record(id, package_id),
    UNIQUE (package_id, target_id),
    CHECK ((outcome = 'emitted' AND record_id IS NOT NULL AND reason IS NULL) OR (outcome = 'failed' AND record_id IS NULL AND reason = 'page_error') OR (outcome = 'not_attempted' AND record_id IS NULL AND reason = 'risk_control'))
);

CREATE TABLE capture_package_coverage (
    package_id bigint PRIMARY KEY REFERENCES capture_package(id),
    unit text NOT NULL CHECK (unit = 'content_detail'),
    attempted integer NOT NULL CHECK (attempted >= 0),
    emitted integer NOT NULL CHECK (emitted >= 0),
    failed integer NOT NULL CHECK (failed >= 0),
    known_not_attempted integer,
    remaining_scope text NOT NULL CHECK (remaining_scope IN ('known_members', 'unknown')),
    CHECK (emitted + failed = attempted),
    CHECK ((remaining_scope = 'known_members' AND known_not_attempted IS NOT NULL AND known_not_attempted >= 0) OR (remaining_scope = 'unknown' AND known_not_attempted IS NULL))
);

CREATE TABLE record_processing_work (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    processing_work_ref uuid NOT NULL UNIQUE,
    capture_record_id bigint NOT NULL UNIQUE REFERENCES capture_record(id),
    processor_version text NOT NULL CHECK (processor_version = 'content-detail-processor-v1')
);
