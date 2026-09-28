-- COMMENT-STUDY-PRODUCTIZATION-001 / immutable membership lineage.
-- A membership must point to the exact Problem revision that was compared for its assignment.
-- Historical rows stay NULL unless their decision manifest proves that revision.

DO $$ BEGIN
    IF to_regclass('linggan_comment_study_problem_membership') IS NULL
       OR to_regclass('linggan_comment_study_problem_revision') IS NULL THEN
        RAISE EXCEPTION 'comment_study_membership_revision_schema_prerequisite_missing';
    END IF;
END $$;

ALTER TABLE linggan_comment_study_problem_membership
    ADD CONSTRAINT cs_membership_revision_problem_fk
        FOREIGN KEY(problem_revision_ref,problem_ref)
        REFERENCES linggan_comment_study_problem_revision(revision_ref,problem_ref);

CREATE FUNCTION cs_membership_revision_validate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' THEN
        IF NEW.signal_ref IS DISTINCT FROM OLD.signal_ref
            OR NEW.problem_ref IS DISTINCT FROM OLD.problem_ref
            OR NEW.problem_revision_ref IS DISTINCT FROM OLD.problem_revision_ref
            OR NEW.resolution_ref IS DISTINCT FROM OLD.resolution_ref THEN
            RAISE EXCEPTION 'comment_study_membership_immutable' USING ERRCODE='23514';
        END IF;
    END IF;
    IF EXISTS (
        SELECT 1 FROM linggan_comment_study_resolution resolution
        WHERE resolution.resolution_ref=NEW.resolution_ref
          AND resolution.signal_ref IS DISTINCT FROM NEW.signal_ref
    ) THEN
        RAISE EXCEPTION 'comment_study_membership_signal_resolution_mismatch'
            USING ERRCODE='23514';
    END IF;
    IF NEW.problem_revision_ref IS NULL OR NOT EXISTS (
        SELECT 1 FROM linggan_comment_study_resolution resolution
        WHERE resolution.resolution_ref=NEW.resolution_ref
          AND resolution.signal_ref=NEW.signal_ref
          AND resolution.state='assigned'
          AND resolution.resolved_problem_ref=NEW.problem_ref
          AND resolution.decision_manifest->>'problemRevisionRef'=NEW.problem_revision_ref::text
    ) THEN
        RAISE EXCEPTION 'comment_study_membership_revision_required' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER cs_membership_revision_guard
    BEFORE INSERT OR UPDATE ON linggan_comment_study_problem_membership
    FOR EACH ROW EXECUTE FUNCTION cs_membership_revision_validate();
