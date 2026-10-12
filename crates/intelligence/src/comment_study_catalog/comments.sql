-- Appended to facts.sql. $6 literal query; $7 voice; $8 study filter;
-- $9 received-before; $10 work-after; $11 external-id-after; $12 page size plus one; $13 signal kind;
-- $14 sort (received_desc | like_desc); $15 minimum known likes; $16 post usage (all | used | unused);
-- $17 like-count-after (NULL with a cursor means the page continues inside the unknown-likes tail).
, signal_kinds AS MATERIALIZED (
    SELECT content_public_ref, comment_external_id,
           array_agg(DISTINCT kind::text ORDER BY kind::text) AS kinds
    FROM linggan_comment_study_effective_signal
    WHERE domain_ref = $1
    GROUP BY content_public_ref, comment_external_id
), like_heads AS MATERIALIZED (
    -- The comment's own latest KNOWN count before asOf; an UNKNOWN newer observation does not hide
    -- an older known one, and nothing is ever inferred from the work's likes.
    SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id)
           comment.content_public_ref, comment.comment_external_id,
           engagement.like_count, comment.observed_at AS like_observed_at
    FROM linggan_material_comment comment
    JOIN linggan_material_comment_engagement engagement
      ON engagement.material_ref = comment.material_ref AND engagement.like_count_state = 'KNOWN'
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE EXISTS (
        SELECT 1 FROM linggan_material_domain_usage usage
        WHERE usage.content_public_ref = comment.content_public_ref AND usage.domain_ref = $1
    )
      AND ($4::uuid IS NULL OR comment.content_public_ref = $4)
      AND ($5::text IS NULL OR comment.comment_external_id = $5)
      AND package.accepted_at <= $2::timestamptz AND comment.created_at <= $2::timestamptz
    ORDER BY comment.content_public_ref, comment.comment_external_id,
             comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC
), usage_heads AS MATERIALIZED (
    SELECT DISTINCT ON (event.content_public_ref, event.comment_external_id)
           event.content_public_ref, event.comment_external_id,
           event.action AS usage_action, event.recorded_at AS usage_recorded_at
    FROM linggan_comment_post_usage_event event
    WHERE event.recorded_at <= $2::timestamptz
      AND ($4::uuid IS NULL OR event.content_public_ref = $4)
      AND ($5::text IS NULL OR event.comment_external_id = $5)
    ORDER BY event.content_public_ref, event.comment_external_id,
             event.recorded_at DESC, event.event_ref DESC
), scoped AS MATERIALIZED (
    SELECT qualified.*, COALESCE(signal_kinds.kinds, ARRAY[]::text[]) AS signal_kinds,
           like_heads.like_count, like_heads.like_observed_at,
           usage_heads.usage_action, usage_heads.usage_recorded_at
    FROM qualified
    LEFT JOIN signal_kinds USING (content_public_ref, comment_external_id)
    LEFT JOIN like_heads USING (content_public_ref, comment_external_id)
    LEFT JOIN usage_heads USING (content_public_ref, comment_external_id)
    WHERE ($7 = 'all' OR voice_role = $7 OR ($7 = 'reader_and_unknown' AND voice_role <> 'creator'))
      AND ($13::text IS NULL OR $13::text = ANY(COALESCE(signal_kinds.kinds, ARRAY[]::text[])))
      AND ($15::bigint IS NULL OR like_heads.like_count >= $15::bigint)
      AND CASE $16::text
          WHEN 'all' THEN true
          WHEN 'used' THEN usage_heads.usage_action = 'marked'
          WHEN 'unused' THEN usage_heads.usage_action IS DISTINCT FROM 'marked'
          ELSE false
      END
      AND CASE $8
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
    WHERE $6::text IS NULL
       OR (readable AND research_text ILIKE $6 ESCAPE E'\\')
), visible AS MATERIALIZED (
    SELECT * FROM matching WHERE readable AND clean_state IN ('direct', 'context')
), page AS MATERIALIZED (
    -- received_desc keeps the original order. like_desc orders by the known count with unknown
    -- counts last, then by the same received/work/comment tie-break; the keyset mirrors that order.
    SELECT * FROM (
        SELECT visible.*,
               (created_at < $9::timestamptz
                OR (created_at = $9::timestamptz
                    AND (content_public_ref > $10::uuid
                         OR (content_public_ref = $10::uuid
                             AND comment_external_id COLLATE "C" > $11::text COLLATE "C"))))
                 AS after_received_key
        FROM visible
    ) positioned
    WHERE $9::timestamptz IS NULL
       OR ($14::text = 'received_desc' AND after_received_key)
       OR ($14::text = 'like_desc' AND $17::bigint IS NOT NULL
           AND (like_count IS NULL OR like_count < $17::bigint
                OR (like_count = $17::bigint AND after_received_key)))
       OR ($14::text = 'like_desc' AND $17::bigint IS NULL
           AND like_count IS NULL AND after_received_key)
    ORDER BY CASE WHEN $14::text = 'like_desc' THEN like_count END DESC NULLS LAST,
             created_at DESC, content_public_ref, comment_external_id COLLATE "C"
    LIMIT $12
), coverage AS (
    SELECT (SELECT count(*) FROM matching WHERE readable AND cached_source_ref IS NOT NULL) AS indexed,
           count(*) FILTER (WHERE readable AND cached_source_ref IS NULL) AS pending
    FROM scoped
)
SELECT jsonb_build_object(
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
    ) FROM qualified WHERE $5::text IS NOT NULL),
    'indexCoverage', (SELECT jsonb_build_object(
        'state', CASE WHEN pending = 0 THEN 'ready' ELSE 'partial' END,
        'indexedCount', indexed,
        'pendingCount', CASE WHEN $6::text IS NOT NULL AND pending > 0 THEN NULL ELSE pending END,
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
        'sourceRestrictedCount', CASE WHEN $6::text IS NOT NULL THEN NULL ELSE (SELECT count(*) FROM scoped WHERE source_restricted) END,
        'bodyUnavailableCount', CASE WHEN $6::text IS NOT NULL THEN NULL ELSE (SELECT count(*) FROM scoped WHERE NOT source_restricted AND NOT readable) END
    ),
    'rows', COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'position', jsonb_build_object(
            'receivedAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'workRef', page.content_public_ref, 'commentExternalId', page.comment_external_id,
            'likeCount', page.like_count
        ),
        'item', jsonb_build_object(
            'commentKey', jsonb_build_object('workRef', page.content_public_ref, 'commentExternalId', page.comment_external_id),
            'sourceRef', page.material_ref, 'commentText', raw.body_text,
            'observationRole', page.observation_role,
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
            'signalKinds', to_jsonb(page.signal_kinds),
            'observedAt', to_char(page.observed_at::timestamptz AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'receivedAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'authorDisplayName', page.author_display_name, 'authorExternalId', page.author_external_id,
            'parentCommentKey', CASE WHEN page.parent_comment_external_id IS NULL THEN NULL
                ELSE jsonb_build_object('workRef', page.content_public_ref, 'commentExternalId', page.parent_comment_external_id) END,
            'likeCount', page.like_count,
            'likeCountAsOf', CASE WHEN page.like_count IS NULL THEN NULL
                ELSE to_char(page.like_observed_at::timestamptz AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"') END,
            'publishedAt', NULL,
            'postUsage', jsonb_build_object(
                'state', CASE WHEN page.usage_action = 'marked' THEN 'used' ELSE 'unused' END,
                'markedAt', CASE WHEN page.usage_action = 'marked'
                    THEN to_char(page.usage_recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"') END
            )
        )
    ) ORDER BY CASE WHEN $14::text = 'like_desc' THEN page.like_count END DESC NULLS LAST,
               page.created_at DESC, page.content_public_ref, page.comment_external_id COLLATE "C")
      FROM page JOIN linggan_material_comment raw ON raw.material_ref = page.material_ref), '[]'::jsonb)
);
