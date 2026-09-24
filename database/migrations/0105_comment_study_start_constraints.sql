-- COMMENT-STUDY-PRODUCTIZATION-001 / P2 start constraints CANDIDATE.
-- Not registered for shared upgrade. Apply after 0103/0104 and worker drain, never reset.
DO $$ BEGIN
    IF to_regclass('linggan_comment_study_start_request') IS NULL
       OR to_regprocedure('cs_policy_validate_insert()') IS NULL THEN
        RAISE EXCEPTION 'comment_study_start_schema_prerequisite_missing';
    END IF;
END $$;

-- Only identities proven by each immutable source_ref are backfilled. No fingerprint invention.
UPDATE linggan_comment_study_target t SET comment_external_id=c.comment_external_id
FROM linggan_material_comment c WHERE c.material_ref=t.source_ref AND t.comment_external_id IS NULL;

ALTER TABLE linggan_comment_study_target DROP CONSTRAINT linggan_comment_study_target_state_check;
ALTER TABLE linggan_comment_study_target ADD CONSTRAINT cs_target_state_ck
    CHECK(state IN ('ready','needs_context','excluded','queued','running','succeeded','no_signal','failed','cancelled'));
CREATE UNIQUE INDEX cs_target_run_comment_uq
    ON linggan_comment_study_target(run_ref,content_public_ref,comment_external_id) WHERE input_fingerprint IS NOT NULL;
CREATE UNIQUE INDEX cs_target_active_comment_uq
    ON linggan_comment_study_target(content_public_ref,comment_external_id)
    WHERE input_fingerprint IS NOT NULL AND state IN ('ready','queued','running');
CREATE INDEX cs_target_comment_history_idx
    ON linggan_comment_study_target(content_public_ref,comment_external_id,created_at DESC,target_ref DESC);

CREATE FUNCTION cs_start_reject_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'comment_study_start_receipt_immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER cs_start_immutable BEFORE UPDATE OR DELETE ON linggan_comment_study_start_request
    FOR EACH ROW EXECUTE FUNCTION cs_start_reject_mutation();
CREATE TRIGGER cs_start_no_truncate BEFORE TRUNCATE ON linggan_comment_study_start_request
    FOR EACH STATEMENT EXECUTE FUNCTION cs_start_reject_mutation();

CREATE FUNCTION cs_run_validate_insert() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p linggan_comment_study_policy;
BEGIN
    SELECT * INTO p FROM linggan_comment_study_policy WHERE policy_ref=NEW.policy_ref;
    IF p.method_hash IS NULL OR NEW.comment_budget IS NULL OR NEW.context_character_budget IS NULL
       OR NEW.token_limit IS NULL OR NEW.state<>'queued' OR NEW.dispatch_state<>'enabled'
       OR NEW.dispatch_reason IS NOT NULL OR NEW.control_version<>0
       OR NEW.selection_manifest->>'contract' IS DISTINCT FROM 'comment-study.run-selection.v2'
       OR NEW.execution_manifest->>'contract' IS DISTINCT FROM 'comment-study.execution.v1'
       OR NEW.execution_manifest->>'methodHash' IS DISTINCT FROM p.method_hash
       OR NEW.execution_manifest->>'builderRevision' IS DISTINCT FROM p.method_manifest->>'builderRevision'
       OR NOT COALESCE(NEW.execution_manifest->>'engineRevision' ~ '^[0-9a-f]{40}$',false)
       OR NEW.execution_manifest->>'selectionOrder' IS DISTINCT FROM 'work_round_robin_oldest_first.v1'
       OR NEW.execution_manifest->'maxTargetsPerBatch' IS DISTINCT FROM '12'::jsonb
       OR NEW.selection_manifest->'commentBudget' IS DISTINCT FROM to_jsonb(NEW.comment_budget)
       OR NEW.selection_manifest->'contextCharacterBudget' IS DISTINCT FROM to_jsonb(NEW.context_character_budget)
       OR NEW.selection_manifest->'tokenLimit' IS DISTINCT FROM to_jsonb(NEW.token_limit)
       OR jsonb_typeof(NEW.selection_manifest->'requestedWorkRefs') IS DISTINCT FROM 'array'
       OR jsonb_typeof(NEW.selection_manifest->'coveredWorkRefs') IS DISTINCT FROM 'array'
       OR jsonb_typeof(NEW.selection_manifest->'targetSourceRefs') IS DISTINCT FROM 'array'
       OR jsonb_array_length(NEW.selection_manifest->'targetSourceRefs') NOT BETWEEN 1 AND NEW.comment_budget THEN
        RAISE EXCEPTION 'comment_study_run_frozen_contract_required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cs_run_insert_guard BEFORE INSERT ON linggan_comment_study_run
    FOR EACH ROW EXECUTE FUNCTION cs_run_validate_insert();
CREATE FUNCTION cs_run_protect_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'comment_study_run_immutable' USING ERRCODE='23514'; END IF;
    IF (to_jsonb(NEW)-ARRAY['state','finished_at','dispatch_state','dispatch_reason','control_version'])
       IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','finished_at','dispatch_state','dispatch_reason','control_version']) THEN
        RAISE EXCEPTION 'comment_study_run_frozen_immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cs_run_frozen_guard BEFORE UPDATE OR DELETE ON linggan_comment_study_run
    FOR EACH ROW EXECUTE FUNCTION cs_run_protect_frozen();

CREATE FUNCTION cs_target_validate_input() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE r linggan_comment_study_run;
BEGIN
    IF TG_OP='UPDATE' THEN
        IF (to_jsonb(NEW)-ARRAY['state','dependency_state','exclusion_reason','finished_at','terminal_reason'])
           IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','dependency_state','exclusion_reason','finished_at','terminal_reason']) THEN
            RAISE EXCEPTION 'comment_study_target_frozen_immutable' USING ERRCODE='23514';
        END IF;
        IF OLD.input_fingerprint IS NULL THEN RETURN NEW; END IF;
    ELSE
        SELECT * INTO r FROM linggan_comment_study_run WHERE run_ref=NEW.run_ref;
        IF NEW.comment_external_id IS NULL OR length(btrim(NEW.comment_external_id))=0
           OR octet_length(NEW.comment_external_id)>512 OR NEW.input_fingerprint IS NULL
           OR NEW.input_manifest->>'contract' IS DISTINCT FROM 'comment-study.target-input.v2'
           OR NEW.input_manifest->>'targetSourceRef' IS DISTINCT FROM NEW.source_ref::text
           OR NEW.input_manifest->>'workRef' IS DISTINCT FROM NEW.content_public_ref::text
           OR r.selection_manifest->>'contract' IS DISTINCT FROM 'comment-study.run-selection.v2'
           OR NOT EXISTS(SELECT 1 FROM linggan_material_comment c
                WHERE c.material_ref=NEW.source_ref AND c.content_public_ref=NEW.content_public_ref
                  AND c.comment_external_id=NEW.comment_external_id)
           OR NOT EXISTS(SELECT 1 FROM linggan_comment_study_work w
                WHERE w.run_ref=NEW.run_ref AND w.content_public_ref=NEW.content_public_ref
                  AND w.context_hash=NEW.input_manifest->>'workContextHash') THEN
            RAISE EXCEPTION 'comment_study_target_input_required' USING ERRCODE='23514';
        END IF;
    END IF;
    IF (NEW.state IN ('succeeded','no_signal','needs_context','failed','excluded','cancelled'))
        IS DISTINCT FROM (NEW.finished_at IS NOT NULL)
       OR (NEW.state IN ('succeeded','no_signal','needs_context') AND NEW.terminal_reason IS NOT NULL)
       OR (NEW.state='failed' AND NEW.terminal_reason IS NULL)
       OR (NEW.state='cancelled' AND NOT COALESCE(NEW.terminal_reason IN
           ('user_stopped','budget_exhausted','legacy_execution_stopped'),false)) THEN
        RAISE EXCEPTION 'comment_study_target_terminal_required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cs_target_input_guard BEFORE INSERT OR UPDATE ON linggan_comment_study_target
    FOR EACH ROW EXECUTE FUNCTION cs_target_validate_input();
