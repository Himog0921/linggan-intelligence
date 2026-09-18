-- COMMENT-STUDY-001 · local derived-layer reset
--
-- Caller must run this in the same transaction as comment-study-001.sql. This is deliberately an
-- explicit list: an unfamiliar dependency must abort the reset instead of being swallowed by
-- CASCADE. Evidence, restriction and media-disposition facts do not appear in this list.

-- A relation that no longer exists counts as zero destroyed rows, which is different from an
-- unfamiliar dependency: only names this file already knows are ever passed in.
CREATE OR REPLACE FUNCTION linggan_comment_study_reset_count(relation text)
RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE total bigint;
BEGIN
    IF to_regclass(relation) IS NULL THEN
        RETURN 0;
    END IF;
    EXECUTE format('SELECT count(*) FROM %I', relation) INTO total;
    RETURN total;
END $$;

CREATE TABLE IF NOT EXISTS linggan_material_comment_restriction (
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    restricted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
    PRIMARY KEY(content_public_ref,comment_external_id)
);

-- Counted through to_regclass rather than direct references: after the first reset the
-- comment_research relations are gone, and a second reset must still be able to state what it is
-- about to destroy instead of aborting on a missing table.
CREATE TEMP TABLE comment_study_reset_counts AS
SELECT jsonb_build_object(
  'commentResearchPolicy', linggan_comment_study_reset_count('linggan_comment_research_policy_revision'),
  'commentResearchDerivation', linggan_comment_study_reset_count('linggan_comment_research_derivation'),
  'commentResearchRun', linggan_comment_study_reset_count('linggan_comment_research_run'),
  'commentResearchAtom', linggan_comment_study_reset_count('linggan_comment_research_atom'),
  'commentResearchProblem', linggan_comment_study_reset_count('linggan_comment_research_problem'),
  'commentStudyRun', linggan_comment_study_reset_count('linggan_comment_study_run'),
  'commentStudyTarget', linggan_comment_study_reset_count('linggan_comment_study_target'),
  'commentStudySignal', linggan_comment_study_reset_count('linggan_comment_study_signal'),
  'commentStudyProblem', linggan_comment_study_reset_count('linggan_comment_study_problem'),
  'commentStudyMembership', linggan_comment_study_reset_count('linggan_comment_study_problem_membership')
) AS old_relation_counts;

DROP VIEW IF EXISTS linggan_comment_research_result_revision_readable;
DROP VIEW IF EXISTS linggan_comment_research_derivation_current;
DROP VIEW IF EXISTS linggan_comment_research_derivation_readable;
DROP VIEW IF EXISTS linggan_comment_research_source_current;
DROP VIEW IF EXISTS linggan_comment_research_readable;

DO $$
BEGIN
    IF to_regclass('linggan_comment_research_restriction') IS NOT NULL THEN
        INSERT INTO linggan_material_comment_restriction(
            content_public_ref,comment_external_id,restricted_at,reason
        ) SELECT content_public_ref,comment_external_id,restricted_at,reason
          FROM linggan_comment_research_restriction
        ON CONFLICT(content_public_ref,comment_external_id) DO NOTHING;
        DROP TABLE linggan_comment_research_restriction;
    END IF;
END $$;

-- The comment-study derived layer itself. Listed children-first for the same reason as the
-- comment_research list below: an unfamiliar dependency must abort the reset rather than be
-- swallowed by CASCADE. Raw Evidence, restrictions and media dispositions never appear here, and
-- neither does the reset receipt, which is the audit trail of resets rather than research output.
DROP TABLE IF EXISTS linggan_comment_study_problem_pair;
DROP TABLE IF EXISTS linggan_comment_study_problem_membership;
DROP TABLE IF EXISTS linggan_comment_study_comparison;
DROP TABLE IF EXISTS linggan_comment_study_resolution;
DROP TABLE IF EXISTS linggan_comment_study_signal;
DROP TABLE IF EXISTS linggan_comment_study_semantic_attempt;
DROP TABLE IF EXISTS linggan_comment_study_batch_target;
DROP TABLE IF EXISTS linggan_comment_study_batch;
DROP TABLE IF EXISTS linggan_comment_study_target;
DROP TABLE IF EXISTS linggan_comment_study_work;
DROP TABLE IF EXISTS linggan_comment_study_problem_revision;
DROP TABLE IF EXISTS linggan_comment_study_problem;
DROP TABLE IF EXISTS linggan_comment_study_embedding_cache;
DROP TABLE IF EXISTS linggan_comment_study_embedding_profile;
DROP TABLE IF EXISTS linggan_comment_study_run;
DROP TABLE IF EXISTS linggan_comment_study_active_policy;
DROP TABLE IF EXISTS linggan_comment_study_policy;

DROP TABLE IF EXISTS linggan_comment_research_problem_pair_evaluation;
DROP TABLE IF EXISTS linggan_comment_research_problem_resolution_execution;
DROP TABLE IF EXISTS linggan_comment_research_problem_resolution;
DROP TABLE IF EXISTS linggan_comment_research_problem_catalog_guard;
DROP TABLE IF EXISTS linggan_comment_research_change_observation;
DROP TABLE IF EXISTS linggan_comment_research_problem_window_stat;
DROP TABLE IF EXISTS linggan_comment_research_result_revision;
DROP TABLE IF EXISTS linggan_comment_research_atom_problem_membership;
DROP TABLE IF EXISTS linggan_comment_research_problem_definition_embedding;
DROP TABLE IF EXISTS linggan_comment_research_atom_embedding;
DROP TABLE IF EXISTS linggan_comment_research_embedding_space;
DROP TABLE IF EXISTS linggan_comment_research_problem_definition;
DROP TABLE IF EXISTS linggan_comment_research_problem;
DROP TABLE IF EXISTS linggan_comment_research_atom;
DROP TABLE IF EXISTS linggan_comment_research_run_item;
DROP TABLE IF EXISTS linggan_comment_research_run;
DROP TABLE IF EXISTS linggan_comment_research_derivation;
DROP TABLE IF EXISTS linggan_comment_research_policy_active;
DROP TABLE IF EXISTS linggan_comment_research_policy_revision;
DROP FUNCTION IF EXISTS linggan_comment_research_run_item_requires_eligible_derivation();
