-- LOCAL-001 / 001B-001C1 Discovery admission storage.
--
-- This migration stores a discovery surface's accepted, immutable account of visible cards. It
-- intentionally does not create detail evidence, comments, creator profiles, media blobs,
-- Observation, Topic, Research, Insight, or any market conclusion.

CREATE TABLE local_discovery_package (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    package_ref uuid NOT NULL UNIQUE,
    package_hash text NOT NULL UNIQUE CHECK (package_hash ~ '^[0-9a-f]{64}$'),
    accepted_receipt_ref uuid NOT NULL UNIQUE,
    contract_version text NOT NULL CHECK (contract_version = 'xhs.discovery.visible-card.v1'),
    platform text NOT NULL CHECK (platform = 'xhs'),
    query_text text NOT NULL CHECK (query_text = 'ADHD'),
    sort text NOT NULL CHECK (sort = 'comprehensive'),
    target_basis text NOT NULL CHECK (target_basis = 'maximum_quota'),
    target_unit text NOT NULL CHECK (target_unit = 'visible_search_card'),
    maximum_quota integer NOT NULL CHECK (maximum_quota = 20),
    observed_at timestamptz NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    payload jsonb NOT NULL
);

CREATE TABLE local_discovery_ingress_delivery (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    delivery_ref uuid NOT NULL UNIQUE,
    package_id bigint NOT NULL REFERENCES local_discovery_package(id),
    outcome text NOT NULL CHECK (outcome IN ('accepted', 'replay')),
    received_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_id, outcome)
);

CREATE TABLE local_discovery_content_item (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    platform text NOT NULL CHECK (platform = 'xhs'),
    platform_content_id text NOT NULL CHECK (length(platform_content_id) > 0),
    UNIQUE (platform, platform_content_id)
);

CREATE TABLE local_discovery_occurrence (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    package_id bigint NOT NULL REFERENCES local_discovery_package(id),
    content_item_id bigint NOT NULL REFERENCES local_discovery_content_item(id),
    result_position integer NOT NULL CHECK (result_position BETWEEN 1 AND 20),
    observed_at timestamptz NOT NULL,
    title text,
    creator_display_name text,
    published_at_source_text text,
    published_at timestamptz,
    published_at_source_kind text NOT NULL CHECK (published_at_source_kind = 'visible_card_text'),
    published_at_reference_observed_at timestamptz NOT NULL,
    published_at_parser_version text NOT NULL CHECK (published_at_parser_version IN ('none', 'rfc3339-source-text-v1')),
    published_at_precision text NOT NULL CHECK (published_at_precision IN ('unknown', 'exact')),
    cover_candidate_external_uri text,
    cover_presentation_state text NOT NULL CHECK (cover_presentation_state = 'media_not_acquired'),
    UNIQUE (package_id, result_position),
    CHECK (
        (published_at IS NULL AND published_at_parser_version = 'none' AND published_at_precision = 'unknown')
        OR (published_at IS NOT NULL AND published_at_parser_version = 'rfc3339-source-text-v1' AND published_at_precision = 'exact')
    )
);

CREATE TABLE local_discovery_coverage (
    package_id bigint PRIMARY KEY REFERENCES local_discovery_package(id),
    unit text NOT NULL CHECK (unit = 'visible_search_card'),
    visible_cards integer NOT NULL CHECK (visible_cards BETWEEN 0 AND 20),
    stopped_reason text NOT NULL CHECK (stopped_reason IN ('quota_reached', 'surface_ended', 'risk_control', 'manual_stop', 'unknown'))
);

CREATE OR REPLACE FUNCTION local_001_forbid_discovery_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN RAISE EXCEPTION 'LOCAL-001 discovery facts are append-only'; END $$;

CREATE TRIGGER local_discovery_package_is_append_only
    BEFORE UPDATE OR DELETE ON local_discovery_package
    FOR EACH ROW EXECUTE FUNCTION local_001_forbid_discovery_mutation();

CREATE TRIGGER local_discovery_delivery_is_append_only
    BEFORE UPDATE OR DELETE ON local_discovery_ingress_delivery
    FOR EACH ROW EXECUTE FUNCTION local_001_forbid_discovery_mutation();

CREATE TRIGGER local_discovery_content_item_is_append_only
    BEFORE UPDATE OR DELETE ON local_discovery_content_item
    FOR EACH ROW EXECUTE FUNCTION local_001_forbid_discovery_mutation();

CREATE TRIGGER local_discovery_occurrence_is_append_only
    BEFORE UPDATE OR DELETE ON local_discovery_occurrence
    FOR EACH ROW EXECUTE FUNCTION local_001_forbid_discovery_mutation();

CREATE TRIGGER local_discovery_coverage_is_append_only
    BEFORE UPDATE OR DELETE ON local_discovery_coverage
    FOR EACH ROW EXECUTE FUNCTION local_001_forbid_discovery_mutation();
