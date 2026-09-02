-- AUTHOR-TARGET-SYNC-001
--
-- A profile-only author avatar is an author-owned media observation.  It has no honest work
-- context, so it must not fabricate a `linggan_material_content` row just to reuse the durable
-- Slot → Observation → Blob → Materialization lifecycle.  Existing detail-context author
-- avatars remain unchanged and still retain their real content_public_ref.

ALTER TABLE linggan_material_media_origin
    ALTER COLUMN content_public_ref DROP NOT NULL;

ALTER TABLE linggan_material_lane_observation
    DROP CONSTRAINT linggan_material_lane_observation_check,
    ADD CONSTRAINT linggan_material_lane_observation_check
        CHECK (
            (lane = 'author' AND content_public_ref IS NULL AND author_external_id IS NOT NULL)
            OR (
                lane = 'media_slots'
                AND (
                    (content_public_ref IS NOT NULL AND author_external_id IS NULL)
                    OR (content_public_ref IS NULL AND author_external_id IS NOT NULL)
                )
            )
            OR (lane NOT IN ('author','media_slots') AND content_public_ref IS NOT NULL AND author_external_id IS NULL)
        );

COMMENT ON COLUMN linggan_material_media_origin.content_public_ref IS
    'Containing work when media was observed in a content context; NULL only for an independently observed author avatar.';
