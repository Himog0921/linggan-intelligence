-- C3: append-only comment platform times projected from C2 capture packages.
-- The table name is fixed by D-028: this is the one authorized exception to the
-- linggan_opportunity_* object-name rule in D-005.

CREATE TABLE linggan_material_comment_platform_time (
 content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 comment_external_id text NOT NULL CHECK (length(comment_external_id) > 0),
 published_at timestamptz NOT NULL,
 precision text NOT NULL CHECK (precision IN ('millisecond')),
 source_package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
 protocol_version integer NOT NULL CHECK (protocol_version >= 1),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY (content_public_ref, comment_external_id)
);
CREATE TRIGGER linggan_opportunity_material_comment_platform_time_immutable
 BEFORE UPDATE OR DELETE ON linggan_material_comment_platform_time
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE FUNCTION linggan_opportunity_project_comment_platform_times_for_package(p_package_ref uuid)
RETURNS integer LANGUAGE plpgsql AS $$
DECLARE inserted integer := 0;
BEGIN
 INSERT INTO linggan_material_comment_platform_time(
  content_public_ref, comment_external_id, published_at, precision, source_package_ref, protocol_version
 )
 SELECT comment.content_public_ref,
        comment.comment_external_id,
        to_timestamp(
          ((record.value #>> '{payload,publishedAt}')::numeric / 1000)::double precision
        ),
        'millisecond',
        package.package_ref,
        (package.payload->'collectionProtocol'->>'version')::integer
 FROM linggan_runtime_capture_package package
 CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records')
   WITH ORDINALITY record(value, ordinality)
 JOIN linggan_material_comment comment
   ON comment.package_ref = package.package_ref
  AND comment.record_ordinal = (record.ordinality - 1)::integer
 WHERE package.package_ref = p_package_ref
   AND package.package_kind IN ('comments', 'replies')
   AND package.payload->'collectionProtocol'->>'version' = '1'
   AND package.payload->'collectionProtocol'->>'sortMode' = 'platform_default'
   AND jsonb_typeof(record.value #> '{payload,publishedAt}') = 'number'
   AND (record.value #>> '{payload,publishedAt}') ~ '^[0-9]+$'
   AND (record.value #>> '{payload,publishedAt}')::numeric >= 1000000000000
   AND (record.value #>> '{payload,publishedAt}')::numeric < 253402300800000
 ON CONFLICT (content_public_ref, comment_external_id) DO NOTHING;
 GET DIAGNOSTICS inserted = ROW_COUNT;
 RETURN inserted;
END $$;

CREATE FUNCTION linggan_opportunity_project_comment_platform_times()
RETURNS TABLE(inserted bigint, conflicts bigint)
LANGUAGE plpgsql AS $$
DECLARE
 total bigint := 0;
 package_ref uuid;
 added integer;
BEGIN
 FOR package_ref IN
  SELECT package.package_ref
  FROM linggan_runtime_capture_package package
  WHERE package.package_kind IN ('comments', 'replies')
    AND package.payload->'collectionProtocol'->>'version' = '1'
    AND package.payload->'collectionProtocol'->>'sortMode' = 'platform_default'
  ORDER BY package.accepted_at, package.package_ref
 LOOP
  added := linggan_opportunity_project_comment_platform_times_for_package(package_ref);
  total := total + added;
 END LOOP;

 RETURN QUERY
 WITH observations AS (
  SELECT comment.content_public_ref,
         comment.comment_external_id,
         to_timestamp(
           ((record.value #>> '{payload,publishedAt}')::numeric / 1000)::double precision
         ) AS published_at
  FROM linggan_runtime_capture_package package
  CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records')
    WITH ORDINALITY record(value, ordinality)
  JOIN linggan_material_comment comment
    ON comment.package_ref = package.package_ref
   AND comment.record_ordinal = (record.ordinality - 1)::integer
  WHERE package.package_kind IN ('comments', 'replies')
    AND package.payload->'collectionProtocol'->>'version' = '1'
    AND package.payload->'collectionProtocol'->>'sortMode' = 'platform_default'
    AND jsonb_typeof(record.value #> '{payload,publishedAt}') = 'number'
    AND (record.value #>> '{payload,publishedAt}') ~ '^[0-9]+$'
    AND (record.value #>> '{payload,publishedAt}')::numeric >= 1000000000000
    AND (record.value #>> '{payload,publishedAt}')::numeric < 253402300800000
 )
 SELECT total, count(*)
 FROM (
  SELECT content_public_ref, comment_external_id
  FROM observations
  GROUP BY content_public_ref, comment_external_id
  HAVING count(DISTINCT published_at) > 1
 ) conflicting;
END $$;

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
         CASE WHEN p_period_basis = 'comment_published'
              THEN platform.published_at
              ELSE anchor.published_at END AS period_anchor_at,
         (p_period_basis = 'comment_published'
           AND platform.published_at IS NULL) AS comment_time_unknown,
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
  LEFT JOIN linggan_material_comment_platform_time platform
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

CREATE FUNCTION linggan_opportunity_platform_time_coverage(
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
 LEFT JOIN linggan_material_comment_platform_time platform
   ON platform.content_public_ref = eligible.content_ref
  AND platform.comment_external_id = eligible.comment_external_id
 GROUP BY periods.period_start
 ORDER BY periods.period_start DESC
$$;

CREATE FUNCTION linggan_opportunity_platform_time_display_comparison(
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
  JOIN linggan_material_comment_platform_time platform
    ON platform.content_public_ref = eligible.content_ref
   AND platform.comment_external_id = eligible.comment_external_id
  JOIN linggan_material_comment comment
    ON comment.content_public_ref = platform.content_public_ref
   AND comment.comment_external_id = platform.comment_external_id
   AND comment.package_ref = platform.source_package_ref
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
$$;

CREATE FUNCTION linggan_opportunity_platform_time_display_agreement(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text
)
RETURNS TABLE(compared bigint, matching bigint, ratio double precision)
LANGUAGE sql STABLE AS $$
 SELECT count(*)::bigint AS compared,
        count(*) FILTER (WHERE same_day)::bigint AS matching,
        CASE WHEN count(*) = 0 THEN NULL
             ELSE count(*) FILTER (WHERE same_day)::double precision
                  / count(*)::double precision END AS ratio
 FROM linggan_opportunity_platform_time_display_comparison($1, $2, $3)
$$;

CREATE FUNCTION linggan_opportunity_platform_time_display_mismatches(
 p_domain_ref uuid,
 p_as_of timestamptz,
 p_tz text
)
RETURNS TABLE(
 content_ref uuid,
 comment_external_id text,
 published_at timestamptz,
 local_date date,
 display_text text
)
LANGUAGE sql STABLE AS $$
 SELECT comparison.content_ref,
        comparison.comment_external_id,
        comparison.published_at,
        comparison.local_date,
        comparison.display_text
 FROM linggan_opportunity_platform_time_display_comparison($1, $2, $3) comparison
 WHERE NOT comparison.same_day
 ORDER BY comparison.content_ref, comparison.comment_external_id
$$;
