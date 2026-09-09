-- COMMENT-RESEARCH-RESET-001
--
-- 评论研究的唯一正式路径从不可变评论事实开始。旧 Task B / P4 表在切换前仍作为
-- 历史实现存在，但本 migration 不读取、不迁移、不双写它们；新代码只使用本文件的
-- `linggan_comment_research_*` 内核。
--
-- 这里故意不在 schema migration 中清空旧派生数据。共享开发库的清空是一次需要
-- 单独授权、精确依赖检查和 worker drain 回执的运行操作，而不是部署新 schema 时
-- 的副作用。见 COMMENT-RESEARCH-RESET-001 R5。

CREATE VIEW linggan_comment_research_source_current AS
SELECT current_comment.*
FROM linggan_material_comment_current current_comment
JOIN linggan_runtime_capture_package package
  ON package.package_ref = current_comment.package_ref
WHERE package.accepted_at <= scope_001_now()
  AND NOT EXISTS (
      SELECT 1
      FROM linggan_comment_research_restriction restriction
      WHERE restriction.content_public_ref = current_comment.content_public_ref
        AND restriction.comment_external_id = current_comment.comment_external_id
  );

COMMENT ON VIEW linggan_comment_research_source_current IS
  '当前、已接受且未受限的评论事实；ResearchDerivation 从这里读取，绝不改写 body_text。';

CREATE TABLE linggan_comment_research_policy_revision (
    policy_revision_ref uuid PRIMARY KEY,
    config_ref uuid REFERENCES linggan_model_config(config_ref),
    contract_version text NOT NULL CHECK(contract_version='comment-research.semantic.v1'),
    derivation_version text NOT NULL CHECK(char_length(derivation_version) BETWEEN 1 AND 100),
    extraction_rule_hash text NOT NULL CHECK(extraction_rule_hash ~ '^[0-9a-f]{64}$'),
    membership_policy_hash text NOT NULL CHECK(membership_policy_hash ~ '^[0-9a-f]{64}$'),
    source_limit integer NOT NULL CHECK(source_limit BETWEEN 1 AND 3000),
    token_limit bigint NOT NULL CHECK(token_limit BETWEEN 1024 AND 10000000),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_research_policy_active (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    revision bigint NOT NULL DEFAULT 0 CHECK(revision >= 0),
    policy_revision_ref uuid REFERENCES linggan_comment_research_policy_revision(policy_revision_ref),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_comment_research_policy_active(singleton) VALUES(true);

CREATE TRIGGER linggan_comment_research_policy_revision_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_policy_revision
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_research_derivation (
    derivation_ref uuid PRIMARY KEY,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    derivation_version text NOT NULL CHECK(char_length(derivation_version) BETWEEN 1 AND 100),
    source_sha256 text NOT NULL CHECK(source_sha256 ~ '^[0-9a-f]{64}$'),
    cleaner_version text NOT NULL CHECK(char_length(cleaner_version) BETWEEN 1 AND 100),
    clean_state text NOT NULL CHECK(clean_state IN ('direct','context','dropped','anomaly')),
    research_text text NOT NULL CHECK(char_length(research_text) <= 16000),
    research_sha256 text NOT NULL CHECK(research_sha256 ~ '^[0-9a-f]{64}$'),
    research_offsets jsonb NOT NULL CHECK(jsonb_typeof(research_offsets)='array'),
    normalization_reasons jsonb NOT NULL CHECK(jsonb_typeof(normalization_reasons)='array'),
    author_role text NOT NULL CHECK(author_role IN ('ordinary_user','content_author_reply','author_identity_unknown')),
    attribution_source text CHECK(attribution_source IN ('content_detail','profile_discovery')),
    attribution_observed_at timestamptz,
    eligibility text NOT NULL CHECK(eligibility IN ('eligible','excluded_author_reply','author_identity_unknown','source_body_unknown','dropped_or_anomalous')),
    eligibility_reason text NOT NULL CHECK(char_length(eligibility_reason) BETWEEN 1 AND 200),
    context_manifest jsonb NOT NULL CHECK(jsonb_typeof(context_manifest)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(source_ref,derivation_version,source_sha256),
    CHECK((eligibility <> 'eligible') OR char_length(research_text) > 0),
    CHECK((eligibility <> 'eligible') OR author_role='ordinary_user')
);
CREATE INDEX linggan_comment_research_derivation_source_idx
  ON linggan_comment_research_derivation(source_ref,created_at DESC);
CREATE INDEX linggan_comment_research_derivation_eligible_idx
  ON linggan_comment_research_derivation(eligibility,created_at)
  WHERE eligibility='eligible';
CREATE TRIGGER linggan_comment_research_derivation_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_derivation
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE VIEW linggan_comment_research_derivation_readable AS
SELECT derivation.*
FROM linggan_comment_research_derivation derivation
JOIN linggan_comment_research_readable source
  ON source.material_ref = derivation.source_ref;

COMMENT ON VIEW linggan_comment_research_derivation_readable IS
  '仍有可读原始来源的 ResearchDerivation；来源被限制后，不再允许进入 Run 或读取结果。';

CREATE TABLE linggan_comment_research_run (
    run_ref uuid PRIMARY KEY,
    policy_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_policy_revision(policy_revision_ref),
    state text NOT NULL CHECK(state IN ('preparing','queued','running','completed','completed_with_failures','failed','cancelled')),
    as_of timestamptz NOT NULL,
    scope jsonb NOT NULL CHECK(jsonb_typeof(scope)='object'),
    manifest_hash text CHECK(manifest_hash IS NULL OR manifest_hash ~ '^[0-9a-f]{64}$'),
    exclusion_counts jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(exclusion_counts)='object'),
    failure_counts jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(failure_counts)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    CHECK((state IN ('queued','running','completed','completed_with_failures','failed','cancelled')) = (manifest_hash IS NOT NULL)),
    CHECK((state IN ('completed','completed_with_failures','failed','cancelled')) = (finished_at IS NOT NULL))
);
CREATE INDEX linggan_comment_research_run_state_idx
  ON linggan_comment_research_run(state,created_at);

CREATE TABLE linggan_comment_research_run_item (
    run_ref uuid NOT NULL REFERENCES linggan_comment_research_run(run_ref),
    derivation_ref uuid NOT NULL REFERENCES linggan_comment_research_derivation(derivation_ref),
    state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','running','succeeded','no_signal','retryable','incompatible','unrecoverable','model_failed','restricted','cancelled')),
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0 AND attempts <= 3),
    next_attempt_at timestamptz,
    failure_code text,
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    context_hash text NOT NULL CHECK(context_hash ~ '^[0-9a-f]{64}$'),
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    PRIMARY KEY(run_ref,derivation_ref),
    CHECK((state IN ('succeeded','no_signal','incompatible','unrecoverable','model_failed','restricted','cancelled')) = (finished_at IS NOT NULL)),
    CHECK((state='retryable') = (next_attempt_at IS NOT NULL))
);
CREATE INDEX linggan_comment_research_run_item_queue_idx
  ON linggan_comment_research_run_item(run_ref,state,next_attempt_at,created_at);

CREATE FUNCTION linggan_comment_research_run_item_requires_eligible_derivation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM linggan_comment_research_derivation_readable derivation
        WHERE derivation.derivation_ref=NEW.derivation_ref
          AND derivation.eligibility='eligible'
    ) THEN
        RAISE EXCEPTION 'research run item requires an eligible, readable ordinary-user derivation';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER linggan_comment_research_run_item_eligible
BEFORE INSERT OR UPDATE OF derivation_ref ON linggan_comment_research_run_item
FOR EACH ROW EXECUTE FUNCTION linggan_comment_research_run_item_requires_eligible_derivation();

CREATE TABLE linggan_comment_research_atom (
    atom_ref uuid PRIMARY KEY,
    run_ref uuid NOT NULL,
    derivation_ref uuid NOT NULL,
    ordinal integer NOT NULL CHECK(ordinal >= 0),
    kind text NOT NULL CHECK(kind IN ('problem','need','solution','experience')),
    proposition text NOT NULL CHECK(char_length(proposition) BETWEEN 1 AND 1000),
    basis text NOT NULL CHECK(basis IN ('explicit','context_resolved')),
    research_start integer NOT NULL CHECK(research_start >= 0),
    research_end integer NOT NULL CHECK(research_end > research_start),
    source_start integer NOT NULL CHECK(source_start >= 0),
    source_end integer NOT NULL CHECK(source_end > source_start),
    rule_hash text NOT NULL CHECK(rule_hash ~ '^[0-9a-f]{64}$'),
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(run_ref,derivation_ref,ordinal),
    FOREIGN KEY(run_ref,derivation_ref)
      REFERENCES linggan_comment_research_run_item(run_ref,derivation_ref)
);
CREATE INDEX linggan_comment_research_atom_derivation_idx
  ON linggan_comment_research_atom(derivation_ref,kind,created_at);
CREATE TRIGGER linggan_comment_research_atom_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_atom
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_research_embedding_space (
    space_ref uuid PRIMARY KEY,
    config_ref uuid REFERENCES linggan_embedding_config(config_ref),
    model_ref uuid REFERENCES linggan_model_entry(model_ref),
    dimensions integer NOT NULL CHECK(dimensions BETWEEN 1 AND 8192),
    policy_hash text NOT NULL UNIQUE CHECK(policy_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK(config_ref IS NOT NULL OR model_ref IS NOT NULL)
);
CREATE TRIGGER linggan_comment_research_embedding_space_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_embedding_space
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_research_atom_embedding (
    atom_ref uuid NOT NULL REFERENCES linggan_comment_research_atom(atom_ref),
    space_ref uuid NOT NULL REFERENCES linggan_comment_research_embedding_space(space_ref),
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('pending','running','succeeded','failed','incompatible')),
    dimensions integer,
    vector jsonb,
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    failure_code text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY(atom_ref,space_ref,input_hash),
    CHECK((state='succeeded') = (dimensions IS NOT NULL AND vector IS NOT NULL)),
    CHECK(vector IS NULL OR jsonb_typeof(vector)='array'),
    CHECK(vector IS NULL OR jsonb_array_length(vector)=dimensions)
);
CREATE INDEX linggan_comment_research_atom_embedding_queue_idx
  ON linggan_comment_research_atom_embedding(space_ref,state,created_at);

CREATE TABLE linggan_comment_research_problem (
    problem_ref uuid PRIMARY KEY,
    state text NOT NULL DEFAULT 'active' CHECK(state IN ('active','retired')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    retired_at timestamptz
);

CREATE TABLE linggan_comment_research_problem_definition (
    problem_ref uuid NOT NULL REFERENCES linggan_comment_research_problem(problem_ref),
    revision integer NOT NULL CHECK(revision > 0),
    name text NOT NULL CHECK(char_length(name) BETWEEN 1 AND 120),
    meaning text NOT NULL CHECK(char_length(meaning) BETWEEN 1 AND 1000),
    definition_hash text NOT NULL CHECK(definition_hash ~ '^[0-9a-f]{64}$'),
    policy_hash text NOT NULL CHECK(policy_hash ~ '^[0-9a-f]{64}$'),
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY(problem_ref,revision),
    UNIQUE(problem_ref,definition_hash)
);
CREATE TRIGGER linggan_comment_research_problem_definition_immutable
BEFORE UPDATE OR DELETE ON linggan_comment_research_problem_definition
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_comment_research_atom_problem_membership (
    membership_ref uuid PRIMARY KEY,
    atom_ref uuid NOT NULL REFERENCES linggan_comment_research_atom(atom_ref),
    problem_ref uuid NOT NULL,
    definition_revision integer NOT NULL,
    relation text NOT NULL CHECK(relation='same'),
    basis text NOT NULL CHECK(basis IN ('deterministic','model_decision','manual')),
    policy_hash text NOT NULL CHECK(policy_hash ~ '^[0-9a-f]{64}$'),
    decision_evidence jsonb NOT NULL CHECK(jsonb_typeof(decision_evidence)='object'),
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    current boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY(problem_ref,definition_revision)
      REFERENCES linggan_comment_research_problem_definition(problem_ref,revision)
);
CREATE UNIQUE INDEX linggan_comment_research_one_current_membership
  ON linggan_comment_research_atom_problem_membership(atom_ref)
  WHERE current;
CREATE INDEX linggan_comment_research_membership_problem_idx
  ON linggan_comment_research_atom_problem_membership(problem_ref,definition_revision,created_at)
  WHERE current;

CREATE TABLE linggan_comment_research_result_revision (
    result_revision_ref uuid PRIMARY KEY,
    run_ref uuid NOT NULL UNIQUE REFERENCES linggan_comment_research_run(run_ref),
    state text NOT NULL CHECK(state IN ('building','published','unavailable')),
    as_of timestamptz NOT NULL,
    manifest_hash text NOT NULL CHECK(manifest_hash ~ '^[0-9a-f]{64}$'),
    policy_hashes jsonb NOT NULL CHECK(jsonb_typeof(policy_hashes)='object'),
    input_counts jsonb NOT NULL CHECK(jsonb_typeof(input_counts)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    published_at timestamptz,
    CHECK((state='published') = (published_at IS NOT NULL))
);
CREATE INDEX linggan_comment_research_result_published_idx
  ON linggan_comment_research_result_revision(published_at DESC)
  WHERE state='published';

CREATE TABLE linggan_comment_research_problem_window_stat (
    result_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_result_revision(result_revision_ref),
    problem_ref uuid NOT NULL REFERENCES linggan_comment_research_problem(problem_ref),
    definition_revision integer NOT NULL,
    window_kind text NOT NULL CHECK(window_kind IN ('baseline','current')),
    window_start timestamptz NOT NULL,
    window_end timestamptz NOT NULL,
    comment_count integer NOT NULL CHECK(comment_count >= 0),
    comment_denominator integer NOT NULL CHECK(comment_denominator >= 0),
    work_count integer NOT NULL CHECK(work_count >= 0),
    work_denominator integer NOT NULL CHECK(work_denominator >= 0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY(result_revision_ref,problem_ref,definition_revision,window_kind),
    FOREIGN KEY(problem_ref,definition_revision)
      REFERENCES linggan_comment_research_problem_definition(problem_ref,revision),
    CHECK(window_end > window_start),
    CHECK(comment_count <= comment_denominator),
    CHECK(work_count <= work_denominator)
);

CREATE TABLE linggan_comment_research_change_observation (
    observation_ref uuid PRIMARY KEY,
    result_revision_ref uuid NOT NULL REFERENCES linggan_comment_research_result_revision(result_revision_ref),
    problem_ref uuid NOT NULL REFERENCES linggan_comment_research_problem(problem_ref),
    definition_revision integer NOT NULL,
    kind text CHECK(kind IN ('rising','falling','spreading','newly_observed')),
    status text NOT NULL CHECK(status IN ('published','not_comparable')),
    reason_code text NOT NULL CHECK(char_length(reason_code) BETWEEN 1 AND 200),
    evidence jsonb NOT NULL CHECK(jsonb_typeof(evidence)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(result_revision_ref,problem_ref,definition_revision),
    FOREIGN KEY(problem_ref,definition_revision)
      REFERENCES linggan_comment_research_problem_definition(problem_ref,revision),
    CHECK((status='published') = (kind IS NOT NULL))
);

CREATE VIEW linggan_comment_research_result_revision_readable AS
SELECT result.*
FROM linggan_comment_research_result_revision result
WHERE result.state='published'
  AND NOT EXISTS (
      SELECT 1
      FROM linggan_comment_research_run_item item
      JOIN linggan_comment_research_derivation derivation
        ON derivation.derivation_ref=item.derivation_ref
      WHERE item.run_ref=result.run_ref
        AND NOT EXISTS (
            SELECT 1
            FROM linggan_comment_research_readable source
            WHERE source.material_ref=derivation.source_ref
        )
  );

COMMENT ON VIEW linggan_comment_research_result_revision_readable IS
  '只返回每个输入来源仍可读的已发布结果；任一来源受限后整版结果不可读，等待重算。';
