-- A restricted definition seed cannot be reused for a new clean pair. Keep the historical
-- revision intact while permitting a new Problem with the same definition hash. Application
-- creation holds an advisory transaction lock on Domain + hash before selecting the readable
-- current Problem, preserving concurrent deduplication without hiding the old identity.
DO $definition_hash_uniqueness$
DECLARE unique_name text;
BEGIN
  SELECT constraint_row.conname INTO unique_name
  FROM pg_constraint constraint_row
  WHERE constraint_row.conrelid='linggan_comment_study_problem_revision'::regclass
    AND constraint_row.contype='u'
    AND pg_get_constraintdef(constraint_row.oid) LIKE 'UNIQUE (domain_ref, definition_hash)%'
  LIMIT 1;
  IF unique_name IS NOT NULL THEN
    EXECUTE format('ALTER TABLE linggan_comment_study_problem_revision DROP CONSTRAINT %I', unique_name);
  END IF;
END
$definition_hash_uniqueness$;
CREATE INDEX IF NOT EXISTS linggan_comment_study_problem_revision_definition_lookup_idx
  ON linggan_comment_study_problem_revision(domain_ref,definition_hash);

-- Select the latest successful attempt before deciding whether it is still usable.
-- Failed, cancelled and needs_context attempts remain in history and do not replace this head.
CREATE VIEW linggan_comment_study_effective_target AS
SELECT DISTINCT ON (policy.domain_ref, source.content_public_ref, source.comment_external_id)
       policy.domain_ref, source.content_public_ref, source.comment_external_id,
       target.target_ref, target.run_ref, target.source_ref, target.state,
       target.created_at
FROM linggan_comment_study_target target
JOIN linggan_comment_study_run run ON run.run_ref = target.run_ref
JOIN linggan_comment_study_policy policy ON policy.policy_ref = run.policy_ref
JOIN linggan_material_comment source ON source.material_ref = target.source_ref
WHERE target.state IN ('succeeded', 'no_signal')
  AND source.comment_external_id IS NOT NULL
  AND target.content_public_ref = source.content_public_ref
ORDER BY policy.domain_ref, source.content_public_ref, source.comment_external_id,
         target.created_at DESC, target.target_ref DESC;

COMMENT ON VIEW linggan_comment_study_effective_target IS
    'Latest succeeded/no_signal Target per Domain and stable comment identity; history is not rewritten.';

-- Match the catalog's accepted-package and observation ordering without an as-of parameter.
CREATE VIEW linggan_comment_study_current_comment AS
SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id) comment.*
FROM linggan_material_comment comment
JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
WHERE package.accepted_at IS NOT NULL
ORDER BY comment.content_public_ref, comment.comment_external_id,
         comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC;

COMMENT ON VIEW linggan_comment_study_current_comment IS
    'Latest accepted raw comment observation per stable identity for current Comment Study knowledge.';

-- Only the chosen head can contribute current Signals. A later no_signal has no Signal rows.
-- Current source restrictions and changed raw text invalidate the chosen head without falling
-- back to an older successful Target. A restricted frozen parent also hides derived Signals.
CREATE VIEW linggan_comment_study_effective_signal AS
SELECT signal.*, head.domain_ref, head.content_public_ref, head.comment_external_id,
       head.source_ref,
       current_comment.author_external_id AS current_author_external_id
FROM linggan_comment_study_effective_target head
JOIN linggan_comment_study_target target ON target.target_ref = head.target_ref
JOIN linggan_comment_study_signal signal ON signal.target_ref = head.target_ref
JOIN linggan_material_comment studied ON studied.material_ref = head.source_ref
JOIN linggan_comment_study_current_comment current_comment
  ON current_comment.content_public_ref = head.content_public_ref
 AND current_comment.comment_external_id = head.comment_external_id
JOIN linggan_material_content_author work_author
  ON work_author.content_public_ref = head.content_public_ref
WHERE head.state = 'succeeded'
  AND studied.body_state = 'KNOWN' AND studied.body_text IS NOT NULL
  AND current_comment.body_state = 'KNOWN' AND current_comment.body_text IS NOT NULL
  AND current_comment.body_text = studied.body_text
  AND NULLIF(btrim(current_comment.author_external_id), '') IS NOT NULL
  AND NULLIF(btrim(work_author.author_external_id), '') IS NOT NULL
  AND btrim(current_comment.author_external_id) <> btrim(work_author.author_external_id)
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
    WHERE parent.material_ref = target.parent_source_ref
  );

COMMENT ON VIEW linggan_comment_study_effective_signal IS
    'Current readable Signals from the latest successful Target; per-Run history remains in base tables.';

-- A readable stable definition remains available for candidate comparison when a seed
-- is superseded or becomes no_signal. Current support is counted separately from the
-- effective Signal view. A restricted seed or frozen parent hides the definition.
CREATE VIEW linggan_comment_study_current_problem AS
SELECT problem.*
FROM linggan_comment_study_problem problem
JOIN linggan_comment_study_problem_revision revision
  ON revision.revision_ref = problem.current_revision_ref
WHERE problem.state IN ('active', 'support_insufficient')
  AND cardinality(revision.seed_signal_refs) > 0
  AND NOT EXISTS (
    SELECT 1 FROM unnest(revision.seed_signal_refs) seed(signal_ref)
    JOIN linggan_comment_study_signal signal ON signal.signal_ref = seed.signal_ref
    JOIN linggan_comment_study_target target ON target.target_ref = signal.target_ref
    JOIN linggan_material_comment source ON source.material_ref = target.source_ref
    WHERE EXISTS (
      SELECT 1 FROM linggan_material_comment_restriction restriction
      WHERE restriction.content_public_ref = source.content_public_ref
        AND restriction.comment_external_id = source.comment_external_id
    )
    OR EXISTS (
      SELECT 1 FROM linggan_material_comment parent
      JOIN linggan_material_comment_restriction restriction
        ON restriction.content_public_ref = parent.content_public_ref
       AND restriction.comment_external_id = parent.comment_external_id
      WHERE parent.material_ref = target.parent_source_ref
    )
  );

COMMENT ON VIEW linggan_comment_study_current_problem IS
    'Readable active Problem definitions for candidate comparison; current support is counted from effective Signals separately.';
