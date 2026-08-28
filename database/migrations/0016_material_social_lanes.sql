-- MATERIAL-SOCIAL-001
-- Append-only lane coverage plus stable comment/reply and versioned author observations.

ALTER TABLE linggan_material_content_detail
    ADD COLUMN author_external_id text;

CREATE TABLE linggan_material_lane_observation (
    package_ref uuid PRIMARY KEY REFERENCES linggan_runtime_capture_package(package_ref),
    lane text NOT NULL CHECK (lane IN ('discovery','detail','comments','replies','author','media_slots','media_bytes','ocr','asr')),
    content_public_ref uuid REFERENCES linggan_material_content(public_ref),
    author_external_id text,
    observed integer CHECK (observed >= 0),
    retained integer CHECK (retained >= 0),
    failed integer CHECK (failed >= 0),
    known_unattempted integer CHECK (known_unattempted >= 0),
    unknown_count integer CHECK (unknown_count >= 0),
    maximum_quota integer CHECK (maximum_quota > 0),
    stopped_reason text,
    observed_at text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (content_public_ref IS NOT NULL OR author_external_id IS NOT NULL)
);

CREATE TABLE linggan_material_comment (
    material_ref uuid PRIMARY KEY,
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    comment_external_id text NOT NULL CHECK (length(comment_external_id) > 0),
    root_comment_external_id text NOT NULL CHECK (length(root_comment_external_id) > 0),
    parent_comment_external_id text,
    parent_identity_source_field text,
    is_reply boolean NOT NULL,
    body_text text,
    body_state text NOT NULL CHECK (body_state IN ('KNOWN','UNKNOWN')),
    author_external_id text,
    author_display_name text,
    observed_at text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_runtime_record_disposition(package_ref, record_ordinal),
    CHECK ((body_state = 'KNOWN') = (body_text IS NOT NULL)),
    CHECK ((NOT is_reply AND root_comment_external_id = comment_external_id AND parent_comment_external_id IS NULL)
        OR (is_reply AND parent_comment_external_id IS NOT NULL))
);

CREATE INDEX linggan_material_comment_identity_idx
    ON linggan_material_comment(content_public_ref, comment_external_id, observed_at DESC);
CREATE INDEX linggan_material_comment_tree_idx
    ON linggan_material_comment(content_public_ref, root_comment_external_id, is_reply, observed_at);

CREATE TABLE linggan_material_author_profile (
    material_ref uuid PRIMARY KEY,
    platform text NOT NULL CHECK (platform IN ('xhs','douyin')),
    author_external_id text NOT NULL CHECK (length(author_external_id) > 0),
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    observed_at text NOT NULL,
    display_name text,
    display_name_state text NOT NULL CHECK (display_name_state IN ('KNOWN','UNKNOWN')),
    biography text,
    biography_state text NOT NULL CHECK (biography_state IN ('KNOWN','UNKNOWN')),
    follower_count bigint,
    follower_count_state text NOT NULL CHECK (follower_count_state IN ('KNOWN','UNKNOWN')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_runtime_record_disposition(package_ref, record_ordinal),
    CHECK ((display_name_state = 'KNOWN') = (display_name IS NOT NULL)),
    CHECK ((biography_state = 'KNOWN') = (biography IS NOT NULL)),
    CHECK ((follower_count_state = 'KNOWN') = (follower_count IS NOT NULL)),
    CHECK (follower_count IS NULL OR follower_count >= 0)
);

CREATE INDEX linggan_material_author_profile_identity_idx
    ON linggan_material_author_profile(platform, author_external_id, observed_at DESC);

CREATE TRIGGER linggan_material_lane_observation_is_append_only BEFORE UPDATE OR DELETE
    ON linggan_material_lane_observation FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_material_comment_is_append_only BEFORE UPDATE OR DELETE
    ON linggan_material_comment FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_material_author_profile_is_append_only BEFORE UPDATE OR DELETE
    ON linggan_material_author_profile FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
