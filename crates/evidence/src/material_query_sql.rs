//! Focused SQL seam for the work-level keyset page.

pub(crate) const MATERIAL_PAGE_SQL: &str = "WITH latest_detail AS ( \
   SELECT DISTINCT ON (detail.content_public_ref) \
     detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at, \
     detail.title,detail.title_state,detail.body_text,detail.body_state, \
     detail.creator_display_name,detail.creator_display_name_state, \
     detail.published_at_source_text,detail.published_at_source_text_state,detail.author_external_id \
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
     detail.author_external_id \
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
   OR EXISTS (SELECT 1 FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package author_package USING(package_ref) WHERE author.platform=content.platform AND author.author_external_id=detail.author_external_id AND author_package.accepted_at <= $2::timestamptz AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%'))) \
   AND COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) IS NOT NULL \
   AND ($3::text IS NULL OR COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz < $3::timestamptz \
     OR (COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz = $3::timestamptz \
       AND (content.platform > $4 OR (content.platform=$4 AND content.content_external_id > $5)))) \
 ORDER BY COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at)::timestamptz DESC,content.platform,content.content_external_id \
 LIMIT 51";
