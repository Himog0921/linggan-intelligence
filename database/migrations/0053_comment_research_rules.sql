-- CI-AUTO-004 P2: immutable research-rule revisions and one mutable active pointer.
CREATE TABLE linggan_comment_research_rule_revision (
    rule_revision_ref uuid PRIMARY KEY,
    rule_version text NOT NULL CHECK(rule_version IN ('comment-research.v4','comment-research.v5')),
    parent_rule_revision_ref uuid REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    purpose jsonb NOT NULL CHECK(jsonb_typeof(purpose)='object'),
    field_definitions jsonb NOT NULL CHECK(jsonb_typeof(field_definitions)='array'),
    examples jsonb NOT NULL CHECK(jsonb_typeof(examples)='array'),
    program_contract_version text NOT NULL CHECK(char_length(program_contract_version) BETWEEN 1 AND 100),
    schema_version text NOT NULL CHECK(char_length(schema_version) BETWEEN 1 AND 100),
    validator_version text NOT NULL CHECK(char_length(validator_version) BETWEEN 1 AND 100),
    selector_version text NOT NULL CHECK(char_length(selector_version) BETWEEN 1 AND 100),
    canonical_hash text NOT NULL UNIQUE CHECK(canonical_hash ~ '^[0-9a-f]{64}$'),
    creation_source text NOT NULL CHECK(creation_source IN ('system_seed','user_candidate')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK(
        (rule_version='comment-research.v4' AND parent_rule_revision_ref IS NULL)
        OR (rule_version='comment-research.v5' AND parent_rule_revision_ref IS NOT NULL)
    )
);

CREATE TRIGGER linggan_comment_research_rule_revision_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_rule_revision
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_research_rule_active (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision integer NOT NULL DEFAULT 0 CHECK(revision>=0),
    rule_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_rule_revision(rule_revision_ref),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

INSERT INTO linggan_comment_research_rule_revision(
    rule_revision_ref,rule_version,parent_rule_revision_ref,purpose,field_definitions,examples,
    program_contract_version,schema_version,validator_version,selector_version,canonical_hash,creation_source
) VALUES (
    '6e9cc99c-5f57-4a22-9d91-7c4a085e5004',
    'comment-research.v4',
    NULL,
    '{"title":"逐条评论表达提取","instruction":"只提取目标评论直接表达或经上下文消解的研究信号；不归并问题，不把作品内容当作评论者表达。"}'::jsonb,
    '[{"kind":"need","name":"需求","definition":"评论者明确提出的需求、期待或待解决事项。"},{"kind":"solution","name":"方案","definition":"评论者自述已经采用、正在采用或建议的具体方案。"},{"kind":"story","name":"经历","definition":"评论者描述的个人经历、场景或过程。"},{"kind":"quote","name":"典型表达","definition":"保留能够代表评论者原意的短表达，不替代原文证据。"}]'::jsonb,
    '[{"field":"need","polarity":"positive","comment":"我想知道第一步怎么做","expected":"提取 need：希望获得开始步骤。"},{"field":"solution","polarity":"negative","comment":"收藏了","expected":"不把收藏动作当作 solution 或 need。"}]'::jsonb,
    'comment-research.program.v1',
    'comment-extraction.schema.v4',
    'comment-packet-validator.v4',
    'comment-context.lexical-excerpts.v1',
    '2546c0faf2b4afc1467c8c2e68809466fae00776acac1303353574234a0c4f52',
    'system_seed'
);

INSERT INTO linggan_comment_research_rule_active(singleton,rule_revision_ref)
VALUES(true,'6e9cc99c-5f57-4a22-9d91-7c4a085e5004');
