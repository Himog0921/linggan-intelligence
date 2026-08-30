//! Focused SQL seam for the work-level keyset page.

pub(crate) const MATERIAL_PAGE_SQL: &str = "WITH latest_detail AS ( \
   SELECT DISTINCT ON (detail.content_public_ref) \
     detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at, \
     detail.title,detail.title_state,detail.body_text,detail.body_state, \
     detail.creator_display_name,detail.creator_display_name_state, \
     detail.published_at_source_text,detail.published_at_source_text_state,detail.author_external_id, \
     detail.like_count,detail.like_count_state,detail.comment_count,detail.comment_count_state, \
     detail.collect_count,detail.collect_count_state,detail.share_count,detail.share_count_state \
   FROM linggan_material_content_detail detail JOIN linggan_runtime_capture_package detail_package USING(package_ref) \
   WHERE detail_package.accepted_at <= $2::timestamptz \
   ORDER BY detail.content_public_ref,detail.observed_at::timestamptz DESC,detail.created_at DESC \
 ), latest_discovery AS ( \
   SELECT DISTINCT ON (finding.content_public_ref) finding.* FROM linggan_material_discovery_finding finding JOIN linggan_runtime_capture_package discovery_package USING(package_ref) \
   WHERE discovery_package.accepted_at <= $2::timestamptz \
   ORDER BY finding.content_public_ref,finding.observed_at::timestamptz DESC,finding.created_at DESC \
 ), latest_lane AS ( \
   SELECT lane.content_public_ref,max(lane.observed_at::timestamptz)::text AS observed_at \
   FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package lane_package USING(package_ref) \
   WHERE content_public_ref IS NOT NULL AND lane_package.accepted_at <= $2::timestamptz GROUP BY content_public_ref \
 ) SELECT content.platform,content.content_external_id,content.public_ref, \
     detail.material_ref AS detail_material_ref,COALESCE(detail.material_ref,discovery.material_ref) AS material_ref,COALESCE(detail.package_ref,discovery.package_ref) AS package_ref,COALESCE(detail.record_ordinal,discovery.record_ordinal) AS record_ordinal, \
     COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) AS observed_at, \
     COALESCE(detail.title,discovery.title) AS title,CASE WHEN detail.title IS NOT NULL THEN detail.title_state ELSE COALESCE(discovery.title_state,'UNKNOWN') END AS title_state,detail.body_text, \
     COALESCE(detail.body_state,'UNKNOWN') AS body_state,COALESCE(detail.creator_display_name,discovery.creator_display_name) AS creator_display_name, \
     CASE WHEN detail.creator_display_name IS NOT NULL THEN detail.creator_display_name_state ELSE COALESCE(discovery.creator_state,'UNKNOWN') END AS creator_display_name_state, \
     COALESCE(detail.published_at_source_text,discovery.published_at_source_text) AS published_at_source_text,CASE WHEN detail.published_at_source_text IS NOT NULL THEN detail.published_at_source_text_state ELSE COALESCE(discovery.published_at_source_text_state,'UNKNOWN') END AS published_at_source_text_state, \
     detail.author_external_id,discovery.cover_source_url,COALESCE(discovery.cover_source_state,'UNKNOWN') AS cover_source_state, \
     COALESCE(detail.like_count,discovery.like_count) AS like_count,CASE WHEN detail.like_count IS NOT NULL THEN detail.like_count_state ELSE COALESCE(discovery.like_count_state,'UNKNOWN') END AS like_count_state, \
     COALESCE(detail.comment_count,discovery.comment_count) AS comment_count,CASE WHEN detail.comment_count IS NOT NULL THEN detail.comment_count_state ELSE COALESCE(discovery.comment_count_state,'UNKNOWN') END AS comment_count_state, \
     COALESCE(detail.collect_count,discovery.collect_count) AS collect_count,CASE WHEN detail.collect_count IS NOT NULL THEN detail.collect_count_state ELSE COALESCE(discovery.collect_count_state,'UNKNOWN') END AS collect_count_state, \
     COALESCE(detail.share_count,discovery.share_count) AS share_count,CASE WHEN detail.share_count IS NOT NULL THEN detail.share_count_state ELSE COALESCE(discovery.share_count_state,'UNKNOWN') END AS share_count_state \
 FROM linggan_material_content content \
 LEFT JOIN latest_detail detail ON detail.content_public_ref = content.public_ref \
 LEFT JOIN latest_discovery discovery ON discovery.content_public_ref = content.public_ref \
 LEFT JOIN latest_lane lane_latest ON lane_latest.content_public_ref = content.public_ref \
 WHERE ($1::text IS NULL \
   OR lower(COALESCE(detail.title,'')) LIKE '%' || lower($1) || '%' \
   OR lower(COALESCE(detail.body_text,'')) LIKE '%' || lower($1) || '%' \
   OR lower(COALESCE(detail.creator_display_name,'')) LIKE '%' || lower($1) || '%' \
   OR lower(COALESCE(discovery.title,'')) LIKE '%' || lower($1) || '%' \
   OR lower(COALESCE(discovery.creator_display_name,'')) LIKE '%' || lower($1) || '%' \
   OR EXISTS (SELECT 1 FROM linggan_material_comment comment WHERE comment.content_public_ref=content.public_ref AND lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($1) || '%') \
   OR EXISTS (SELECT 1 FROM linggan_material_derived_text derived WHERE derived.content_public_ref=content.public_ref AND lower(derived.text_content) LIKE '%' || lower($1) || '%') \
   OR EXISTS (SELECT 1 FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package author_package USING(package_ref) WHERE author.platform=content.platform AND author.author_external_id=detail.author_external_id AND author_package.accepted_at <= $2::timestamptz AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%'))) \
   AND ($8::uuid IS NULL OR content.public_ref=$8) \
   AND COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) IS NOT NULL \
   AND ($6::text IS NULL \
     OR ($6='detail' AND detail.material_ref IS NOT NULL) \
     OR ($6='discovery' AND discovery.material_ref IS NOT NULL) \
     OR ($6 IN ('comments','replies') AND EXISTS (SELECT 1 FROM linggan_material_lane_observation filtered_lane JOIN linggan_runtime_capture_package filtered_package USING(package_ref) WHERE filtered_lane.content_public_ref=content.public_ref AND filtered_lane.lane=$6 AND filtered_package.accepted_at <= $2::timestamptz)) \
     OR ($6='author' AND EXISTS (SELECT 1 FROM linggan_material_author_profile filtered_author JOIN linggan_runtime_capture_package filtered_author_package USING(package_ref) WHERE filtered_author.platform=content.platform AND filtered_author.author_external_id=detail.author_external_id AND filtered_author_package.accepted_at <= $2::timestamptz)) \
     OR ($6='media_slots' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) WHERE filtered_origin.content_public_ref=content.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz)) \
     OR ($6='media_bytes' AND EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_origin JOIN linggan_runtime_capture_package filtered_origin_package USING(package_ref) JOIN linggan_media_download_attempt filtered_attempt ON filtered_attempt.media_observation_ref=filtered_origin.observation_ref JOIN linggan_media_materialization filtered_materialization USING(download_attempt_ref) WHERE filtered_origin.content_public_ref=content.public_ref AND filtered_origin_package.accepted_at <= $2::timestamptz AND filtered_materialization.verified_at <= $2::timestamptz)) \
     OR ($6='ocr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=content.platform AND filtered_slot.content_external_id=content.content_external_id AND filtered_job.processor_kind IN ('image_ocr','video_frame_ocr') AND filtered_job.created_at <= $2::timestamptz)) \
     OR ($6='asr' AND EXISTS (SELECT 1 FROM linggan_media_processing_job filtered_job JOIN linggan_media_slot filtered_slot USING(slot_key) WHERE filtered_slot.platform=content.platform AND filtered_slot.content_external_id=content.content_external_id AND filtered_job.processor_kind='asr' AND filtered_job.created_at <= $2::timestamptz))) \
   AND ($7::text IS NULL OR EXISTS (SELECT 1 FROM linggan_material_media_origin filtered_kind JOIN linggan_runtime_capture_package filtered_kind_package USING(package_ref) WHERE filtered_kind.content_public_ref=content.public_ref AND filtered_kind.purpose=$7 AND filtered_kind_package.accepted_at <= $2::timestamptz)) \
   AND ($3::text IS NULL OR COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz < $3::timestamptz \
     OR (COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz = $3::timestamptz \
       AND (content.platform > $4 OR (content.platform=$4 AND content.content_external_id > $5)))) \
 ORDER BY COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz DESC,content.platform,content.content_external_id \
 LIMIT 51";
