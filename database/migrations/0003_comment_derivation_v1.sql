-- Comment derivation V1.
--
-- Cleaning never rewrites a captured comment. Each deterministic research
-- representation is an immutable child of exactly one CommentObservation.
-- There is deliberately no derivation-current projection: every reader must
-- join through comment_current_v0 to avoid presenting a stale body.

CREATE TABLE comment_derivation_v1 (
  id UUID PRIMARY KEY,
  comment_observation_id UUID NOT NULL
    REFERENCES comment_observation_v0(id) ON DELETE RESTRICT,
  cleaning_contract TEXT NOT NULL
    CHECK (cleaning_contract ~ '^comment-cleaning\.v[1-9][0-9]*$'),
  research_state TEXT NOT NULL
    CHECK (research_state IN ('dropped', 'analyzable', 'needs_context', 'anomaly')),
  research_text TEXT,
  -- PostgreSQL computes this from the stored derived text. It makes the
  -- materialized expression independently inspectable without retaining a raw
  -- source copy or exposing internal integrity data through the browser DTO.
  research_text_integrity_md5 CHAR(32) GENERATED ALWAYS AS (
    CASE WHEN research_text IS NULL THEN NULL ELSE md5(research_text) END
  ) STORED,
  reason_codes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  derived_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT comment_derivation_v1_reason_codes_known_check CHECK (
    reason_codes <@ ARRAY[
      'blank',
      'mention_only',
      'emoji_only',
      'mention_and_emoji_only',
      'punctuation_only',
      'no_effective_text',
      'control_character',
      'context_dependent_reply'
    ]::TEXT[]
  ),
  CONSTRAINT comment_derivation_v1_state_text_reason_check CHECK (
    (research_state = 'analyzable'
      AND research_text IS NOT NULL
      AND btrim(research_text) <> ''
      AND reason_codes = ARRAY[]::TEXT[])
    OR
    (research_state = 'needs_context'
      AND research_text IS NOT NULL
      AND btrim(research_text) <> ''
      AND reason_codes = ARRAY['context_dependent_reply']::TEXT[])
    OR
    (research_state = 'anomaly'
      AND research_text IS NULL
      AND reason_codes = ARRAY['control_character']::TEXT[])
    OR
    (research_state = 'dropped'
      AND research_text IS NULL
      AND cardinality(reason_codes) = 1
      AND reason_codes[1] IN (
        'blank',
        'mention_only',
        'emoji_only',
        'mention_and_emoji_only',
        'punctuation_only',
        'no_effective_text'
      ))
  ),
  CONSTRAINT comment_derivation_v1_observation_contract_key
    UNIQUE (comment_observation_id, cleaning_contract)
);

CREATE INDEX comment_derivation_v1_research_state_idx
  ON comment_derivation_v1(cleaning_contract, research_state);

CREATE TRIGGER comment_derivation_v1_append_only
  BEFORE UPDATE OR DELETE ON comment_derivation_v1
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();
