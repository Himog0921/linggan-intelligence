-- Comment fact storage V0.
--
-- This is a greenfield migration. It intentionally does not reproduce the
-- legacy Prisma schema. The only accepted external input is a pair validated
-- by xhs.comment-capture-source.v0 before storage admission.

CREATE TABLE source_evidence_v0 (
  id UUID PRIMARY KEY,
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  platform TEXT NOT NULL CHECK (platform = 'xhs'),
  source_contract TEXT NOT NULL CHECK (source_contract = 'xhs.comment-capture-source.v0'),
  package_kind TEXT NOT NULL CHECK (package_kind IN ('content_detail', 'comments')),
  payload_sha256 CHAR(64) NOT NULL CHECK (payload_sha256 ~ '^[0-9a-f]{64}$'),
  source_payload JSONB NOT NULL,
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT source_evidence_v0_identity_key
    UNIQUE (workspace_id, platform, source_contract, package_kind, payload_sha256)
);

CREATE TABLE comment_identity_v0 (
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  platform TEXT NOT NULL CHECK (platform = 'xhs'),
  note_id TEXT NOT NULL CHECK (btrim(note_id) <> ''),
  comment_id TEXT NOT NULL CHECK (btrim(comment_id) <> ''),
  created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (workspace_id, platform, note_id, comment_id)
);

CREATE TABLE source_capture_pair_v0 (
  id UUID PRIMARY KEY,
  workspace_id TEXT NOT NULL CHECK (btrim(workspace_id) <> ''),
  detail_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  comments_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT source_capture_pair_v0_identity_key
    UNIQUE (workspace_id, detail_evidence_id, comments_evidence_id),
  CONSTRAINT source_capture_pair_v0_distinct_evidence_check
    CHECK (detail_evidence_id <> comments_evidence_id)
);

CREATE TABLE evidence_comment_record_v0 (
  source_evidence_id UUID NOT NULL REFERENCES source_evidence_v0(id) ON DELETE RESTRICT,
  source_record_index INTEGER NOT NULL CHECK (source_record_index >= 0),
  workspace_id TEXT NOT NULL,
  platform TEXT NOT NULL,
  note_id TEXT NOT NULL,
  comment_id TEXT NOT NULL,
  text_sha256 CHAR(64) NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (source_evidence_id, source_record_index),
  FOREIGN KEY (workspace_id, platform, note_id, comment_id)
    REFERENCES comment_identity_v0(workspace_id, platform, note_id, comment_id)
    ON DELETE RESTRICT
);

CREATE TABLE comment_observation_v0 (
  id UUID PRIMARY KEY,
  admission_sequence BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE NOT NULL,
  workspace_id TEXT NOT NULL,
  platform TEXT NOT NULL,
  note_id TEXT NOT NULL,
  comment_id TEXT NOT NULL,
  source_evidence_id UUID NOT NULL,
  source_record_index INTEGER NOT NULL,
  text_sha256 CHAR(64) NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
  source_text TEXT NOT NULL CHECK (btrim(source_text) <> ''),
  admitted_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CONSTRAINT comment_observation_v0_identity_id_key
    UNIQUE (workspace_id, platform, note_id, comment_id, id),
  CONSTRAINT comment_observation_v0_source_record_key
    UNIQUE (source_evidence_id, source_record_index),
  FOREIGN KEY (workspace_id, platform, note_id, comment_id)
    REFERENCES comment_identity_v0(workspace_id, platform, note_id, comment_id)
    ON DELETE RESTRICT,
  FOREIGN KEY (source_evidence_id, source_record_index)
    REFERENCES evidence_comment_record_v0(source_evidence_id, source_record_index)
    ON DELETE RESTRICT
);

CREATE TABLE comment_current_v0 (
  workspace_id TEXT NOT NULL,
  platform TEXT NOT NULL,
  note_id TEXT NOT NULL,
  comment_id TEXT NOT NULL,
  current_observation_id UUID NOT NULL,
  text_sha256 CHAR(64) NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
  advanced_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (workspace_id, platform, note_id, comment_id),
  FOREIGN KEY (workspace_id, platform, note_id, comment_id)
    REFERENCES comment_identity_v0(workspace_id, platform, note_id, comment_id)
    ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id, platform, note_id, comment_id, current_observation_id)
    REFERENCES comment_observation_v0(workspace_id, platform, note_id, comment_id, id)
    ON DELETE RESTRICT
);

CREATE INDEX evidence_comment_record_v0_identity_idx
  ON evidence_comment_record_v0(workspace_id, platform, note_id, comment_id);

CREATE INDEX comment_observation_v0_identity_admitted_idx
  ON comment_observation_v0(workspace_id, platform, note_id, comment_id, admitted_at);

CREATE OR REPLACE FUNCTION reject_comment_fact_history_mutation_v0() RETURNS trigger AS $$
BEGIN
  RAISE EXCEPTION '% is append-only; % is not allowed', TG_TABLE_NAME, TG_OP
    USING ERRCODE = '55000';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION validate_source_capture_pair_v0() RETURNS trigger AS $$
DECLARE
  detail_workspace_id TEXT;
  detail_package_kind TEXT;
  comments_workspace_id TEXT;
  comments_package_kind TEXT;
BEGIN
  SELECT workspace_id, package_kind INTO detail_workspace_id, detail_package_kind
  FROM source_evidence_v0 WHERE id = NEW.detail_evidence_id;
  SELECT workspace_id, package_kind INTO comments_workspace_id, comments_package_kind
  FROM source_evidence_v0 WHERE id = NEW.comments_evidence_id;

  IF detail_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR comments_workspace_id IS DISTINCT FROM NEW.workspace_id
     OR detail_package_kind IS DISTINCT FROM 'content_detail'
     OR comments_package_kind IS DISTINCT FROM 'comments' THEN
    RAISE EXCEPTION 'source_capture_pair_v0 must reference one workspace-matching content_detail and comments Evidence pair'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER source_evidence_v0_append_only
  BEFORE UPDATE OR DELETE ON source_evidence_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER comment_identity_v0_immutable
  BEFORE UPDATE OR DELETE ON comment_identity_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER source_capture_pair_v0_append_only
  BEFORE UPDATE OR DELETE ON source_capture_pair_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER source_capture_pair_v0_relationship_guard
  BEFORE INSERT ON source_capture_pair_v0
  FOR EACH ROW EXECUTE FUNCTION validate_source_capture_pair_v0();

CREATE TRIGGER evidence_comment_record_v0_append_only
  BEFORE UPDATE OR DELETE ON evidence_comment_record_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE TRIGGER comment_observation_v0_append_only
  BEFORE UPDATE OR DELETE ON comment_observation_v0
  FOR EACH ROW EXECUTE FUNCTION reject_comment_fact_history_mutation_v0();

CREATE OR REPLACE FUNCTION validate_comment_current_v0() RETURNS trigger AS $$
DECLARE
  observation_text_sha256 CHAR(64);
  observation_admission_sequence BIGINT;
  current_admission_sequence BIGINT;
BEGIN
  SELECT text_sha256, admission_sequence INTO observation_text_sha256, observation_admission_sequence
  FROM comment_observation_v0
  WHERE workspace_id = NEW.workspace_id
    AND platform = NEW.platform
    AND note_id = NEW.note_id
    AND comment_id = NEW.comment_id
    AND id = NEW.current_observation_id;

  IF NOT FOUND OR observation_text_sha256 IS DISTINCT FROM NEW.text_sha256 THEN
    RAISE EXCEPTION 'comment_current_v0 must materialize its referenced immutable observation'
      USING ERRCODE = '55000';
  END IF;

  IF TG_OP = 'UPDATE' THEN
    SELECT admission_sequence INTO current_admission_sequence
    FROM comment_observation_v0
    WHERE workspace_id = OLD.workspace_id
      AND platform = OLD.platform
      AND note_id = OLD.note_id
      AND comment_id = OLD.comment_id
      AND id = OLD.current_observation_id;

    IF NOT FOUND OR observation_admission_sequence <= current_admission_sequence THEN
      RAISE EXCEPTION 'comment_current_v0 may only advance to a later locally admitted observation'
        USING ERRCODE = '55000';
    END IF;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER comment_current_v0_materialization_guard
  BEFORE INSERT OR UPDATE ON comment_current_v0
  FOR EACH ROW EXECUTE FUNCTION validate_comment_current_v0();
