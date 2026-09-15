-- COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2
--
-- A Stable Problem is a durable, scoped user obstacle, not an Atom title. This migration adds
-- the evidence-frame and closed-world decision record without rewriting V1 evidence, atoms,
-- memberships, definitions, or execution history. A previously saved V1 policy remains an
-- immutable historical policy and is deliberately rejected by the V2 start boundary until saved
-- again under the explicit ADHD scope below.

ALTER TABLE linggan_comment_research_policy_revision
  ADD COLUMN problem_resolution_contract text,
  ADD COLUMN problem_scope_domain_ref uuid REFERENCES observation_domain(domain_ref),
  ADD COLUMN problem_scope_definition jsonb,
  ADD COLUMN problem_scope_hash text;

ALTER TABLE linggan_comment_research_policy_revision
  ADD CONSTRAINT linggan_comment_research_policy_resolution_v2_shape CHECK (
    (problem_resolution_contract IS NULL
      AND problem_scope_domain_ref IS NULL
      AND problem_scope_definition IS NULL
      AND problem_scope_hash IS NULL)
    OR (
      problem_resolution_contract='comment-research.problem-resolution.v2'
      AND problem_scope_domain_ref IS NOT NULL
      AND jsonb_typeof(problem_scope_definition)='object'
      AND problem_scope_hash ~ '^[0-9a-f]{64}$'
    )
  );

COMMENT ON COLUMN linggan_comment_research_policy_revision.problem_scope_definition IS
  'Frozen admissibility boundary. V2 currently names ADHD, but does not infer commentator role or diagnosis.';

ALTER TABLE linggan_comment_research_atom
  DROP CONSTRAINT linggan_comment_research_atom_kind_check,
  ADD CONSTRAINT linggan_comment_research_atom_kind_check
    CHECK(kind IN ('problem','need','belief','emotion','experience','solution','quote','context','question')),
  ADD COLUMN problem_frame jsonb,
  ADD COLUMN problem_frame_hash text;

ALTER TABLE linggan_comment_research_atom
  ADD CONSTRAINT linggan_comment_research_atom_problem_frame_shape CHECK (
    (problem_frame IS NULL AND problem_frame_hash IS NULL)
    OR (jsonb_typeof(problem_frame)='object' AND problem_frame_hash ~ '^[0-9a-f]{64}$')
  );

COMMENT ON COLUMN linggan_comment_research_atom.problem_frame IS
  'V2 evidence-grounded frame for a problem/need Atom. Null is an honest V1 historical value, not an empty frame.';

ALTER TABLE linggan_comment_research_problem_definition
  ADD COLUMN stable_identity jsonb,
  ADD COLUMN scope_domain_ref uuid REFERENCES observation_domain(domain_ref);

ALTER TABLE linggan_comment_research_problem_definition
  ADD CONSTRAINT linggan_comment_research_problem_definition_v2_shape CHECK (
    (stable_identity IS NULL AND scope_domain_ref IS NULL)
    OR (jsonb_typeof(stable_identity)='object' AND scope_domain_ref IS NOT NULL)
  );

COMMENT ON COLUMN linggan_comment_research_problem_definition.stable_identity IS
  'V2 identity with definition, include and exclude boundaries. Legacy name/meaning rows are not silently enriched.';

-- One cheap, transactional revision guard per scope. It prevents a pair of deferred signals
-- from creating a Problem after another worker has changed the catalog they compared against.
CREATE TABLE linggan_comment_research_problem_catalog_guard (
    scope_domain_ref uuid PRIMARY KEY REFERENCES observation_domain(domain_ref),
    revision bigint NOT NULL DEFAULT 0 CHECK(revision >= 0),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

INSERT INTO linggan_comment_research_problem_catalog_guard(scope_domain_ref)
VALUES ('00000000-0000-4000-8000-000000000001');

COMMENT ON TABLE linggan_comment_research_problem_catalog_guard IS
  'Scoped catalog revision used only to fence V2 pair creation; it is not a product entity.';

ALTER TABLE linggan_comment_research_problem_resolution
  ADD COLUMN decision_kind text,
  ADD COLUMN decision_payload jsonb,
  ADD COLUMN recheck_conditions jsonb,
  ADD COLUMN resolution_input_hash text,
  ADD COLUMN catalog_revision_at_recall bigint;

ALTER TABLE linggan_comment_research_problem_resolution
  ADD CONSTRAINT linggan_comment_research_problem_resolution_v2_decision_shape CHECK (
    (decision_kind IS NULL
      AND decision_payload IS NULL
      AND recheck_conditions IS NULL
      AND resolution_input_hash IS NULL
      AND catalog_revision_at_recall IS NULL)
    OR (
      decision_kind IN (
        'existing_problem','deferred_novel','deferred_ambiguous','deferred_context',
        'out_of_scope','not_user_problem','protocol_failure'
      )
      AND jsonb_typeof(decision_payload)='object'
      AND jsonb_typeof(recheck_conditions)='array'
      AND resolution_input_hash ~ '^[0-9a-f]{64}$'
      AND catalog_revision_at_recall >= 0
    )
  );

COMMENT ON COLUMN linggan_comment_research_problem_resolution.decision_kind IS
  'V2 durable disposition. deferred_novel is a completed decision without a membership, never a failed CREATE.';

ALTER TABLE linggan_comment_research_problem_resolution_execution
  ADD COLUMN decision_kind text,
  ADD COLUMN decision_payload jsonb,
  ADD COLUMN recheck_conditions jsonb;

ALTER TABLE linggan_comment_research_problem_resolution_execution
  ADD CONSTRAINT linggan_comment_research_problem_resolution_execution_v2_decision_shape CHECK (
    (decision_kind IS NULL AND decision_payload IS NULL AND recheck_conditions IS NULL)
    OR (
      decision_kind IN (
        'existing_problem','deferred_novel','deferred_ambiguous','deferred_context',
        'out_of_scope','not_user_problem','protocol_failure'
      )
      AND jsonb_typeof(decision_payload)='object'
      AND jsonb_typeof(recheck_conditions)='array'
    )
  );

CREATE INDEX linggan_comment_research_problem_resolution_v2_deferred_idx
  ON linggan_comment_research_problem_resolution(decision_kind,updated_at,atom_ref)
  WHERE decision_kind IN ('deferred_novel','deferred_ambiguous','deferred_context');
