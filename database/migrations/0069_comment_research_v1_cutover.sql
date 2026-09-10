-- COMMENT-RESEARCH-RESET-001 terminal cutover.
--
-- The development database deliberately does not migrate old research results.  This migration
-- removes only retired comment-research derivations and their dedicated queue/config links.  It
-- does not touch raw comments, evidence packages, content-author attribution, source-restriction
-- facts, generic model connections, model configurations, embedding settings, or invocation
-- receipts.  Every drop is explicit: no CASCADE is used to guess at an unrelated consumer.

DROP VIEW IF EXISTS linggan_ci_semantic_atom_current;
DROP VIEW IF EXISTS linggan_ci_source;
DROP VIEW IF EXISTS linggan_comment_asset_current;
DROP VIEW IF EXISTS linggan_comment_query_current;
DROP TRIGGER IF EXISTS linggan_ci_comment_identity_insert ON linggan_material_comment;
DROP FUNCTION IF EXISTS linggan_ci_remember_comment_identity();
DROP INDEX IF EXISTS linggan_comment_identity_history_idx;

-- A V1 invocation is not attached to a retired daily work item or automatic plan.  Preserve the
-- ledger row itself while deleting obsolete links and the automatic-plan workspace field.
ALTER TABLE linggan_model_workspace DROP COLUMN IF EXISTS active_auto_plan_ref;
ALTER TABLE linggan_model_invocation DROP COLUMN IF EXISTS work_ref;
ALTER TABLE linggan_model_invocation DROP COLUMN IF EXISTS plan_ref;
ALTER TABLE linggan_model_config DROP COLUMN IF EXISTS auto_source_limit;
ALTER TABLE linggan_model_config DROP COLUMN IF EXISTS auto_token_limit;

-- Earlier development branches may already contain preliminary rows in the new V1 tables. They
-- are not a migration source: the user explicitly chose a clean research history. Keep the V1
-- schema and generic invocation ledger, but clear every V1-derived row in dependency order.
-- The immutable triggers are disabled only inside this transaction and restored before commit.
ALTER TABLE linggan_comment_research_policy_revision
  DISABLE TRIGGER linggan_comment_research_policy_revision_immutable;
ALTER TABLE linggan_comment_research_derivation
  DISABLE TRIGGER linggan_comment_research_derivation_immutable;
ALTER TABLE linggan_comment_research_atom
  DISABLE TRIGGER linggan_comment_research_atom_immutable;
ALTER TABLE linggan_comment_research_embedding_space
  DISABLE TRIGGER linggan_comment_research_embedding_space_immutable;
ALTER TABLE linggan_comment_research_problem_definition
  DISABLE TRIGGER linggan_comment_research_problem_definition_immutable;

DELETE FROM linggan_comment_research_change_observation;
DELETE FROM linggan_comment_research_problem_window_stat;
DELETE FROM linggan_comment_research_result_revision;
DELETE FROM linggan_comment_research_problem_resolution;
DELETE FROM linggan_comment_research_atom_problem_membership;
DELETE FROM linggan_comment_research_problem_definition_embedding;
DELETE FROM linggan_comment_research_atom_embedding;
DELETE FROM linggan_comment_research_problem_definition;
DELETE FROM linggan_comment_research_problem;
DELETE FROM linggan_comment_research_atom;
DELETE FROM linggan_comment_research_run_item;
DELETE FROM linggan_comment_research_run;
DELETE FROM linggan_comment_research_derivation;
UPDATE linggan_comment_research_policy_active
SET policy_revision_ref=NULL,revision=0,updated_at=scope_001_now();
DELETE FROM linggan_comment_research_policy_revision;
DELETE FROM linggan_comment_research_embedding_space;

ALTER TABLE linggan_comment_research_policy_revision
  ENABLE TRIGGER linggan_comment_research_policy_revision_immutable;
ALTER TABLE linggan_comment_research_derivation
  ENABLE TRIGGER linggan_comment_research_derivation_immutable;
ALTER TABLE linggan_comment_research_atom
  ENABLE TRIGGER linggan_comment_research_atom_immutable;
ALTER TABLE linggan_comment_research_embedding_space
  ENABLE TRIGGER linggan_comment_research_embedding_space_immutable;
ALTER TABLE linggan_comment_research_problem_definition
  ENABLE TRIGGER linggan_comment_research_problem_definition_immutable;

-- Retired P4/Task-B/topic-association projections: leaves before their semantic parents.
DROP TABLE IF EXISTS linggan_ci_comment_topic_association;
DROP TABLE IF EXISTS linggan_ci_atom_embedding_work;
DROP TABLE IF EXISTS linggan_ci_cluster_review;
DROP TABLE IF EXISTS linggan_ci_semantic_lineage;
DROP TABLE IF EXISTS linggan_ci_atom_membership;
DROP TABLE IF EXISTS linggan_ci_cluster_assignment;
DROP TABLE IF EXISTS linggan_ci_cluster_input;
DROP TABLE IF EXISTS linggan_ci_semantic_group;
DROP TABLE IF EXISTS linggan_ci_cluster_run;
DROP TABLE IF EXISTS linggan_ci_atom_vector;
DROP TABLE IF EXISTS linggan_ci_atom_projection;
DROP TABLE IF EXISTS linggan_ci_semantic_atom;
DROP TABLE IF EXISTS linggan_ci_semantic_space;
DROP TABLE IF EXISTS linggan_ci_comment_identity;
DROP TABLE IF EXISTS linggan_ci_definition_vector;
DROP TABLE IF EXISTS linggan_ci_problem_task;
DROP TABLE IF EXISTS linggan_ci_problem_boundary_decision;
DROP TABLE IF EXISTS linggan_ci_problem_candidate;
DROP TABLE IF EXISTS linggan_ci_problem_member;
DROP TABLE IF EXISTS linggan_ci_problem_version;
DROP TABLE IF EXISTS linggan_ci_problem;
DROP TABLE IF EXISTS linggan_ci_projection_cursor;
DROP TABLE IF EXISTS linggan_ci_rule_release;
DROP TABLE IF EXISTS linggan_ci_prepare;
DROP TABLE IF EXISTS linggan_ci_command;
DROP TABLE IF EXISTS linggan_ci_source_revision;
DROP TABLE IF EXISTS linggan_ci_source_research;
DROP TABLE IF EXISTS linggan_ci_term_index;
DROP TABLE IF EXISTS linggan_ci_term_setting;

-- Retired replay, repair, daily and legacy recovery state.  Raw material/comment tables are not
-- named below and remain their own Evidence facts.
DROP TABLE IF EXISTS linggan_comment_rule_rollback_receipt;
DROP TABLE IF EXISTS linggan_comment_replay_explanation;
DROP TABLE IF EXISTS linggan_comment_replay_trace;
DROP TABLE IF EXISTS linggan_comment_replay_follow_up;
DROP TABLE IF EXISTS linggan_comment_replay_item;
DROP TABLE IF EXISTS linggan_comment_replay_member;
DROP TABLE IF EXISTS linggan_comment_replay_run;
DROP TABLE IF EXISTS linggan_comment_replay_sample_set;
DROP TABLE IF EXISTS linggan_comment_field_repair_trace;
DROP TABLE IF EXISTS linggan_comment_field_repair;
DROP TABLE IF EXISTS linggan_comment_local_recovery;
DROP TABLE IF EXISTS linggan_comment_request_trace;
DROP TABLE IF EXISTS linggan_comment_daily_packet;
DROP TABLE IF EXISTS linggan_comment_daily_item;
DROP TABLE IF EXISTS linggan_comment_daily_adjustment;
DROP TABLE IF EXISTS linggan_comment_daily_command;
DROP TABLE IF EXISTS linggan_comment_daily_schedule;
DROP TABLE IF EXISTS linggan_comment_model_work;
DROP TABLE IF EXISTS linggan_comment_research_eligibility_current;
DROP TABLE IF EXISTS linggan_comment_semantic_work;
DROP TABLE IF EXISTS linggan_comment_legacy_fingerprint_alias;
DROP TABLE IF EXISTS linggan_comment_auto_upgrade_policy;
DROP TABLE IF EXISTS linggan_comment_research_rule_active;
DROP TABLE IF EXISTS linggan_comment_research_rule_revision;
DROP TABLE IF EXISTS linggan_comment_analysis_work;
DROP TABLE IF EXISTS linggan_comment_daily_batch;
DROP TABLE IF EXISTS linggan_comment_clean;
DROP TABLE IF EXISTS linggan_comment_annotation;
DROP TABLE IF EXISTS linggan_comment_asset_revision;
DROP TABLE IF EXISTS linggan_comment_asset;
DROP TABLE IF EXISTS linggan_comment_query_revision;
DROP TABLE IF EXISTS linggan_comment_saved_query;
DROP TABLE IF EXISTS linggan_comment_collection;
DROP TABLE IF EXISTS linggan_comment_context_settings;
DROP TABLE IF EXISTS linggan_model_plan;

-- These old functions are referenced by constraints/triggers on the retired tables above.
-- Drop them only after their dependencies are gone; no CASCADE may hide a surviving consumer.
DROP FUNCTION IF EXISTS linggan_comment_research_current(timestamptz);
DROP FUNCTION IF EXISTS linggan_comment_research_context_revision(uuid);
DROP FUNCTION IF EXISTS linggan_comment_auto_policy_valid(jsonb);
DROP FUNCTION IF EXISTS linggan_comment_execution_day_sources();
DROP FUNCTION IF EXISTS linggan_comment_day_source_allowed(uuid, text);
DROP FUNCTION IF EXISTS linggan_comment_daily_freeze_manifest();
DROP FUNCTION IF EXISTS linggan_comment_replay_item_matches_run();
DROP FUNCTION IF EXISTS linggan_ci_analysis_context_readable(jsonb);
DROP FUNCTION IF EXISTS linggan_ci_research_labels(jsonb);
DROP FUNCTION IF EXISTS linggan_ci_research_stances(jsonb);
DROP FUNCTION IF EXISTS linggan_ci_group_problem_member_current(uuid, uuid, uuid);

COMMENT ON TABLE linggan_comment_research_derivation IS
  'V1-only research derivation. Raw comment text and content-author attribution remain in Evidence tables.';
