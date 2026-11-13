-- C3b: day-precision comment platform times, decided by D-034.
-- The platform's display date (YYYY-MM-DD or MM-DD) and relative labels (N days/hours/minutes
-- ago, yesterday, just now) are accepted as a lower-fidelity time source. Relative labels are
-- resolved against the record's own collectedAt, i.e. the moment that note page was collected;
-- package.accepted_at is only a fallback. Day values are stored at 00:00 Asia/Shanghai.
-- Millisecond rows stay authoritative: the current view prefers them for the same comment.

ALTER TABLE linggan_material_comment_platform_time
 ADD COLUMN source_kind text NOT NULL DEFAULT 'epoch'
 CHECK (source_kind IN ('epoch', 'display_full_date', 'display_month_day', 'display_relative'));

ALTER TABLE linggan_material_comment_platform_time
 DROP CONSTRAINT linggan_material_comment_platform_time_precision_check;
ALTER TABLE linggan_material_comment_platform_time
 ADD CONSTRAINT linggan_material_comment_platform_time_precision_check
 CHECK (precision IN ('millisecond', 'day'));

ALTER TABLE linggan_material_comment_platform_time
 DROP CONSTRAINT linggan_material_comment_platform_time_protocol_version_check;
ALTER TABLE linggan_material_comment_platform_time
 ADD CONSTRAINT linggan_material_comment_platform_time_protocol_version_check
 CHECK (protocol_version >= 0);

-- One row per comment and precision. A later epoch can therefore be added even after the
-- historical day backfill has stored a lower-fidelity value; readers use the current view.
ALTER TABLE linggan_material_comment_platform_time
 DROP CONSTRAINT linggan_material_comment_platform_time_pkey;
ALTER TABLE linggan_material_comment_platform_time
 ADD PRIMARY KEY (content_public_ref, comment_external_id, precision);

CREATE VIEW linggan_material_comment_platform_time_current AS
SELECT DISTINCT ON (content_public_ref, comment_external_id)
       content_public_ref,
       comment_external_id,
       published_at,
       precision,
       source_package_ref,
       protocol_version,
       source_kind,
       created_at
FROM linggan_material_comment_platform_time
ORDER BY content_public_ref,
         comment_external_id,
         CASE precision WHEN 'millisecond' THEN 0 ELSE 1 END,
         created_at,
         source_package_ref;

-- Resolve a display label to a calendar date. The collected-at value is the page collection
-- moment for that record; Asia/Shanghai is the platform's display timezone, not the metric tz.
CREATE FUNCTION linggan_opportunity_parse_comment_display_date(
 p_text text,
 p_collected_at timestamptz
)
RETURNS date LANGUAGE plpgsql STABLE AS $$
DECLARE
 v_text text := btrim(coalesce(p_text, ''));
 v_local date;
 v_candidate date;
 v_year integer;
 v_month integer;
 v_day integer;
 v_count integer;
BEGIN
 IF v_text = '' OR p_collected_at IS NULL THEN
  RETURN NULL;
 END IF;
 v_local := (p_collected_at AT TIME ZONE 'Asia/Shanghai')::date;

 IF v_text ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' THEN
  BEGIN
   v_candidate := to_date(v_text, 'YYYY-MM-DD');
  EXCEPTION WHEN others THEN
   RETURN NULL;
  END;
  IF to_char(v_candidate, 'YYYY-MM-DD') <> v_text THEN
   RETURN NULL;
  END IF;
  RETURN v_candidate;
 END IF;

 IF v_text ~ '^[0-9]{2}-[0-9]{2}$' THEN
  v_month := substring(v_text, 1, 2)::integer;
  v_day := substring(v_text, 4, 2)::integer;
  IF v_month < 1 OR v_month > 12 OR v_day < 1 OR v_day > 31 THEN
   RETURN NULL;
  END IF;
  v_year := extract(year FROM v_local)::integer;
  BEGIN
   v_candidate := make_date(v_year, v_month, v_day);
  EXCEPTION WHEN others THEN
   RETURN NULL;
  END;
  IF to_char(v_candidate, 'MM-DD') <> v_text THEN
   RETURN NULL;
  END IF;
  -- XHS prints MM-DD for the current year only. A label later than the collection date must
  -- belong to the previous year (for example, 12-31 collected on 2026-01-05).
  IF v_candidate > v_local THEN
   BEGIN
    v_candidate := make_date(v_year - 1, v_month, v_day);
   EXCEPTION WHEN others THEN
    RETURN NULL;
   END;
   IF to_char(v_candidate, 'MM-DD') <> v_text THEN
    RETURN NULL;
   END IF;
  END IF;
  RETURN v_candidate;
 END IF;

 IF v_text = '昨天' THEN
  RETURN v_local - 1;
 END IF;
 IF v_text = '刚刚' THEN
  RETURN v_local;
 END IF;

 IF v_text ~ '^[0-9]+天前$' THEN
  v_count := substring(v_text from '^([0-9]+)天前$')::integer;
  IF v_count > 3650 THEN
   RETURN NULL;
  END IF;
  RETURN v_local - v_count;
 END IF;

 IF v_text ~ '^[0-9]+小时前$' THEN
  v_count := substring(v_text from '^([0-9]+)小时前$')::integer;
  IF v_count > 87600 THEN
   RETURN NULL;
  END IF;
  RETURN ((p_collected_at - make_interval(hours => v_count)) AT TIME ZONE 'Asia/Shanghai')::date;
 END IF;

 IF v_text ~ '^[0-9]+分钟前$' THEN
  v_count := substring(v_text from '^([0-9]+)分钟前$')::integer;
  IF v_count > 5256000 THEN
   RETURN NULL;
  END IF;
  RETURN ((p_collected_at - make_interval(mins => v_count)) AT TIME ZONE 'Asia/Shanghai')::date;
 END IF;

 RETURN NULL;
END $$;

CREATE FUNCTION linggan_opportunity_comment_display_source_kind(p_text text)
RETURNS text LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE
  WHEN btrim(coalesce(p_text, '')) ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}$' THEN 'display_full_date'
  WHEN btrim(coalesce(p_text, '')) ~ '^[0-9]{2}-[0-9]{2}$' THEN 'display_month_day'
  WHEN btrim(coalesce(p_text, '')) ~ '(天前|小时前|分钟前|昨天|刚刚)$' THEN 'display_relative'
  ELSE NULL
 END
$$;

CREATE FUNCTION linggan_opportunity_comment_collected_at(
 p_record_payload jsonb,
 p_package_accepted_at timestamptz
)
RETURNS timestamptz LANGUAGE sql STABLE AS $$
 SELECT CASE
  WHEN (p_record_payload->>'collectedAt') ~ '^[0-9]{13}$'
   THEN to_timestamp(((p_record_payload->>'collectedAt')::numeric) / 1000)
  ELSE p_package_accepted_at
 END
$$;

-- Epoch observations keep the C2 trust rules: protocol v1 packages, or legacy packages whose
-- record came from the page API (sourceTier = api). Legacy fallback createdAt values never
-- qualify because they are sourceTier = dom.
CREATE VIEW linggan_opportunity_comment_epoch_observation AS
SELECT package.package_ref,
       package.accepted_at,
       comment.content_public_ref,
       comment.comment_external_id,
       comment.record_ordinal,
       to_timestamp(
         ((record.value #>> '{payload,publishedAt}')::numeric / 1000)::double precision
       ) AS published_at,
       COALESCE(
         NULLIF(package.payload->'collectionProtocol'->>'version', '')::integer,
         0
       ) AS protocol_version
FROM linggan_runtime_capture_package package
CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records')
  WITH ORDINALITY record(value, ordinality)
JOIN linggan_material_comment comment
  ON comment.package_ref = package.package_ref
 AND comment.record_ordinal = (record.ordinality - 1)::integer
WHERE package.package_kind IN ('comments', 'replies')
  AND jsonb_typeof(record.value #> '{payload,publishedAt}') = 'number'
  AND (record.value #>> '{payload,publishedAt}') ~ '^[0-9]+$'
  AND (record.value #>> '{payload,publishedAt}')::numeric >= 1000000000000
  AND (record.value #>> '{payload,publishedAt}')::numeric < 253402300800000
  AND (
   (
    package.payload->'collectionProtocol'->>'version' = '1'
    AND package.payload->'collectionProtocol'->>'sortMode' = 'platform_default'
   )
   OR (
    package.payload->'collectionProtocol' IS NULL
    AND record.value #>> '{payload,sourceTier}' = 'api'
   )
  );

CREATE VIEW linggan_opportunity_comment_day_observation AS
SELECT package.package_ref,
       package.accepted_at,
       comment.content_public_ref,
       comment.comment_external_id,
       comment.record_ordinal,
       collected.collected_at,
       parsed.published_date,
       linggan_opportunity_comment_display_source_kind(
         record.value #>> '{payload,publishedAtText}'
       ) AS source_kind,
       COALESCE(
         NULLIF(package.payload->'collectionProtocol'->>'version', '')::integer,
         0
       ) AS protocol_version
FROM linggan_runtime_capture_package package
CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records')
  WITH ORDINALITY record(value, ordinality)
JOIN linggan_material_comment comment
  ON comment.package_ref = package.package_ref
 AND comment.record_ordinal = (record.ordinality - 1)::integer
CROSS JOIN LATERAL (
 SELECT linggan_opportunity_comment_collected_at(
   record.value->'payload',
   package.accepted_at
 ) AS collected_at
) collected
CROSS JOIN LATERAL (
 SELECT linggan_opportunity_parse_comment_display_date(
   record.value #>> '{payload,publishedAtText}',
   collected.collected_at
 ) AS published_date
) parsed
WHERE package.package_kind IN ('comments', 'replies')
  AND parsed.published_date IS NOT NULL
  AND linggan_opportunity_comment_display_source_kind(
        record.value #>> '{payload,publishedAtText}'
      ) IS NOT NULL;

DROP FUNCTION linggan_opportunity_project_comment_platform_times_for_package(uuid);

CREATE OR REPLACE FUNCTION linggan_opportunity_project_comment_platform_times_for_package(
 p_package_ref uuid,
 p_precision text DEFAULT 'millisecond'
)
RETURNS integer LANGUAGE plpgsql AS $$
DECLARE inserted integer := 0;
BEGIN
 IF p_precision = 'millisecond' THEN
  INSERT INTO linggan_material_comment_platform_time(
   content_public_ref, comment_external_id, published_at, precision,
   source_package_ref, protocol_version, source_kind
  )
  SELECT observation.content_public_ref,
         observation.comment_external_id,
         observation.published_at,
         'millisecond',
         observation.package_ref,
         observation.protocol_version,
         'epoch'
  FROM linggan_opportunity_comment_epoch_observation observation
  WHERE observation.package_ref = p_package_ref
  ON CONFLICT (content_public_ref, comment_external_id, precision) DO NOTHING;
 ELSIF p_precision = 'day' THEN
  INSERT INTO linggan_material_comment_platform_time(
   content_public_ref, comment_external_id, published_at, precision,
   source_package_ref, protocol_version, source_kind
  )
  SELECT DISTINCT ON (observation.content_public_ref, observation.comment_external_id)
         observation.content_public_ref,
         observation.comment_external_id,
         (observation.published_date::timestamp AT TIME ZONE 'Asia/Shanghai'),
         'day',
         observation.package_ref,
         observation.protocol_version,
         observation.source_kind
  FROM linggan_opportunity_comment_day_observation observation
  WHERE observation.package_ref = p_package_ref
  ORDER BY observation.content_public_ref,
           observation.comment_external_id,
           observation.record_ordinal
  ON CONFLICT (content_public_ref, comment_external_id, precision) DO NOTHING;
 ELSE
  RAISE EXCEPTION 'unknown_platform_time_precision: %', p_precision USING ERRCODE = '22023';
 END IF;
 GET DIAGNOSTICS inserted = ROW_COUNT;
 RETURN inserted;
END $$;

CREATE OR REPLACE FUNCTION linggan_opportunity_project_comment_platform_times()
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
  ORDER BY package.accepted_at, package.package_ref
 LOOP
  added := linggan_opportunity_project_comment_platform_times_for_package(
    package_ref, 'millisecond'
  );
  total := total + added;
 END LOOP;

 FOR package_ref IN
  SELECT package.package_ref
  FROM linggan_runtime_capture_package package
  WHERE package.package_kind IN ('comments', 'replies')
  ORDER BY package.accepted_at, package.package_ref
 LOOP
  added := linggan_opportunity_project_comment_platform_times_for_package(
    package_ref, 'day'
  );
  total := total + added;
 END LOOP;

 RETURN QUERY
 WITH conflicting AS (
  SELECT observation.content_public_ref,
         observation.comment_external_id
  FROM linggan_opportunity_comment_epoch_observation observation
  GROUP BY observation.content_public_ref, observation.comment_external_id
  HAVING count(DISTINCT observation.published_at) > 1
 )
 SELECT total, count(*) FROM conflicting;
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
  LEFT JOIN linggan_material_comment_platform_time_current platform
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
 LEFT JOIN linggan_material_comment_platform_time_current platform
   ON platform.content_public_ref = eligible.content_ref
  AND platform.comment_external_id = eligible.comment_external_id
 GROUP BY periods.period_start
 ORDER BY periods.period_start DESC
$$;

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
  JOIN linggan_material_comment_platform_time_current platform
    ON platform.content_public_ref = eligible.content_ref
   AND platform.comment_external_id = eligible.comment_external_id
   AND platform.precision = 'millisecond'
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
