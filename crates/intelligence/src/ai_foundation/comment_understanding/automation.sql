SELECT q.content_public_ref, q.comment_external_id, q.material_ref
FROM qualified q
JOIN linggan_material_comment_first_admission first USING (content_public_ref, comment_external_id)
JOIN linggan_runtime_capture_package package ON package.package_ref = first.first_package_ref
WHERE q.exclusion_reason IS NULL AND q.observation_role = 'primary' AND NOT q.in_progress
  AND q.effective_raw_same IS DISTINCT FROM true
  AND package.accepted_at >= $6::timestamptz AND package.accepted_at <= $2::timestamptz
  AND NOT EXISTS (
      SELECT 1 FROM linggan_comment_research_selection_target target
      WHERE target.domain_ref = $1 AND target.work_ref = q.content_public_ref
        AND target.comment_external_id = q.comment_external_id
  )
ORDER BY package.accepted_at, q.content_public_ref, q.comment_external_id
LIMIT $7
