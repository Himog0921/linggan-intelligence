-- MATERIAL-DEEPENING-001 · field-state detail observations and component-addressed media

ALTER TABLE linggan_material_content_detail
    ADD COLUMN like_count bigint,
    ADD COLUMN like_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (like_count_state IN ('KNOWN','UNKNOWN','NOT_VISIBLE','NOT_OBSERVED','PARSE_FAILED')),
    ADD COLUMN comment_count bigint,
    ADD COLUMN comment_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (comment_count_state IN ('KNOWN','UNKNOWN','NOT_VISIBLE','NOT_OBSERVED','PARSE_FAILED')),
    ADD COLUMN collect_count bigint,
    ADD COLUMN collect_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (collect_count_state IN ('KNOWN','UNKNOWN','NOT_VISIBLE','NOT_OBSERVED','PARSE_FAILED')),
    ADD COLUMN share_count bigint,
    ADD COLUMN share_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (share_count_state IN ('KNOWN','UNKNOWN','NOT_VISIBLE','NOT_OBSERVED','PARSE_FAILED')),
    ADD CONSTRAINT linggan_material_detail_like_state_check
        CHECK ((like_count_state='KNOWN')=(like_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_detail_comment_state_check
        CHECK ((comment_count_state='KNOWN')=(comment_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_detail_collect_state_check
        CHECK ((collect_count_state='KNOWN')=(collect_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_detail_share_state_check
        CHECK ((share_count_state='KNOWN')=(share_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_detail_nonnegative_engagement_check
        CHECK (like_count >= 0 AND comment_count >= 0 AND collect_count >= 0 AND share_count >= 0);

-- The detail and discovery lanes are both append-only observations.  This view exposes a single
-- time line without copying either lane into a mutable current-fact table.
CREATE VIEW linggan_material_engagement_observation AS
SELECT detail.content_public_ref,
       detail.package_ref,
       'detail'::text AS source_lane,
       detail.observed_at,
       detail.created_at,
       detail.like_count,detail.like_count_state,
       detail.comment_count,detail.comment_count_state,
       detail.collect_count,detail.collect_count_state,
       detail.share_count,detail.share_count_state
FROM linggan_material_content_detail detail
UNION ALL
SELECT finding.content_public_ref,
       finding.package_ref,
       'discovery'::text AS source_lane,
       finding.observed_at,
       finding.created_at,
       finding.like_count,finding.like_count_state,
       finding.comment_count,finding.comment_count_state,
       finding.collect_count,finding.collect_count_state,
       finding.share_count,finding.share_count_state
FROM linggan_material_discovery_finding finding;

ALTER TABLE linggan_material_media_candidate
    ADD COLUMN component_kind text NOT NULL DEFAULT 'single'
        CHECK (component_kind IN ('single','still','motion'));

DROP INDEX linggan_material_media_candidate_primary_idx;
CREATE UNIQUE INDEX linggan_material_media_candidate_primary_component_idx
    ON linggan_material_media_candidate(observation_ref, component_kind)
    WHERE producer_primary;

ALTER TABLE linggan_media_download_attempt
    ADD COLUMN candidate_ref uuid REFERENCES linggan_material_media_candidate(candidate_ref);

ALTER TABLE linggan_media_acquisition_work
    ADD COLUMN component_kind text NOT NULL DEFAULT 'single'
        CHECK (component_kind IN ('single','still','motion'));

ALTER TABLE linggan_media_acquisition_work
    DROP CONSTRAINT linggan_media_acquisition_work_observation_ref_key,
    ADD CONSTRAINT linggan_media_acquisition_work_observation_component_key
        UNIQUE (observation_ref, component_kind);

COMMENT ON COLUMN linggan_material_media_candidate.component_kind IS
    'single for ordinary media, still/motion for the two components of one logical Live Photo slot.';
COMMENT ON COLUMN linggan_media_download_attempt.candidate_ref IS
    'The exact candidate assertion attempted; null only for facts recorded before component-addressed acquisition.';
