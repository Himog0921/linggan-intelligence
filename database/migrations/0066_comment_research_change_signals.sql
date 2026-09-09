-- COMMENT-RESEARCH-RESET-001: a Problem may have more than one independently evidenced change.
--
-- A single `(result, problem, definition)` uniqueness rule would force the system to hide either
-- a rising share or wider work coverage. Published signals are therefore unique by kind; an
-- incomparable Problem remains one explicit, kindless record with its reason.

DO $$
DECLARE
    constraint_name text;
BEGIN
    SELECT constraint_row.conname INTO constraint_name
    FROM pg_constraint constraint_row
    WHERE constraint_row.conrelid='linggan_comment_research_change_observation'::regclass
      AND constraint_row.contype='u'
      AND constraint_row.conkey=ARRAY[
          (SELECT attnum FROM pg_attribute WHERE attrelid='linggan_comment_research_change_observation'::regclass AND attname='result_revision_ref'),
          (SELECT attnum FROM pg_attribute WHERE attrelid='linggan_comment_research_change_observation'::regclass AND attname='problem_ref'),
          (SELECT attnum FROM pg_attribute WHERE attrelid='linggan_comment_research_change_observation'::regclass AND attname='definition_revision')
      ];
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE linggan_comment_research_change_observation DROP CONSTRAINT %I', constraint_name);
    END IF;
END;
$$;

CREATE UNIQUE INDEX linggan_comment_research_change_signal_unique
  ON linggan_comment_research_change_observation(
      result_revision_ref,problem_ref,definition_revision,kind
  )
  WHERE status='published';

CREATE UNIQUE INDEX linggan_comment_research_change_not_comparable_unique
  ON linggan_comment_research_change_observation(
      result_revision_ref,problem_ref,definition_revision
  )
  WHERE status='not_comparable';

COMMENT ON TABLE linggan_comment_research_change_observation IS
  'Published change signals are independently evidenced; not_comparable records state why a Problem has no valid comparison.';
