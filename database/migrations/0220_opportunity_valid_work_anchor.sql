-- D-041/D-051: a malformed detail cannot supply the work publication anchor.
-- Select the latest qualifying detail even when its publication time is NULL.
CREATE OR REPLACE FUNCTION linggan_opportunity_comment_eligibility(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text,
 p_period_basis text
)
RETURNS TABLE(
 content_ref uuid,
 comment_external_id text,
 commenter_external_id text,
 work_author_external_id text,
 period_anchor_at timestamptz,
 period_start date,
 first_observed_at timestamptz,
 is_reply boolean,
 exclusion_reason text
)
LANGUAGE plpgsql STABLE AS $$
BEGIN
 IF p_period_basis IS DISTINCT FROM 'work_published_cohort'
   AND p_period_basis IS DISTINCT FROM 'comment_published' THEN
  RAISE EXCEPTION 'unknown_period_basis: %', p_period_basis USING ERRCODE = '22023';
 END IF;
 IF p_tz IS NULL OR btrim(p_tz) = '' THEN
  RAISE EXCEPTION 'period_tz_required' USING ERRCODE = '22023';
 END IF;
 RETURN QUERY
 WITH domain_work AS (
  SELECT DISTINCT usage.content_public_ref
  FROM linggan_material_domain_usage usage
  WHERE usage.domain_ref = p_domain_ref
    AND (usage.basis_kind = 'legacy_domain_migration' OR usage.created_at <= p_as_of)
 ), primary_work AS (
  SELECT DISTINCT usage.content_public_ref
  FROM linggan_material_domain_usage usage
  WHERE usage.domain_ref = p_domain_ref
    AND usage.use_kind = 'domain_research'
    AND usage.role = 'primary'
    AND (usage.basis_kind = 'legacy_domain_migration' OR usage.created_at <= p_as_of)
 ), anchor AS (
  SELECT DISTINCT ON (detail.content_public_ref)
         detail.content_public_ref,
         detail.published_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= p_as_of
    AND detail.created_at <= p_as_of
    AND linggan_opportunity_observed_at(detail.observed_at) IS NOT NULL
  ORDER BY detail.content_public_ref,
           linggan_opportunity_observed_at(detail.observed_at) DESC NULLS LAST,
           detail.created_at DESC,
           detail.package_ref DESC,
           detail.material_ref DESC
 ), candidate AS (
  SELECT fact.content_ref,
         fact.comment_external_id,
         fact.commenter_external_id,
         fact.first_observed_at,
         fact.is_reply,
         author.author_external_id AS work_author_external_id,
         CASE WHEN p_period_basis = 'comment_published'
              THEN platform.published_at
              ELSE anchor.published_at END AS period_anchor_at,
         (p_period_basis = 'comment_published'
           AND platform.published_at IS NULL) AS comment_time_unknown,
         primary_work.content_public_ref IS NOT NULL AS in_primary
  FROM linggan_opportunity_comment_fact_at(p_as_of) fact
  JOIN domain_work ON domain_work.content_public_ref = fact.content_ref
  LEFT JOIN primary_work ON primary_work.content_public_ref = fact.content_ref
  LEFT JOIN LATERAL (
   SELECT attribution.author_external_id
   FROM linggan_material_content_author_at(fact.content_ref, p_as_of) attribution
   LIMIT 1
  ) author ON TRUE
  LEFT JOIN anchor ON anchor.content_public_ref = fact.content_ref
  LEFT JOIN linggan_opportunity_platform_time_at(p_as_of) platform
    ON platform.content_public_ref = fact.content_ref
   AND platform.comment_external_id = fact.comment_external_id
  WHERE fact.first_observed_at IS NULL OR fact.first_observed_at <= p_as_of
 )
 SELECT candidate.content_ref,
        candidate.comment_external_id,
        candidate.commenter_external_id,
        candidate.work_author_external_id,
        candidate.period_anchor_at,
        linggan_opportunity_period_start(candidate.period_anchor_at, p_tz),
        candidate.first_observed_at,
        candidate.is_reply,
        CASE
         WHEN NOT candidate.in_primary THEN 'not_in_domain_primary'
         WHEN candidate.work_author_external_id IS NULL THEN 'work_author_unknown'
         WHEN candidate.comment_time_unknown THEN 'comment_time_unknown'
         WHEN candidate.period_anchor_at IS NULL THEN 'period_anchor_missing'
         WHEN candidate.first_observed_at IS NULL THEN 'observed_at_unparseable'
         WHEN EXISTS (
          SELECT 1 FROM linggan_material_comment_restriction restriction
          WHERE restriction.content_public_ref = candidate.content_ref
            AND restriction.comment_external_id = candidate.comment_external_id
            AND restriction.restricted_at <= p_as_of
         ) THEN 'restricted'
         WHEN candidate.commenter_external_id IS NOT NULL
           AND candidate.commenter_external_id = candidate.work_author_external_id THEN 'creator_self'
         ELSE NULL
        END
 FROM candidate;
END $$;
