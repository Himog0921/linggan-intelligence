-- CI-AUTO-003: versioned, source-gated definition vectors, never an equivalence decision.
CREATE TABLE linggan_ci_definition_vector (
 vector_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 entity_kind text NOT NULL CHECK(entity_kind IN ('expression','problem')),
 entity_ref uuid NOT NULL,
 definition_fingerprint text NOT NULL CHECK(length(definition_fingerprint)=64),
 model_ref uuid NOT NULL REFERENCES linggan_model_entry(model_ref),
 dimensions integer NOT NULL CHECK(dimensions BETWEEN 1 AND 8192),
 embedding jsonb NOT NULL CHECK(jsonb_typeof(embedding)='array'),
 source_guards jsonb NOT NULL CHECK(jsonb_typeof(source_guards)='array'),
 invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation(invocation_ref),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(domain_ref,entity_kind,entity_ref,definition_fingerprint,model_ref),
 CHECK(jsonb_array_length(embedding)=dimensions)
);
CREATE INDEX ci_definition_vector_entity ON linggan_ci_definition_vector(domain_ref,entity_kind,entity_ref);

-- Acceptance must survive a worker crash before accounting finalization. This minimal
-- receipt contains decisions already persisted as research assets, never model raw output.
ALTER TABLE linggan_ci_problem_task
  ADD COLUMN applied_receipt jsonb,
  ADD COLUMN applied_at timestamptz,
  ADD CONSTRAINT problem_task_receipt_pair CHECK ((applied_receipt IS NULL) = (applied_at IS NULL));

-- Canonical UUIDs are scoped to the source domain. Rebuild only the disposable cursor;
-- raw comments, analyses, terms and decisions remain intact.
DELETE FROM linggan_ci_projection_cursor;
ALTER TABLE linggan_ci_projection_cursor
  ADD COLUMN domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
  DROP CONSTRAINT linggan_ci_projection_cursor_pkey,
  ADD PRIMARY KEY (canonical_ref, domain_ref);
ALTER TABLE linggan_ci_term_index
  DROP CONSTRAINT linggan_ci_term_index_pkey,
  ADD PRIMARY KEY (canonical_ref, domain_ref, term);
