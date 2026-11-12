-- T0.2: one stable-comment fact view and one shared eligibility implementation. C3 must replace
-- the comment_published branch here; no other module may parse observed_at or decide eligibility.

CREATE FUNCTION linggan_opportunity_observed_at(value text)
RETURNS timestamptz LANGUAGE plpgsql STABLE STRICT AS $$
BEGIN
 RETURN value::timestamptz;
EXCEPTION WHEN others THEN
 RETURN NULL;
END $$;

CREATE VIEW linggan_opportunity_comment_fact AS
WITH observation AS (
 SELECT comment.content_public_ref AS content_ref,
        comment.comment_external_id,
        comment.author_external_id,
        comment.is_reply,
        comment.package_ref,
        comment.material_ref,
        comment.created_at,
        linggan_opportunity_observed_at(comment.observed_at) AS observed_at
 FROM linggan_material_comment comment
), identity AS (
 SELECT content_ref,
        comment_external_id,
        min(observed_at) AS earliest_observed_at,
        count(DISTINCT author_external_id) FILTER (WHERE author_external_id IS NOT NULL) AS distinct_commenters,
        count(*) FILTER (WHERE author_external_id IS NULL) AS null_commenters,
        bool_or(is_reply) AS is_reply,
        count(*) AS observations,
        count(*) FILTER (WHERE observed_at IS NULL) AS unparseable_observations
 FROM observation
 GROUP BY content_ref, comment_external_id
), admission AS (
 SELECT first_admission.content_public_ref AS content_ref,
        first_admission.comment_external_id,
        first_admission.first_package_ref,
        package.accepted_at
 FROM linggan_material_comment_first_admission first_admission
 JOIN linggan_runtime_capture_package package
   ON package.package_ref = first_admission.first_package_ref
)
SELECT identity.content_ref,
       identity.comment_external_id,
       chosen.author_external_id AS commenter_external_id,
       COALESCE(admission.accepted_at, identity.earliest_observed_at) AS first_observed_at,
       admission.first_package_ref IS NULL AS first_observed_backfilled,
       COALESCE(admission.accepted_at, identity.earliest_observed_at) IS NOT NULL AS first_observed_parseable,
       (identity.distinct_commenters > 1
         OR (identity.distinct_commenters > 0 AND identity.null_commenters > 0)) AS commenter_external_id_conflict,
       identity.is_reply,
       identity.observations,
       identity.unparseable_observations
FROM identity
LEFT JOIN admission USING (content_ref, comment_external_id)
LEFT JOIN LATERAL (
 SELECT observation.author_external_id
 FROM observation
 WHERE observation.content_ref = identity.content_ref
   AND observation.comment_external_id = identity.comment_external_id
 ORDER BY CASE WHEN admission.first_package_ref IS NOT NULL
                AND observation.package_ref = admission.first_package_ref THEN 0 ELSE 1 END,
          observation.observed_at ASC NULLS LAST,
          observation.created_at ASC,
          observation.package_ref,
          observation.material_ref
 LIMIT 1
) chosen ON TRUE;

CREATE FUNCTION linggan_opportunity_comment_eligibility(
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
 IF p_period_basis = 'comment_published' THEN
  RAISE EXCEPTION 'comment_published_basis_not_ready' USING ERRCODE = '0A000';
 END IF;
 IF p_period_basis IS DISTINCT FROM 'work_published_cohort' THEN
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
  WHERE detail.published_at IS NOT NULL
    AND package.accepted_at <= p_as_of
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
         anchor.published_at AS period_anchor_at,
         primary_work.content_public_ref IS NOT NULL AS in_primary
  FROM linggan_opportunity_comment_fact fact
  JOIN domain_work ON domain_work.content_public_ref = fact.content_ref
  LEFT JOIN primary_work ON primary_work.content_public_ref = fact.content_ref
  LEFT JOIN LATERAL (
   SELECT attribution.author_external_id
   FROM linggan_material_content_author_at(fact.content_ref, p_as_of) attribution
   LIMIT 1
  ) author ON TRUE
  LEFT JOIN anchor ON anchor.content_public_ref = fact.content_ref
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

CREATE FUNCTION linggan_opportunity_eligible_comments(
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
 first_observed_at timestamptz
)
LANGUAGE sql STABLE AS $$
 SELECT eligible.content_ref,
        eligible.comment_external_id,
        eligible.commenter_external_id,
        eligible.work_author_external_id,
        eligible.period_anchor_at,
        eligible.period_start,
        eligible.first_observed_at
 FROM linggan_opportunity_comment_eligibility($1, $2, $3, $4) eligible
 WHERE eligible.exclusion_reason IS NULL
$$;

CREATE FUNCTION linggan_opportunity_eligibility_exclusions(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text,
 p_period_basis text
)
RETURNS TABLE(reason text, comments bigint)
LANGUAGE sql STABLE AS $$
 SELECT excluded.exclusion_reason, count(*)::bigint
 FROM linggan_opportunity_comment_eligibility($1, $2, $3, $4) excluded
 WHERE excluded.exclusion_reason IS NOT NULL
 GROUP BY excluded.exclusion_reason
 ORDER BY excluded.exclusion_reason
$$;
