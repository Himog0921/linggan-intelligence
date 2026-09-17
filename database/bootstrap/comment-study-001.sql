-- COMMENT-STUDY-001 · clean-development initialization
--
-- This file is intentionally not a migration. It initializes only the replacement comment-study
-- derived layer after the caller has performed the separately authorized local reset. Raw Evidence
-- tables are referenced, never altered, deleted, or copied into an unqualified store.

CREATE TABLE linggan_comment_study_reset_receipt (
    receipt_ref uuid PRIMARY KEY,
    reset_scope text NOT NULL CHECK(reset_scope='local_comment_study_derived_only'),
    old_relation_counts jsonb NOT NULL CHECK(jsonb_typeof(old_relation_counts)='object'),
    preserved_relation_counts jsonb NOT NULL CHECK(jsonb_typeof(preserved_relation_counts)='object'),
    requested_by text NOT NULL CHECK(char_length(requested_by) BETWEEN 1 AND 200),
    completed_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE IF NOT EXISTS linggan_material_comment_restriction (
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    restricted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
    PRIMARY KEY(content_public_ref,comment_external_id)
);

CREATE TABLE linggan_comment_study_policy (
    policy_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    model_config_ref uuid REFERENCES linggan_model_config(config_ref),
    contract text NOT NULL CHECK(contract='comment-study.v1'),
    comment_budget integer NOT NULL CHECK(comment_budget BETWEEN 1 AND 3000),
    context_character_budget integer NOT NULL CHECK(context_character_budget BETWEEN 1 AND 20000),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_study_active_policy (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    policy_ref uuid NOT NULL REFERENCES linggan_comment_study_policy(policy_ref),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_study_run (
    run_ref uuid PRIMARY KEY,
    policy_ref uuid NOT NULL REFERENCES linggan_comment_study_policy(policy_ref),
    as_of timestamptz NOT NULL,
    state text NOT NULL CHECK(state IN ('prepared','queued','running','completed','completed_with_failures','cancelled')),
    selection_manifest jsonb NOT NULL CHECK(jsonb_typeof(selection_manifest)='object'),
    selection_hash text NOT NULL CHECK(selection_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    CHECK((state IN ('completed','completed_with_failures','cancelled')) = (finished_at IS NOT NULL))
);

CREATE TABLE linggan_comment_study_work (
    run_ref uuid NOT NULL REFERENCES linggan_comment_study_run(run_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    selection_reason text NOT NULL CHECK(selection_reason IN ('user_selected','budget_selected')),
    context_state text NOT NULL CHECK(context_state IN ('ready','partial','missing')),
    context_manifest jsonb NOT NULL CHECK(jsonb_typeof(context_manifest)='object'),
    context_hash text NOT NULL CHECK(context_hash ~ '^[0-9a-f]{64}$'),
    PRIMARY KEY(run_ref,content_public_ref),
    CHECK(context_manifest->>'workRef'=content_public_ref::text)
);

CREATE TABLE linggan_comment_study_target (
    target_ref uuid PRIMARY KEY,
    run_ref uuid NOT NULL,
    content_public_ref uuid NOT NULL,
    source_ref uuid NOT NULL REFERENCES linggan_material_comment(material_ref),
    parent_source_ref uuid REFERENCES linggan_material_comment(material_ref),
    research_text text NOT NULL CHECK(char_length(research_text) BETWEEN 1 AND 16000),
    research_sha256 text NOT NULL CHECK(research_sha256 ~ '^[0-9a-f]{64}$'),
    dependency_state text NOT NULL CHECK(dependency_state IN ('self_contained','parent_available','parent_required_missing','input_invalid')),
    state text NOT NULL CHECK(state IN ('ready','needs_context','excluded','queued','running','succeeded','no_signal','failed')),
    exclusion_reason text,
    input_manifest jsonb NOT NULL CHECK(jsonb_typeof(input_manifest)='object'),
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(run_ref,source_ref),
    FOREIGN KEY(run_ref,content_public_ref)
        REFERENCES linggan_comment_study_work(run_ref,content_public_ref),
    CHECK((state='excluded') = (exclusion_reason IS NOT NULL)),
    CHECK((dependency_state='input_invalid') = (state='excluded'))
);

CREATE INDEX linggan_comment_study_target_queue_idx
    ON linggan_comment_study_target(run_ref,state,created_at);

CREATE TABLE linggan_comment_study_batch (
    batch_ref uuid PRIMARY KEY,
    run_ref uuid NOT NULL,
    content_public_ref uuid NOT NULL,
    state text NOT NULL CHECK(state IN ('prepared','leased','accepted','completed_with_failures','failed','cancelled')),
    input_manifest jsonb NOT NULL CHECK(jsonb_typeof(input_manifest)='object'),
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    output_manifest jsonb,
    model_invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    lease_token uuid,
    leased_by uuid,
    lease_expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    FOREIGN KEY(run_ref,content_public_ref)
        REFERENCES linggan_comment_study_work(run_ref,content_public_ref),
    CHECK((state IN ('accepted','completed_with_failures','failed','cancelled')) = (finished_at IS NOT NULL)),
    CHECK((state='leased') = (lease_token IS NOT NULL AND leased_by IS NOT NULL AND lease_expires_at IS NOT NULL))
);

CREATE TABLE linggan_comment_study_batch_target (
    batch_ref uuid NOT NULL REFERENCES linggan_comment_study_batch(batch_ref),
    target_ref uuid NOT NULL REFERENCES linggan_comment_study_target(target_ref),
    ordinal integer NOT NULL CHECK(ordinal >= 1),
    PRIMARY KEY(batch_ref,target_ref),
    UNIQUE(batch_ref,ordinal)
);

CREATE INDEX linggan_comment_study_batch_pending_idx
    ON linggan_comment_study_batch(run_ref,state,created_at)
    WHERE state IN ('prepared','leased');

CREATE TABLE linggan_comment_study_semantic_attempt (
    attempt_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES linggan_comment_study_target(target_ref),
    batch_ref uuid REFERENCES linggan_comment_study_batch(batch_ref),
    attempt_ordinal integer NOT NULL CHECK(attempt_ordinal BETWEEN 1 AND 3),
    model_invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('pending','leased','accepted','rejected','failed','cancelled')),
    output_manifest jsonb,
    rejection_code text CHECK(rejection_code IN (
        'semantic_json_schema','semantic_contract','evidence_not_contiguous',
        'evidence_ambiguous','unsupported_problem_frame','semantic_batch_contract',
        'semantic_target_missing','provider_failure'
    )),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    UNIQUE(target_ref,attempt_ordinal),
    CHECK((state IN ('accepted','rejected','failed','cancelled')) = (finished_at IS NOT NULL)),
    CHECK(state <> 'accepted' OR output_manifest IS NOT NULL),
    CHECK((state='rejected') = (rejection_code IS NOT NULL))
);

CREATE TABLE linggan_comment_study_signal (
    signal_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES linggan_comment_study_target(target_ref),
    semantic_attempt_ref uuid NOT NULL REFERENCES linggan_comment_study_semantic_attempt(attempt_ref),
    kind text NOT NULL CHECK(kind IN ('problem','need','belief','emotion','experience','solution','quote','context','question')),
    proposition text NOT NULL CHECK(char_length(proposition) BETWEEN 1 AND 1000),
    evidence text NOT NULL CHECK(char_length(evidence) BETWEEN 1 AND 1000),
    evidence_start integer NOT NULL CHECK(evidence_start >= 0),
    evidence_end integer NOT NULL CHECK(evidence_end > evidence_start),
    problem_frame jsonb CHECK(problem_frame IS NULL OR jsonb_typeof(problem_frame)='object'),
    eligibility_state text NOT NULL CHECK(eligibility_state IN ('eligible','deferred_context','not_user_problem','not_applicable')),
    eligibility_reason text CHECK(char_length(eligibility_reason) BETWEEN 1 AND 200),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(target_ref,kind,evidence_start,evidence_end),
    CHECK((kind IN ('problem','need')) = (problem_frame IS NOT NULL)),
    CHECK((kind IN ('problem','need')) = (eligibility_state <> 'not_applicable')),
    CHECK(eligibility_state <> 'not_applicable' OR eligibility_reason IS NULL)
);

CREATE TABLE linggan_comment_study_problem (
    problem_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    definition text NOT NULL CHECK(char_length(definition) BETWEEN 1 AND 1000),
    stable_identity jsonb NOT NULL CHECK(jsonb_typeof(stable_identity)='object'),
    include_criteria jsonb NOT NULL CHECK(jsonb_typeof(include_criteria)='array'),
    exclude_criteria jsonb NOT NULL CHECK(jsonb_typeof(exclude_criteria)='array'),
    definition_hash text NOT NULL CHECK(definition_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('active','retired')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    retired_at timestamptz,
    UNIQUE(domain_ref,definition_hash),
    CHECK((state='retired') = (retired_at IS NOT NULL))
);

CREATE TABLE linggan_comment_study_resolution (
    resolution_ref uuid PRIMARY KEY,
    signal_ref uuid NOT NULL UNIQUE REFERENCES linggan_comment_study_signal(signal_ref),
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    state text NOT NULL CHECK(state IN (
        'pending','assigned','deferred_context','deferred_ambiguous','deferred_novel',
        'not_user_problem','protocol_rejected','failed'
    )),
    candidate_manifest jsonb NOT NULL CHECK(jsonb_typeof(candidate_manifest)='object'),
    decision_manifest jsonb,
    model_invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    resolved_problem_ref uuid REFERENCES linggan_comment_study_problem(problem_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    resolved_at timestamptz,
    CHECK((state='assigned') = (resolved_problem_ref IS NOT NULL)),
    CHECK((state IN ('assigned','deferred_context','deferred_ambiguous','deferred_novel',
        'not_user_problem','protocol_rejected','failed')) = (resolved_at IS NOT NULL)),
    CHECK(state <> 'pending' OR (decision_manifest IS NULL AND resolved_problem_ref IS NULL))
);

CREATE TABLE linggan_comment_study_problem_membership (
    membership_ref uuid PRIMARY KEY,
    signal_ref uuid NOT NULL UNIQUE REFERENCES linggan_comment_study_signal(signal_ref),
    problem_ref uuid NOT NULL REFERENCES linggan_comment_study_problem(problem_ref),
    resolution_ref uuid NOT NULL UNIQUE REFERENCES linggan_comment_study_resolution(resolution_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_comment_study_problem_pair (
    pair_ref uuid PRIMARY KEY,
    first_signal_ref uuid NOT NULL REFERENCES linggan_comment_study_signal(signal_ref),
    second_signal_ref uuid NOT NULL REFERENCES linggan_comment_study_signal(signal_ref),
    state text NOT NULL CHECK(state IN ('pending','approved','rejected')),
    pair_manifest jsonb NOT NULL CHECK(jsonb_typeof(pair_manifest)='object'),
    proposed_problem jsonb CHECK(proposed_problem IS NULL OR jsonb_typeof(proposed_problem)='object'),
    model_invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation(invocation_ref),
    created_problem_ref uuid REFERENCES linggan_comment_study_problem(problem_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    resolved_at timestamptz,
    UNIQUE(first_signal_ref,second_signal_ref),
    CHECK(first_signal_ref <> second_signal_ref),
    CHECK((state='approved') = (created_problem_ref IS NOT NULL)),
    CHECK((state IN ('approved','rejected')) = (resolved_at IS NOT NULL))
);

CREATE INDEX linggan_comment_study_resolution_pending_idx
    ON linggan_comment_study_resolution(domain_ref,state,created_at)
    WHERE state='pending';

CREATE INDEX linggan_comment_study_problem_active_idx
    ON linggan_comment_study_problem(domain_ref,created_at)
    WHERE state='active';

COMMENT ON TABLE linggan_comment_study_work IS
  'A domain-qualified work selected for one user-confirmed StudyRun. Its context manifest records only material that passed the same readability gate as Evidence.';
COMMENT ON TABLE linggan_comment_study_target IS
  'A frozen current-comment target. Work and parent context can explain references but never replace this target as Signal evidence.';
COMMENT ON TABLE linggan_comment_study_batch IS
  'One frozen, same-work model envelope. Batch membership explains a shared invocation but never lets one target comment serve as another target’s evidence.';
COMMENT ON TABLE linggan_comment_study_semantic_attempt IS
  'One bounded model-output acceptance attempt. A malformed output is rejected as a whole and creates no StudySignal.';
COMMENT ON TABLE linggan_comment_study_problem IS
  'A durable user-problem definition with explicit inclusion and exclusion boundaries. It is never a per-comment title.';
COMMENT ON TABLE linggan_comment_study_resolution IS
  'A closed candidate comparison for one eligible Problem or Need signal. No candidate match yields deferred_novel, never immediate Problem creation.';
COMMENT ON TABLE linggan_comment_study_problem_pair IS
  'A candidate new Problem requires two independently authored source comments and an approved shared definition.';
COMMENT ON TABLE linggan_material_comment_restriction IS
  'Qualification facts that prevent a comment from becoming a Study target; this is raw material eligibility, not research output.';
