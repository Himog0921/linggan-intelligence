-- TOPIC-MAP-CORE-001: durable evidence induction and semantic assignment.
-- Stores derived statements and canonical references, never another raw-text corpus.
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;
ALTER TABLE linggan_topic_map_research_run ADD COLUMN input_scope jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE linggan_topic_map_research_task
 ADD COLUMN phase text NOT NULL DEFAULT 'extract' CHECK(phase IN ('extract','resolve','compare')),
 ADD COLUMN phase_attempt_count integer NOT NULL DEFAULT 0 CHECK(phase_attempt_count BETWEEN 0 AND 3),
 ADD COLUMN distilled_json jsonb,
 ADD COLUMN resolution_cursor integer NOT NULL DEFAULT 0 CHECK(resolution_cursor >= 0),
 ADD COLUMN resolutions_json jsonb NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(resolutions_json)='array'),
 ADD COLUMN recall_manifest jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE linggan_topic_map_research_task DROP CONSTRAINT linggan_topic_map_research_task_attempt_count_check;
ALTER TABLE linggan_topic_map_research_task ADD CHECK(attempt_count >= 0);
ALTER TABLE linggan_topic_map_research_request DROP CONSTRAINT linggan_topic_map_research_request_attempt_ordinal_check;
ALTER TABLE linggan_topic_map_research_request ADD CHECK(attempt_ordinal > 0);
-- Failed/stopped runs can be explicitly retried without destroying their request ledger.
DO $$ DECLARE key_name text; BEGIN
 SELECT conname INTO key_name FROM pg_constraint WHERE conrelid='linggan_topic_map_research_task'::regclass AND contype='u';
 IF key_name IS NOT NULL THEN EXECUTE format('ALTER TABLE linggan_topic_map_research_task DROP CONSTRAINT %I',key_name); END IF;
END $$;
CREATE UNIQUE INDEX topic_map_core_active_input ON linggan_topic_map_research_task(domain_ref,work_public_ref,input_hash) WHERE state IN ('queued','running');
ALTER TABLE linggan_topic_map_research_request ADD COLUMN phase text NOT NULL DEFAULT 'extract' CHECK(phase IN ('extract','resolve','compare'));
ALTER TABLE linggan_topic_map_research_result ADD COLUMN core_json jsonb NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE linggan_topic_map_concept_rule (
 definition_ref uuid PRIMARY KEY REFERENCES linggan_topic_definition,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 identity_hash text NOT NULL CHECK(identity_hash ~ '^[0-9a-f]{64}$'),
 inclusion_criteria text[] NOT NULL CHECK(cardinality(inclusion_criteria) BETWEEN 1 AND 8),
 exclusion_criteria text[] NOT NULL CHECK(cardinality(exclusion_criteria) BETWEEN 1 AND 8),
 method_version text NOT NULL,
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX topic_map_concept_identity ON linggan_topic_map_concept_rule(domain_ref,identity_hash);
CREATE TABLE linggan_topic_map_discussion_unit (
 unit_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 unit_key text NOT NULL CHECK(unit_key ~ '^[0-9a-f]{64}$'),
 statement text NOT NULL CHECK(length(btrim(statement)) BETWEEN 1 AND 1000),
 speaker_role text NOT NULL CHECK(speaker_role IN ('author','commenter','quoted','unknown')),
 evidence_role text NOT NULL CHECK(evidence_role IN ('support','challenge','context')),
 evidence_refs jsonb NOT NULL CHECK(jsonb_typeof(evidence_refs)='array' AND jsonb_array_length(evidence_refs)>0),
 method_version text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(domain_ref,work_public_ref,unit_key)
);
CREATE TABLE linggan_topic_map_unit_resolution (
 resolution_ref uuid PRIMARY KEY,
 unit_ref uuid NOT NULL REFERENCES linggan_topic_map_discussion_unit,
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation,
 task_ref uuid NOT NULL REFERENCES linggan_topic_map_research_task,
 status text NOT NULL CHECK(status IN ('matched','new','uncertain','out_of_scope')),
 reason text NOT NULL CHECK(length(btrim(reason)) BETWEEN 1 AND 2000),
 candidate_manifest jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(unit_ref,invocation_ref)
);
CREATE TABLE linggan_topic_map_unit_assignment (
 resolution_ref uuid NOT NULL REFERENCES linggan_topic_map_unit_resolution,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 reason text NOT NULL CHECK(length(btrim(reason)) BETWEEN 1 AND 2000),
 PRIMARY KEY(resolution_ref,topic_ref)
);
CREATE TABLE linggan_topic_map_concept_relation (
 relation_ref uuid PRIMARY KEY,
 resolution_ref uuid NOT NULL REFERENCES linggan_topic_map_unit_resolution,
 source_topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 source_definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 target_topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 target_definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 relation text NOT NULL CHECK(relation IN ('broader','narrower','related','distinct','uncertain')),
 reason text NOT NULL CHECK(length(btrim(reason)) BETWEEN 1 AND 2000),
 CHECK(source_topic_ref <> target_topic_ref),
 UNIQUE(resolution_ref,source_topic_ref,target_topic_ref)
);
-- Reuses the qualified local model identity; the topic template has its own cache namespace.
CREATE TABLE linggan_topic_map_embedding (
 -- Profile is owned by the separately initialized Comment Study layer.
 -- This disposable cache never blocks that layer's reset; readers only use a currently qualified profile.
 profile_ref uuid NOT NULL,
 template_version text NOT NULL,
 text_hash text NOT NULL CHECK(text_hash ~ '^[0-9a-f]{64}$'),
 embedding public.vector(512) NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(profile_ref,template_version,text_hash)
);
CREATE INDEX topic_map_core_units_work ON linggan_topic_map_discussion_unit(domain_ref,work_public_ref);
CREATE INDEX topic_map_core_resolution_task ON linggan_topic_map_unit_resolution(task_ref,created_at);
CREATE INDEX topic_map_core_assignment_topic ON linggan_topic_map_unit_assignment(topic_ref,definition_ref);

-- A definition event recalls bounded existing discussions once. It never starts a fresh
-- extraction run or acquires a new budget just because a topic was added or revised.
CREATE TABLE linggan_topic_map_definition_scan (
 definition_ref uuid PRIMARY KEY REFERENCES linggan_topic_definition,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 reason text NOT NULL CHECK(reason IN ('definition_changed','new_topic')),
 recall_manifest jsonb NOT NULL DEFAULT '{}'::jsonb,
 scanned_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_backfill_queue (
 definition_ref uuid NOT NULL REFERENCES linggan_topic_map_definition_scan,
 source_task_ref uuid NOT NULL REFERENCES linggan_topic_map_research_task,
 unit_keys text[] NOT NULL CHECK(cardinality(unit_keys)>0),
 state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','active','completed','skipped')),
 queued_task_ref uuid REFERENCES linggan_topic_map_research_task,
 reason text NOT NULL,
 last_reason text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(definition_ref,source_task_ref)
);
CREATE INDEX topic_map_backfill_pending_idx ON linggan_topic_map_backfill_queue(state,created_at);
CREATE TRIGGER topic_map_concept_rule_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_concept_rule FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_discussion_unit_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_discussion_unit FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_unit_resolution_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_unit_resolution FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_unit_assignment_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_unit_assignment FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_concept_relation_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_concept_relation FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
