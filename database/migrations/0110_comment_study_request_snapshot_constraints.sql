-- COMMENT-STUDY-PRODUCTIZATION-001 / P2 request snapshot protection.
-- Applied after 0109; protects the immutable request manifest.

DO $$ BEGIN
    IF to_regclass('linggan_comment_study_model_request') IS NULL THEN
        RAISE EXCEPTION 'comment_study_model_request_schema_prerequisite_missing';
    END IF;
END $$;

CREATE FUNCTION cs_model_request_protect() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'comment_study_model_request_immutable' USING ERRCODE='23514';
    END IF;
    IF (to_jsonb(NEW)-'dispatch_started_at') IS DISTINCT FROM (to_jsonb(OLD)-'dispatch_started_at')
       OR (OLD.dispatch_started_at IS NOT NULL AND NEW.dispatch_started_at IS DISTINCT FROM OLD.dispatch_started_at)
       OR (OLD.dispatch_started_at IS NULL AND NEW.dispatch_started_at IS NULL
           AND to_jsonb(NEW) IS DISTINCT FROM to_jsonb(OLD)) THEN
        RAISE EXCEPTION 'comment_study_model_request_frozen' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER cs_request_immutable BEFORE UPDATE OR DELETE ON linggan_comment_study_model_request
    FOR EACH ROW EXECUTE FUNCTION cs_model_request_protect();
CREATE FUNCTION cs_model_request_no_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'comment_study_model_request_immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER cs_request_no_truncate BEFORE TRUNCATE ON linggan_comment_study_model_request
    FOR EACH STATEMENT EXECUTE FUNCTION cs_model_request_no_truncate();
