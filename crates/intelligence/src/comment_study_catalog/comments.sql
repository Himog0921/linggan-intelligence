-- One read-only statement: latest version first, dynamic readability second, then search/page.
-- $1 domain; $2 asOf; $3 cleaner; $4 work; $5 literal pattern; $6 voice; $7 study filter;
-- $8 received-before; $9 work-after; $10 external-id-after; $11 page size plus one (0 for summary).
-- $12 exact stable comment id, used only by the detail reader; public list filters stay unchanged.
WITH latest AS MATERIALIZED (
    SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id)
           comment.material_ref, comment.content_public_ref, comment.comment_external_id,
           comment.parent_comment_external_id, comment.body_state,
           comment.author_external_id, comment.author_display_name,
           comment.observed_at, comment.created_at
    FROM linggan_material_comment comment
    JOIN linggan_material_content content ON content.public_ref = comment.content_public_ref
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE content.domain_ref = $1
      AND ($4::uuid IS NULL OR comment.content_public_ref = $4)
      AND ($12::text IS NULL OR comment.comment_external_id = $12)
      AND package.accepted_at <= $2::timestamptz AND comment.created_at <= $2::timestamptz
    ORDER BY comment.content_public_ref, comment.comment_external_id,
             comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC
), history AS MATERIALIZED (
    -- History is scanned as a relation, not once per displayed comment.
    SELECT target.target_ref, target.run_ref, target.source_ref, target.state,
           target.created_at, target.finished_at, original.content_public_ref,
           original.comment_external_id, run.dispatch_state
    FROM linggan_comment_study_target target
    JOIN linggan_material_comment original ON original.material_ref = target.source_ref
    JOIN linggan_comment_study_run run ON run.run_ref = target.run_ref
    JOIN linggan_comment_study_policy policy ON policy.policy_ref = run.policy_ref
    WHERE policy.domain_ref = $1 AND target.created_at <= $2::timestamptz
      AND ($4::uuid IS NULL OR original.content_public_ref = $4)
      AND ($12::text IS NULL OR original.comment_external_id = $12)
), last_attempt AS (
    SELECT DISTINCT ON (content_public_ref, comment_external_id) * FROM history
    ORDER BY content_public_ref, comment_external_id, created_at DESC, target_ref DESC
), success AS (
    -- Select the success head BEFORE checking equality. Do not resurrect an older result.
    SELECT DISTINCT ON (content_public_ref, comment_external_id) * FROM history
    WHERE state IN ('succeeded', 'no_signal')
    ORDER BY content_public_ref, comment_external_id, created_at DESC, target_ref DESC
), active AS (
    SELECT DISTINCT content_public_ref, comment_external_id FROM history
    WHERE state IN ('ready', 'queued', 'running') AND dispatch_state IN ('enabled', 'paused')
), facts AS MATERIALIZED (
    SELECT latest.*, cache.source_ref AS cached_source_ref, cache.clean_state,
           cache.research_text, cache.clean_reasons,
           raw.body_text IS NOT NULL AS has_body,
           CASE
             WHEN NULLIF(btrim(latest.author_external_id), '') IS NULL
               OR NULLIF(btrim(author.author_external_id), '') IS NULL THEN 'unknown'
             WHEN btrim(latest.author_external_id) = btrim(author.author_external_id) THEN 'creator'
             ELSE 'reader'
           END AS voice_role,
           NULLIF(btrim(author.author_external_id), '') IS NULL AS work_author_unknown,
           NULLIF(btrim(latest.author_external_id), '') IS NULL AS comment_author_unknown,
           EXISTS (
               SELECT 1 FROM linggan_material_comment_restriction restriction
               WHERE restriction.content_public_ref = latest.content_public_ref
                 AND restriction.comment_external_id = latest.comment_external_id
           ) AS source_restricted,
           last_attempt.target_ref AS latest_target_ref,
           last_attempt.run_ref AS latest_run_ref, last_attempt.state AS latest_state,
           last_attempt.created_at AS latest_created_at,
           last_attempt.finished_at AS latest_finished_at,
           CASE WHEN last_raw.body_state = 'KNOWN' AND last_raw.body_text IS NOT NULL
                THEN last_raw.body_text IS NOT DISTINCT FROM raw.body_text ELSE NULL END AS latest_raw_same,
           CASE WHEN last_raw.body_state = 'KNOWN' AND last_raw.body_text IS NOT NULL
                THEN 'known' ELSE 'unknown' END AS latest_source_state,
           success.target_ref AS effective_target_ref, success.run_ref AS effective_run_ref,
           success.source_ref AS effective_source_ref, success.state AS effective_state,
           success.created_at AS effective_created_at, success.finished_at AS effective_finished_at,
           CASE WHEN success_raw.body_state = 'KNOWN' AND success_raw.body_text IS NOT NULL
                THEN success_raw.body_text IS NOT DISTINCT FROM raw.body_text ELSE NULL END AS effective_raw_same,
           CASE WHEN success_raw.body_state = 'KNOWN' AND success_raw.body_text IS NOT NULL
                THEN 'known' ELSE 'unknown' END AS effective_source_state,
           active.content_public_ref IS NOT NULL AS in_progress
    FROM latest
    JOIN linggan_material_comment raw ON raw.material_ref = latest.material_ref
    LEFT JOIN linggan_comment_study_clean_cache cache
      ON cache.source_ref = latest.material_ref AND cache.cleaner_version = $3
    LEFT JOIN linggan_material_content_author author ON author.content_public_ref = latest.content_public_ref
    LEFT JOIN last_attempt ON last_attempt.content_public_ref = latest.content_public_ref
      AND last_attempt.comment_external_id = latest.comment_external_id
    LEFT JOIN success ON success.content_public_ref = latest.content_public_ref
      AND success.comment_external_id = latest.comment_external_id
    LEFT JOIN active ON active.content_public_ref = latest.content_public_ref
      AND active.comment_external_id = latest.comment_external_id
    LEFT JOIN linggan_material_comment last_raw ON last_raw.material_ref = last_attempt.source_ref
    LEFT JOIN linggan_material_comment success_raw ON success_raw.material_ref = success.source_ref
), qualified AS MATERIALIZED (
    SELECT facts.*,
           NOT source_restricted AND body_state = 'KNOWN' AND has_body AS readable,
           CASE
             WHEN source_restricted THEN 'sourceRestricted'
             WHEN body_state <> 'KNOWN' OR NOT has_body THEN 'bodyUnavailable'
             WHEN cached_source_ref IS NULL THEN 'indexPending'
             WHEN clean_state NOT IN ('direct', 'context') THEN 'textNotResearchable'
             WHEN work_author_unknown THEN 'workAuthorUnknown'
             WHEN comment_author_unknown THEN 'commentAuthorUnknown'
             WHEN voice_role = 'creator' THEN 'creatorVoice'
             ELSE NULL
           END AS exclusion_reason
    FROM facts
), scoped AS MATERIALIZED (
    SELECT * FROM qualified
    WHERE ($6 = 'all' OR voice_role = $6 OR ($6 = 'reader_and_unknown' AND voice_role <> 'creator'))
      AND CASE $7
          WHEN 'all' THEN true
          WHEN 'never_studied' THEN latest_target_ref IS NULL
          WHEN 'in_progress' THEN in_progress
          WHEN 'studied' THEN effective_target_ref IS NOT NULL
          WHEN 'needs_context' THEN latest_state = 'needs_context'
          WHEN 'failed' THEN latest_state = 'failed'
          ELSE false
      END
), matching AS MATERIALIZED (
    SELECT * FROM scoped
    WHERE $5::text IS NULL
       OR (readable AND research_text ILIKE $5 ESCAPE E'\\')
), visible AS MATERIALIZED (
    SELECT * FROM matching WHERE readable AND clean_state IN ('direct', 'context')
), page AS MATERIALIZED (
    SELECT * FROM visible
    WHERE $8::timestamptz IS NULL OR created_at < $8::timestamptz
       OR (created_at = $8::timestamptz AND (
           content_public_ref > $9::uuid OR (content_public_ref = $9::uuid
               AND comment_external_id COLLATE "C" > $10::text COLLATE "C")
       ))
    ORDER BY created_at DESC, content_public_ref, comment_external_id COLLATE "C"
    LIMIT $11
), coverage AS (
    SELECT (SELECT count(*) FROM matching WHERE readable AND cached_source_ref IS NOT NULL) AS indexed,
           count(*) FILTER (WHERE readable AND cached_source_ref IS NULL) AS pending
    FROM scoped
)
SELECT jsonb_build_object(
    -- This metadata is only used by a stable-key detail read. No raw or derived text survives
    -- here when the current source cannot be displayed, and ordinary lists receive NULL.
    'currentSource', (SELECT jsonb_build_object(
        'commentKey', jsonb_build_object('workRef', content_public_ref, 'commentExternalId', comment_external_id),
        'sourceRef', material_ref,
        'sourceState', CASE WHEN source_restricted THEN 'restricted' WHEN readable THEN 'known' ELSE 'unknown' END,
        'displayState', CASE
            WHEN source_restricted THEN 'restricted'
            WHEN NOT readable THEN 'body_unavailable'
            WHEN cached_source_ref IS NULL THEN 'index_pending'
            WHEN clean_state NOT IN ('direct', 'context') THEN 'not_displayable'
            ELSE 'displayable' END,
        'cleanState', CASE WHEN readable THEN clean_state ELSE NULL END,
        'parentCommentKey', CASE WHEN parent_comment_external_id IS NULL THEN NULL
            ELSE jsonb_build_object('workRef', content_public_ref, 'commentExternalId', parent_comment_external_id) END
    ) FROM qualified WHERE $12::text IS NOT NULL),
    'indexCoverage', (SELECT jsonb_build_object(
        'state', CASE WHEN pending = 0 THEN 'ready' ELSE 'partial' END,
        'indexedCount', indexed,
        -- With a text search, uncleaned rows cannot be classified as matching or non-matching.
        'pendingCount', CASE WHEN $5::text IS NOT NULL AND pending > 0 THEN NULL ELSE pending END,
        'asOf', $2::text
    ) FROM coverage),
    'summary', jsonb_build_object(
        'displayableCommentCount', (SELECT count(*) FROM visible),
        'eligibleCommentCount', (SELECT count(*) FROM visible WHERE exclusion_reason IS NULL),
        'studiedCommentCount', (SELECT count(*) FROM visible WHERE effective_target_ref IS NOT NULL),
        'inProgressCommentCount', (SELECT count(*) FROM visible WHERE in_progress),
        'needsContextCount', (SELECT count(*) FROM visible WHERE latest_state = 'needs_context'),
        'failedCount', (SELECT count(*) FROM visible WHERE latest_state = 'failed'),
        'creatorVoiceCount', (SELECT count(*) FROM visible WHERE voice_role = 'creator'),
        'unknownIdentityCount', (SELECT count(*) FROM visible WHERE voice_role = 'unknown'),
        'textNotResearchableCount', (SELECT count(*) FROM matching WHERE readable AND clean_state IN ('dropped', 'anomaly')),
        'sourceRestrictedCount', CASE WHEN $5::text IS NOT NULL THEN NULL ELSE (SELECT count(*) FROM scoped WHERE source_restricted) END,
        'bodyUnavailableCount', CASE WHEN $5::text IS NOT NULL THEN NULL ELSE (SELECT count(*) FROM scoped WHERE NOT source_restricted AND NOT readable) END
    ),
    'rows', COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'position', jsonb_build_object(
            'receivedAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'workRef', page.content_public_ref, 'commentExternalId', page.comment_external_id
        ),
        'item', jsonb_build_object(
            'commentKey', jsonb_build_object('workRef', page.content_public_ref, 'commentExternalId', page.comment_external_id),
            'sourceRef', page.material_ref, 'commentText', raw.body_text,
            'researchText', page.research_text, 'voiceRole', page.voice_role,
            'cleanState', page.clean_state, 'cleanReasons', page.clean_reasons, 'sourceState', 'known',
            'studyEligibility', jsonb_build_object('eligible', page.exclusion_reason IS NULL,
                'reasons', CASE WHEN page.exclusion_reason IS NULL THEN '[]'::jsonb ELSE jsonb_build_array(page.exclusion_reason) END),
            'latestStudy', CASE WHEN page.latest_target_ref IS NULL THEN NULL ELSE jsonb_build_object(
                'runRef', page.latest_run_ref, 'targetRef', page.latest_target_ref, 'state', page.latest_state,
                'createdAt', to_char(page.latest_created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'finishedAt', to_char(page.latest_finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'inputComparison', CASE WHEN NOT page.latest_raw_same THEN 'changed' ELSE 'unknown' END,
                'sourceState', page.latest_source_state
            ) END,
            'effectiveStudy', CASE WHEN page.effective_target_ref IS NULL THEN NULL ELSE jsonb_build_object(
                'runRef', page.effective_run_ref, 'targetRef', page.effective_target_ref, 'state', page.effective_state,
                'createdAt', to_char(page.effective_created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'finishedAt', to_char(page.effective_finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                'inputComparison', CASE WHEN NOT page.effective_raw_same THEN 'changed' ELSE 'unknown' END,
                'sourceState', page.effective_source_state
            ) END,
            'effectiveState', CASE
                WHEN page.effective_target_ref IS NULL THEN 'none'
                WHEN NOT page.effective_raw_same THEN 'source_changed'
                WHEN page.exclusion_reason IS NOT NULL OR page.effective_source_state <> 'known' THEN 'source_unavailable'
                ELSE 'effective'
            END,
            'observedAt', to_char(page.observed_at::timestamptz AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'receivedAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'authorDisplayName', page.author_display_name, 'authorExternalId', page.author_external_id,
            'parentCommentKey', CASE WHEN page.parent_comment_external_id IS NULL THEN NULL
                ELSE jsonb_build_object('workRef', page.content_public_ref, 'commentExternalId', page.parent_comment_external_id) END,
            'likeCount', NULL, 'likeCountAsOf', NULL, 'publishedAt', NULL
        )
    ) ORDER BY page.created_at DESC, page.content_public_ref, page.comment_external_id COLLATE "C")
      FROM page JOIN linggan_material_comment raw ON raw.material_ref = page.material_ref), '[]'::jsonb)
);
