-- DOMAIN-UNIFICATION-001 · Preserve projectable scopes, then remove retired Domain projections.
--
-- Reprojection of accepted Packages is optional. Existing canonical identities keep their Domain
-- usage and executable scope; unprojected cross rows are renewable projections and are dropped.
-- This migration never invents Content, accepted details, comments, or discovery facts.

DO $domain_unification_cleanup_preflight$
BEGIN
    IF EXISTS (SELECT 1 FROM cross_industry_note) THEN
        RAISE EXCEPTION 'domain cleanup blocked: cross_industry_note contains user-authored notes with no replacement';
    END IF;

    IF EXISTS (SELECT 1 FROM cross_industry_keyword)
       OR EXISTS (SELECT 1 FROM cross_industry_keyword_round) THEN
        RAISE EXCEPTION 'domain cleanup blocked: cross-industry keyword configuration has no replacement';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM collection_detail_page_session legacy_session
          JOIN cross_industry_sample sample
            ON sample.sample_ref=legacy_session.cross_industry_sample_ref
          JOIN linggan_material_content content
            ON content.platform=sample.platform
           AND content.content_external_id=sample.content_external_id
          JOIN collection_detail_page_session existing_session
            ON existing_session.work_order_ref=legacy_session.work_order_ref
           AND existing_session.content_public_ref=content.public_ref
           AND existing_session.session_ref<>legacy_session.session_ref
    ) THEN
        RAISE EXCEPTION 'domain cleanup blocked: a WorkOrder has duplicate detail sessions for one canonical Content';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM collection_detail_page_session legacy_session
          JOIN cross_industry_sample sample
            ON sample.sample_ref=legacy_session.cross_industry_sample_ref
          JOIN linggan_material_content content
            ON content.platform=sample.platform
           AND content.content_external_id=sample.content_external_id
          JOIN collection_detail_page_session other_legacy_session
            ON other_legacy_session.work_order_ref=legacy_session.work_order_ref
           AND other_legacy_session.session_ref>legacy_session.session_ref
          JOIN cross_industry_sample other_sample
            ON other_sample.sample_ref=other_legacy_session.cross_industry_sample_ref
           AND other_sample.platform=content.platform
           AND other_sample.content_external_id=content.content_external_id
    ) THEN
        RAISE EXCEPTION 'domain cleanup blocked: legacy detail sessions converge on one canonical Content';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM collection_work_order_cross_industry_target legacy_scope
          JOIN cross_industry_sample sample USING(sample_ref)
          JOIN linggan_material_content content
            ON content.platform=sample.platform
           AND content.content_external_id=sample.content_external_id
          JOIN collection_work_order_material_target existing_scope
            ON existing_scope.work_order_ref=legacy_scope.work_order_ref
           AND existing_scope.content_public_ref=content.public_ref
         WHERE existing_scope.comment_limit<>legacy_scope.comment_limit
            OR existing_scope.reply_expand_limit<>legacy_scope.reply_expand_limit
    ) THEN
        RAISE EXCEPTION 'domain cleanup blocked: duplicate material scopes have different comment or reply grants';
    END IF;

    IF EXISTS (
        SELECT 1
          FROM collection_work_order_cross_industry_target legacy_scope
          JOIN cross_industry_sample sample USING(sample_ref)
          JOIN linggan_material_content content
            ON content.platform=sample.platform
           AND content.content_external_id=sample.content_external_id
          JOIN collection_work_order_cross_industry_target other_legacy_scope
            ON other_legacy_scope.work_order_ref=legacy_scope.work_order_ref
           AND other_legacy_scope.sample_ref>legacy_scope.sample_ref
          JOIN cross_industry_sample other_sample
            ON other_sample.sample_ref=other_legacy_scope.sample_ref
           AND other_sample.platform=content.platform
           AND other_sample.content_external_id=content.content_external_id
         WHERE other_legacy_scope.comment_limit<>legacy_scope.comment_limit
            OR other_legacy_scope.reply_expand_limit<>legacy_scope.reply_expand_limit
    ) THEN
        RAISE EXCEPTION 'domain cleanup blocked: legacy scopes for one Content have different comment or reply grants';
    END IF;
END
$domain_unification_cleanup_preflight$;

-- Translate sessions whose sample already has a canonical Content identity. Sessions without a
-- canonical identity belong only to the retired projection and carry no accepted Package fact.
UPDATE collection_detail_page_session session
   SET content_public_ref=content.public_ref,
       cross_industry_sample_ref=NULL
  FROM cross_industry_sample sample
  JOIN linggan_material_content content
    ON content.platform=sample.platform
   AND content.content_external_id=sample.content_external_id
 WHERE session.cross_industry_sample_ref=sample.sample_ref;

DELETE FROM collection_detail_page_session
 WHERE cross_industry_sample_ref IS NOT NULL
   AND content_public_ref IS NULL;

-- Preserve each frozen comment/reply grant as a canonical material scope. Old cross scopes never
-- authorized media, so the translated rows keep all media capabilities false. Appended ordinals
-- avoid collisions with pre-existing material scopes while retaining the old per-lane order.
WITH numbered_legacy_scope AS (
    SELECT legacy_scope.work_order_ref,content.public_ref,legacy_scope.sample_ref,
           legacy_scope.comment_limit,legacy_scope.reply_expand_limit,
           COALESCE((
               SELECT max(existing_scope.ordinal)
                 FROM collection_work_order_material_target existing_scope
                WHERE existing_scope.work_order_ref=legacy_scope.work_order_ref
           ),0) + row_number() OVER (
               PARTITION BY legacy_scope.work_order_ref
               ORDER BY legacy_scope.ordinal,legacy_scope.sample_ref
           ) AS canonical_ordinal
      FROM collection_work_order_cross_industry_target legacy_scope
      JOIN cross_industry_sample sample USING(sample_ref)
      JOIN linggan_material_content content
        ON content.platform=sample.platform
       AND content.content_external_id=sample.content_external_id
)
INSERT INTO collection_work_order_material_target
    (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,
     acquire_media,allow_ocr,allow_asr)
SELECT work_order_ref,public_ref,canonical_ordinal,comment_limit,reply_expand_limit,
       false,false,false
  FROM numbered_legacy_scope
ON CONFLICT (work_order_ref,content_public_ref) DO NOTHING;

-- Eligibility is renewable scheduling state. Drop only rows whose identity or locator source
-- names the retired cross projection; canonical material eligibility and its retry budgets remain.
DELETE FROM collection_execution_input_eligibility
 WHERE domain_scope='cross_industry'
    OR object_kind='cross_industry_sample'
    OR input_source_kind='cross_industry_sample';

ALTER TABLE collection_execution_input_eligibility
    DROP CONSTRAINT IF EXISTS collection_execution_input_eligibility_domain_scope_check,
    DROP CONSTRAINT IF EXISTS collection_execution_input_eligibility_object_kind_check,
    DROP CONSTRAINT IF EXISTS collection_execution_input_eligibility_input_source_kind_check,
    ADD CONSTRAINT collection_execution_input_eligibility_domain_scope_check
        CHECK (domain_scope IN ('own_domain')),
    ADD CONSTRAINT collection_execution_input_eligibility_object_kind_check
        CHECK (object_kind IN ('material_content')),
    ADD CONSTRAINT collection_execution_input_eligibility_input_source_kind_check
        CHECK (input_source_kind IS NULL OR input_source_kind IN ('discovery_finding'));

DROP VIEW IF EXISTS cross_industry_sample_lane;
DROP TABLE IF EXISTS cross_industry_creator_sample_observation CASCADE;
DROP TABLE IF EXISTS cross_industry_comment_clean CASCADE;
DROP TABLE IF EXISTS cross_industry_sample_detail CASCADE;
DROP TABLE IF EXISTS cross_industry_sample_observation CASCADE;
DROP TABLE IF EXISTS collection_work_order_cross_industry_target CASCADE;
DROP TABLE IF EXISTS cross_industry_comment CASCADE;
DROP TABLE IF EXISTS cross_industry_note CASCADE;
DROP TABLE IF EXISTS cross_industry_keyword_round CASCADE;
DROP TABLE IF EXISTS cross_industry_keyword CASCADE;
DROP TABLE IF EXISTS cross_industry_sample CASCADE;

ALTER TABLE collection_detail_page_session
    DROP COLUMN cross_industry_sample_ref,
    ALTER COLUMN content_public_ref SET NOT NULL;

ALTER TABLE linggan_material_content
    DROP CONSTRAINT IF EXISTS linggan_material_content_home_domain_only,
    DROP CONSTRAINT IF EXISTS linggan_material_content_domain_fk,
    DROP COLUMN IF EXISTS is_own_domain,
    DROP COLUMN IF EXISTS domain_ref;

DROP INDEX IF EXISTS collection_observation_target_domain_idx;
ALTER TABLE collection_observation_target
    DROP CONSTRAINT IF EXISTS collection_observation_target_domain_ref_fkey,
    DROP COLUMN IF EXISTS domain_ref;

DROP INDEX IF EXISTS observation_domain_own_idx;
ALTER TABLE observation_domain
    DROP CONSTRAINT IF EXISTS observation_domain_domain_ref_is_own_domain_key,
    DROP COLUMN IF EXISTS is_own_domain;
