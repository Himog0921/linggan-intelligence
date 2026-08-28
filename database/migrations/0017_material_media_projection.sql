-- MATERIAL-MEDIA-001
-- Source-observation extension, complete candidate assertions, and append-only disposition facts.

ALTER TABLE linggan_media_materialization
    DROP CONSTRAINT linggan_media_materialization_local_asset_path_check,
    ADD CONSTRAINT linggan_media_materialization_local_asset_path_check
        CHECK (local_asset_path ~ '^/api/local/media/[0-9a-f]{64}$'
            OR local_asset_path ~ '^/api/local/media/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/[0-9a-f]{64}$');

CREATE TABLE linggan_material_media_origin (
    observation_ref uuid PRIMARY KEY REFERENCES linggan_media_observation(observation_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    slot_key text NOT NULL REFERENCES linggan_media_slot(slot_key),
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    source_generation integer NOT NULL CHECK (source_generation > 0),
    purpose text NOT NULL CHECK (purpose IN ('cover','body_image','video','live_photo')),
    producer_ordinal integer NOT NULL CHECK (producer_ordinal > 0),
    display_ordinal integer CHECK (display_ordinal > 0),
    display_order_state text NOT NULL CHECK (display_order_state IN ('KNOWN','UNKNOWN')),
    display_order_basis text NOT NULL CHECK (display_order_basis IN ('platform_explicit','producer_global_sequence_unverified','unknown')),
    candidate_set_state text NOT NULL CHECK (candidate_set_state IN ('OBSERVED_SET','UNKNOWN')),
    composite_state text NOT NULL CHECK (composite_state IN ('NOT_APPLICABLE','PARTIAL','COMPLETE','UNKNOWN')),
    live_photo_still_state text CHECK (live_photo_still_state IN ('OBSERVED','ACQUIRED','UNAVAILABLE','UNKNOWN')),
    live_photo_motion_state text CHECK (live_photo_motion_state IN ('OBSERVED','ACQUIRED','UNAVAILABLE','UNKNOWN')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref,record_ordinal),
    UNIQUE (package_ref,slot_key),
    UNIQUE (slot_key,source_generation),
    FOREIGN KEY (package_ref,record_ordinal) REFERENCES linggan_runtime_record_disposition(package_ref,record_ordinal),
    CHECK ((display_order_state='KNOWN')=(display_ordinal IS NOT NULL)),
    CHECK ((purpose='live_photo' AND composite_state IN ('PARTIAL','UNKNOWN') AND live_photo_still_state IS NOT NULL AND live_photo_motion_state IS NOT NULL)
        OR (purpose<>'live_photo' AND composite_state='NOT_APPLICABLE' AND live_photo_still_state IS NULL AND live_photo_motion_state IS NULL))
);

CREATE TABLE linggan_material_media_candidate (
    candidate_ref uuid PRIMARY KEY,
    observation_ref uuid NOT NULL REFERENCES linggan_material_media_origin(observation_ref),
    candidate_ordinal integer NOT NULL CHECK (candidate_ordinal > 0),
    external_uri text NOT NULL CHECK (length(btrim(external_uri)) > 0),
    producer_primary boolean NOT NULL,
    source_field text,
    source_field_state text NOT NULL CHECK (source_field_state IN ('KNOWN','UNKNOWN')),
    expires_at timestamptz,
    expires_at_state text NOT NULL CHECK (expires_at_state IN ('KNOWN','UNKNOWN')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (observation_ref,candidate_ordinal),
    UNIQUE (observation_ref,external_uri),
    CHECK ((source_field_state='KNOWN')=(source_field IS NOT NULL)),
    CHECK ((expires_at_state='KNOWN')=(expires_at IS NOT NULL))
);
CREATE UNIQUE INDEX linggan_material_media_candidate_primary_idx ON linggan_material_media_candidate(observation_ref) WHERE producer_primary;

CREATE TABLE linggan_material_media_disposition_event (
    event_ref uuid PRIMARY KEY,
    slot_key text REFERENCES linggan_media_slot(slot_key),
    blob_sha256 text REFERENCES linggan_media_blob(sha256),
    materialization_ref uuid REFERENCES linggan_media_materialization(materialization_ref),
    derivative_ref uuid REFERENCES linggan_media_derivative(derivative_ref),
    state text NOT NULL CHECK (state IN ('BYTES_CLEANED','WITHDRAWN_OR_RESTRICTED')),
    authority_ref text NOT NULL CHECK (length(btrim(authority_ref)) > 0),
    reason text NOT NULL CHECK (length(btrim(reason)) > 0),
    effective_at timestamptz NOT NULL,
    supersedes_event_ref uuid REFERENCES linggan_material_media_disposition_event(event_ref),
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (num_nonnulls(slot_key,blob_sha256,materialization_ref,derivative_ref)=1),
    CHECK (state<>'BYTES_CLEANED' OR materialization_ref IS NOT NULL)
);

CREATE TRIGGER linggan_material_media_origin_is_append_only BEFORE UPDATE OR DELETE ON linggan_material_media_origin FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_material_media_candidate_is_append_only BEFORE UPDATE OR DELETE ON linggan_material_media_candidate FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_material_media_disposition_is_append_only BEFORE UPDATE OR DELETE ON linggan_material_media_disposition_event FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
