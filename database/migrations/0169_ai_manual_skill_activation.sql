-- Direct skill switching: trial remains optional and each activation is versioned.
ALTER TABLE linggan_skill_binding DROP CONSTRAINT linggan_skill_binding_approval_source_check;
ALTER TABLE linggan_skill_binding ADD CONSTRAINT linggan_skill_binding_approval_source_check
 CHECK(approval_source IN ('trial','first_version','prior_approval','manual'));
ALTER TABLE linggan_skill_binding_event DROP CONSTRAINT linggan_skill_binding_event_approval_source_check;
ALTER TABLE linggan_skill_binding_event ADD CONSTRAINT linggan_skill_binding_event_approval_source_check
 CHECK(approval_source IN ('trial','first_version','prior_approval','manual'));
CREATE OR REPLACE FUNCTION linggan_guard_skill_binding() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'skill_binding_delete_forbidden'; END IF;
 IF NEW.task_key<>OLD.task_key OR NEW.revision<>OLD.revision+1 OR NEW.command_ref IS NULL
  OR NEW.current_version_ref IS NULL OR NEW.reason IS NULL OR char_length(btrim(NEW.reason)) NOT BETWEEN 1 AND 300
  OR NEW.approval_source IS NULL OR NOT EXISTS(
   SELECT 1 FROM linggan_ai_task_config WHERE task_config_ref=NEW.activation_config_ref
    AND task_key=NEW.task_key AND skill_version_ref=NEW.current_version_ref)
 THEN RAISE EXCEPTION 'skill_binding_revision_invalid'; END IF;
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_task WHERE task_key=NEW.task_key AND skill_allowed)
 THEN RAISE EXCEPTION 'ai_skill_not_allowed_for_task'; END IF;
 IF NEW.approval_source='trial' THEN
  IF NOT linggan_ai_trial_qualifies(NEW.trial_comparison_ref,NEW.task_key,NEW.current_version_ref,NEW.activation_config_ref)
  THEN RAISE EXCEPTION 'skill_trial_required'; END IF;
 ELSIF NEW.approval_source<>'manual' THEN
  RAISE EXCEPTION 'skill_activation_source_invalid';
 END IF;
 INSERT INTO linggan_skill_binding_event(event_ref,task_key,revision,version_ref,activation_config_ref,
  approval_source,trial_comparison_ref,reason,created_at)
 VALUES(NEW.command_ref,NEW.task_key,NEW.revision,NEW.current_version_ref,NEW.activation_config_ref,
  NEW.approval_source,NEW.trial_comparison_ref,NEW.reason,NEW.updated_at);
 RETURN NEW;
END $$;
