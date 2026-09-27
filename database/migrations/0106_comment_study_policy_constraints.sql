-- COMMENT-STUDY-PRODUCTIZATION-001 / constraints phase, policy portion only.
-- CANDIDATE: not registered for shared migration until the start/dispatch cutover is complete.
-- Apply transactionally after 0103. Does not update/delete historical policies or active pointer.
DO $$
BEGIN
    IF (SELECT count(*) FROM information_schema.columns
        WHERE table_schema=current_schema() AND table_name='linggan_comment_study_policy'
        AND (column_name,udt_name) IN
        (('method_name','text'),('method_manifest','jsonb'),('method_hash','text'),('parent_policy_ref','uuid'))) <> 4 THEN
        RAISE EXCEPTION 'comment_study_policy_schema_mismatch';
    END IF;
END $$;

CREATE FUNCTION cs_policy_reject_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'comment_study_policy_immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER cs_policy_immutable BEFORE UPDATE OR DELETE ON linggan_comment_study_policy
    FOR EACH ROW EXECUTE FUNCTION cs_policy_reject_mutation();
CREATE TRIGGER cs_policy_no_truncate BEFORE TRUNCATE ON linggan_comment_study_policy
    FOR EACH STATEMENT EXECUTE FUNCTION cs_policy_reject_mutation();

CREATE FUNCTION cs_policy_validate_insert() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    stage_name text;
    stage_value jsonb;
BEGIN
    IF NEW.method_name IS NULL OR NEW.method_manifest IS NULL OR NEW.method_hash IS NULL
        OR NEW.model_config_ref IS NULL
        OR NEW.method_name <> btrim(NEW.method_name)
        OR NEW.method_manifest->>'contract' IS DISTINCT FROM 'comment-study.method.v1'
        OR NEW.method_manifest->>'modelConfigRef' IS DISTINCT FROM NEW.model_config_ref::text
        OR jsonb_typeof(NEW.method_manifest->'stages') IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'comment_study_policy_method_required' USING ERRCODE='23514';
    END IF;
    FOREACH stage_name IN ARRAY ARRAY['semantic','resolution','pair'] LOOP
        stage_value := NEW.method_manifest->'stages'->stage_name;
        IF jsonb_typeof(stage_value) IS DISTINCT FROM 'object'
            OR jsonb_typeof(stage_value->'systemInstruction') IS DISTINCT FROM 'string'
            OR length(stage_value->>'systemInstruction')=0
            OR jsonb_typeof(stage_value->'outputSchema') IS DISTINCT FROM 'object'
            OR NOT COALESCE(stage_value->>'stageHash' ~ '^[0-9a-f]{64}$',false) THEN
            RAISE EXCEPTION 'comment_study_policy_stage_required' USING ERRCODE='23514';
        END IF;
    END LOOP;
    IF NEW.parent_policy_ref IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM linggan_comment_study_policy p
        WHERE p.policy_ref=NEW.parent_policy_ref AND p.domain_ref=NEW.domain_ref
            AND p.method_manifest IS NOT NULL AND p.method_hash IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'comment_study_policy_parent_unavailable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cs_policy_insert_guard BEFORE INSERT ON linggan_comment_study_policy
    FOR EACH ROW EXECUTE FUNCTION cs_policy_validate_insert();
CREATE INDEX cs_policy_domain_page_idx
    ON linggan_comment_study_policy(domain_ref,created_at DESC,policy_ref DESC);
