-- Appended after shared catalog facts and Evidence's cs_display_titles CTE.
-- $6 is a literal title pattern; $7 study filter; $8 work-after; $9 limit+1.
, work_counts AS (
    SELECT content_public_ref AS work_ref,
           count(*) FILTER (WHERE exclusion_reason IS NULL) AS eligible,
           count(*) FILTER (WHERE readable AND cached_source_ref IS NOT NULL) AS indexed,
           count(*) FILTER (WHERE readable AND cached_source_ref IS NULL) AS pending,
           count(*) FILTER (WHERE readable AND clean_state IN ('direct','context')
                            AND effective_target_ref IS NOT NULL) AS studied,
           count(*) FILTER (WHERE readable AND clean_state IN ('direct','context') AND in_progress) AS in_progress,
           count(*) FILTER (WHERE readable AND clean_state IN ('direct','context')
                            AND latest_state = 'needs_context') AS needs_context,
           count(*) FILTER (WHERE readable AND clean_state IN ('direct','context')
                            AND latest_state = 'failed') AS failed,
           count(*) FILTER (WHERE exclusion_reason IS NULL AND latest_target_ref IS NULL) AS never_studied,
           max(latest_created_at) AS last_study_at
    FROM qualified GROUP BY content_public_ref
), work_rows AS MATERIALIZED (
    SELECT scope.work_ref, scope.work_created_at,
           title.display_title, title.display_title_source,
           COALESCE(counts.eligible,0) AS eligible, COALESCE(counts.indexed,0) AS indexed,
           COALESCE(counts.pending,0) AS pending, COALESCE(counts.studied,0) AS studied,
           COALESCE(counts.in_progress,0) AS in_progress,
           COALESCE(counts.needs_context,0) AS needs_context,
           COALESCE(counts.failed,0) AS failed, COALESCE(counts.never_studied,0) AS never_studied,
           counts.last_study_at
    FROM cs_title_scope scope
    JOIN cs_display_titles title USING (work_ref)
    LEFT JOIN work_counts counts USING (work_ref)
), matching_works AS MATERIALIZED (
    SELECT * FROM work_rows
    WHERE ($6::text IS NULL OR display_title ILIKE $6 ESCAPE E'\\')
      AND CASE $7
          WHEN 'all' THEN true
          WHEN 'never_studied' THEN never_studied > 0
          WHEN 'in_progress' THEN in_progress > 0
          WHEN 'studied' THEN studied > 0
          WHEN 'needs_context' THEN needs_context > 0
          WHEN 'failed' THEN failed > 0
          ELSE false
      END
), work_page AS (
    SELECT * FROM matching_works
    WHERE $8::uuid IS NULL OR work_ref > $8
    ORDER BY work_ref ASC LIMIT $9
), work_totals AS (
    SELECT count(*) AS total_works, COALESCE(sum(indexed),0)::bigint AS indexed,
           COALESCE(sum(pending),0)::bigint AS pending FROM matching_works
)
SELECT jsonb_build_object(
    'totalWorkCount', (SELECT total_works FROM work_totals),
    'indexCoverage', (SELECT jsonb_build_object(
        'state', CASE WHEN pending = 0 THEN 'ready' ELSE 'partial' END,
        'indexedCount', indexed, 'pendingCount', pending, 'asOf', $2::text
    ) FROM work_totals),
    'rows', COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'position', jsonb_build_object(
            'createdAt', to_char(work_created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'reference', work_ref
        ),
        'item', jsonb_build_object(
            'workRef', work_ref, 'displayTitle', COALESCE(display_title,'未命名作品'),
            'displayTitleSource', display_title_source,
            'eligibleCommentCount', eligible, 'indexedCommentCount', indexed,
            'pendingIndexCount', pending, 'studiedCommentCount', studied,
            'inProgressCommentCount', in_progress, 'needsContextCount', needs_context,
            'lastStudyAt', to_char(last_study_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
        )
    ) ORDER BY work_ref) FROM work_page),'[]'::jsonb)
);
