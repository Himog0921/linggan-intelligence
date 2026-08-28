-- MATERIAL-PROJECTION-001
--
-- Typed, append-only material observations derived from already accepted Browser Producer
-- records. These rows reference the immutable package + record ordinal; they do not copy the raw
-- payload or retroactively interpret historical packages.

CREATE TABLE linggan_material_content (
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    content_external_id text NOT NULL CHECK (length(content_external_id) > 0),
    public_ref uuid NOT NULL UNIQUE,
    first_package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (platform, content_external_id)
);

CREATE TABLE linggan_material_content_detail (
    material_ref uuid PRIMARY KEY,
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    observed_at text NOT NULL,
    title text,
    title_state text NOT NULL CHECK (title_state IN ('KNOWN', 'UNKNOWN')),
    body_text text,
    body_state text NOT NULL CHECK (body_state IN ('KNOWN', 'UNKNOWN')),
    creator_display_name text,
    creator_display_name_state text NOT NULL CHECK (creator_display_name_state IN ('KNOWN', 'UNKNOWN')),
    published_at_source_text text,
    published_at_source_text_state text NOT NULL CHECK (published_at_source_text_state IN ('KNOWN', 'UNKNOWN')),
    searchable_text text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_runtime_record_disposition(package_ref, record_ordinal),
    CHECK ((title_state = 'KNOWN') = (title IS NOT NULL)),
    CHECK ((body_state = 'KNOWN') = (body_text IS NOT NULL)),
    CHECK ((creator_display_name_state = 'KNOWN') = (creator_display_name IS NOT NULL)),
    CHECK ((published_at_source_text_state = 'KNOWN') = (published_at_source_text IS NOT NULL))
);

CREATE INDEX linggan_material_content_detail_content_idx
    ON linggan_material_content_detail(content_public_ref, created_at DESC);

CREATE TRIGGER linggan_material_content_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_material_content
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TRIGGER linggan_material_content_detail_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_material_content_detail
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
