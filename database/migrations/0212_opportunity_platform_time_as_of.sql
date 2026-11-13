-- D-037: choose only platform-time rows and packages known by the requested time.
-- Current and historical consumers share this selector. Existing rows are not rewritten.
CREATE FUNCTION linggan_opportunity_platform_time_at(p_as_of timestamptz)
RETURNS TABLE(
 content_public_ref uuid, comment_external_id text, published_at timestamptz,
 "precision" text, source_package_ref uuid, protocol_version integer,
 source_kind text, created_at timestamptz
)
LANGUAGE sql STABLE STRICT AS $$
 SELECT DISTINCT ON (platform.content_public_ref, platform.comment_external_id)
        platform.content_public_ref, platform.comment_external_id, platform.published_at,
        platform.precision, platform.source_package_ref, platform.protocol_version,
        platform.source_kind, platform.created_at
 FROM linggan_material_comment_platform_time platform
 JOIN linggan_runtime_capture_package package ON package.package_ref = platform.source_package_ref
 WHERE platform.created_at <= p_as_of AND package.accepted_at <= p_as_of
 ORDER BY platform.content_public_ref, platform.comment_external_id,
          CASE platform.precision WHEN 'millisecond' THEN 0 ELSE 1 END,
          platform.created_at, platform.source_package_ref
$$;

CREATE OR REPLACE VIEW linggan_material_comment_platform_time_current AS
SELECT * FROM linggan_opportunity_platform_time_at(statement_timestamp());

-- D-039: select only observations already admitted and projected by the requested instant.
-- first_admission has no row-created timestamp; the selected package and observation establish
-- the evidence available at that instant, without inventing a timestamp for the admission row.
CREATE FUNCTION linggan_opportunity_comment_fact_at(p_as_of timestamptz)
RETURNS TABLE(
 content_ref uuid, comment_external_id text, commenter_external_id text,
 first_observed_at timestamptz, first_observed_backfilled boolean,
 first_observed_parseable boolean, commenter_external_id_conflict boolean,
 is_reply boolean, observations bigint, unparseable_observations bigint
)
LANGUAGE sql STABLE STRICT AS $$
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
 JOIN linggan_runtime_capture_package observation_package ON observation_package.package_ref = comment.package_ref
 WHERE observation_package.accepted_at <= p_as_of AND comment.created_at <= p_as_of
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
 WHERE package.accepted_at <= p_as_of
   AND EXISTS (SELECT 1 FROM observation
               WHERE observation.content_ref = first_admission.content_public_ref
                 AND observation.comment_external_id = first_admission.comment_external_id
                 AND observation.package_ref = first_admission.first_package_ref)
), chosen AS (
 SELECT DISTINCT ON (observation.content_ref, observation.comment_external_id)
        observation.content_ref, observation.comment_external_id, observation.author_external_id
 FROM observation
 LEFT JOIN admission USING (content_ref, comment_external_id)
 ORDER BY observation.content_ref, observation.comment_external_id,
          CASE WHEN admission.first_package_ref IS NOT NULL
                     AND observation.package_ref = admission.first_package_ref THEN 0 ELSE 1 END,
          observation.observed_at ASC NULLS LAST, observation.created_at ASC,
          observation.package_ref, observation.material_ref
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
LEFT JOIN chosen USING (content_ref, comment_external_id)
$$;

CREATE OR REPLACE VIEW linggan_opportunity_comment_fact AS
SELECT * FROM linggan_opportunity_comment_fact_at(statement_timestamp());

-- D-040: preserve the shared author selector and its source ordering, but exclude future observations.
CREATE OR REPLACE FUNCTION linggan_material_content_author_at(p_content_public_ref uuid, p_as_of timestamptz)
RETURNS TABLE(author_external_id text, attribution_source text, observed_at timestamptz)
LANGUAGE sql STABLE AS $$
 SELECT candidate.author_external_id,candidate.attribution_source,candidate.observed_at
 FROM (
   SELECT detail.author_external_id,'content_detail'::text AS attribution_source,
          detail.observed_at::timestamptz AS observed_at,detail.created_at AS recorded_at,
          detail.package_ref,detail.material_ref,2 AS source_priority
   FROM linggan_material_content_detail detail
   JOIN linggan_runtime_capture_package package USING(package_ref)
   WHERE detail.content_public_ref=p_content_public_ref
     AND package.accepted_at<=p_as_of
     AND detail.created_at<=p_as_of
     AND NULLIF(btrim(detail.author_external_id),'') IS NOT NULL
   UNION ALL
   SELECT task.task_spec #>> '{target,authorExternalId}','profile_discovery'::text,
          finding.observed_at::timestamptz,finding.created_at,
          finding.package_ref,finding.material_ref,1
   FROM linggan_material_discovery_finding finding
   JOIN linggan_runtime_capture_package package USING(package_ref)
   JOIN linggan_runtime_task task USING(task_id)
   WHERE finding.content_public_ref=p_content_public_ref
     AND package.package_kind='profile_discovery'
     AND package.accepted_at<=p_as_of
     AND finding.created_at<=p_as_of
     AND NULLIF(btrim(task.task_spec #>> '{target,authorExternalId}'),'') IS NOT NULL
 ) candidate
 ORDER BY candidate.source_priority DESC,candidate.observed_at DESC,
          candidate.recorded_at DESC,candidate.package_ref DESC,candidate.material_ref DESC
 LIMIT 1
$$;

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

CREATE OR REPLACE FUNCTION linggan_opportunity_platform_time_coverage(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text,
 p_periods integer
)
RETURNS TABLE(
 period_start date,
 eligible_comments bigint,
 with_platform_time bigint,
 ratio double precision
)
LANGUAGE sql STABLE AS $$
 WITH current_period AS (
  SELECT linggan_opportunity_period_start(p_as_of, p_tz) AS start_date
 ), periods AS (
  SELECT (current.start_date - (step.n * 7)) AS period_start
  FROM current_period current
  CROSS JOIN generate_series(0, p_periods - 1) step(n)
 ), eligible AS (
  SELECT filtered.period_start, filtered.content_ref, filtered.comment_external_id
  FROM linggan_opportunity_eligible_comments(
    p_domain_ref, p_as_of, p_tz, 'work_published_cohort'
  ) filtered
 )
 SELECT periods.period_start,
        count(eligible.content_ref)::bigint AS eligible_comments,
        count(platform.content_public_ref)::bigint AS with_platform_time,
        CASE WHEN count(eligible.content_ref) = 0 THEN NULL
             ELSE count(platform.content_public_ref)::double precision
                  / count(eligible.content_ref)::double precision END AS ratio
 FROM periods
 LEFT JOIN eligible ON eligible.period_start = periods.period_start
 LEFT JOIN linggan_opportunity_platform_time_at(p_as_of) platform
   ON platform.content_public_ref = eligible.content_ref
  AND platform.comment_external_id = eligible.comment_external_id
 GROUP BY periods.period_start
 ORDER BY periods.period_start DESC
$$;

-- C3b follow-up: API-sourced records carry an ISO serialization of their own
-- epoch in publishedAtText, not a platform display label. Comparing that text's
-- month-day against the Shanghai local date reported UTC-vs-local differences as
-- mismatches. Only real display labels are comparable, so ISO timestamps are skipped.

CREATE OR REPLACE FUNCTION linggan_opportunity_platform_time_display_comparison(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text
)
RETURNS TABLE(
 content_ref uuid,
 comment_external_id text,
 published_at timestamptz,
 local_date date,
 display_text text,
 same_day boolean
)
LANGUAGE sql STABLE AS $$
 WITH eligible AS (
  SELECT filtered.content_ref, filtered.comment_external_id
  FROM linggan_opportunity_eligible_comments(
    p_domain_ref, p_as_of, p_tz, 'work_published_cohort'
  ) filtered
 ), compared AS (
  SELECT eligible.content_ref,
         eligible.comment_external_id,
         platform.published_at,
         (platform.published_at AT TIME ZONE p_tz)::date AS local_date,
         (package.payload->'records'->comment.record_ordinal->'payload'->>'publishedAtText') AS display_text,
         regexp_match(
           (package.payload->'records'->comment.record_ordinal->'payload'->>'publishedAtText'),
           '(^|[^0-9])(((19|20)[0-9]{2})-)?(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])([^0-9]|$)'
         ) AS parts
  FROM eligible
  JOIN linggan_opportunity_platform_time_at(p_as_of) platform
    ON platform.content_public_ref = eligible.content_ref
   AND platform.comment_external_id = eligible.comment_external_id
   AND platform.precision = 'millisecond'
  JOIN linggan_material_comment comment
    ON comment.content_public_ref = platform.content_public_ref
   AND comment.comment_external_id = platform.comment_external_id
   AND comment.package_ref = platform.source_package_ref
   AND comment.created_at <= p_as_of
  JOIN linggan_runtime_capture_package package
    ON package.package_ref = platform.source_package_ref
 )
 SELECT compared.content_ref,
        compared.comment_external_id,
        compared.published_at,
        compared.local_date,
        compared.display_text,
        to_char(compared.local_date, 'MM-DD') = compared.parts[5] || '-' || compared.parts[6]
          AND (compared.parts[3] IS NULL
               OR to_char(compared.local_date, 'YYYY') = compared.parts[3])
 FROM compared
 WHERE compared.parts IS NOT NULL
   AND compared.display_text !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T'
$$;
