-- Reconstruct the successful study head that existed at the source cutoff.
-- Current restrictions still apply to both the studied comment and its frozen parent.
WITH head AS (
    SELECT DISTINCT ON (selected.content_public_ref, selected.comment_external_id)
        selected.content_public_ref, selected.comment_external_id,
        selected.body_state AS selected_body_state, selected.body_text AS selected_body,
        selected.author_external_id, studied.body_state AS studied_body_state,
        studied.body_text AS studied_body,
        target.target_ref, target.state, target.parent_source_ref
    FROM linggan_material_comment selected
    JOIN linggan_material_comment studied
      ON studied.content_public_ref = selected.content_public_ref
     AND studied.comment_external_id = selected.comment_external_id
    JOIN linggan_comment_study_target target
      ON target.source_ref = studied.material_ref
     AND target.content_public_ref = selected.content_public_ref
    JOIN linggan_comment_study_run run USING (run_ref)
    JOIN linggan_comment_study_policy policy USING (policy_ref)
    WHERE policy.domain_ref = $1 AND selected.material_ref = ANY($2)
      AND target.state IN ('succeeded', 'no_signal')
      AND target.created_at <= $3::timestamptz
      AND target.finished_at <= $3::timestamptz
    ORDER BY selected.content_public_ref, selected.comment_external_id,
             target.created_at DESC, target.target_ref DESC
)
SELECT head.content_public_ref, head.comment_external_id,
       signal.signal_ref, signal.kind, signal.proposition
FROM head
JOIN linggan_comment_study_signal signal USING (target_ref)
JOIN linggan_comment_study_semantic_attempt attempt
  ON attempt.attempt_ref = signal.semantic_attempt_ref
JOIN linggan_material_content_author work_author
  ON work_author.content_public_ref = head.content_public_ref
WHERE head.state = 'succeeded'
  AND signal.created_at <= $3::timestamptz
  AND attempt.state = 'accepted' AND attempt.finished_at <= $3::timestamptz
  AND head.studied_body_state = 'KNOWN' AND head.studied_body IS NOT NULL
  AND head.selected_body_state = 'KNOWN' AND head.selected_body = head.studied_body
  AND NULLIF(btrim(head.author_external_id), '') IS NOT NULL
  AND NULLIF(btrim(work_author.author_external_id), '') IS NOT NULL
  AND btrim(head.author_external_id) <> btrim(work_author.author_external_id)
  AND NOT EXISTS (
      SELECT 1 FROM linggan_material_comment_restriction restriction
      WHERE restriction.content_public_ref = head.content_public_ref
        AND restriction.comment_external_id = head.comment_external_id
  )
  AND NOT EXISTS (
      SELECT 1 FROM linggan_material_comment parent
      JOIN linggan_material_comment_restriction restriction
        ON restriction.content_public_ref = parent.content_public_ref
       AND restriction.comment_external_id = parent.comment_external_id
      WHERE parent.material_ref = head.parent_source_ref
  )
ORDER BY head.content_public_ref, head.comment_external_id, signal.signal_ref
