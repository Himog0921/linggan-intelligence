//! Revision is a deterministic UI change detector (two order-independent 64-bit hashes);
//! authorization and prepared manifests still use exact SHA-256 source fingerprints.
//! Bound, domain-scoped research projection. All user input is supplied as parameters.
// All fields and table names are compile-time SQL. Caller input is bound, including text search.
pub(super) const SCOPED: &str = r#"
WITH config AS (
 SELECT d.domain_ref,d.name,d.is_own_domain,COALESCE($2::text::timestamptz,scope_001_now()-make_interval(days=>$4)) AS start_at,
 COALESCE($3::text::timestamptz,scope_001_now()) AS end_at,scope_001_now() AS as_of
 FROM observation_domain d WHERE d.domain_ref=COALESCE($1,(SELECT domain_ref FROM observation_domain WHERE is_own_domain))
), active_rule AS MATERIALIZED (
 SELECT revision.rule_revision_ref,revision.canonical_hash,revision.schema_version,revision.selector_version
 FROM linggan_comment_research_rule_active active
 JOIN linggan_comment_research_rule_revision revision USING(rule_revision_ref)
), current_eligibility AS MATERIALIZED (
 -- The local reconcile is the only place that builds context fragments. A page may only consume
 -- an eligibility row under the same active semantic program, never infer current coverage from
 -- a raw analysis source hash alone.
 SELECT eligibility.*
 FROM linggan_comment_research_eligibility_current eligibility
 JOIN active_rule rule
   ON rule.rule_revision_ref=eligibility.rule_revision_ref
  AND rule.canonical_hash=eligibility.rule_hash
  AND rule.schema_version=eligibility.schema_version
  AND rule.selector_version=eligibility.selector_version
), clean_latest AS MATERIALIZED (
 SELECT source_ref,source_sha256,state,result,cleaner_version,true AS own_domain FROM linggan_comment_clean WHERE cleaner_version='comment-clean.v2' UNION ALL SELECT source_ref,source_sha256,state,result,cleaner_version,false AS own_domain FROM cross_industry_comment_clean WHERE cleaner_version='comment-clean.v2'
), operation_latest AS MATERIALIZED (
 SELECT DISTINCT ON(m.content_public_ref,m.comment_external_id) m.content_public_ref,m.comment_external_id,i.state AS item_state,i.failure_code,sw.state AS semantic_state,sw.invocation_ref
 FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) JOIN linggan_material_comment m ON m.material_ref=i.source_ref LEFT JOIN linggan_comment_semantic_work sw ON sw.semantic_ref=i.semantic_ref
 WHERE $16::uuid IS NULL OR i.batch_ref=$16
 ORDER BY m.content_public_ref,m.comment_external_id,b.created_at DESC,b.batch_ref DESC
), accepted_history AS MATERIALIZED (
 SELECT DISTINCT ON(m.content_public_ref,m.comment_external_id) m.content_public_ref,m.comment_external_id,a.work_ref AS analysis_ref
 FROM linggan_comment_analysis_work a JOIN linggan_material_comment m ON m.material_ref=a.source_ref
 WHERE a.state IN('succeeded','no_signal') AND a.result IS NOT NULL
   AND linggan_ci_analysis_context_readable(a.result)
 ORDER BY m.content_public_ref,m.comment_external_id,a.created_at DESC,a.work_ref DESC
), capture_provenance AS MATERIALIZED (
 SELECT m.material_ref AS canonical_ref,
 CASE WHEN receipt.execution_effect='COMPLETED_LIVE_STEP' AND receipt.material_admission='ACCEPTED' AND lt.execution_state='completed' AND w.lane='patrol'
 AND EXISTS(SELECT 1 FROM jsonb_array_elements(p.coverage->'layers') l WHERE l->>'capability' IN('comments','replies') AND l->>'failed'='0' AND l->>'unknown'='0' AND l->>'notAttempted'='0' AND l->>'stoppedReason' IN('surface_ended','maximum_quota'))
 THEN w.target_ref::text||':'||r.rule_payload_digest||':'||w.max_works::text ELSE NULL END AS capture_signature,
 p.accepted_at-m.observed_at::timestamptz>interval '2 days' AS late_material
 FROM linggan_material_comment m JOIN linggan_runtime_capture_package p USING(package_ref)
 LEFT JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
 LEFT JOIN collection_work_order_lease_task lt ON lt.task_id=p.task_id
 LEFT JOIN collection_work_order_lease lease USING(lease_ref)
 LEFT JOIN collection_work_order w USING(work_order_ref)
 LEFT JOIN collection_monitor_rule_revision r ON r.rule_revision_ref=w.monitor_rule_revision_ref
), research_current AS MATERIALIZED (
 SELECT h.* FROM linggan_ci_source_research h JOIN config c USING(domain_ref)
), source AS MATERIALIZED (
 SELECT s.source_ref,s.canonical_ref,s.domain_ref,s.work_ref,s.is_reply,s.parent_external_id,s.comment_external_id,s.author_external_id,s.role,s.likes,s.published_at,s.published_at_text,s.first_observed_at,s.first_observed_known,s.last_observed_at,s.work_title,s.creator_display_name,s.source_sha256,CASE WHEN $20 AND a.result IS NULL THEN NULL ELSE s.body END AS body,a.result,
 COALESCE(a.state,CASE current.execution_state WHEN 'waiting_recovery' THEN 'context_missing' WHEN 'queued' THEN 'pending' WHEN 'running' THEN 'running' WHEN 'failed' THEN 'failed' END,op.item_state,'pending') AS analysis_state,
 op.failure_code,cp.capture_signature,COALESCE(cp.late_material,true) AS late_material,a.work_ref AS analysis_ref,
 COALESCE(current.last_accepted_analysis_ref,history.analysis_ref) AS last_accepted_analysis_ref,current.current_attempt_ref,
 COALESCE(current.result_state,CASE WHEN history.analysis_ref IS NOT NULL THEN 'outdated' ELSE 'unstudied' END) AS result_state,
 COALESCE(current.execution_state,CASE op.item_state WHEN 'failed' THEN 'failed' WHEN 'running' THEN 'running' WHEN 'pending' THEN 'queued' ELSE CASE WHEN history.analysis_ref IS NOT NULL THEN 'waiting_recovery' ELSE 'idle' END END) AS execution_state,
 COALESCE(current.reason_code,op.failure_code,CASE WHEN history.analysis_ref IS NOT NULL THEN 'semantic_input_changed' WHEN COALESCE(cl.state,'pending') IN('dropped','anomaly') THEN 'source_not_researchable' ELSE 'first_research' END) AS reason_code,
 split_part(a.model_version,':',1) AS model_version,a.rule_version,a.created_at AS analysis_at,
 COALESCE(cl.state,'pending') AS clean_state,cl.result->>'text' AS research_body,cl.cleaner_version AS cleaner_version,
 COALESCE(h.revision,0) AS revision,COALESCE(h.locked,false) AS locked,COALESCE(h.needs_context,false) AS needs_context,COALESCE(h.source_sha256<>s.source_sha256,false) AS source_changed,
 COALESCE(h.bookmarked,$21 AND c.is_own_domain AND EXISTS(SELECT 1 FROM linggan_comment_asset_current old JOIN linggan_material_comment om ON om.material_ref=old.source_ref WHERE om.content_public_ref=s.work_ref AND om.comment_external_id=s.comment_external_id AND NOT old.withdrawn)) AS bookmarked,
 CASE WHEN s.role='platform_system' THEN '[]'::jsonb WHEN COALESCE(h.labels_locked,false) THEN CASE WHEN h.source_sha256=s.source_sha256 THEN h.labels ELSE '[]'::jsonb END
 WHEN a.result IS NULL THEN '[]'::jsonb ELSE linggan_ci_research_labels(a.result) END AS labels
 FROM linggan_ci_source s JOIN config c ON s.domain_ref=c.domain_ref
 LEFT JOIN research_current h ON h.canonical_ref=s.canonical_ref AND h.domain_ref=s.domain_ref
 LEFT JOIN LATERAL (
   SELECT eligibility.* FROM current_eligibility eligibility
   WHERE $21 AND c.is_own_domain
     AND eligibility.source_identity=encode(sha256(convert_to(s.work_ref::text||':'||s.comment_external_id,'UTF8')),'hex')
     AND eligibility.source_ref=s.source_ref
     AND eligibility.source_sha256=s.source_sha256
     AND eligibility.source_context_revision=linggan_comment_research_context_revision(s.source_ref)
     AND EXISTS(SELECT 1 FROM linggan_comment_research_readable readable WHERE readable.material_ref=s.source_ref)
     AND (eligibility.current_analysis_ref IS NULL OR EXISTS(
       SELECT 1 FROM linggan_comment_analysis_work analysis
       WHERE analysis.work_ref=eligibility.current_analysis_ref
         AND analysis.state IN('succeeded','no_signal')
         AND linggan_ci_analysis_context_readable(analysis.result)
     ))
   LIMIT 1
 ) current ON true
 LEFT JOIN linggan_comment_analysis_work a ON $21 AND a.work_ref=current.current_analysis_ref
 LEFT JOIN accepted_history history ON $21 AND c.is_own_domain AND history.content_public_ref=s.work_ref AND history.comment_external_id=s.comment_external_id
 LEFT JOIN clean_latest cl ON cl.own_domain=c.is_own_domain AND cl.source_ref=s.source_ref AND cl.source_sha256=s.source_sha256
 LEFT JOIN operation_latest op ON $21 AND c.is_own_domain AND op.content_public_ref=s.work_ref AND op.comment_external_id=s.comment_external_id
 LEFT JOIN capture_provenance cp ON $21 AND c.is_own_domain AND cp.canonical_ref=s.canonical_ref
 WHERE cl.state IS DISTINCT FROM 'dropped' AND ($7::uuid IS NULL OR s.work_ref=$7)
 AND ($6::text='' OR strpos(lower(COALESCE(cl.result->>'text',s.body,'')),lower($6))>0)
 AND (cardinality($17::uuid[])=0 OR s.source_ref=ANY($17) OR s.canonical_ref=ANY($17))
 AND ($16::uuid IS NULL OR c.is_own_domain AND EXISTS(SELECT 1 FROM linggan_comment_daily_item i JOIN linggan_material_comment im ON im.material_ref=i.source_ref WHERE i.batch_ref=$16 AND im.content_public_ref=s.work_ref AND im.comment_external_id=s.comment_external_id))
), timed AS NOT MATERIALIZED (
 SELECT * FROM source s, LATERAL(SELECT CASE WHEN $5='published' THEN s.published_at ELSE s.first_observed_at END AS scope_time) t
 WHERE t.scope_time>=(SELECT start_at FROM config) AND t.scope_time<(SELECT end_at FROM config)
), memberships AS (
 SELECT m.*,p.name,p.meaning,p.revision,p.definition,p.origin AS problem_origin FROM linggan_ci_problem_member m
 JOIN linggan_ci_problem p USING(problem_ref) JOIN timed s USING(canonical_ref)
 WHERE p.domain_ref=(SELECT domain_ref FROM config) AND p.redirect_ref IS NULL
 AND linggan_ci_group_problem_member_current(m.problem_ref,m.canonical_ref,m.analysis_ref)
 AND ((m.origin='manual' AND EXISTS(SELECT 1 FROM linggan_ci_source_research h WHERE h.canonical_ref=s.canonical_ref AND h.domain_ref=s.domain_ref AND h.source_sha256=s.source_sha256)) OR (m.origin<>'manual' AND m.analysis_ref=s.analysis_ref))
), positions AS (
 SELECT s.canonical_ref,s.work_ref,st->>'target' AS target,st->>'position' AS position FROM timed s CROSS JOIN LATERAL jsonb_array_elements(linggan_ci_research_stances(s.result)) st
 WHERE s.result IS NOT NULL AND s.role NOT IN('platform_system','author') AND st->>'position' IN('support','oppose')
), resonance AS (
 SELECT p.target FROM positions p JOIN timed s USING(canonical_ref) WHERE p.position='support' AND s.clean_state NOT IN('anomaly','low_information')
 GROUP BY p.target HAVING count(DISTINCT s.canonical_ref)>=5 AND count(DISTINCT s.work_ref)>=3 AND count(DISTINCT regexp_replace(s.body,'[[:space:][:punct:]]','','g'))>=5
), conflicts AS (
 SELECT p.target FROM positions p GROUP BY p.target HAVING count(DISTINCT p.canonical_ref) FILTER(WHERE p.position='support')>=2
 AND count(DISTINCT p.canonical_ref) FILTER(WHERE p.position='oppose')>=2 AND (count(DISTINCT p.work_ref)>=2 OR EXISTS(
 SELECT 1 FROM positions a JOIN positions b ON a.target=b.target JOIN timed sa ON sa.canonical_ref=a.canonical_ref JOIN timed sb ON sb.canonical_ref=b.canonical_ref
 WHERE a.target=p.target AND a.position='support' AND b.position='oppose' AND sa.work_ref=sb.work_ref AND (sa.parent_external_id=sb.comment_external_id OR sb.parent_external_id=sa.comment_external_id)))
), enriched AS NOT MATERIALIZED (
 SELECT s.*,CASE WHEN s.result IS NULL AND NOT s.locked THEN s.labels ELSE s.labels || CASE WHEN EXISTS(SELECT 1 FROM positions p JOIN resonance USING(target) WHERE p.canonical_ref=s.canonical_ref AND p.position='support') THEN '["resonance"]'::jsonb ELSE '[]' END
 || CASE WHEN EXISTS(SELECT 1 FROM positions p JOIN conflicts USING(target) WHERE p.canonical_ref=s.canonical_ref) THEN '["conflict"]'::jsonb ELSE '[]' END END AS all_labels,
 CASE WHEN s.result IS NULL AND NOT s.locked THEN '[]'::jsonb ELSE COALESCE((SELECT jsonb_agg(DISTINCT m.problem_ref) FROM memberships m WHERE m.canonical_ref=s.canonical_ref),'[]') END AS problem_refs
 FROM timed s
), filtered AS MATERIALIZED (
 SELECT s.* FROM enriched s WHERE (cardinality($8::text[])=0 OR s.all_labels ?| $8)
 AND ($9::uuid IS NULL OR EXISTS(SELECT 1 FROM memberships m WHERE m.problem_ref=$9 AND m.canonical_ref=s.canonical_ref))
 AND ($10::text='' OR EXISTS(SELECT 1 FROM linggan_ci_term_index t WHERE t.canonical_ref=s.canonical_ref AND t.domain_ref=s.domain_ref AND t.term=$10 AND t.source_sha256=s.source_sha256))
 AND (NOT $11 OR s.bookmarked)
 AND ($18::text='' OR CASE
      WHEN $18='analyzed' THEN s.result IS NOT NULL
      WHEN $18 IN('unstudied','studied','no_signal','outdated') THEN s.result_state=$18
      WHEN $18 IN('idle','queued','running','failed','waiting_recovery') THEN s.execution_state=$18
      WHEN $18 IN('succeeded','no_signal') THEN s.analysis_state=$18 AND s.result IS NOT NULL
      WHEN $18='pending' THEN s.result IS NULL AND s.analysis_state='pending'
      ELSE s.clean_state=$18
    END)
), scoped_atoms AS MATERIALIZED (
 SELECT atom.*,source.work_ref,source.source_ref AS current_source_ref,
  NOT EXISTS(SELECT 1 FROM linggan_ci_atom_membership m JOIN linggan_ci_semantic_group g USING(group_ref) WHERE m.atom_ref=atom.atom_ref AND m.relation='same' AND m.current AND g.state='active') AS unclustered,
  EXISTS(SELECT 1 FROM linggan_ci_atom_vector v JOIN linggan_ci_semantic_space space USING(space_ref)
   JOIN linggan_model_entry model ON model.model_id=space.model_id
   JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref AND version.api=space.provider_api AND version.base_url=space.endpoint
   JOIN linggan_embedding_config cfg ON cfg.model_ref=model.model_ref AND cfg.dimensions=space.dimensions AND cfg.enabled AND cfg.qualified
   JOIN linggan_embedding_settings active ON active.config_ref=cfg.config_ref
   WHERE v.definition_hash=atom.definition_hash) AS embedded
 FROM linggan_ci_semantic_atom atom JOIN filtered source ON source.analysis_ref=atom.analysis_ref AND source.canonical_ref=atom.canonical_ref AND source.domain_ref=atom.domain_ref
 WHERE NOT $20 AND source.result IS NOT NULL
), totals AS (
 SELECT count(*) AS comments,(SELECT count(DISTINCT work_ref) FROM filtered) AS works,count(*) FILTER(WHERE result IS NOT NULL) AS analyzed,count(*) FILTER(WHERE result IS NULL AND analysis_state='pending') AS pending,
 count(*) FILTER(WHERE result_state='unstudied') AS result_unstudied,count(*) FILTER(WHERE result_state='studied') AS result_studied,count(*) FILTER(WHERE result_state='no_signal') AS result_no_signal,count(*) FILTER(WHERE result_state='outdated') AS result_outdated,
 count(*) FILTER(WHERE execution_state='idle') AS execution_idle,count(*) FILTER(WHERE execution_state='queued') AS execution_queued,count(*) FILTER(WHERE execution_state='running') AS execution_running,count(*) FILTER(WHERE execution_state='failed') AS execution_failed,count(*) FILTER(WHERE execution_state='waiting_recovery') AS execution_waiting_recovery,
 count(*) FILTER(WHERE role<>'platform_system' AND (result IS NOT NULL OR labels<>'[]'::jsonb OR clean_state IN('direct','context'))) AS eligible,
 bool_or(result IS NOT NULL OR labels<>'[]'::jsonb) AS research_available,count(*) FILTER(WHERE all_labels?'need') AS need_count,count(*) FILTER(WHERE all_labels?'solution') AS solution_count,count(*) FILTER(WHERE all_labels?'story') AS story_count,count(*) FILTER(WHERE all_labels?'quote') AS quote_count,count(*) FILTER(WHERE all_labels?'resonance') AS resonance_count,count(*) FILTER(WHERE all_labels?'conflict') AS conflict_count,
 md5(count(*)::text||COALESCE(bit_xor(hash_record_extended(ROW(canonical_ref,source_sha256,analysis_ref,revision,likes,analysis_state,clean_state,cleaner_version),0))::text,'')||COALESCE(bit_xor(hash_record_extended(ROW(canonical_ref,source_sha256,analysis_ref,revision,likes,analysis_state,clean_state,cleaner_version),1))::text,'')||COALESCE((SELECT string_agg(problem_ref::text||revision::text,',' ORDER BY problem_ref) FROM linggan_ci_problem WHERE domain_ref=(SELECT domain_ref FROM config)),'')||COALESCE((SELECT string_agg(term||revision::text,',' ORDER BY term) FROM linggan_ci_term_setting WHERE domain_ref=(SELECT domain_ref FROM config)),'')||COALESCE((SELECT max(updated_at)::text FROM linggan_ci_projection_cursor c JOIN source s ON c.canonical_ref=s.canonical_ref AND c.domain_ref=s.domain_ref),'')||COALESCE((SELECT revision::text FROM linggan_ci_rule_release WHERE domain_ref=(SELECT domain_ref FROM config)),'')) AS revision FROM filtered
), paged AS (
 SELECT f.*,CASE WHEN f.research_body IS NOT NULL THEN f.research_body WHEN f.body IS NOT NULL THEN f.body WHEN f.domain_ref=(SELECT domain_ref FROM observation_domain WHERE is_own_domain) THEN (SELECT m.body_text FROM linggan_material_comment m WHERE m.material_ref=f.source_ref) ELSE (SELECT x.body_text FROM cross_industry_comment x WHERE x.comment_ref=f.source_ref AND x.domain_ref=f.domain_ref) END AS display_body FROM (
 SELECT * FROM filtered WHERE ($19::uuid IS NULL OR source_ref=$19) ORDER BY CASE WHEN $12='likes' THEN likes END DESC NULLS LAST,first_observed_at DESC,canonical_ref LIMIT $14 OFFSET $13) f
), problem_counts AS (
 SELECT m.problem_ref,m.name,m.meaning,m.revision,m.definition,count(DISTINCT s.canonical_ref) AS comments,count(DISTINCT s.work_ref) AS works,
 (array_agg(s.source_ref ORDER BY s.likes DESC NULLS LAST,s.canonical_ref))[1] AS representative_ref,
 (array_agg(left(COALESCE(s.research_body,s.body),180) ORDER BY s.likes DESC NULLS LAST,s.canonical_ref))[1] AS representative_body
 FROM memberships m JOIN filtered s USING(canonical_ref) GROUP BY m.problem_ref,m.name,m.meaning,m.revision,m.definition
 HAVING count(DISTINCT s.canonical_ref)>=3 OR bool_or(m.problem_origin='manual') OR m.definition->>'lifecycle'='emerging'
), candidate_counts AS (
 SELECT c.definition_key,min(c.candidate_ref::text) AS candidate_ref,(array_agg(DISTINCT c.candidate_ref))[1:1000] AS candidate_refs,min(c.name) AS name,min(c.meaning) AS meaning,
 count(DISTINCT s.canonical_ref) AS comments,count(DISTINCT s.work_ref) AS works,
 (array_agg(DISTINCT s.source_ref))[1:1000] AS refs,(jsonb_agg((SELECT jsonb_agg(e||jsonb_build_object('quote',substring(s.body FROM (e->>'startChar')::integer+1 FOR (e->>'endChar')::integer-(e->>'startChar')::integer))) FROM jsonb_array_elements(c.evidence) e) ORDER BY c.candidate_ref)->0) AS evidence
 FROM linggan_ci_problem_candidate c JOIN filtered s ON s.canonical_ref=c.canonical_ref AND s.analysis_ref=c.analysis_ref AND s.domain_ref=c.domain_ref
 WHERE c.state='unmerged' AND s.result IS NOT NULL AND s.role<>'platform_system'
 GROUP BY c.definition_key
), term_counts AS (
 SELECT t.term,count(DISTINCT s.canonical_ref) AS count,count(DISTINCT s.work_ref) AS works,
 (array_agg(DISTINCT s.source_ref))[1:1000] AS refs
 FROM linggan_ci_term_index t JOIN filtered s USING(canonical_ref)
 WHERE EXISTS(SELECT 1 FROM linggan_ci_term_index available WHERE available.domain_ref=(SELECT domain_ref FROM config)) AND t.source_sha256=s.source_sha256 AND t.domain_ref=s.domain_ref AND s.role<>'platform_system'
 AND NOT EXISTS(SELECT 1 FROM linggan_ci_term_setting h WHERE h.domain_ref=s.domain_ref AND h.term=t.term AND h.hidden)
 GROUP BY t.term ORDER BY count DESC,t.term LIMIT 30
), daily_trend AS (
 SELECT to_char((scope_time AT TIME ZONE 'Asia/Shanghai')::date,'YYYY-MM-DD') AS day,count(*) AS comments,count(DISTINCT work_ref) AS works,
 count(*) FILTER(WHERE result IS NOT NULL) AS analyzed,count(*) FILTER(WHERE role<>'platform_system' AND (result IS NOT NULL OR labels<>'[]'::jsonb OR clean_state IN('direct','context'))) AS eligible,
 bool_or(result IS NOT NULL OR labels<>'[]'::jsonb) research_available,count(*) FILTER(WHERE all_labels?'solution') solutions,count(*) FILTER(WHERE all_labels?'story') stories,count(*) FILTER(WHERE all_labels?'quote') quotes,count(*) FILTER(WHERE all_labels?'need') AS needs,count(*) FILTER(WHERE all_labels?'resonance') AS resonance,count(*) FILTER(WHERE all_labels?'conflict') AS conflict
 FROM filtered GROUP BY (scope_time AT TIME ZONE 'Asia/Shanghai')::date ORDER BY day
)
"#;
pub(super) const READ: &str = r#"
SELECT jsonb_build_object(
 'scope',jsonb_build_object('domain',c.domain_ref,'domainName',c.name,'from',c.start_at,'to',c.end_at,'timeBasis',$5,'asOf',c.as_of,'resultRevision',t.revision,'updated',$15<>'' AND $15<>t.revision,'days',$4),
 'summary',jsonb_build_object('comments',t.comments,'works',t.works,'analyzed',t.analyzed,'eligible',t.eligible,'pending',t.pending,
 'resultStates',jsonb_build_object('unstudied',t.result_unstudied,'studied',t.result_studied,'noSignal',t.result_no_signal,'outdated',t.result_outdated),
 'executionStates',jsonb_build_object('idle',t.execution_idle,'queued',t.execution_queued,'running',t.execution_running,'failed',t.execution_failed,'waitingRecovery',t.execution_waiting_recovery),
 'excludedPublished',CASE WHEN $5='published' THEN (SELECT count(*) FROM source WHERE published_at IS NULL) ELSE 0 END,
 'lenses',jsonb_build_object('need',CASE WHEN t.research_available THEN t.need_count ELSE NULL END,'solution',CASE WHEN t.research_available THEN t.solution_count ELSE NULL END,'story',CASE WHEN t.research_available THEN t.story_count ELSE NULL END,'quote',CASE WHEN t.research_available THEN t.quote_count ELSE NULL END,'resonance',CASE WHEN t.research_available THEN t.resonance_count ELSE NULL END,'conflict',CASE WHEN t.research_available THEN t.conflict_count ELSE NULL END)),
 'page',jsonb_build_object('items',COALESCE((SELECT jsonb_agg(jsonb_build_object(
 'sourceRef',source_ref,'sourceSha256',source_sha256,'canonicalRef',canonical_ref,'workRef',work_ref,'body',left(display_body,600),'bodyTruncated',length(display_body)>600,
 'isReply',is_reply,'likes',likes,'publishedAt',published_at,'publishedAtText',published_at_text,'firstObservedAt',first_observed_at,'firstObservedKnown',first_observed_known,'lastObservedAt',last_observed_at,
 'workTitle',work_title,'creatorDisplayName',creator_display_name,'labels',all_labels,'problemRefs',problem_refs,'cleanState',clean_state,'cleanerVersion',cleaner_version,
 'analysisState',analysis_state,'analysisRef',analysis_ref,'resultState',result_state,'executionState',execution_state,'lastAcceptedAnalysisRef',last_accepted_analysis_ref,'currentAttemptRef',current_attempt_ref,'reasonCode',reason_code,'modelVersion',model_version,'ruleVersion',rule_version,'failureCode',failure_code,'hasAnalysis',result IS NOT NULL,'sourceChanged',source_changed,'bookmarked',bookmarked,'bookmarkRevision',revision,'researchRevision',revision,'research',jsonb_build_object('revision',revision),'role',role)) FROM paged),'[]'),'total',t.comments,'offset',$13,'limit',$14),
 'works',COALESCE((SELECT jsonb_agg(jsonb_build_object('workRef',work_ref,'title',title)) FROM (SELECT work_ref,max(work_title) title FROM source GROUP BY work_ref ORDER BY work_ref LIMIT 200) w),'[]'),
 'terms',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('term',term,'count',count,'works',works,'sourceRefs',refs,'revision',COALESCE((SELECT h.revision FROM linggan_ci_term_setting h WHERE h.domain_ref=c.domain_ref AND h.term=term_counts.term),0))) FROM term_counts),'[]') END,
 'problems',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('problemRef',problem_ref,'name',name,'meaning',meaning,'revision',revision,'definition',definition,'lifecycle',definition->>'lifecycle','comments',comments,'works',works,'representative',jsonb_build_object('sourceRef',representative_ref,'body',representative_body),'change',NULL) ORDER BY comments DESC,problem_ref) FROM problem_counts),'[]') END,
 'problemCandidates',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('candidateRef',candidate_ref,'candidateRefs',candidate_refs,'name',name,'meaning',meaning,'sourceRefs',refs,'comments',comments,'works',works,'evidence',evidence,'state','unmerged') ORDER BY comments DESC,candidate_ref) FROM (SELECT * FROM candidate_counts ORDER BY comments DESC,candidate_ref LIMIT 100) candidates),'[]') END,
 'candidateTotal',CASE WHEN $20 THEN NULL ELSE (SELECT count(*) FROM candidate_counts) END,
 'problemSummary',CASE WHEN $20 THEN NULL ELSE jsonb_build_object(
 'analyzedComments',t.analyzed,
 'expressionCount',(SELECT count(*) FROM linggan_ci_problem_candidate pc JOIN filtered f ON f.canonical_ref=pc.canonical_ref AND f.domain_ref=pc.domain_ref AND f.analysis_ref=pc.analysis_ref WHERE pc.state<>'superseded'),
 'unmergedExpressions',(SELECT count(*) FROM linggan_ci_problem_candidate pc JOIN filtered f ON f.canonical_ref=pc.canonical_ref AND f.domain_ref=pc.domain_ref AND f.analysis_ref=pc.analysis_ref WHERE pc.state='unmerged'),
 'needsJudgment',(SELECT count(DISTINCT pc.candidate_ref) FROM linggan_ci_problem_candidate pc JOIN filtered f ON f.canonical_ref=pc.canonical_ref AND f.domain_ref=pc.domain_ref AND f.analysis_ref=pc.analysis_ref JOIN linggan_ci_problem_task pt USING(candidate_ref) WHERE pc.state='unmerged' AND pt.state='needs_judgment'),
 'emergingProblems',(SELECT count(*) FROM problem_counts WHERE definition->>'lifecycle'='emerging'),
 'stableProblems',(SELECT count(*) FROM problem_counts WHERE definition->>'lifecycle'='stable'),
 'unclassifiedProblems',(SELECT count(*) FROM problem_counts WHERE COALESCE(definition->>'lifecycle','') NOT IN ('emerging','stable'))) END,

 'representatives',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('sourceRef',source_ref,'workRef',work_ref,'body',left(COALESCE(research_body,body),600),'labels',all_labels,'hasAnalysis',true,'analysisState',analysis_state,'likes',likes,'bookmarked',bookmarked,'bookmarkRevision',revision)) FROM (SELECT * FROM filtered WHERE result IS NOT NULL ORDER BY analysis_at DESC,canonical_ref LIMIT 4) representatives),'[]') END,
 'trend',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('day',day,'comments',comments,'eligible',eligible,'analyzed',analyzed,'workCount',works,'researchAvailable',research_available,'lenses',jsonb_build_object('solution',solutions,'story',stories,'quote',quotes,'need',needs,'resonance',resonance,'conflict',conflict),'partial',day=to_char(c.as_of AT TIME ZONE 'Asia/Shanghai','YYYY-MM-DD'))) FROM daily_trend),'[]') END,
  'comparisonWindows',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object(
 'problemRef',pc.problem_ref,'name',pc.name,'period',periods.period,'captureSignature',periods.capture_signature,'unknownCapture',periods.unknown_capture,'classificationChanges',periods.classification_changes,'organizationChanges',organization.changes,'semanticSignature',organization.signature,'organizationEligible',periods.organization_eligible,'organized',periods.organized,'eligible',periods.eligible,'analyzed',periods.analyzed,'comments',periods.comments,'works',periods.works,'observedDays',periods.observed_days,'modelVersions',periods.model_versions,'ruleVersions',periods.rule_versions,'modelSignature',periods.model_signature,'ruleSignature',periods.rule_signature,'lateMaterial',periods.late_material,'sourceRefs',periods.refs,'workRefs',periods.work_refs,'firstProblemObservedAt',(SELECT min(s.first_observed_at) FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) WHERE m.problem_ref=pc.problem_ref AND s.domain_ref=c.domain_ref),'firstProblemIsNew',COALESCE((SELECT min(s.first_observed_at)>=date_trunc('day',c.as_of AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'-interval '7 days' FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) WHERE m.problem_ref=pc.problem_ref AND s.domain_ref=c.domain_ref),false),'windowStart',date_trunc('day',c.as_of AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'-CASE WHEN periods.period='current' THEN interval '7 days' ELSE interval '14 days' END))
 FROM problem_counts pc CROSS JOIN LATERAL (
 SELECT string_agg(DISTINCT g.group_ref::text||':'||g.revision::text||':'||space.space_hash||':'||run.policy_hash,',' ORDER BY g.group_ref::text||':'||g.revision::text||':'||space.space_hash||':'||run.policy_hash) signature,
 count(DISTINCT lineage.lineage_ref) changes
 FROM linggan_ci_semantic_group g
 JOIN linggan_ci_atom_membership membership ON membership.group_ref=g.group_ref AND membership.current AND membership.relation='same'
 JOIN linggan_ci_cluster_run run ON run.run_ref=membership.run_ref
 JOIN linggan_ci_semantic_space space ON space.space_ref=run.space_ref
 LEFT JOIN linggan_ci_semantic_lineage lineage ON (g.group_ref=ANY(lineage.from_groups) OR g.group_ref=ANY(lineage.to_groups))
  AND lineage.kind IN('rename','split','merge') AND lineage.created_at>=c.as_of-interval '14 days'
 WHERE g.problem_ref=pc.problem_ref AND g.state='active'
 ) organization CROSS JOIN LATERAL (
 SELECT CASE WHEN f.scope_time >= date_trunc('day',c.as_of AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'-interval '7 days' THEN 'current' ELSE 'previous' END period,
 count(*) FILTER(WHERE f.role<>'platform_system' AND (f.result IS NOT NULL OR f.labels<>'[]'::jsonb OR f.clean_state IN('direct','context'))) eligible,
 count(*) FILTER(WHERE f.result IS NOT NULL) analyzed,
 count(*) FILTER(WHERE EXISTS(SELECT 1 FROM scoped_atoms atom WHERE atom.canonical_ref=f.canonical_ref AND atom.kind='problem')) organization_eligible,
 count(*) FILTER(WHERE EXISTS(SELECT 1 FROM scoped_atoms atom WHERE atom.canonical_ref=f.canonical_ref AND atom.kind='problem')
   AND NOT EXISTS(SELECT 1 FROM scoped_atoms atom WHERE atom.canonical_ref=f.canonical_ref AND atom.kind='problem' AND atom.unclustered)) organized,
 count(*) FILTER(WHERE f.result IS NOT NULL AND f.problem_refs @> jsonb_build_array(pc.problem_ref)) comments,
 count(DISTINCT f.work_ref) FILTER(WHERE f.result IS NOT NULL AND f.problem_refs @> jsonb_build_array(pc.problem_ref)) works,
 count(DISTINCT to_char(f.scope_time AT TIME ZONE 'Asia/Shanghai','YYYY-MM-DD')) FILTER(WHERE f.capture_signature IS NOT NULL) observed_days,string_agg(DISTINCT f.capture_signature,',' ORDER BY f.capture_signature) capture_signature,count(*) FILTER(WHERE f.capture_signature IS NULL) unknown_capture,
 count(*) FILTER(WHERE EXISTS(SELECT 1 FROM linggan_ci_source_revision h WHERE h.canonical_ref=f.canonical_ref AND h.kind IN('correct','problem_create','problem_merge','problem_split','undo') AND h.created_at>=c.as_of-interval '14 days')) classification_changes,
 count(DISTINCT f.model_version) model_versions,count(DISTINCT f.rule_version) rule_versions,string_agg(DISTINCT f.model_version,',' ORDER BY f.model_version) model_signature,string_agg(DISTINCT f.rule_version,',' ORDER BY f.rule_version) rule_signature,
 count(*) FILTER(WHERE NOT f.first_observed_known OR f.late_material) late_material,
 (array_agg(f.source_ref) FILTER(WHERE f.result IS NOT NULL AND f.problem_refs @> jsonb_build_array(pc.problem_ref)))[1:1000] refs,(array_agg(DISTINCT f.work_ref) FILTER(WHERE f.result IS NOT NULL AND f.problem_refs @> jsonb_build_array(pc.problem_ref)))[1:1000] work_refs
 FROM filtered f WHERE f.scope_time<date_trunc('day',c.as_of AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'
 AND f.scope_time>=date_trunc('day',c.as_of AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'-interval '14 days'
 GROUP BY period
 ) periods),'[]') END,
 'distribution',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('workRef',work_ref,'title',title,'comments',comments)) FROM (SELECT work_ref,max(work_title) title,count(*) comments FROM filtered GROUP BY work_ref ORDER BY comments DESC,work_ref LIMIT 200) ds),'[]') END,
 'stances',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('target',target,'position',position,'comments',comments,'sourceRefs',refs)) FROM (SELECT st->>'target' target,st->>'position' position,count(DISTINCT source_ref) comments,(array_agg(DISTINCT source_ref))[1:1000] refs FROM filtered CROSS JOIN LATERAL jsonb_array_elements(linggan_ci_research_stances(result)) st WHERE result IS NOT NULL GROUP BY st->>'target',st->>'position' ORDER BY comments DESC LIMIT 100) st),'[]') END,
 'problemOptions',COALESCE((SELECT jsonb_agg(jsonb_build_object('problemRef',problem_ref,'name',name)) FROM (SELECT DISTINCT problem_ref,name FROM memberships ORDER BY name,problem_ref LIMIT 200) opts),'[]'),
 'hiddenTerms',COALESCE((SELECT jsonb_agg(jsonb_build_object('term',term,'revision',revision)) FROM linggan_ci_term_setting WHERE domain_ref=c.domain_ref AND hidden),'[]'),
 'rules',jsonb_build_object('advancedReleaseEnabled',COALESCE((SELECT advanced_release_enabled FROM linggan_ci_rule_release WHERE domain_ref=c.domain_ref),false),'calibrationState',CASE WHEN EXISTS(SELECT 1 FROM linggan_ci_rule_release WHERE domain_ref=c.domain_ref AND advanced_release_enabled) THEN 'APPROVED' ELSE 'NOT_QUALIFIED' END),
 'groupObservations',CASE WHEN $20 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('type',kind,'target',target,'comments',comments,'works',works,'distinctExpressions',expressions,'sourceRefs',refs,'representative',representative)) FROM (
 SELECT g.kind,g.target,count(DISTINCT f.canonical_ref) comments,count(DISTINCT f.work_ref) works,count(DISTINCT regexp_replace(f.body,'[[:space:][:punct:]]','','g')) expressions,
 (array_agg(DISTINCT f.source_ref))[1:1000] refs,(array_agg(jsonb_build_object('sourceRef',f.source_ref,'body',left(COALESCE(f.research_body,f.body),180),'likes',f.likes) ORDER BY f.likes DESC NULLS LAST,f.canonical_ref))[1] representative
 FROM (SELECT target,'recurrence' AS kind FROM resonance UNION ALL SELECT target,'disagreement' FROM conflicts) g JOIN positions p ON p.target=g.target AND (g.kind<>'recurrence' OR p.position='support') JOIN filtered f USING(canonical_ref)
 GROUP BY g.kind,g.target HAVING (g.kind='recurrence' AND count(DISTINCT f.canonical_ref)>=5 AND count(DISTINCT f.work_ref)>=3) OR (g.kind='disagreement' AND count(DISTINCT f.canonical_ref) FILTER(WHERE p.position='support')>=2 AND count(DISTINCT f.canonical_ref) FILTER(WHERE p.position='oppose')>=2)
 ORDER BY count(DISTINCT f.work_ref) DESC,count(DISTINCT f.canonical_ref) DESC,g.target LIMIT 10) signals),'[]') END,
 'linkedProblems',COALESCE((SELECT jsonb_agg(p) FROM (SELECT DISTINCT m.problem_ref AS "problemRef",m.name FROM memberships m JOIN filtered f USING(canonical_ref) WHERE f.source_ref=$19) p),'[]'),
 'dailySelection',CASE WHEN $20 OR NOT $21 THEN NULL ELSE COALESCE((SELECT jsonb_agg(jsonb_build_object('batchRef',batch_ref,'total',total,'works',works,'counts',counts,'cleaning',cleaning)) FROM (
 SELECT batch_ref,count(*) total,count(DISTINCT work_ref) works,
 (SELECT jsonb_object_agg(state,n) FROM (SELECT state,count(*) n FROM (SELECT DISTINCT ON(f.canonical_ref) f.canonical_ref,i.state FROM linggan_comment_daily_item i JOIN linggan_material_comment m ON m.material_ref=i.source_ref JOIN filtered f ON f.work_ref=m.content_public_ref AND f.comment_external_id=m.comment_external_id WHERE i.batch_ref=rows.batch_ref ORDER BY f.canonical_ref,i.source_ref) unique_rows GROUP BY state) sc) counts,
 (SELECT jsonb_object_agg(clean_state,n) FROM (SELECT f.clean_state,count(DISTINCT f.canonical_ref) n FROM linggan_comment_daily_item i JOIN linggan_material_comment m ON m.material_ref=i.source_ref JOIN filtered f ON f.work_ref=m.content_public_ref AND f.comment_external_id=m.comment_external_id WHERE i.batch_ref=rows.batch_ref GROUP BY f.clean_state) cc) cleaning
 FROM (SELECT DISTINCT i.batch_ref,f.canonical_ref,f.work_ref FROM linggan_comment_daily_item i JOIN linggan_material_comment m ON m.material_ref=i.source_ref JOIN filtered f ON f.work_ref=m.content_public_ref AND f.comment_external_id=m.comment_external_id) rows GROUP BY batch_ref) batches),'[]') END,
 'semanticOrganization',CASE WHEN $20 THEN NULL ELSE jsonb_build_object(
   'atomCount',(SELECT count(*) FROM scoped_atoms),
   'unclusteredCount',(SELECT count(*) FROM scoped_atoms WHERE unclustered),
   'byKind',COALESCE((SELECT jsonb_object_agg(kind,n) FROM (SELECT kind,count(*) n FROM scoped_atoms GROUP BY kind) counts),'{}'),
   'unclustered',COALESCE((SELECT jsonb_agg(jsonb_build_object('atomRef',atom_ref,'kind',kind,'meaning',meaning,'sourceRef',current_source_ref,'workRef',work_ref,
      'reason',CASE WHEN NOT embedded THEN COALESCE((SELECT work.failure_code FROM linggan_ci_atom_embedding_work work
        JOIN linggan_ci_semantic_space space USING(space_ref) JOIN linggan_model_entry model ON model.model_id=space.model_id
        JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref AND version.api=space.provider_api AND version.base_url=space.endpoint
        JOIN linggan_embedding_config cfg ON cfg.model_ref=model.model_ref AND cfg.dimensions=space.dimensions AND cfg.enabled AND cfg.qualified
        JOIN linggan_embedding_settings active ON active.config_ref=cfg.config_ref
        WHERE work.definition_hash=a.definition_hash ORDER BY work.updated_at DESC LIMIT 1),'missing_embedding') ELSE COALESCE((SELECT run.failure_code FROM linggan_ci_cluster_run run JOIN linggan_ci_cluster_input input USING(run_ref) WHERE input.atom_ref=a.atom_ref ORDER BY run.created_at DESC,run.run_ref DESC LIMIT 1),'awaiting_organization') END))
      FROM (SELECT * FROM scoped_atoms WHERE unclustered ORDER BY created_at DESC,atom_ref LIMIT 30) a),'[]'),
   'scope','current_filtered_accepted_results','qualityMeaning','计算稳定性不等于语义正确率；未通过组织条件的表达仍可查看原声。') END,
 'ownDomain',(SELECT is_own_domain FROM observation_domain WHERE domain_ref=c.domain_ref),
 'deferredAggregates',$20,'observations','[]'::jsonb
) FROM config c CROSS JOIN totals t WHERE c.start_at<c.end_at
"#;
