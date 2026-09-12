-- LOCAL-EMBEDDING-001: one local WeMM profile, Atom-only pgvector storage and exact cosine.
-- No model registry, HTTP service, queue or scheduling state is introduced here.

-- The application schema is normally `public`, while PostgreSQL proof tests deliberately use
-- one private schema per case.  Installing pgvector once in `public` and qualifying the type
-- below preserves both the production deployment and isolated-schema proof semantics.
CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public;

-- A local runtime is an auditable invocation source, not an OpenAI-compatible endpoint.  The
-- existing generic model ledger remains the receipt store, but this profile never reads a secret.
ALTER TABLE linggan_model_connection_version
  DROP CONSTRAINT linggan_model_connection_version_api_check;
ALTER TABLE linggan_model_connection_version
  ADD CONSTRAINT linggan_model_connection_version_api_check
  CHECK(api IN ('openai-completions','openai-responses','anthropic-messages','local-wemm-runtime'));
ALTER TABLE linggan_model_entry DROP CONSTRAINT linggan_model_entry_origin_check;
ALTER TABLE linggan_model_entry
  ADD CONSTRAINT linggan_model_entry_origin_check CHECK(origin IN ('manual','account_endpoint','local_runtime'));

INSERT INTO linggan_model_connection(connection_ref,enabled,revision)
VALUES ('1f849394-87aa-48b8-8616-e6a65ec8e952',true,1)
ON CONFLICT (connection_ref) DO NOTHING;
INSERT INTO linggan_model_connection_version(
  version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref
) VALUES (
  '4f7b584a-643a-4989-9671-4312bc11aa85','1f849394-87aa-48b8-8616-e6a65ec8e952',1,
  '本机 WeMM Runtime','local-wemm-runtime','local://wemm-embedding-2b',true,
  'e179d4a5-8cf4-40a8-9e91-0422f79ce44e'
) ON CONFLICT (version_ref) DO NOTHING;
INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)
VALUES (
  'a73e2e25-b20a-426c-bf20-a062564dce9c','4f7b584a-643a-4989-9671-4312bc11aa85',
  'Tencent/WeMM-Embedding-2B','local_runtime'
) ON CONFLICT (model_ref) DO NOTHING;

CREATE TABLE linggan_comment_research_embedding_profile (
  singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
  profile_ref uuid NOT NULL UNIQUE,
  model_ref uuid NOT NULL REFERENCES linggan_model_entry(model_ref),
  model_id text NOT NULL CHECK(model_id='Tencent/WeMM-Embedding-2B'),
  model_revision text NOT NULL CHECK(model_revision='bbd6cd4bf52cfc6716f752a2df80b2706720bd95'),
  encoding_mode text NOT NULL CHECK(encoding_mode='document'),
  dimension integer NOT NULL CHECK(dimension=512),
  preprocessing_version text NOT NULL CHECK(preprocessing_version='comment-research.atom-canonical-text.v1'),
  enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_comment_research_embedding_profile(
  singleton,profile_ref,model_ref,model_id,model_revision,encoding_mode,dimension,preprocessing_version,enabled
) VALUES (
  true,'2a2d5d49-80cc-4465-a9e2-cb0d0d4a2bc4','a73e2e25-b20a-426c-bf20-a062564dce9c',
  'Tencent/WeMM-Embedding-2B','bbd6cd4bf52cfc6716f752a2df80b2706720bd95','document',512,
  'comment-research.atom-canonical-text.v1',true
);

-- This cutover intentionally has no vector-data conversion: current V1 has no persisted vector.
-- Refuse to pretend that legacy JSON arrays are pgvector if an operator applies it elsewhere.
DO $$
BEGIN
  IF EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding)
     OR EXISTS(SELECT 1 FROM linggan_comment_research_problem_definition_embedding) THEN
    RAISE EXCEPTION 'LOCAL-EMBEDDING-001 requires an empty V1 embedding cache; rebuild after explicit review';
  END IF;
END;
$$;

ALTER TABLE linggan_comment_research_embedding_space
  DROP CONSTRAINT linggan_comment_research_embedding_space_check,
  ADD COLUMN profile_ref uuid NOT NULL DEFAULT '2a2d5d49-80cc-4465-a9e2-cb0d0d4a2bc4'
    REFERENCES linggan_comment_research_embedding_profile(profile_ref),
  DROP COLUMN config_ref,
  DROP COLUMN model_ref,
  DROP CONSTRAINT linggan_comment_research_embedding_space_dimensions_check,
  ADD CONSTRAINT linggan_comment_research_embedding_space_dimensions_check CHECK(dimensions=512);
ALTER TABLE linggan_comment_research_embedding_space ALTER COLUMN profile_ref DROP DEFAULT;

ALTER TABLE linggan_comment_research_atom_embedding
  -- PostgreSQL derives names for anonymous multi-column CHECKs from different
  -- referenced columns across the historic V1 migration path.  Remove only
  -- the retired JSON-vector guards; the surviving state/input-hash guards are
  -- column constraints and do not need to be reconstructed here.
  DROP CONSTRAINT IF EXISTS linggan_comment_research_atom_embedding_check,
  DROP CONSTRAINT IF EXISTS linggan_comment_research_atom_embedding_vector_check,
  DROP CONSTRAINT IF EXISTS linggan_comment_research_atom_embedding_vector_check1,
  DROP CONSTRAINT IF EXISTS linggan_comment_research_atom_embedding_dimensions_check,
  DROP COLUMN vector,
  ADD COLUMN vector public.vector(512),
  ADD CONSTRAINT linggan_comment_research_atom_embedding_check
    CHECK((state='succeeded') = (dimensions IS NOT NULL AND vector IS NOT NULL)),
  ADD CONSTRAINT linggan_comment_research_atom_embedding_dimension_check CHECK(dimensions IS NULL OR dimensions=512);

-- A V1 vector is only an Atom canonical_text document embedding.  Problems are resolved by a
-- model after exact nearest-Atom recall, so definition-vector work is deliberately removed.
DROP TABLE linggan_comment_research_problem_definition_embedding;

COMMENT ON TABLE linggan_comment_research_embedding_profile IS
  'LOCAL-EMBEDDING-001 immutable identity: WeMM commit, document encoding, 512d and preprocessing version.';
COMMENT ON COLUMN linggan_comment_research_atom_embedding.vector IS
  'L2-normalized 512d WeMM document vector of Atom canonical_text; exact cosine only, no ANN index.';
