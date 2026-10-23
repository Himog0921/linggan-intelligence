-- Research dossier v2: complete canonical context. No persisted character budget.
DO $migration$ BEGIN
  IF to_regclass('linggan_comment_study_policy') IS NOT NULL THEN
    ALTER TABLE linggan_comment_study_policy DROP COLUMN context_character_budget;
  END IF;
  IF to_regclass('linggan_comment_study_run') IS NOT NULL THEN
    ALTER TABLE linggan_comment_study_run DROP COLUMN context_character_budget;
  END IF;
  IF to_regprocedure('cs_run_validate_insert()') IS NOT NULL THEN
    EXECUTE $function$CREATE OR REPLACE FUNCTION cs_run_validate_insert() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p linggan_comment_study_policy;
BEGIN
    SELECT * INTO p FROM linggan_comment_study_policy WHERE policy_ref=NEW.policy_ref;
    IF p.method_hash IS NULL OR NEW.comment_budget IS NULL
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
       OR NEW.selection_manifest->'tokenLimit' IS DISTINCT FROM to_jsonb(NEW.token_limit)
       OR jsonb_typeof(NEW.selection_manifest->'requestedWorkRefs') IS DISTINCT FROM 'array'
       OR jsonb_typeof(NEW.selection_manifest->'coveredWorkRefs') IS DISTINCT FROM 'array'
       OR jsonb_typeof(NEW.selection_manifest->'targetSourceRefs') IS DISTINCT FROM 'array'
       OR jsonb_array_length(NEW.selection_manifest->'targetSourceRefs') NOT BETWEEN 1 AND NEW.comment_budget THEN
        RAISE EXCEPTION 'comment_study_run_frozen_contract_required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
$function$;
  END IF;
END $migration$;
