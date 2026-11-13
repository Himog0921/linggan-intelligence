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
   AND compared.display_text !~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T'
$$;
