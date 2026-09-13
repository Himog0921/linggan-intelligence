-- Comment context storage V0.
--
-- This migration extends the immutable Evidence baseline from
-- 0001_comment_fact_storage_v0.sql. It admits a verified content_detail +
-- comments + replies set as three independent source packages. A context set
-- is an audit relationship only: work and discussion records may help explain
-- a current comment, but never become claims made by that comment's author.

ALTER TABLE source_evidence_v0
  DROP CONSTRAINT source_evidence_v0_source_contract_check,
  DROP CONSTRAINT source_evidence_v0_package_kind_check;

ALTER TABLE source_evidence_v0
  ADD CONSTRAINT source_evidence_v0_contract_package_kind_check CHECK (
    (source_contract = 'xhs.comment-capture-source.v0'
      AND package_kind IN ('content_detail', 'comments'))
    OR
    (source_contract = 'xhs.comment-context-source.v0'
      AND package_kind IN ('content_detail', 'comments', 'replies'))
  );

-- The original pair remains a relation for the original source contract only.
-- Context sets are represented separately below and must not be inserted here.
CREATE OR REPLACE FUNCTION validate_source_capture_pair_v0() RETURNS trigger AS $$
DECLARE
  detail_workspace_id TEXT;
  detail_platform TEXT;
  detail_source_contract TEXT;
  detail_package_kind TEXT;
  comments_workspace_id TEXT;
  comments_platform TEXT;
  comments_source_contract TEXT;
  comments_package_kind TEXT;
BEGIN
  SELECT workspace_id, platform, source_contract, package_kind
    INTO detail_workspace_id, detail_platform, detail_source_contract, detail_package_kind
    FROM source_evidence_v0 WHERE id = NEW.detail_evidence_id;
  SELECT workspace_id, platform, source_contract, package_kind
    INTO comments_workspace_id, comments_platform, comments_source_contract, comments_package_kind
    FROM source_evidence_v0 WHERE id = NEW.comments_evidence_id;

  IF detail_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR comments_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR detail_platform IS DISTINCT FROM 'xhs'
     OR comments_platform IS DISTINCT FROM 'xhs'
     OR detail_source_contract IS DISTINCT FROM 'xhs.comment-capture-source.v0'
     OR comments_source_contract IS DISTINCT FROM 'xhs.comment-capture-source.v0'
     OR detail_package_kind IS DISTINCT FROM 'content_detail'
     OR comments_package_kind IS DISTINCT FROM 'comments' THEN
    RAISE EXCEPTION 'source_capture_pair_v0 must reference one workspace-matching legacy content_detail and comments Evidence pair'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE context_work_record_v0 (
  source_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  source_record_index INTEGER NOT NULL CHECK (source_record_index = 0),
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  platform TEXT NOT NULL CHECK (platform = 'xhs'),
  note_id TEXT NOT NULL CHECK (btrim(note_id) <> ''),
  title_availability TEXT NOT NULL CHECK (title_availability IN ('unavailable', 'blank', 'observed')),
  title_source_text TEXT,
  body_text_availability TEXT NOT NULL CHECK (body_text_availability IN ('unavailable', 'blank', 'observed')),
  body_text_source_text TEXT,
  author_id TEXT CHECK (author_id IS NULL OR btrim(author_id) <> ''),
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (source_evidence_id, source_record_index),
  CONSTRAINT context_work_record_v0_title_text_coherence_check CHECK (
    (title_availability = 'observed' AND title_source_text IS NOT NULL AND btrim(title_source_text) <> '')
    OR
    (title_availability IN ('unavailable', 'blank') AND title_source_text IS NULL)
  ),
  CONSTRAINT context_work_record_v0_body_text_coherence_check CHECK (
    (body_text_availability = 'observed' AND body_text_source_text IS NOT NULL AND btrim(body_text_source_text) <> '')
    OR
    (body_text_availability IN ('unavailable', 'blank') AND body_text_source_text IS NULL)
  )
);

CREATE TABLE context_discussion_record_v0 (
  source_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  source_record_index INTEGER NOT NULL CHECK (source_record_index >= 0),
  record_kind TEXT NOT NULL CHECK (record_kind IN ('comment', 'reply')),
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  platform TEXT NOT NULL CHECK (platform = 'xhs'),
  note_id TEXT NOT NULL CHECK (btrim(note_id) <> ''),
  comment_id TEXT NOT NULL CHECK (btrim(comment_id) <> ''),
  text_sha256 CHAR(64) NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
  source_text TEXT NOT NULL CHECK (btrim(source_text) <> ''),
  author_id TEXT CHECK (author_id IS NULL OR btrim(author_id) <> ''),
  root_comment_id TEXT CHECK (root_comment_id IS NULL OR btrim(root_comment_id) <> ''),
  parent_comment_id TEXT CHECK (parent_comment_id IS NULL OR btrim(parent_comment_id) <> ''),
  reply_to_comment_id TEXT CHECK (reply_to_comment_id IS NULL OR btrim(reply_to_comment_id) <> ''),
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (source_evidence_id, source_record_index),
  CONSTRAINT context_discussion_record_v0_comment_pointer_check CHECK (
    record_kind = 'reply'
    OR (root_comment_id IS NULL AND parent_comment_id IS NULL AND reply_to_comment_id IS NULL)
  )
);

CREATE TABLE source_context_capture_set_v0 (
  id UUID PRIMARY KEY,
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  platform TEXT NOT NULL CHECK (platform = 'xhs'),
  note_id TEXT NOT NULL CHECK (btrim(note_id) <> ''),
  detail_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  comments_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  replies_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  comments_record_count INTEGER NOT NULL CHECK (comments_record_count > 0),
  replies_record_count INTEGER NOT NULL CHECK (replies_record_count > 0),
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT source_context_capture_set_v0_identity_key
    UNIQUE (workspace_id, detail_evidence_id, comments_evidence_id, replies_evidence_id),
  CONSTRAINT source_context_capture_set_v0_distinct_evidence_check
    CHECK (
      detail_evidence_id <> comments_evidence_id
      AND detail_evidence_id <> replies_evidence_id
      AND comments_evidence_id <> replies_evidence_id
    )
);

CREATE INDEX context_discussion_record_v0_current_match_idx
  ON context_discussion_record_v0(workspace_id, platform, note_id, comment_id, text_sha256, record_kind);

CREATE INDEX context_discussion_record_v0_related_reply_idx
  ON context_discussion_record_v0(workspace_id, platform, note_id, root_comment_id, parent_comment_id, reply_to_comment_id)
  WHERE record_kind = 'reply';

CREATE INDEX source_context_capture_set_v0_current_match_idx
  ON source_context_capture_set_v0(workspace_id, platform, note_id, comments_evidence_id, admitted_at DESC, id);

CREATE OR REPLACE FUNCTION validate_context_work_record_v0() RETURNS trigger AS $$
DECLARE
  evidence_workspace_id TEXT;
  evidence_platform TEXT;
  evidence_source_contract TEXT;
  evidence_package_kind TEXT;
BEGIN
  SELECT workspace_id, platform, source_contract, package_kind
    INTO evidence_workspace_id, evidence_platform, evidence_source_contract, evidence_package_kind
    FROM source_evidence_v0 WHERE id = NEW.source_evidence_id;

  IF evidence_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR evidence_platform IS DISTINCT FROM NEW.platform
     OR evidence_source_contract IS DISTINCT FROM 'xhs.comment-context-source.v0'
     OR evidence_package_kind IS DISTINCT FROM 'content_detail' THEN
    RAISE EXCEPTION 'context_work_record_v0 must reference workspace-matching context content_detail Evidence'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_context_discussion_record_v0() RETURNS trigger AS $$
DECLARE
  evidence_workspace_id TEXT;
  evidence_platform TEXT;
  evidence_source_contract TEXT;
  evidence_package_kind TEXT;
  expected_package_kind TEXT;
BEGIN
  expected_package_kind := CASE NEW.record_kind
    WHEN 'comment' THEN 'comments'
    WHEN 'reply' THEN 'replies'
  END;

  SELECT workspace_id, platform, source_contract, package_kind
    INTO evidence_workspace_id, evidence_platform, evidence_source_contract, evidence_package_kind
    FROM source_evidence_v0 WHERE id = NEW.source_evidence_id;

  IF evidence_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR evidence_platform IS DISTINCT FROM NEW.platform
     OR evidence_source_contract IS DISTINCT FROM 'xhs.comment-context-source.v0'
     OR evidence_package_kind IS DISTINCT FROM expected_package_kind THEN
    RAISE EXCEPTION 'context_discussion_record_v0 must reference workspace-matching context Evidence of its declared kind'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_source_context_capture_set_v0() RETURNS trigger AS $$
DECLARE
  detail_workspace_id TEXT;
  detail_platform TEXT;
  detail_source_contract TEXT;
  detail_package_kind TEXT;
  comments_workspace_id TEXT;
  comments_platform TEXT;
  comments_source_contract TEXT;
  comments_package_kind TEXT;
  replies_workspace_id TEXT;
  replies_platform TEXT;
  replies_source_contract TEXT;
  replies_package_kind TEXT;
  work_record_count BIGINT;
  work_note_id TEXT;
  comment_record_count BIGINT;
  comments_all_match BOOLEAN;
  reply_record_count BIGINT;
  replies_all_match BOOLEAN;
BEGIN
  SELECT workspace_id, platform, source_contract, package_kind
    INTO detail_workspace_id, detail_platform, detail_source_contract, detail_package_kind
    FROM source_evidence_v0 WHERE id = NEW.detail_evidence_id;
  SELECT workspace_id, platform, source_contract, package_kind
    INTO comments_workspace_id, comments_platform, comments_source_contract, comments_package_kind
    FROM source_evidence_v0 WHERE id = NEW.comments_evidence_id;
  SELECT workspace_id, platform, source_contract, package_kind
    INTO replies_workspace_id, replies_platform, replies_source_contract, replies_package_kind
    FROM source_evidence_v0 WHERE id = NEW.replies_evidence_id;

  IF detail_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR comments_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR replies_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR detail_platform IS DISTINCT FROM NEW.platform
     OR comments_platform IS DISTINCT FROM NEW.platform
     OR replies_platform IS DISTINCT FROM NEW.platform
     OR detail_source_contract IS DISTINCT FROM 'xhs.comment-context-source.v0'
     OR comments_source_contract IS DISTINCT FROM 'xhs.comment-context-source.v0'
     OR replies_source_contract IS DISTINCT FROM 'xhs.comment-context-source.v0'
     OR detail_package_kind IS DISTINCT FROM 'content_detail'
     OR comments_package_kind IS DISTINCT FROM 'comments'
     OR replies_package_kind IS DISTINCT FROM 'replies' THEN
    RAISE EXCEPTION 'source_context_capture_set_v0 must reference one workspace-matching context Evidence triplet'
      USING ERRCODE = '55000';
  END IF;

  SELECT count(*), min(note_id)
    INTO work_record_count, work_note_id
    FROM context_work_record_v0
   WHERE source_evidence_id = NEW.detail_evidence_id;

  SELECT count(*), bool_and(note_id = NEW.note_id)
    INTO comment_record_count, comments_all_match
    FROM context_discussion_record_v0
   WHERE source_evidence_id = NEW.comments_evidence_id
     AND record_kind = 'comment';

  SELECT count(*), bool_and(note_id = NEW.note_id)
    INTO reply_record_count, replies_all_match
    FROM context_discussion_record_v0
   WHERE source_evidence_id = NEW.replies_evidence_id
     AND record_kind = 'reply';

  IF work_record_count <> 1
     OR work_note_id IS DISTINCT FROM NEW.note_id
     OR comment_record_count <> NEW.comments_record_count
     OR reply_record_count <> NEW.replies_record_count
     OR comments_all_match IS DISTINCT FROM TRUE
     OR replies_all_match IS DISTINCT FROM TRUE THEN
    RAISE EXCEPTION 'source_context_capture_set_v0 must reference complete context records for its note'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER context_work_record_v0_append_only
  BEFORE UPDATE OR DELETE ON context_work_record_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER context_work_record_v0_relationship_guard
  BEFORE INSERT ON context_work_record_v0
  FOR EACH ROW EXECUTE FUNCTION validate_context_work_record_v0();

CREATE TRIGGER context_discussion_record_v0_append_only
  BEFORE UPDATE OR DELETE ON context_discussion_record_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER context_discussion_record_v0_relationship_guard
  BEFORE INSERT ON context_discussion_record_v0
  FOR EACH ROW EXECUTE FUNCTION validate_context_discussion_record_v0();

CREATE TRIGGER source_context_capture_set_v0_append_only
  BEFORE UPDATE OR DELETE ON source_context_capture_set_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER source_context_capture_set_v0_relationship_guard
  BEFORE INSERT ON source_context_capture_set_v0
  FOR EACH ROW EXECUTE FUNCTION validate_source_context_capture_set_v0();
