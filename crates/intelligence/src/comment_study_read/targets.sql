-- One Run, independently counted and cursor-paged. Never return derived text past a restriction.
WITH scoped AS MATERIALIZED (
    SELECT target.target_ref,target.source_ref,target.parent_source_ref,target.research_text,
           target.dependency_state,target.state,target.exclusion_reason,target.created_at,
           work.context_state,work.content_public_ref,
           comment.body_text,comment.body_state,
           EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction
             WHERE restriction.content_public_ref=comment.content_public_ref
               AND restriction.comment_external_id=comment.comment_external_id) AS source_restricted
    FROM linggan_comment_study_target target
    JOIN linggan_comment_study_work work ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref
    JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref
    WHERE target.run_ref=$1 AND target.created_at<=$2::timestamptz
), page AS MATERIALIZED (
    SELECT * FROM scoped WHERE $3::timestamptz IS NULL OR (created_at,target_ref)>($3::timestamptz,$4::uuid)
    ORDER BY created_at,target_ref LIMIT $5
)
SELECT jsonb_build_object('totalCount',(SELECT count(*) FROM scoped),'rows',
    COALESCE((SELECT jsonb_agg(jsonb_build_object(
      'position',jsonb_build_object('createdAt',to_char(page.created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),'reference',page.target_ref),
      'item',jsonb_build_object('targetRef',page.target_ref,'sourceRef',page.source_ref,
        'parentSourceRef',page.parent_source_ref,'workRef',page.content_public_ref,
        'sourceState',CASE WHEN source_restricted THEN 'restricted' WHEN body_state='KNOWN' AND body_text IS NOT NULL THEN 'known' ELSE 'unknown' END,
        'commentText',CASE WHEN NOT source_restricted AND body_state='KNOWN' THEN body_text ELSE NULL END,
        'researchText',CASE WHEN NOT source_restricted AND body_state='KNOWN' THEN research_text ELSE NULL END,
        'dependencyState',dependency_state,'contextState',context_state,'state',state,
        'exclusionReason',exclusion_reason,'createdAt',page.created_at,
        'signalCount',(SELECT count(*) FROM linggan_comment_study_signal signal WHERE signal.target_ref=page.target_ref),
        'resolutionState',(SELECT resolution.state FROM linggan_comment_study_resolution resolution JOIN linggan_comment_study_signal signal USING(signal_ref)
            WHERE signal.target_ref=page.target_ref ORDER BY resolution.created_at DESC LIMIT 1)
      )) ORDER BY page.created_at,page.target_ref) FROM page),'[]'::jsonb));
