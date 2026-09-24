-- One insensitive cursor statement. $1 domain, $2 cutoff, $3 cleaner, $4 works,
-- $5 exact comment keys or null. FETCH consumes this snapshot; it is not another source SELECT.
WITH requested_works AS MATERIALIZED (
    SELECT public_ref FROM linggan_material_content
    WHERE domain_ref=$1 AND public_ref=ANY($4::uuid[])
), requested_keys AS (
    SELECT * FROM jsonb_to_recordset(COALESCE($5::jsonb,'[]')) AS k("workRef" uuid,"commentExternalId" text)
), latest AS MATERIALIZED (
    SELECT DISTINCT ON (c.content_public_ref,c.comment_external_id) c.material_ref,
        c.content_public_ref,c.comment_external_id,c.parent_comment_external_id,c.body_state,
        c.author_external_id,c.created_at
    FROM linggan_material_comment c JOIN requested_works w ON w.public_ref=c.content_public_ref
    JOIN linggan_runtime_capture_package p USING(package_ref)
    WHERE p.accepted_at<=$2::text::timestamptz AND c.created_at<=$2::text::timestamptz
      AND ($5::jsonb IS NULL OR EXISTS (SELECT 1 FROM requested_keys k
          WHERE k."workRef"=c.content_public_ref AND k."commentExternalId"=c.comment_external_id))
    ORDER BY c.content_public_ref,c.comment_external_id,c.observed_at::timestamptz DESC,
        c.created_at DESC,c.material_ref DESC
), live_runs AS MATERIALIZED (
    SELECT r.run_ref FROM linggan_comment_study_model_request r
      JOIN linggan_model_invocation i USING(invocation_ref) WHERE i.state='running'
    UNION
    SELECT b.run_ref FROM linggan_comment_study_batch b
      JOIN linggan_model_invocation i ON i.invocation_ref=b.model_invocation_ref WHERE i.state='running'
    UNION
    SELECT t.run_ref FROM linggan_comment_study_semantic_attempt a
      JOIN linggan_comment_study_target t USING(target_ref)
      JOIN linggan_model_invocation i ON i.invocation_ref=a.model_invocation_ref WHERE i.state='running'
    UNION
    SELECT t.run_ref FROM linggan_comment_study_resolution r
      JOIN linggan_comment_study_signal s USING(signal_ref)
      JOIN linggan_comment_study_target t USING(target_ref)
      JOIN linggan_model_invocation i ON i.invocation_ref=r.model_invocation_ref WHERE i.state='running'
    UNION
    SELECT t.run_ref FROM linggan_comment_study_problem_pair p
      JOIN linggan_comment_study_signal s ON s.signal_ref IN (p.first_signal_ref,p.second_signal_ref)
      JOIN linggan_comment_study_target t USING(target_ref)
      JOIN linggan_model_invocation i ON i.invocation_ref=p.model_invocation_ref WHERE i.state='running'
), history AS MATERIALIZED (
    SELECT t.target_ref,t.state,t.input_fingerprint,t.created_at,c.content_public_ref,c.comment_external_id,
        (r.dispatch_state='stopped' AND r.dispatch_reason='legacy_unrecorded'
         AND t.input_fingerprint IS NULL AND live.run_ref IS NULL) AS safely_stopped_legacy,
        (live.run_ref IS NOT NULL OR (t.state IN ('ready','queued','running')
         AND (r.dispatch_state IN ('enabled','paused') OR t.input_fingerprint IS NOT NULL
              OR r.dispatch_reason IS DISTINCT FROM 'legacy_unrecorded'))) AS in_progress
    FROM linggan_comment_study_target t JOIN linggan_material_comment c ON c.material_ref=t.source_ref
    JOIN latest l ON l.content_public_ref=c.content_public_ref AND l.comment_external_id=c.comment_external_id
    JOIN linggan_comment_study_run r ON r.run_ref=t.run_ref
    JOIN linggan_comment_study_policy p ON p.policy_ref=r.policy_ref AND p.domain_ref=$1
    LEFT JOIN live_runs live ON live.run_ref=r.run_ref
), last_attempt AS (
    SELECT DISTINCT ON (content_public_ref,comment_external_id) * FROM history
    ORDER BY content_public_ref,comment_external_id,created_at DESC,target_ref DESC
), active AS (
    SELECT content_public_ref,comment_external_id,bool_or(in_progress) AS in_progress FROM history GROUP BY 1,2
), facts AS MATERIALIZED (
    SELECT l.*,cache.source_ref AS cached_source_ref,cache.clean_state,cache.raw_sha256,
        cache.research_text,raw.body_text IS NOT NULL AS has_body,
        NULLIF(btrim(a.author_external_id),'') IS NULL AS work_author_unknown,
        NULLIF(btrim(l.author_external_id),'') IS NULL AS comment_author_unknown,
        CASE WHEN btrim(l.author_external_id)=btrim(a.author_external_id) THEN 'creator' ELSE 'reader' END AS voice_role,
        EXISTS(SELECT 1 FROM linggan_material_comment_restriction x
          WHERE x.content_public_ref=l.content_public_ref AND x.comment_external_id=l.comment_external_id) AS source_restricted,
        last.state AS last_state,last.input_fingerprint AS last_fingerprint,
        COALESCE(last.safely_stopped_legacy,false) AS safely_stopped_legacy,
        COALESCE(active.in_progress,false) AS in_progress,
        row_number() OVER(PARTITION BY l.content_public_ref ORDER BY l.created_at,l.material_ref) AS source_rank
    FROM latest l JOIN linggan_material_comment raw ON raw.material_ref=l.material_ref
    LEFT JOIN linggan_comment_study_clean_cache cache ON cache.source_ref=l.material_ref AND cache.cleaner_version=$3
    LEFT JOIN linggan_material_content_author a ON a.content_public_ref=l.content_public_ref
    LEFT JOIN last_attempt last ON last.content_public_ref=l.content_public_ref AND last.comment_external_id=l.comment_external_id
    LEFT JOIN active ON active.content_public_ref=l.content_public_ref AND active.comment_external_id=l.comment_external_id
), qualified AS MATERIALIZED (
    SELECT facts.*,/*SOURCE_ELIGIBILITY_CASE*/ AS exclusion_reason FROM facts
), contexts AS MATERIALIZED (
    /*SHARED_WORK_CONTEXT_QUERY*/
), output AS (
    SELECT 0 AS kind,0::bigint AS rank,w.public_ref AS work_ref,''::text AS comment_id,
        jsonb_build_object('workRef',w.public_ref,'fragments',c.fragments) AS payload
    FROM requested_works w JOIN contexts c ON c.work_ref=w.public_ref
    UNION ALL
    SELECT 1,q.source_rank,q.content_public_ref,q.comment_external_id,
        jsonb_build_object('commentKey',jsonb_build_object('workRef',q.content_public_ref,'commentExternalId',q.comment_external_id),
          'sourceRef',q.material_ref,'sourceRank',q.source_rank,'exclusionReason',q.exclusion_reason,
          'readable',NOT q.source_restricted AND q.body_state='KNOWN' AND q.has_body,
          'indexed',q.cached_source_ref IS NOT NULL,'inProgress',q.in_progress,
          'latest',CASE WHEN q.last_state IS NULL THEN NULL ELSE jsonb_build_object('state',q.last_state,
              'inputFingerprint',q.last_fingerprint,'legacyStoppedWithoutLiveInvocation',q.safely_stopped_legacy) END,
          'rawText',CASE WHEN q.exclusion_reason IS NULL AND NOT q.in_progress THEN left(raw.body_text,16001) ELSE NULL END,
          'rawSha256',q.raw_sha256,'researchText',CASE WHEN q.exclusion_reason IS NULL AND NOT q.in_progress THEN q.research_text ELSE NULL END,
          'parent',CASE WHEN q.parent_comment_external_id IS NULL OR q.clean_state<>'context' OR q.exclusion_reason IS NOT NULL OR q.in_progress THEN NULL
              ELSE jsonb_build_object('commentKey',jsonb_build_object('workRef',q.content_public_ref,'commentExternalId',q.parent_comment_external_id),
                  'sourceRef',parent.material_ref,'sourceState',CASE WHEN restricted.content_public_ref IS NOT NULL THEN 'restricted'
                    WHEN parent.body_state='KNOWN' AND parent.body_text IS NOT NULL THEN 'known' ELSE 'unknown' END,
                  'commentText',CASE WHEN restricted.content_public_ref IS NULL AND parent.body_state='KNOWN' THEN left(parent.body_text,16001) ELSE NULL END) END)
    FROM qualified q JOIN linggan_material_comment raw ON raw.material_ref=q.material_ref
    LEFT JOIN LATERAL (
        SELECT c.material_ref,c.body_state,c.body_text FROM linggan_material_comment c
        JOIN linggan_runtime_capture_package p USING(package_ref)
        WHERE q.exclusion_reason IS NULL AND NOT q.in_progress AND q.clean_state='context'
          AND c.content_public_ref=q.content_public_ref AND c.comment_external_id=q.parent_comment_external_id
          AND p.accepted_at<=$2::text::timestamptz AND c.created_at<=$2::text::timestamptz
        ORDER BY c.observed_at::timestamptz DESC,c.created_at DESC,c.material_ref DESC LIMIT 1
    ) parent ON true
    LEFT JOIN linggan_material_comment_restriction restricted
      ON restricted.content_public_ref=q.content_public_ref AND restricted.comment_external_id=q.parent_comment_external_id
)
SELECT kind,payload FROM output ORDER BY kind,rank,work_ref,comment_id COLLATE "C"
