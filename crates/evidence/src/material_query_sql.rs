//! SQL owned by the shared Work Resource Current projection.
//!
//! A material record is append-only. Every consumer therefore selects the newest qualified
//! value per field from this one CTE graph. Page filters and target-scoped candidate discovery
//! may choose *which* Works are read, but they must not copy title, author, publication-time or
//! engagement Current adjudication.

const WORK_RESOURCE_CURRENT_CTES: &str = r#"
latest_detail AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,
    detail.observed_at,detail.created_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_title AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.title,detail.title_state,detail.material_ref,
    detail.package_ref,detail.observed_at,detail.created_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.title_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_body AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.body_text,detail.body_state,detail.material_ref,
    detail.package_ref,detail.observed_at,detail.created_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.body_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_creator AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.creator_display_name,detail.creator_display_name_state,
    detail.material_ref,detail.package_ref,detail.observed_at,detail.created_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.creator_display_name_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_author AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.author_external_id,detail.material_ref,
    detail.package_ref,detail.observed_at,detail.created_at
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.author_external_id IS NOT NULL
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_published_at AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.published_at,detail.material_ref,detail.package_ref,
    detail.observed_at,detail.created_at,
    detail.published_at_source_field,detail.published_at_source_kind,
    detail.published_at_precision,detail.published_at_reference_observed_at,
    detail.published_at_parser_version
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND detail.published_at IS NOT NULL
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_detail_published_text AS (
  SELECT DISTINCT ON (detail.content_public_ref)
    detail.content_public_ref,detail.published_at_source_text,detail.material_ref,
    detail.package_ref,detail.observed_at,detail.created_at,
    detail.published_at_source_text_state,detail.published_at_source_field,
    detail.published_at_source_kind,detail.published_at_precision,
    detail.published_at_reference_observed_at,detail.published_at_parser_version
  FROM linggan_material_content_detail detail
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz
    AND detail.published_at_source_text_state='KNOWN'
  ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,
           detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC
), latest_discovery AS (
  SELECT DISTINCT ON (finding.content_public_ref) finding.*
  FROM linggan_material_discovery_finding finding
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz
  ORDER BY finding.content_public_ref,finding.observed_at::timestamptz DESC,
           finding.created_at DESC,finding.package_ref DESC,finding.material_ref DESC
), latest_like AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.like_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM linggan_material_engagement_observation observation
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND observation.like_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_comment_count AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.comment_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM linggan_material_engagement_observation observation
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND observation.comment_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_collect AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.collect_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM linggan_material_engagement_observation observation
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND observation.collect_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_share AS (
  SELECT DISTINCT ON (observation.content_public_ref)
    observation.content_public_ref,observation.share_count,observation.package_ref,
    observation.source_lane,observation.observed_at,observation.created_at
  FROM linggan_material_engagement_observation observation
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz AND observation.share_count_state='KNOWN'
  ORDER BY observation.content_public_ref,observation.observed_at::timestamptz DESC,
           observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
), latest_lane AS (
  SELECT lane.content_public_ref,max(lane.observed_at::timestamptz)::text AS observed_at
  FROM linggan_material_lane_observation lane
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE lane.content_public_ref IS NOT NULL AND package.accepted_at <= $2::timestamptz
  GROUP BY lane.content_public_ref
), work_resource_current AS (
  SELECT content.platform,content.content_external_id,content.public_ref,
    detail.material_ref AS detail_material_ref,
    COALESCE(detail.material_ref,discovery.material_ref) AS material_ref,
    COALESCE(detail.package_ref,discovery.package_ref) AS package_ref,
    COALESCE(detail.record_ordinal,discovery.record_ordinal) AS record_ordinal,
    COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) AS observed_at,
    COALESCE(detail_title.title,discovery.title) AS title,
    COALESCE(detail_title.title_state,discovery.title_state,'UNKNOWN') AS title_state,
    COALESCE(detail_title.material_ref,discovery.material_ref) AS title_source_material_ref,
    COALESCE(detail_title.package_ref,discovery.package_ref) AS title_source_package_ref,
    COALESCE(detail_title.observed_at,discovery.observed_at) AS title_source_observed_at,
    COALESCE(detail_title.created_at,discovery.created_at)::text AS title_source_recorded_at,
    detail_body.body_text,COALESCE(detail_body.body_state,'UNKNOWN') AS body_state,
    detail_body.material_ref AS body_source_material_ref,
    detail_body.package_ref AS body_source_package_ref,
    detail_body.observed_at AS body_source_observed_at,
    detail_body.created_at::text AS body_source_recorded_at,
    COALESCE(detail_creator.creator_display_name,discovery.creator_display_name) AS creator_display_name,
    COALESCE(detail_creator.creator_display_name_state,discovery.creator_state,'UNKNOWN') AS creator_display_name_state,
    COALESCE(detail_creator.material_ref,discovery.material_ref) AS creator_source_material_ref,
    COALESCE(detail_creator.package_ref,discovery.package_ref) AS creator_source_package_ref,
    COALESCE(detail_creator.observed_at,discovery.observed_at) AS creator_source_observed_at,
    COALESCE(detail_creator.created_at,discovery.created_at)::text AS creator_source_recorded_at,
    detail_published_at.published_at::text AS published_at,
    CASE WHEN detail_published_at.published_at IS NULL THEN NULL
         ELSE (detail_published_at.published_at AT TIME ZONE 'Asia/Shanghai')::date::text END AS published_local_date,
    CASE WHEN detail_published_at.published_at IS NULL THEN NULL
         ELSE (extract(epoch FROM detail_published_at.published_at)*1000)::bigint END AS published_at_epoch_ms,
    COALESCE(detail_published_text.published_at_source_text,discovery.published_at_source_text) AS published_at_source_text,
    COALESCE(detail_published_text.published_at_source_text_state,discovery.published_at_source_text_state,'UNKNOWN') AS published_at_source_text_state,
    COALESCE(detail_published_at.published_at_source_field,detail_published_text.published_at_source_field) AS published_at_source_field,
    COALESCE(detail_published_at.published_at_source_kind,detail_published_text.published_at_source_kind,'unknown') AS published_at_source_kind,
    COALESCE(detail_published_at.published_at_precision,detail_published_text.published_at_precision,'unknown') AS published_at_precision,
    COALESCE(detail_published_at.published_at_reference_observed_at,detail_published_text.published_at_reference_observed_at)::text AS published_at_reference_observed_at,
    COALESCE(detail_published_at.published_at_parser_version,detail_published_text.published_at_parser_version) AS published_at_parser_version,
    COALESCE(detail_published_at.material_ref,detail_published_text.material_ref,discovery.material_ref) AS published_source_material_ref,
    COALESCE(detail_published_at.package_ref,detail_published_text.package_ref,discovery.package_ref) AS published_source_package_ref,
    COALESCE(detail_published_at.observed_at,detail_published_text.observed_at,discovery.observed_at) AS published_source_observed_at,
    COALESCE(detail_published_at.created_at,detail_published_text.created_at,discovery.created_at)::text AS published_source_recorded_at,
    detail_author.author_external_id,
    detail_author.package_ref AS author_source_package_ref,
    detail_author.observed_at AS author_source_observed_at,
    detail_author.created_at::text AS author_source_recorded_at,
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
  FROM linggan_material_content content
  LEFT JOIN latest_detail detail ON detail.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_title detail_title ON detail_title.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_body detail_body ON detail_body.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_creator detail_creator ON detail_creator.content_public_ref=content.public_ref
  LEFT JOIN latest_detail_author detail_author ON detail_author.content_public_ref=content.public_ref
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

pub(crate) fn material_page_sql() -> String {
    format!(
        r#"WITH {WORK_RESOURCE_CURRENT_CTES}, current_comment AS (
  SELECT DISTINCT ON (comment.content_public_ref,comment.comment_external_id) comment.*
  FROM linggan_material_comment comment
  JOIN linggan_runtime_capture_package package USING(package_ref)
  WHERE package.accepted_at <= $2::timestamptz
  ORDER BY comment.content_public_ref,comment.comment_external_id,
           comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC
)
SELECT current.* FROM work_resource_current current
WHERE ($1::text IS NULL
  OR lower(COALESCE(current.title,'')) LIKE '%' || lower($1) || '%'
  OR lower(COALESCE(current.body_text,'')) LIKE '%' || lower($1) || '%'
  OR lower(COALESCE(current.creator_display_name,'')) LIKE '%' || lower($1) || '%'
  OR EXISTS (SELECT 1 FROM current_comment comment WHERE comment.content_public_ref=current.public_ref AND lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($1) || '%')
  OR EXISTS (SELECT 1 FROM linggan_material_derived_text derived WHERE derived.content_public_ref=current.public_ref AND lower(derived.text_content) LIKE '%' || lower($1) || '%')
  OR EXISTS (SELECT 1 FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package author_package USING(package_ref) WHERE author.platform=current.platform AND author.author_external_id=current.author_external_id AND author_package.accepted_at <= $2::timestamptz AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%')))
AND ($8::uuid IS NULL OR current.public_ref=$8)
AND current.observed_at IS NOT NULL
AND ($6::text IS NULL
  OR ($6='detail' AND current.detail_material_ref IS NOT NULL)
  OR ($6='discovery' AND current.material_ref IS NOT NULL)
  OR ($6 IN ('comments','replies') AND EXISTS (SELECT 1 FROM linggan_material_lane_observation filtered_lane JOIN linggan_runtime_capture_package filtered_package USING(package_ref) WHERE filtered_lane.content_public_ref=current.public_ref AND filtered_lane.lane=$6 AND filtered_package.accepted_at <= $2::timestamptz))
  OR ($6='author' AND EXISTS (SELECT 1 FROM linggan_material_author_profile filtered_author JOIN linggan_runtime_capture_package filtered_author_package USING(package_ref) WHERE filtered_author.platform=current.platform AND filtered_author.author_external_id=current.author_external_id AND filtered_author_package.accepted_at <= $2::timestamptz))
  OR ($6='media_slots' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) WHERE filtered_origin.content_public_ref=current.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz))
  OR ($6='media_bytes' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) JOIN linggan_media_download_attempt filtered_attempt ON filtered_attempt.media_observation_ref=filtered_origin.observation_ref JOIN linggan_media_materialization filtered_materialization USING(download_attempt_ref) WHERE filtered_origin.content_public_ref=current.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz AND filtered_materialization.verified_at <= $2::timestamptz))
  OR ($6='ocr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=current.platform AND filtered_slot.content_external_id=current.content_external_id AND filtered_job.processor_kind IN ('image_ocr','video_frame_ocr') AND filtered_job.created_at <= $2::timestamptz))
  OR ($6='asr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=current.platform AND filtered_slot.content_external_id=current.content_external_id AND filtered_job.processor_kind='asr' AND filtered_job.created_at <= $2::timestamptz)))
AND ($7::text IS NULL OR EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_kind JOIN linggan_runtime_capture_package filtered_kind_package USING(package_ref) WHERE filtered_kind.content_public_ref=current.public_ref AND filtered_kind.purpose=$7 AND filtered_kind_package.accepted_at <= $2::timestamptz))
AND ($3::text IS NULL OR current.observed_at::timestamptz < $3::timestamptz
  OR (current.observed_at::timestamptz=$3::timestamptz
    AND (current.platform > $4 OR (current.platform=$4 AND current.content_external_id > $5))))
ORDER BY current.observed_at::timestamptz DESC,current.platform,current.content_external_id
LIMIT 51"#,
    )
}

pub(crate) fn work_resource_currents_sql() -> String {
    format!(
        "WITH {WORK_RESOURCE_CURRENT_CTES} \
         SELECT current.* FROM work_resource_current current \
         WHERE current.public_ref=ANY($1::uuid[]) AND current.observed_at IS NOT NULL"
    )
}
