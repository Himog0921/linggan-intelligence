-- COMMENT-STUDY-PRODUCTIZATION-001 / P3 bounded pair execution.
-- Adds a truthful terminal state for pair attempts that cannot produce an admissible result;
-- distinct from a semantic rejection.

DO $$ BEGIN
    IF to_regclass('linggan_comment_study_problem_pair') IS NULL THEN
        RAISE EXCEPTION 'comment_study_pair_schema_prerequisite_missing';
    END IF;
END $$;

DO $$
DECLARE
    item record;
BEGIN
    FOR item IN
        SELECT conname, pg_get_constraintdef(oid) AS definition
        FROM pg_constraint
        WHERE conrelid='linggan_comment_study_problem_pair'::regclass
          AND contype='c'
    LOOP
        IF item.definition LIKE '%pending%' AND item.definition LIKE '%approved%'
           AND item.definition LIKE '%rejected%' THEN
            EXECUTE format('ALTER TABLE linggan_comment_study_problem_pair DROP CONSTRAINT %I', item.conname);
        ELSIF item.definition LIKE '%resolved_at%' AND item.definition LIKE '%approved%'
              AND item.definition LIKE '%rejected%' THEN
            EXECUTE format('ALTER TABLE linggan_comment_study_problem_pair DROP CONSTRAINT %I', item.conname);
        END IF;
    END LOOP;
END $$;

ALTER TABLE linggan_comment_study_problem_pair
    ADD CONSTRAINT cs_problem_pair_state_ck
        CHECK (state IN ('pending','approved','rejected','failed')),
    ADD CONSTRAINT cs_problem_pair_terminal_ck
        CHECK ((state IN ('approved','rejected','failed')) = (resolved_at IS NOT NULL));
