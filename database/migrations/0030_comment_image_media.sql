-- PLUGIN-XHS-FINALIZATION-001 · comment images reuse the existing media fact chain.
--
-- The canonical relationship vocabulary already reserves `comment.image`. This migration only
-- widens the typed material-origin purpose so a comment-owned slot can be observed, downloaded,
-- materialized and read beside the content media from the same detail Attempt.

ALTER TABLE linggan_material_media_origin
    DROP CONSTRAINT linggan_material_media_origin_purpose_check,
    ADD CONSTRAINT linggan_material_media_origin_purpose_check
        CHECK (purpose IN (
            'cover','body_image','video','live_photo','author_avatar','comment_image'
        ));

COMMENT ON COLUMN linggan_material_media_origin.purpose IS
    'Observed media purpose. author_avatar and comment_image retain their explicit relationship owner while using the containing work as read context.';
