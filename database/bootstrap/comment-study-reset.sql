-- COMMENT-STUDY-001 · local derived-layer reset
--
-- Caller must run this in the same transaction as comment-study-001.sql. This is deliberately an
-- explicit list: an unfamiliar dependency must abort the reset instead of being swallowed by
-- CASCADE. Evidence, restriction and media-disposition facts do not appear in this list.

CREATE TABLE IF NOT EXISTS linggan_material_comment_restriction (
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    restricted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
    PRIMARY KEY(content_public_ref,comment_external_id)
);

CREATE TEMP TABLE comment_study_reset_counts AS
SELECT jsonb_build_object(
  'policy', (SELECT count(*) FROM linggan_comment_research_policy_revision),
  'derivation', (SELECT count(*) FROM linggan_comment_research_derivation),
  'run', (SELECT count(*) FROM linggan_comment_research_run),
  'runItem', (SELECT count(*) FROM linggan_comment_research_run_item),
  'atom', (SELECT count(*) FROM linggan_comment_research_atom),
  'problem', (SELECT count(*) FROM linggan_comment_research_problem),
  'membership', (SELECT count(*) FROM linggan_comment_research_atom_problem_membership),
  'signalResolution', (SELECT count(*) FROM linggan_comment_research_problem_resolution)
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
