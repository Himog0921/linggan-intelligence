WITH params AS (
 SELECT $1::uuid domain_ref,($2::timestamptz AT TIME ZONE 'Asia/Shanghai')::date end_date
), days AS (
 SELECT generate_series(end_date-55,end_date,'1 day'::interval)::date AS day FROM params
), first_observation AS (
 SELECT comment.content_public_ref,comment.comment_external_id,min(package.accepted_at) first_at
 FROM linggan_material_comment comment JOIN linggan_runtime_capture_package package USING(package_ref),params
 WHERE package.accepted_at<=$2::timestamptz AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage
 WHERE usage.content_public_ref=comment.content_public_ref AND usage.domain_ref=params.domain_ref AND usage.role='primary')
 GROUP BY comment.content_public_ref,comment.comment_external_id
), observations AS (
 SELECT (first_at AT TIME ZONE 'Asia/Shanghai')::date AS day,count(*) amount FROM first_observation GROUP BY 1
), entries AS (
 SELECT (request.created_at AT TIME ZONE 'Asia/Shanghai')::date AS day,
 count(DISTINCT (request.work_ref,source.comment_external_id)) amount
 FROM linggan_comment_research_request request JOIN linggan_material_comment source ON source.material_ref=request.source_ref
 WHERE request.domain_ref=$1 AND request.created_at<=$2::timestamptz GROUP BY 1
), signals AS (
 SELECT (result.created_at AT TIME ZONE 'Asia/Shanghai')::date AS day,
 sum(jsonb_array_length(result.result->'signals')) amount
 FROM linggan_comment_research_result result JOIN linggan_comment_research_request request USING(request_ref)
 WHERE request.domain_ref=$1 AND result.created_at<=$2::timestamptz AND result.result IS NOT NULL GROUP BY 1
), covered AS (
 SELECT day,count(DISTINCT content_public_ref) amount FROM (
 SELECT (first_at AT TIME ZONE 'Asia/Shanghai')::date AS day,content_public_ref FROM first_observation
 UNION ALL SELECT (created_at AT TIME ZONE 'Asia/Shanghai')::date,work_ref FROM linggan_comment_research_request
 WHERE domain_ref=$1 AND created_at<=$2::timestamptz
 ) activity GROUP BY day
), coverage AS (
 SELECT (package.accepted_at AT TIME ZONE 'Asia/Shanghai')::date AS day,count(*) ledger_count,
 bool_or(COALESCE(lane.failed,0)>0 OR COALESCE(lane.known_unattempted,0)>0 OR COALESCE(lane.unknown_count,0)>0
 OR package.coverage->'target'->'commentCollection'->>'state' IS DISTINCT FROM 'complete') has_gap
 FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package package USING(package_ref),params
 WHERE lane.lane IN('comments','replies') AND package.accepted_at<=$2::timestamptz
 AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=lane.content_public_ref
 AND usage.domain_ref=params.domain_ref AND usage.role='primary') GROUP BY 1
)
SELECT jsonb_build_object('timezone','Asia/Shanghai','points',jsonb_agg(jsonb_build_object(
 'date',to_char(days.day,'YYYY-MM-DD'),'newObservedCommentCount',COALESCE(observations.amount,0),
 'studiedCommentCount',COALESCE(entries.amount,0),'acceptedSignalCount',COALESCE(signals.amount,0),
 'coveredWorkCount',COALESCE(covered.amount,0),'observationCoverage',CASE WHEN coverage.ledger_count IS NULL THEN 'none'
 WHEN coverage.has_gap THEN 'partial' ELSE 'recorded' END) ORDER BY days.day))
FROM days LEFT JOIN observations USING(day) LEFT JOIN entries USING(day) LEFT JOIN signals USING(day)
LEFT JOIN covered USING(day) LEFT JOIN coverage USING(day);
