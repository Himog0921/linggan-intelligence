//! SQL owned by the shared Work Resource Current projection.
//!
//! A material record is append-only. Every consumer therefore selects the newest qualified
//! value per field from this one CTE graph. Page filters and target-scoped candidate discovery
//! may choose *which* Works are read, but they must not copy title, author, publication-time or
//! engagement Current adjudication.

const WORK_RESOURCE_CURRENT_CTES: &str = r#"
current_detail AS MATERIALIZED (
  SELECT detail.* FROM linggan_material_content_detail detail
  JOIN requested_work scope ON scope.public_ref=detail.content_public_ref
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.created_at <= $2::timestamptz
), current_observation AS MATERIALIZED (
  SELECT observation.* FROM linggan_material_engagement_observation observation
  JOIN requested_work scope ON scope.public_ref=observation.content_public_ref
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND observation.created_at <= $2::timestamptz
), latest_detail AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,
    detail.observed_at,detail.created_at
  FROM current_detail detail
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_title AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.title,detail.title_state,detail.material_ref,
    detail.package_ref,detail.record_ordinal,detail.observed_at,detail.created_at
  FROM current_detail detail WHERE detail.title_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_body AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.body_text,detail.body_state,detail.material_ref,
    detail.package_ref,detail.record_ordinal,detail.observed_at,detail.created_at
  FROM current_detail detail WHERE detail.body_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_creator AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.creator_display_name,detail.creator_display_name_state,
    detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at,detail.created_at
  FROM current_detail detail WHERE detail.creator_display_name_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_published_at AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.published_at,detail.published_at_source_text,
    detail.published_at_source_text_state,detail.material_ref,detail.package_ref,
    detail.record_ordinal,detail.observed_at,detail.created_at,
    detail.published_at_source_field,detail.published_at_source_kind,
    detail.published_at_precision,detail.published_at_reference_observed_at,
    detail.published_at_parser_version
  FROM current_detail detail WHERE detail.published_at IS NOT NULL
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_published_text AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.published_at_source_text,detail.material_ref,
    detail.package_ref,detail.record_ordinal,detail.observed_at,detail.created_at,
    detail.published_at_source_text_state,detail.published_at_source_field,
    detail.published_at_source_kind,detail.published_at_precision,
    detail.published_at_reference_observed_at,detail.published_at_parser_version
  FROM current_detail detail WHERE detail.published_at_source_text_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_discovery AS (
  SELECT DISTINCT ON (finding.content_public_ref) finding.*
  FROM linggan_material_discovery_finding finding
  JOIN requested_work scope ON scope.public_ref=finding.content_public_ref
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND finding.created_at <= $2::timestamptz
  ORDER BY finding.content_public_ref,finding.observed_at::timestamptz DESC,
           finding.created_at DESC,finding.package_ref DESC,finding.material_ref DESC
), latest_like AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.like_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM current_observation observation WHERE observation.like_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_comment_count AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.comment_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM current_observation observation WHERE observation.comment_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_collect AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.collect_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM current_observation observation WHERE observation.collect_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_share AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.share_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM current_observation observation WHERE observation.share_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_lane AS (
  SELECT lane.content_public_ref,linggan_human_moment(max(lane.observed_at::timestamptz)) AS observed_at
  FROM linggan_material_lane_observation lane
  JOIN requested_work scope ON scope.public_ref=lane.content_public_ref
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE lane.content_public_ref IS NOT NULL AND package.accepted_at <= $2::timestamptz
    AND lane.created_at <= $2::timestamptz
  GROUP BY lane.content_public_ref
), work_resource_current AS (
  SELECT content.platform,content.content_external_id,content.public_ref,
    detail.material_ref AS detail_material_ref,
    COALESCE(detail.material_ref,discovery.material_ref) AS material_ref,
    COALESCE(detail.package_ref,discovery.package_ref) AS package_ref,
    COALESCE(detail.record_ordinal,discovery.record_ordinal) AS record_ordinal,
    linggan_human_moment(COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)) AS observed_at,
    CASE WHEN detail_title.title IS NULL AND discovery.title ~ '^[0-9]+$'
      AND discovery.title=discovery.like_count::text THEN NULL
      ELSE COALESCE(detail_title.title,discovery.title) END AS title,
    CASE WHEN detail_title.title IS NULL AND discovery.title ~ '^[0-9]+$'
      AND discovery.title=discovery.like_count::text THEN 'UNKNOWN'
      ELSE COALESCE(detail_title.title_state,discovery.title_state,'UNKNOWN') END AS title_state,
    CASE WHEN detail_title.title IS NULL AND discovery.title ~ '^[0-9]+$'
      AND discovery.title=discovery.like_count::text THEN 'suspected_count'
      ELSE 'source_value' END AS title_qualification,
    COALESCE(detail_title.material_ref,discovery.material_ref) AS title_source_material_ref,
    COALESCE(detail_title.package_ref,discovery.package_ref) AS title_source_package_ref,
    COALESCE(detail_title.record_ordinal,discovery.record_ordinal) AS title_source_record_ordinal,
    COALESCE(detail_title.observed_at,discovery.observed_at) AS title_source_observed_at,
    linggan_human_moment(COALESCE(detail_title.created_at,discovery.created_at)) AS title_source_recorded_at,
    detail_body.body_text,COALESCE(detail_body.body_state,'UNKNOWN') AS body_state,
    detail_body.material_ref AS body_source_material_ref,
    detail_body.package_ref AS body_source_package_ref,
    detail_body.record_ordinal AS body_source_record_ordinal,
    detail_body.observed_at AS body_source_observed_at,
    linggan_human_moment(detail_body.created_at) AS body_source_recorded_at,
    COALESCE(detail_creator.creator_display_name,discovery.creator_display_name) AS creator_display_name,
    COALESCE(detail_creator.creator_display_name_state,discovery.creator_state,'UNKNOWN') AS creator_display_name_state,
    COALESCE(detail_creator.material_ref,discovery.material_ref) AS creator_source_material_ref,
    COALESCE(detail_creator.package_ref,discovery.package_ref) AS creator_source_package_ref,
    COALESCE(detail_creator.record_ordinal,discovery.record_ordinal) AS creator_source_record_ordinal,
    COALESCE(detail_creator.observed_at,discovery.observed_at) AS creator_source_observed_at,
    linggan_human_moment(COALESCE(detail_creator.created_at,discovery.created_at)) AS creator_source_recorded_at,
    linggan_human_moment(detail_published_at.published_at) AS published_at,
    CASE WHEN detail_published_at.published_at IS NULL THEN NULL
         ELSE (detail_published_at.published_at AT TIME ZONE 'Asia/Shanghai')::date::text END AS published_local_date,
    CASE WHEN detail_published_at.published_at IS NULL THEN NULL
         ELSE (extract(epoch FROM detail_published_at.published_at)*1000)::bigint END AS published_at_epoch_ms,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_source_text
         ELSE COALESCE(detail_published_text.published_at_source_text,discovery.published_at_source_text) END AS published_at_source_text,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_source_text_state
         ELSE COALESCE(detail_published_text.published_at_source_text_state,discovery.published_at_source_text_state,'UNKNOWN') END AS published_at_source_text_state,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_source_field
         ELSE detail_published_text.published_at_source_field END AS published_at_source_field,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_source_kind
         ELSE COALESCE(detail_published_text.published_at_source_kind,'unknown') END AS published_at_source_kind,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_precision
         ELSE COALESCE(detail_published_text.published_at_precision,'unknown') END AS published_at_precision,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_reference_observed_at
         ELSE detail_published_text.published_at_reference_observed_at END::text AS published_at_reference_observed_at,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.published_at_parser_version
         ELSE detail_published_text.published_at_parser_version END AS published_at_parser_version,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.material_ref
         ELSE COALESCE(detail_published_text.material_ref,discovery.material_ref) END AS published_source_material_ref,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.package_ref
         ELSE COALESCE(detail_published_text.package_ref,discovery.package_ref) END AS published_source_package_ref,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.record_ordinal
         ELSE COALESCE(detail_published_text.record_ordinal,discovery.record_ordinal) END AS published_source_record_ordinal,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.observed_at
         ELSE COALESCE(detail_published_text.observed_at,discovery.observed_at) END AS published_source_observed_at,
    CASE WHEN detail_published_at.published_at IS NOT NULL
         THEN detail_published_at.created_at
         ELSE COALESCE(detail_published_text.created_at,discovery.created_at) END::text AS published_source_recorded_at,
    attribution.author_external_id,
    attribution.attribution_source AS author_attribution_source,
    discovery.cover_source_url,
    COALESCE(discovery.cover_source_state,'UNKNOWN') AS cover_source_state,
    latest_like.like_count,CASE WHEN latest_like.like_count IS NULL THEN 'UNKNOWN' ELSE 'KNOWN' END AS like_count_state,
    latest_like.package_ref AS like_source_package_ref,latest_like.source_lane AS like_source_lane,
    latest_like.observed_at AS like_source_observed_at,latest_like.created_at::text AS like_source_recorded_at,
    latest_comment_count.comment_count,CASE WHEN latest_comment_count.comment_count IS NULL THEN 'UNKNOWN' ELSE 'KNOWN' END AS comment_count_state,
    latest_comment_count.package_ref AS comment_source_package_ref,latest_comment_count.source_lane AS comment_source_lane,
    latest_comment_count.observed_at AS comment_source_observed_at,latest_comment_count.created_at::text AS comment_source_recorded_at,
    latest_collect.collect_count,CASE WHEN latest_collect.collect_count IS NULL THEN 'UNKNOWN' ELSE 'KNOWN' END AS collect_count_state,
    latest_collect.package_ref AS collect_source_package_ref,latest_collect.source_lane AS collect_source_lane,
    latest_collect.observed_at AS collect_source_observed_at,latest_collect.created_at::text AS collect_source_recorded_at,
    latest_share.share_count,CASE WHEN latest_share.share_count IS NULL THEN 'UNKNOWN' ELSE 'KNOWN' END AS share_count_state,
    latest_share.package_ref AS share_source_package_ref,latest_share.source_lane AS share_source_lane,
    latest_share.observed_at AS share_source_observed_at,latest_share.created_at::text AS share_source_recorded_at
  FROM requested_work content
  LEFT JOIN latest_detail detail ON detail.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_title detail_title ON detail_title.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_body detail_body ON detail_body.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_creator detail_creator ON detail_creator.content_public_ref=content.public_ref
  LEFT JOIN LATERAL linggan_material_content_author_at(content.public_ref,$2::timestamptz) attribution ON true
  LEFT JOIN latest_detail_published_at detail_published_at ON detail_published_at.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_published_text detail_published_text ON detail_published_text.content_public_ref=content.public_ref
  LEFT JOIN latest_discovery discovery ON discovery.content_public_ref=content.public_ref
  LEFT JOIN latest_like ON latest_like.content_public_ref=content.public_ref
  LEFT JOIN latest_comment_count ON latest_comment_count.content_public_ref=content.public_ref
  LEFT JOIN latest_collect ON latest_collect.content_public_ref=content.public_ref
  LEFT JOIN latest_share ON latest_share.content_public_ref=content.public_ref
  LEFT JOIN latest_lane lane_latest ON lane_latest.content_public_ref=content.public_ref
)
"#;

const FILTERED_PAGE_SCOPE: &str = r#"
requested_work AS MATERIALIZED (
 SELECT content.* FROM linggan_material_content content
 WHERE ($8::uuid IS NULL OR content.public_ref=$8)
 AND ($9::uuid IS NULL OR EXISTS (SELECT 1 FROM linggan_material_domain_usage usage
      WHERE usage.content_public_ref=content.public_ref AND usage.domain_ref=$9))
 AND ($10::uuid[] IS NULL OR content.public_ref=ANY($10))
)
"#;

// Keep the original detail > discovery > lane time priority and human-minute precision.
// Computing KNOWN title/body/metrics for every work is unnecessary on an unfiltered page.
const IDENTITY_PAGE_SCOPE: &str = r#"
scoped_work AS MATERIALIZED (
 SELECT content.* FROM linggan_material_content content
 WHERE ($8::uuid IS NULL OR content.public_ref=$8)
 AND ($9::uuid IS NULL OR EXISTS (SELECT 1 FROM linggan_material_domain_usage usage
      WHERE usage.content_public_ref=content.public_ref AND usage.domain_ref=$9))
 AND ($10::uuid[] IS NULL OR content.public_ref=ANY($10))
), recent_detail_time AS (
 SELECT detail.content_public_ref,max(detail.observed_at::timestamptz) AS observed_at
 FROM linggan_material_content_detail detail JOIN scoped_work scope ON scope.public_ref=detail.content_public_ref
 JOIN linggan_runtime_capture_package package USING(package_ref)
 WHERE package.accepted_at <= $2::timestamptz AND detail.created_at <= $2::timestamptz GROUP BY detail.content_public_ref
), recent_discovery_time AS (
 SELECT finding.content_public_ref,max(finding.observed_at::timestamptz) AS observed_at
 FROM linggan_material_discovery_finding finding JOIN scoped_work scope ON scope.public_ref=finding.content_public_ref
 JOIN linggan_runtime_capture_package package USING(package_ref)
 WHERE package.accepted_at <= $2::timestamptz AND finding.created_at <= $2::timestamptz GROUP BY finding.content_public_ref
), recent_lane_time AS (
 SELECT lane.content_public_ref,max(lane.observed_at::timestamptz) AS observed_at
 FROM linggan_material_lane_observation lane JOIN scoped_work scope ON scope.public_ref=lane.content_public_ref
 JOIN linggan_runtime_capture_package package USING(package_ref)
 WHERE package.accepted_at <= $2::timestamptz AND lane.created_at <= $2::timestamptz GROUP BY lane.content_public_ref
), requested_work AS MATERIALIZED (
 SELECT content.* FROM scoped_work content
 LEFT JOIN recent_detail_time detail ON detail.content_public_ref=content.public_ref
 LEFT JOIN recent_discovery_time discovery ON discovery.content_public_ref=content.public_ref
 LEFT JOIN recent_lane_time lane ON lane.content_public_ref=content.public_ref
 CROSS JOIN LATERAL (SELECT linggan_human_moment(COALESCE(detail.observed_at,discovery.observed_at,lane.observed_at))::timestamptz AS observed_at) moment
 WHERE moment.observed_at IS NOT NULL
 AND ($3::text IS NULL OR moment.observed_at < $3::timestamptz
   OR (moment.observed_at=$3::timestamptz
     AND (content.platform>$4 OR (content.platform=$4 AND content.content_external_id>$5))))
 ORDER BY moment.observed_at DESC,content.platform,content.content_external_id LIMIT 51
)
"#;

pub(crate) fn material_page_sql(identity_page: bool) -> String {
    let scope = if identity_page {
        IDENTITY_PAGE_SCOPE
    } else {
        FILTERED_PAGE_SCOPE
    };
    format!(
        r#"WITH {scope}, {WORK_RESOURCE_CURRENT_CTES}, current_comment AS (
  SELECT DISTINCT ON (comment.content_public_ref,comment.comment_external_id) comment.*
  FROM linggan_material_comment comment
  JOIN requested_work scope ON scope.public_ref=comment.content_public_ref
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND comment.created_at <= $2::timestamptz
  ORDER BY comment.content_public_ref,comment.comment_external_id,
           comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC
)
SELECT current.* FROM work_resource_current current
WHERE ($1::text IS NULL
  OR lower(COALESCE(current.title,'')) LIKE '%' || lower($1) || '%'
  OR lower(COALESCE(current.creator_display_name,'')) LIKE '%' || lower($1) || '%'
  OR EXISTS (SELECT 1 FROM current_comment comment WHERE comment.content_public_ref=current.public_ref AND lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($1) || '%')
  OR EXISTS (
    SELECT 1 FROM linggan_material_unit u
    JOIN LATERAL(SELECT * FROM linggan_material_reading_selection s WHERE s.unit_ref=u.unit_ref AND s.created_at<=$2::timestamptz ORDER BY revision DESC LIMIT 1) selected ON true
    JOIN linggan_material_reading reading ON reading.reading_ref=selected.reading_ref
    WHERE u.content_public_ref=current.public_ref
      AND lower((jsonb_path_query_array(reading.blocks,'$[*].text') || jsonb_path_query_array(reading.blocks,'$[*].items[*]') || jsonb_path_query_array(reading.blocks,'$[*].columns[*]') || jsonb_path_query_array(reading.blocks,'$[*].rows[*][*]') || jsonb_path_query_array(reading.blocks,'$[*].turns[*].text'))::text) LIKE '%' || lower($1) || '%'
      AND ((u.kind='body' AND reading.source_version=current.body_source_material_ref::text) OR (u.kind<>'body' AND EXISTS(
        SELECT 1 FROM linggan_material_media_origin origin
        JOIN linggan_runtime_capture_package source_package ON source_package.package_ref=origin.package_ref
        JOIN linggan_runtime_record_disposition source_disposition ON source_disposition.package_ref=origin.package_ref AND source_disposition.record_ordinal=origin.record_ordinal
        JOIN linggan_media_download_attempt attempt ON attempt.media_observation_ref=origin.observation_ref
        JOIN linggan_media_materialization media USING(download_attempt_ref)
        WHERE origin.content_public_ref=u.content_public_ref AND origin.slot_key=u.slot_key AND linggan_dossier_slot_active(u.content_public_ref,u.slot_key,$2::timestamptz)
          AND origin.created_at<=$2::timestamptz AND source_package.accepted_at<=$2::timestamptz AND source_disposition.disposition='accepted_for_media_identity'
          AND origin.source_generation=(SELECT max(o.source_generation) FROM linggan_material_media_origin o JOIN linggan_runtime_capture_package p USING(package_ref) JOIN linggan_runtime_record_disposition d ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal WHERE o.slot_key=u.slot_key AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz AND d.disposition='accepted_for_media_identity')
          AND media.materialization_ref::text=reading.source_version AND media.verified_at<=$2::timestamptz
          AND media.materialization_ref=(SELECT m2.materialization_ref FROM linggan_media_materialization m2 JOIN linggan_media_download_attempt a2 USING(download_attempt_ref) JOIN linggan_media_blob b2 ON b2.sha256=m2.blob_sha256 WHERE ((u.kind='image' AND b2.mime_type LIKE 'image/%') OR (u.kind='video_frame' AND b2.mime_type LIKE 'video/%') OR (u.kind='audio' AND (b2.mime_type LIKE 'video/%' OR (origin.purpose<>'live_photo' AND b2.mime_type LIKE 'audio/%')))) AND a2.media_observation_ref=origin.observation_ref AND m2.verified_at<=$2::timestamptz ORDER BY m2.verified_at DESC,m2.materialization_ref DESC LIMIT 1)
          AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition dis WHERE dis.state='WITHDRAWN_OR_RESTRICTED' AND (dis.slot_key=u.slot_key OR dis.blob_sha256=media.blob_sha256 OR dis.materialization_ref=media.materialization_ref OR dis.derivative_ref=reading.derivative_ref))
          AND NOT EXISTS(SELECT 1 FROM linggan_media_derivative d JOIN linggan_media_ocr_retirement retired ON retired.retired_job_ref=d.job_ref WHERE d.derivative_ref=reading.derivative_ref)
      )))
  )
  OR EXISTS (SELECT 1 FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package author_package USING(package_ref) WHERE author.platform=current.platform AND author.author_external_id=current.author_external_id AND author_package.accepted_at <= $2::timestamptz AND author.created_at <= $2::timestamptz AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%')))
AND ($8::uuid IS NULL OR current.public_ref=$8)
AND ($9::uuid IS NULL OR EXISTS (
  SELECT 1 FROM linggan_material_domain_usage domain_usage
  WHERE domain_usage.content_public_ref=current.public_ref
    AND domain_usage.domain_ref=$9))
AND ($10::uuid[] IS NULL OR current.public_ref=ANY($10))
AND current.observed_at IS NOT NULL
AND ($6::text IS NULL
  OR ($6='detail' AND current.detail_material_ref IS NOT NULL)
  OR ($6='discovery' AND current.material_ref IS NOT NULL)
  OR ($6 IN ('comments','replies') AND EXISTS (SELECT 1 FROM linggan_material_lane_observation filtered_lane JOIN linggan_runtime_capture_package filtered_package USING(package_ref) WHERE filtered_lane.content_public_ref=current.public_ref AND filtered_lane.lane=$6 AND filtered_package.accepted_at <= $2::timestamptz AND filtered_lane.created_at <= $2::timestamptz))
  OR ($6='author' AND EXISTS (SELECT 1 FROM linggan_material_author_profile filtered_author JOIN linggan_runtime_capture_package filtered_author_package USING(package_ref) WHERE filtered_author.platform=current.platform AND filtered_author.author_external_id=current.author_external_id AND filtered_author_package.accepted_at <= $2::timestamptz AND filtered_author.created_at <= $2::timestamptz))
  OR ($6='media_slots' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) WHERE filtered_origin.content_public_ref=current.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz AND filtered_origin.created_at <= $2::timestamptz))
  OR ($6='media_bytes' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) JOIN linggan_media_download_attempt filtered_attempt ON filtered_attempt.media_observation_ref=filtered_origin.observation_ref JOIN linggan_media_materialization filtered_materialization USING(download_attempt_ref) WHERE filtered_origin.content_public_ref=current.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz AND filtered_origin.created_at <= $2::timestamptz AND filtered_materialization.verified_at <= $2::timestamptz))
  OR ($6='ocr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=current.platform AND filtered_slot.content_external_id=current.content_external_id AND filtered_job.processor_kind IN ('image_ocr','video_frame_ocr') AND filtered_job.created_at <= $2::timestamptz))
  OR ($6='asr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=current.platform AND filtered_slot.content_external_id=current.content_external_id AND filtered_job.processor_kind='asr' AND filtered_job.created_at <= $2::timestamptz)))
AND ($7::text IS NULL OR EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_kind JOIN linggan_runtime_capture_package filtered_kind_package USING(package_ref) WHERE filtered_kind.content_public_ref=current.public_ref AND filtered_kind.purpose=$7 AND filtered_kind_package.accepted_at <= $2::timestamptz AND filtered_kind.created_at <= $2::timestamptz))
AND ($3::text IS NULL OR current.observed_at::timestamptz < $3::timestamptz
  OR (current.observed_at::timestamptz=$3::timestamptz
    AND (current.platform > $4 OR (current.platform=$4 AND current.content_external_id > $5))))
ORDER BY current.observed_at::timestamptz DESC,current.platform,current.content_external_id
LIMIT 51"#,
    )
}

pub(crate) fn work_resource_currents_sql() -> String {
    format!(
        "WITH requested_work AS MATERIALIZED (SELECT * FROM linggan_material_content WHERE public_ref=ANY($1::uuid[])), \
         {WORK_RESOURCE_CURRENT_CTES} \
         SELECT current.* FROM work_resource_current current WHERE current.observed_at IS NOT NULL"
    )
}

#[cfg(test)]
mod tests {
    use super::{material_page_sql, work_resource_currents_sql};

    #[test]
    fn shared_work_resource_reads_do_not_expand_target_scoped_retirement_to_all_corpus_uses() {
        for sql in [
            material_page_sql(false),
            material_page_sql(true),
            work_resource_currents_sql(),
        ] {
            assert!(
                !sql.contains("collection_material_retirement"),
                "a target-scoped human retirement conclusion cannot erase the content globally"
            );
        }
    }
}
