-- Current workflow metadata for one stable comment; never load historical bodies.
WITH scoped AS (
 SELECT r.request_ref,r.source_ref,r.created_at,
 CASE WHEN bool_or(c.state IN ('pending','running')) THEN 'running'
 WHEN bool_or(c.state='failed' OR (c.state='skipped' AND c.failure_code IS DISTINCT FROM 'comment_no_problem_signal')) THEN 'failed'
 WHEN bool_or(c.state='cancelled') THEN 'cancelled' ELSE 'succeeded' END AS state
 FROM linggan_comment_research_request r JOIN linggan_material_comment source ON source.material_ref=r.source_ref
 JOIN linggan_comment_research_stage s USING(request_ref) JOIN linggan_ai_batch_item_control c USING(item_ref)
 WHERE r.domain_ref=$1 AND r.work_ref=$2 AND source.comment_external_id=$3 AND r.created_at<=$4::timestamptz
 GROUP BY r.request_ref
), page AS (SELECT * FROM scoped WHERE $5::timestamptz IS NULL OR (created_at,request_ref)<($5::timestamptz,$6::uuid)
 ORDER BY created_at DESC,request_ref DESC LIMIT $7)
SELECT jsonb_build_object('totalCount',(SELECT count(*) FROM scoped),'rows',COALESCE((SELECT jsonb_agg(jsonb_build_object(
 'position',jsonb_build_object('createdAt',to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),'reference',request_ref),
 'item',jsonb_build_object('requestRef',request_ref,'sourceRef',source_ref,'state',state,'createdAt',created_at::text))
 ORDER BY created_at DESC,request_ref DESC) FROM page),'[]'::jsonb));
