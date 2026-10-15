-- COMMENT-RESULTS-V3-001: one read-only page projection. All filtering, counts and
-- ordering precede pagination. Re-analysis of a comment is not a new comment.
WITH scoped AS MATERIALIZED (
  SELECT p.problem_ref,p.created_at,r.revision_ref,r.identity_version,
         r.created_at AS revised_at,r.title,r.definition,r.core_frame,
         cardinality(r.seed_signal_refs)>0 AND NOT EXISTS (
           SELECT 1 FROM unnest(r.seed_signal_refs) seed(signal_ref)
           LEFT JOIN linggan_comment_study_signal s ON s.signal_ref=seed.signal_ref
           LEFT JOIN linggan_comment_study_target t ON t.target_ref=s.target_ref
           LEFT JOIN linggan_comment_study_run run ON run.run_ref=t.run_ref
           LEFT JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref
           LEFT JOIN linggan_material_comment c ON c.material_ref=t.source_ref
           WHERE c.material_ref IS NULL OR c.body_state<>'KNOWN' OR c.body_text IS NULL
             OR policy.domain_ref IS DISTINCT FROM p.domain_ref
             OR EXISTS (SELECT 1 FROM linggan_material_comment_restriction x
                WHERE x.content_public_ref=c.content_public_ref
                  AND x.comment_external_id=c.comment_external_id)
             OR EXISTS (SELECT 1 FROM linggan_material_comment parent
                JOIN linggan_material_comment_restriction x
                  ON x.content_public_ref=parent.content_public_ref
                 AND x.comment_external_id=parent.comment_external_id
                WHERE parent.material_ref=t.parent_source_ref)
         ) AS readable,
         cardinality(r.seed_signal_refs)>0 AND NOT EXISTS (
           SELECT 1 FROM unnest(r.seed_signal_refs) seed(signal_ref)
           WHERE NOT EXISTS (SELECT 1 FROM linggan_comment_study_effective_signal s
             WHERE s.signal_ref=seed.signal_ref AND s.domain_ref=p.domain_ref
               AND s.eligibility_state='eligible')
         ) AS definition_current
  FROM linggan_comment_study_problem p
  JOIN linggan_comment_study_problem_revision r
    ON r.revision_ref=p.current_revision_ref AND r.domain_ref=p.domain_ref
  WHERE p.domain_ref=$1 AND p.state IN ('active','support_insufficient')
    AND p.created_at<=to_timestamp($2::double precision/1000000)
    AND r.created_at<=to_timestamp($2::double precision/1000000)
), first_link AS MATERIALIZED (
  SELECT m.problem_ref,c.content_public_ref,c.comment_external_id,min(m.created_at) AS added_at
  FROM scoped p JOIN linggan_comment_study_problem_membership m USING(problem_ref)
  JOIN linggan_comment_study_signal s USING(signal_ref)
  JOIN linggan_comment_study_target t ON t.target_ref=s.target_ref
  JOIN linggan_comment_study_run run ON run.run_ref=t.run_ref
  JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref
  JOIN linggan_material_comment c ON c.material_ref=t.source_ref
  WHERE policy.domain_ref=$1 AND m.created_at<=to_timestamp($2::double precision/1000000)
  GROUP BY m.problem_ref,c.content_public_ref,c.comment_external_id
), support AS MATERIALIZED (
  SELECT DISTINCT m.problem_ref,s.content_public_ref,s.comment_external_id,
         content.platform,c.author_external_id,f.added_at
  FROM scoped p JOIN linggan_comment_study_problem_membership m USING(problem_ref)
  JOIN linggan_comment_study_effective_signal s USING(signal_ref)
  JOIN linggan_comment_study_target t ON t.target_ref=s.target_ref
  JOIN linggan_comment_study_current_comment c
    ON c.content_public_ref=s.content_public_ref AND c.comment_external_id=s.comment_external_id
  JOIN linggan_material_content content ON content.public_ref=s.content_public_ref
  JOIN first_link f ON f.problem_ref=m.problem_ref AND f.content_public_ref=s.content_public_ref
    AND f.comment_external_id=s.comment_external_id
  WHERE s.domain_ref=$1 AND s.eligibility_state='eligible'
    AND m.created_at<=to_timestamp($2::double precision/1000000)
    AND c.body_text IS NOT NULL AND btrim(c.body_text)<>''
    AND NOT EXISTS (SELECT 1 FROM linggan_material_comment_restriction x
      WHERE x.content_public_ref=s.content_public_ref AND x.comment_external_id=s.comment_external_id)
    AND NOT EXISTS (SELECT 1 FROM linggan_material_comment parent
      JOIN linggan_material_comment_restriction x ON x.content_public_ref=parent.content_public_ref
        AND x.comment_external_id=parent.comment_external_id
      WHERE parent.material_ref=t.parent_source_ref)
), counts AS (
  SELECT problem_ref,count(*) AS comments,
    count(DISTINCT (platform,author_external_id)) FILTER (
      WHERE author_external_id IS NOT NULL AND btrim(author_external_id)<>'') AS accounts,
    count(DISTINCT content_public_ref) AS works,max(added_at) AS added_at,
    count(*) FILTER (WHERE added_at>=to_timestamp($2::double precision/1000000)-interval '28 days') AS recent
  FROM support GROUP BY problem_ref
), catalog AS (
  SELECT p.*,coalesce(c.comments,0) AS comments,coalesce(c.accounts,0) AS accounts,
    coalesce(c.works,0) AS works,coalesce(c.recent,0) AS recent,c.added_at,
    greatest(p.created_at,CASE WHEN p.readable THEN p.revised_at END,c.added_at) AS updated_at,
    CASE WHEN p.readable AND p.identity_version>1
       AND p.revised_at>=to_timestamp($2::double precision/1000000)-interval '28 days'
      THEN 'definition_revision'
      WHEN p.created_at>=to_timestamp($2::double precision/1000000)-interval '28 days' THEN 'first_record'
      WHEN c.recent>0 THEN 'evidence_added' ELSE 'unchanged' END AS change_kind
  FROM scoped p LEFT JOIN counts c USING(problem_ref)
  WHERE $3='' OR (p.readable AND
    (p.title ILIKE '%'||$3||'%' ESCAPE E'\\' OR p.definition ILIKE '%'||$3||'%' ESCAPE E'\\'))
), ranked AS (
  SELECT *,CASE $5 WHEN 'support' THEN comments
    WHEN 'created' THEN floor(extract(epoch FROM created_at)*1000000)::bigint
    ELSE floor(extract(epoch FROM updated_at)*1000000)::bigint END AS sort_key
  FROM catalog WHERE $4='all' OR change_kind<>'unchanged'
), page AS MATERIALIZED (
  SELECT * FROM ranked WHERE $6::bigint IS NULL OR sort_key<$6
    OR (sort_key=$6 AND problem_ref<$7::uuid)
  ORDER BY sort_key DESC,problem_ref DESC LIMIT $8
), previews AS (
  SELECT DISTINCT ON (s.problem_ref) s.*,c.body_text,c.author_display_name
  FROM support s JOIN page p USING(problem_ref)
  JOIN linggan_comment_study_current_comment c
    ON c.content_public_ref=s.content_public_ref AND c.comment_external_id=s.comment_external_id
  ORDER BY s.problem_ref,s.added_at DESC,s.content_public_ref,s.comment_external_id
)
SELECT jsonb_build_object(
  'scopeRevision',(SELECT md5(coalesce(string_agg(
    problem_ref::text||':'||sort_key::text||':'||revision_ref::text,',' ORDER BY problem_ref),'')) FROM ranked),
  'totalCount',(SELECT count(*) FROM ranked),
  'scopeCounts',jsonb_build_object('all',(SELECT count(*) FROM catalog),
    'changes',(SELECT count(*) FROM catalog WHERE change_kind<>'unchanged')),
  'problems',coalesce((SELECT jsonb_agg(jsonb_build_object(
    'problemRef',p.problem_ref,'revisionRef',p.revision_ref,'identityVersion',p.identity_version,
    'title',CASE WHEN p.readable THEN p.title END,
    'definition',CASE WHEN p.readable THEN p.definition END,
    'stableIdentity',CASE WHEN p.readable THEN p.core_frame END,
    'definitionReadable',p.readable,'definitionCurrent',p.definition_current,
    'supportCommentCount',p.comments,'supportAuthorCount',p.accounts,'supportWorkCount',p.works,
    'recentAddedSupportCommentCount',p.recent,'changeKind',p.change_kind,
    'createdAt',p.created_at,'revisionCreatedAt',CASE WHEN p.readable THEN p.revised_at END,
    'recentAddedAt',p.added_at,'updatedAt',p.updated_at,'sortKey',p.sort_key,
    'preview',CASE WHEN v.comment_external_id IS NOT NULL THEN jsonb_build_object(
      'commentKey',jsonb_build_object('workRef',v.content_public_ref,'commentExternalId',v.comment_external_id),
      'commentText',v.body_text,'authorDisplayName',v.author_display_name,
      'sourceState','known','addedAt',v.added_at) END
  ) ORDER BY p.sort_key DESC,p.problem_ref DESC) FROM page p LEFT JOIN previews v USING(problem_ref)), '[]'::jsonb)
)
