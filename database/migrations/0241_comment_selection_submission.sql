-- An existing schedule remains selection-only until an operator grants automatic submission.
ALTER TABLE linggan_comment_research_automation
    ADD COLUMN auto_submit boolean NOT NULL DEFAULT false,
    ADD CHECK (NOT auto_submit OR enabled);

ALTER TABLE linggan_comment_research_selection_target
    ADD COLUMN submission_state text NOT NULL DEFAULT 'manual'
        CHECK (submission_state IN ('manual','pending','waiting_l1','admission_blocked','skipped','submitted')),
    ADD COLUMN submission_reason text,
    ADD COLUMN next_check_at timestamptz,
    ADD COLUMN submitted_batch_ref uuid REFERENCES linggan_ai_batch,
    ADD CHECK ((submission_state='submitted') = (submitted_batch_ref IS NOT NULL)),
    ADD CHECK ((submission_state IN ('pending','waiting_l1','admission_blocked')) = (next_check_at IS NOT NULL));

CREATE INDEX linggan_comment_selection_next_submission
    ON linggan_comment_research_selection_target(next_check_at, domain_ref, selection_ref, position)
    WHERE next_check_at IS NOT NULL;

ALTER TABLE linggan_ai_batch DROP CONSTRAINT ai_batch_task_kind_valid;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe')
 OR (task_key IN('writing_legacy','creation_title','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe')
 OR (task_key='content_understanding' AND kind IN('interactive','automatic'))
 OR (task_key IN('ops_title_label','content_understanding_deep','work_comparison','creator_research','creator_account_research','comment_study','comment_problem_resolution','comment_problem_pair_review') AND kind='interactive')
 OR (task_key IN('comment_understanding','comment_problem_consolidation','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND kind IN('interactive','automatic')));
