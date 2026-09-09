-- COMMENT-RESEARCH-RESET-001: versioned definition vectors for candidate recall only.
--
-- An atom vector recalls a bounded set of still-readable Problem definitions. It does not create
-- a membership, merge Problems, or make a publish decision. The definition revision and embedding
-- space are both part of the vector identity so changing either requires an explicit recompute.

CREATE TABLE linggan_comment_research_problem_definition_embedding (
    problem_ref uuid NOT NULL,
    definition_revision integer NOT NULL,
    space_ref uuid NOT NULL REFERENCES linggan_comment_research_embedding_space(space_ref),
    input_hash text NOT NULL CHECK(input_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK(state IN ('pending','running','succeeded','failed','incompatible')),
    dimensions integer,
    vector jsonb,
    invocation_ref uuid REFERENCES linggan_model_invocation(invocation_ref),
    failure_code text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY(problem_ref,definition_revision,space_ref,input_hash),
    FOREIGN KEY(problem_ref,definition_revision)
      REFERENCES linggan_comment_research_problem_definition(problem_ref,revision),
    CHECK((state='succeeded') = (dimensions IS NOT NULL AND vector IS NOT NULL)),
    CHECK(vector IS NULL OR jsonb_typeof(vector)='array'),
    CHECK(vector IS NULL OR jsonb_array_length(vector)=dimensions)
);

CREATE INDEX linggan_comment_research_definition_embedding_queue_idx
  ON linggan_comment_research_problem_definition_embedding(space_ref,state,created_at);

COMMENT ON TABLE linggan_comment_research_problem_definition_embedding IS
  'Problem definition embeddings scoped to one configured space; only exact cosine candidate recall may consume them.';
