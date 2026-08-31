-- XHS-MEDIA-AUTHOR-EVIDENCE-001 · author avatars use the existing media fact chain.
--
-- The slot/origin tables predate the unified relationship vocabulary. This additive migration
-- widens only the origin purpose; author identity remains explicit in
-- linggan_media_resource_relation and no remote URL becomes presentation data.

ALTER TABLE linggan_material_media_origin
    DROP CONSTRAINT linggan_material_media_origin_purpose_check,
    ADD CONSTRAINT linggan_material_media_origin_purpose_check
        CHECK (purpose IN ('cover','body_image','video','live_photo','author_avatar'));

COMMENT ON COLUMN linggan_material_media_origin.purpose IS
    'Observed media purpose. author_avatar is an author-owned relationship observed in a content-detail context.';
