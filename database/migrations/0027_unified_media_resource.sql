-- PLUGIN-XHS-FINALIZATION-001 · one media relationship vocabulary over the existing asset chain
--
-- This is additive and intentionally does not backfill immutable historical Packages or slots.
-- Existing content slots remain readable through the compatibility projection; newly admitted
-- media records also receive one canonical relationship row.

ALTER TABLE linggan_media_blob
    ADD COLUMN pixel_width integer CHECK (pixel_width > 0),
    ADD COLUMN pixel_height integer CHECK (pixel_height > 0),
    ADD COLUMN duration_ms bigint CHECK (duration_ms >= 0),
    ADD CONSTRAINT linggan_media_blob_dimensions_pair_check
        CHECK ((pixel_width IS NULL) = (pixel_height IS NULL));

CREATE TABLE linggan_media_resource_relation (
    relation_ref uuid PRIMARY KEY,
    platform text NOT NULL CHECK (platform IN ('xhs','douyin')),
    subject_kind text NOT NULL CHECK (subject_kind IN ('author','content','comment')),
    subject_external_id text NOT NULL CHECK (length(btrim(subject_external_id)) > 0),
    subject_public_ref uuid,
    relationship_kind text NOT NULL CHECK (relationship_kind IN (
        'author.avatar',
        'content.cover',
        'content.image',
        'content.video',
        'content.ocr',
        'content.transcript',
        'comment.image'
    )),
    relationship_ordinal integer NOT NULL CHECK (relationship_ordinal > 0),
    slot_key text REFERENCES linggan_media_slot(slot_key),
    derivative_ref uuid REFERENCES linggan_media_derivative(derivative_ref),
    source_package_ref uuid REFERENCES linggan_runtime_capture_package(package_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (platform,subject_kind,subject_external_id,relationship_kind,relationship_ordinal),
    CHECK (num_nonnulls(slot_key,derivative_ref)=1),
    CHECK (
        (subject_kind='author' AND relationship_kind='author.avatar')
        OR (subject_kind='comment' AND relationship_kind='comment.image')
        OR (subject_kind='content' AND relationship_kind LIKE 'content.%')
    ),
    CHECK (
        (relationship_kind IN ('content.ocr','content.transcript') AND derivative_ref IS NOT NULL)
        OR (relationship_kind NOT IN ('content.ocr','content.transcript') AND slot_key IS NOT NULL)
    )
);

CREATE INDEX linggan_media_resource_relation_subject_idx
    ON linggan_media_resource_relation(platform,subject_kind,subject_external_id,relationship_kind,relationship_ordinal);
CREATE INDEX linggan_media_resource_relation_public_idx
    ON linggan_media_resource_relation(subject_public_ref,relationship_kind)
    WHERE subject_public_ref IS NOT NULL;
CREATE UNIQUE INDEX linggan_media_resource_relation_slot_idx
    ON linggan_media_resource_relation(slot_key)
    WHERE slot_key IS NOT NULL;

CREATE TRIGGER linggan_media_resource_relation_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_media_resource_relation
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE linggan_media_resource_relation IS
    'Canonical business relationship vocabulary over the existing Slot/Blob/Materialization/Derivative chain; not a second asset model.';
COMMENT ON COLUMN linggan_media_resource_relation.relationship_ordinal IS
    'Ordinal inside one relationship kind. It is never the mixed producer record position.';
