-- MATERIAL-DISCOVERY-001
-- Typed discovery findings join new accepted Producer packages to the work-level material identity.

CREATE TABLE linggan_material_discovery_finding (
    material_ref uuid PRIMARY KEY,
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    discovery_kind text NOT NULL CHECK (discovery_kind IN ('discovery_search','profile_discovery')),
    result_position integer CHECK (result_position > 0),
    observed_at text NOT NULL,
    title text,
    title_state text NOT NULL CHECK (title_state IN ('KNOWN','UNKNOWN')),
    creator_display_name text,
    creator_state text NOT NULL CHECK (creator_state IN ('KNOWN','UNKNOWN')),
    published_at_source_text text,
    published_at_source_text_state text NOT NULL CHECK (published_at_source_text_state IN ('KNOWN','UNKNOWN')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref,record_ordinal),
    FOREIGN KEY (package_ref,record_ordinal) REFERENCES linggan_runtime_record_disposition(package_ref,record_ordinal),
    CHECK ((title_state='KNOWN')=(title IS NOT NULL)),
    CHECK ((creator_state='KNOWN')=(creator_display_name IS NOT NULL)),
    CHECK ((published_at_source_text_state='KNOWN')=(published_at_source_text IS NOT NULL))
);
CREATE INDEX linggan_material_discovery_finding_content_idx ON linggan_material_discovery_finding(content_public_ref,created_at DESC);
CREATE TRIGGER linggan_material_discovery_finding_is_append_only BEFORE UPDATE OR DELETE ON linggan_material_discovery_finding FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
