SELECT jsonb_build_object('displayableCommentCount',count(*) FILTER(WHERE readable AND clean_state IN('direct','context') AND voice_role<>'creator'),'eligibleCommentCount',count(*) FILTER(WHERE exclusion_reason IS NULL),'rawReadableCommentCount',count(*) FILTER(WHERE readable AND voice_role<>'creator'),'studiedCommentCount',count(*) FILTER(WHERE latest_request_ref IS NOT NULL AND exclusion_reason IS NULL),'indexedCount',count(*) FILTER(WHERE cached_source_ref IS NOT NULL),'pendingCount',count(*) FILTER(WHERE readable AND cached_source_ref IS NULL)) AS counts,
-- The eligible comments split into four exclusive parts, in this precedence: a current result
-- (the directory's "studied" filter), research in progress, never researched, otherwise the
-- latest attempt ended without a result (failed or cancelled).
jsonb_build_object(
  'withResult',count(*) FILTER(WHERE exclusion_reason IS NULL AND effective_request_ref IS NOT NULL),
  'running',count(*) FILTER(WHERE exclusion_reason IS NULL AND effective_request_ref IS NULL AND in_progress),
  'failedWithoutResult',count(*) FILTER(WHERE exclusion_reason IS NULL AND effective_request_ref IS NULL AND NOT in_progress AND latest_request_ref IS NOT NULL),
  'never',count(*) FILTER(WHERE exclusion_reason IS NULL AND effective_request_ref IS NULL AND NOT in_progress AND latest_request_ref IS NULL)
) AS research_breakdown
FROM qualified
