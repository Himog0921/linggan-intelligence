-- Per-domain opt-in for queueing L1 content understanding. No domain is enabled by this migration;
-- task configuration, budgets, egress permission and cancellation remain authoritative.
CREATE TABLE linggan_content_understanding_automation (
 domain_ref uuid PRIMARY KEY REFERENCES observation_domain,
 enabled boolean NOT NULL DEFAULT false,
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
ALTER TABLE linggan_ai_batch DROP CONSTRAINT ai_batch_task_kind_valid;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe')
 OR (task_key IN('writing_legacy','creation_title','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe')
 OR (task_key='content_understanding' AND kind IN('interactive','automatic'))
 OR (task_key IN('content_understanding_deep','work_comparison','creator_research','creator_account_research','comment_study','comment_problem_resolution','comment_problem_pair_review') AND kind='interactive')
 OR (task_key IN('topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND kind IN('interactive','automatic')));
