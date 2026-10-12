-- COMMENT-LIKE-LIBRARY-001 / Issue #397: per-observation comment like counts and the team's
-- "post written from this comment" marks. Both tables are append-only evidence-adjacent facts.

-- One row per accepted comment observation (`linggan_material_comment` is append-only, so the
-- count cannot be added to it in place). Rule comment-like.v1, shared with material admission:
-- the platform renders zero likes as「赞」; a positive numeric `likes` is a known count; anything
-- else (including the producer's default "0" when no like element was found) stays UNKNOWN.
CREATE TABLE linggan_material_comment_engagement (
  material_ref uuid PRIMARY KEY REFERENCES linggan_material_comment(material_ref),
  like_count bigint,
  like_count_state text NOT NULL,
  like_source_text text,
  parser_version text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT scope_001_now(),
  CONSTRAINT linggan_material_comment_engagement_state_check
    CHECK (like_count_state IN ('KNOWN', 'UNKNOWN')),
  CONSTRAINT linggan_material_comment_engagement_known_check
    CHECK ((like_count_state = 'KNOWN') = (like_count IS NOT NULL)),
  CONSTRAINT linggan_material_comment_engagement_nonnegative_check
    CHECK (like_count IS NULL OR like_count >= 0),
  CONSTRAINT linggan_material_comment_engagement_source_text_check
    CHECK (like_source_text IS NULL OR length(like_source_text) BETWEEN 1 AND 32),
  CONSTRAINT linggan_material_comment_engagement_parser_check
    CHECK (parser_version = 'comment-like.v1')
);

CREATE TRIGGER linggan_material_comment_engagement_is_append_only
  BEFORE DELETE OR UPDATE ON linggan_material_comment_engagement
  FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE linggan_material_comment_engagement IS
  'Comment like count as observed in one accepted package record; never inferred from the work.';

-- Insert-only backfill from the immutable package records. The record identity guard skips any
-- row whose stored record does not carry the same comment id; such comments stay unknown.
INSERT INTO linggan_material_comment_engagement
  (material_ref, like_count, like_count_state, like_source_text, parser_version)
SELECT comment.material_ref,
       CASE WHEN parsed.like_text = '赞' THEN 0
            WHEN parsed.likes > 0 THEN parsed.likes END,
       CASE WHEN parsed.like_text = '赞' OR parsed.likes > 0 THEN 'KNOWN' ELSE 'UNKNOWN' END,
       NULLIF(left(parsed.like_text, 32), ''),
       'comment-like.v1'
FROM linggan_material_comment comment
JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
CROSS JOIN LATERAL (
  SELECT package.payload -> 'records' -> comment.record_ordinal -> 'payload' AS payload
) record
CROSS JOIN LATERAL (
  SELECT btrim(record.payload ->> 'likeText') AS like_text,
         CASE WHEN jsonb_typeof(record.payload -> 'likes') = 'number'
                   AND (record.payload ->> 'likes') ~ '^[0-9]{1,15}$'
              THEN (record.payload ->> 'likes')::bigint END AS likes
) parsed
WHERE record.payload ->> 'commentId' = comment.comment_external_id
ON CONFLICT (material_ref) DO NOTHING;

-- A person's record that a stable comment was (or no longer is) used as material for a post.
-- Only state changes are stored; the current state is the latest event per comment.
CREATE TABLE linggan_comment_post_usage_event (
  event_ref uuid PRIMARY KEY,
  request_ref uuid NOT NULL,
  content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
  comment_external_id text NOT NULL,
  action text NOT NULL,
  domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
  actor text NOT NULL DEFAULT 'person',
  request_digest text NOT NULL,
  recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
  CONSTRAINT linggan_comment_post_usage_event_request_key UNIQUE (request_ref),
  CONSTRAINT linggan_comment_post_usage_event_action_check
    CHECK (action IN ('marked', 'unmarked')),
  CONSTRAINT linggan_comment_post_usage_event_actor_check CHECK (actor = 'person'),
  CONSTRAINT linggan_comment_post_usage_event_comment_id_check
    CHECK (length(comment_external_id) BETWEEN 1 AND 512),
  CONSTRAINT linggan_comment_post_usage_event_digest_check
    CHECK (request_digest ~ '^[0-9a-f]{64}$')
);

CREATE INDEX linggan_comment_post_usage_event_current_idx
  ON linggan_comment_post_usage_event
  (content_public_ref, comment_external_id, recorded_at DESC, event_ref DESC);

CREATE TRIGGER linggan_comment_post_usage_event_is_append_only
  BEFORE DELETE OR UPDATE ON linggan_comment_post_usage_event
  FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE linggan_comment_post_usage_event IS
  'Team marks that a comment was used to write a post; latest event is the current state.';

-- Accepted commands retain their original receipt, even when they did not change the state.
-- Otherwise retrying an old no-op after another person's edit would overwrite that later edit.
CREATE TABLE linggan_comment_post_usage_receipt (
  request_ref uuid PRIMARY KEY,
  request_digest text NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
  content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
  comment_external_id text NOT NULL CHECK (length(comment_external_id) BETWEEN 1 AND 512),
  domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
  action text NOT NULL CHECK (action IN ('marked', 'unmarked')),
  outcome text NOT NULL CHECK (outcome IN ('applied', 'unchanged')),
  post_usage_state text NOT NULL CHECK (post_usage_state IN ('used', 'unused')),
  marked_at timestamptz,
  recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
  CHECK ((post_usage_state = 'used') = (marked_at IS NOT NULL)),
  CHECK ((action = 'marked') = (post_usage_state = 'used'))
);

CREATE TRIGGER linggan_comment_post_usage_receipt_is_append_only
  BEFORE DELETE OR UPDATE ON linggan_comment_post_usage_receipt
  FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

ALTER TABLE linggan_comment_post_usage_event
  ADD CONSTRAINT linggan_comment_post_usage_event_receipt_fk
  FOREIGN KEY (request_ref) REFERENCES linggan_comment_post_usage_receipt(request_ref)
  DEFERRABLE INITIALLY DEFERRED;

COMMENT ON TABLE linggan_comment_post_usage_receipt IS
  'Original accepted command receipt, including no-ops; replay never overwrites a later mark.';
