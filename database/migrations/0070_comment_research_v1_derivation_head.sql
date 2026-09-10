-- COMMENT-RESEARCH-RESET-001: a V1 derivation is a snapshot of every fact that changes its
-- research meaning.  It is never just a snapshot of raw comment text.
--
-- Older V1 rows, if any exist in development, remain immutable audit history.  A frozen Run
-- must retain access to its exact readable input; a separate current projection is used only
-- when selecting *new* Run inputs.  A published Result becomes unreadable as a whole when any
-- frozen input is no longer current, rather than returning a mixed old/new interpretation.

ALTER TABLE linggan_comment_research_derivation
  ADD COLUMN derivation_input_hash text;

UPDATE linggan_comment_research_derivation
SET derivation_input_hash=encode(sha256(convert_to(
    source_ref::text || E'\n' || derivation_version || E'\n' || source_sha256 || E'\n' ||
    research_sha256 || E'\n' || cleaner_version || E'\n' || author_role || E'\n' ||
    COALESCE(attribution_source,'') || E'\n' || COALESCE(attribution_observed_at::text,'') || E'\n' ||
    context_manifest::text,
    'UTF8')),'hex');

ALTER TABLE linggan_comment_research_derivation
  ALTER COLUMN derivation_input_hash SET NOT NULL;
ALTER TABLE linggan_comment_research_derivation
  ADD CONSTRAINT linggan_comment_research_derivation_input_hash_check
  CHECK(derivation_input_hash ~ '^[0-9a-f]{64}$');

DO $$
DECLARE
  legacy_constraint name;
BEGIN
  SELECT constraint_name INTO legacy_constraint
  FROM information_schema.table_constraints
  WHERE table_schema=current_schema()
    AND table_name='linggan_comment_research_derivation'
    AND constraint_type='UNIQUE'
  ORDER BY constraint_name
  LIMIT 1;
  IF legacy_constraint IS NULL THEN
    RAISE EXCEPTION 'expected legacy V1 derivation unique constraint is missing';
  END IF;
  EXECUTE format('ALTER TABLE linggan_comment_research_derivation DROP CONSTRAINT %I',legacy_constraint);
END;
$$;

ALTER TABLE linggan_comment_research_derivation
  ADD CONSTRAINT linggan_comment_research_derivation_identity
  UNIQUE(source_ref,derivation_version,derivation_input_hash);

CREATE OR REPLACE VIEW linggan_comment_research_derivation_readable AS
SELECT derivation.*
FROM linggan_comment_research_derivation derivation
JOIN linggan_comment_research_readable source
  ON source.material_ref=derivation.source_ref;

COMMENT ON VIEW linggan_comment_research_derivation_readable IS
  'All immutable V1 derivations whose raw sources remain readable. Frozen Runs read their exact input here.';

CREATE VIEW linggan_comment_research_derivation_current AS
SELECT DISTINCT ON (source_ref,derivation_version) *
FROM linggan_comment_research_derivation_readable
ORDER BY source_ref,derivation_version,created_at DESC,derivation_ref DESC;

COMMENT ON VIEW linggan_comment_research_derivation_current IS
  'The latest readable V1 derivation per source/version. New Runs select only this projection.';

CREATE OR REPLACE VIEW linggan_comment_research_result_revision_readable AS
SELECT result.*
FROM linggan_comment_research_result_revision result
WHERE result.state='published'
  AND NOT EXISTS (
      SELECT 1
      FROM linggan_comment_research_run_item item
      WHERE item.run_ref=result.run_ref
        AND NOT EXISTS (
            SELECT 1
            FROM linggan_comment_research_derivation_current current
            WHERE current.derivation_ref=item.derivation_ref
        )
  );

COMMENT ON VIEW linggan_comment_research_result_revision_readable IS
  '只返回全部冻结输入仍是当前可读事实的已发布结果；任一输入更新或受限后整版结果不可读，等待重算。';
