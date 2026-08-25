-- PLUGIN-RUNTIME-001
--
-- A platform-neutral Browser Producer Runtime.  This is deliberately separate from the frozen
-- SCOPE-001 synthetic ingress and from the retired Workbench transport.  It persists execution
-- provenance plus immutable capture packages; downstream Evidence/Observation eligibility is
-- never inferred merely because a producer package was accepted.

CREATE TABLE linggan_runtime_task (
    task_id uuid PRIMARY KEY,
    task_spec_hash text NOT NULL UNIQUE CHECK (task_spec_hash ~ '^[0-9a-f]{64}$'),
    task_spec jsonb NOT NULL,
    source text NOT NULL CHECK (source IN ('manual', 'scheduled')),
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    page_type text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_runtime_attempt (
    attempt_id uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    producer_instance_id uuid NOT NULL,
    started_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (attempt_id, task_id)
);

CREATE TABLE linggan_runtime_capture_package (
    package_ref uuid PRIMARY KEY,
    attempt_id uuid NOT NULL UNIQUE,
    task_id uuid NOT NULL,
    producer_instance_id uuid NOT NULL,
    package_kind text NOT NULL CHECK (package_kind IN (
        'discovery_search', 'profile_discovery', 'content_detail', 'comments', 'replies',
        'author_profile', 'media_slots', 'media_bytes', 'batch_checkpoint'
    )),
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    package_hash text NOT NULL UNIQUE CHECK (package_hash ~ '^[0-9a-f]{64}$'),
    observed_at text NOT NULL,
    captured_at text NOT NULL,
    coverage jsonb NOT NULL,
    checkpoint jsonb,
    payload jsonb NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (attempt_id, task_id) REFERENCES linggan_runtime_attempt(attempt_id, task_id)
);

CREATE TABLE linggan_runtime_submission_receipt (
    submission_id uuid PRIMARY KEY,
    task_id uuid NOT NULL,
    attempt_id uuid NOT NULL,
    producer_instance_id uuid NOT NULL,
    package_hash text NOT NULL CHECK (package_hash ~ '^[0-9a-f]{64}$'),
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    receipt_ref uuid NOT NULL UNIQUE,
    received_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (attempt_id),
    FOREIGN KEY (attempt_id, task_id) REFERENCES linggan_runtime_attempt(attempt_id, task_id)
);

-- Each source record receives an explicit handling result. Package admission is atomic, but a
-- malformed record must not either disappear or invalidate its healthy neighbours. These rows
-- do not make a record Evidence; they only preserve its current ingress disposition.
CREATE TABLE linggan_runtime_record_disposition (
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    record_ordinal integer NOT NULL CHECK (record_ordinal >= 0),
    disposition text NOT NULL CHECK (disposition IN (
        'retained_uninterpreted',
        'accepted_for_library_discovery',
        'accepted_for_library_content',
        'accepted_for_media_identity',
        'quarantined'
    )),
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (package_ref, record_ordinal)
);

-- Media has its own history.  A slot is the stable content relation; an observed URL is neither
-- a media identity nor a presentation URL.  Browser temporary paths must never enter these rows.
CREATE TABLE linggan_media_slot (
    slot_key text PRIMARY KEY CHECK (length(slot_key) > 0),
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    content_external_id text NOT NULL CHECK (length(content_external_id) > 0),
    role text NOT NULL CHECK (length(role) > 0),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    first_package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    UNIQUE (platform, content_external_id, role, ordinal)
);

CREATE TABLE linggan_media_observation (
    observation_ref uuid PRIMARY KEY,
    slot_key text NOT NULL REFERENCES linggan_media_slot(slot_key),
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    observed_external_uri text NOT NULL CHECK (length(observed_external_uri) > 0),
    observed_at text NOT NULL,
    UNIQUE (package_ref, slot_key, observed_external_uri)
);

CREATE TABLE linggan_media_download_attempt (
    download_attempt_ref uuid PRIMARY KEY,
    media_observation_ref uuid NOT NULL REFERENCES linggan_media_observation(observation_ref),
    started_at timestamptz NOT NULL DEFAULT scope_001_now(),
    ended_at timestamptz,
    terminal_reason text CHECK (terminal_reason IN ('acquired', 'expired_url', 'mime_mismatch', 'size_limit', 'cancelled', 'network_error', 'unknown')),
    attempted_uri text NOT NULL CHECK (length(attempted_uri) > 0)
);

CREATE TABLE linggan_media_blob (
    sha256 text PRIMARY KEY CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    mime_type text NOT NULL,
    byte_size bigint NOT NULL CHECK (byte_size >= 0),
    storage_key text NOT NULL UNIQUE CHECK (storage_key !~ '(^/|\\.\\.)'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_media_materialization (
    materialization_ref uuid PRIMARY KEY,
    blob_sha256 text NOT NULL REFERENCES linggan_media_blob(sha256),
    download_attempt_ref uuid NOT NULL REFERENCES linggan_media_download_attempt(download_attempt_ref),
    local_asset_path text NOT NULL CHECK (local_asset_path ~ '^/api/local/media/[0-9a-f]{64}$'),
    verified_at timestamptz NOT NULL DEFAULT scope_001_now()
);

-- Upload delivery is intentionally mutable runtime state, not Evidence. It survives extension
-- reloads and lets a bounded media lane resume from a byte offset without holding the text
-- Capture Package open. The final blob/admission records above remain append-only facts.
CREATE TABLE linggan_media_upload_session (
    session_ref uuid PRIMARY KEY,
    media_observation_ref uuid NOT NULL REFERENCES linggan_media_observation(observation_ref),
    expected_sha256 text NOT NULL CHECK (expected_sha256 ~ '^[0-9a-f]{64}$'),
    mime_type text NOT NULL,
    expected_byte_size bigint NOT NULL CHECK (expected_byte_size >= 0),
    temporary_storage_key text NOT NULL UNIQUE CHECK (temporary_storage_key !~ '(^/|\\.\\.)'),
    received_byte_size bigint NOT NULL DEFAULT 0 CHECK (received_byte_size >= 0 AND received_byte_size <= expected_byte_size),
    state text NOT NULL CHECK (state IN ('receiving', 'ready_to_finalize', 'finalizing', 'materialized', 'failed')),
    download_attempt_ref uuid REFERENCES linggan_media_download_attempt(download_attempt_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (media_observation_ref, expected_sha256, expected_byte_size)
);

CREATE TABLE linggan_media_processing_job (
    job_ref uuid PRIMARY KEY,
    blob_sha256 text NOT NULL REFERENCES linggan_media_blob(sha256),
    slot_key text REFERENCES linggan_media_slot(slot_key),
    processor_kind text NOT NULL CHECK (processor_kind IN ('thumbnail', 'image_ocr', 'audio_extract', 'asr', 'video_frame_ocr')),
    processor_version text NOT NULL,
    input_scope text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (blob_sha256, slot_key, processor_kind, processor_version, input_scope)
);

-- A job is an immutable admission request. Its lifecycle is a separate append-only event stream,
-- so completing OCR/ASR never rewrites either the raw blob or the original request.
CREATE TABLE linggan_media_processing_job_event (
    event_ref uuid PRIMARY KEY,
    job_ref uuid NOT NULL REFERENCES linggan_media_processing_job(job_ref),
    state text NOT NULL CHECK (state IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'invalidated')),
    reason text,
    occurred_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_media_derivative (
    derivative_ref uuid PRIMARY KEY,
    job_ref uuid NOT NULL REFERENCES linggan_media_processing_job(job_ref),
    blob_sha256 text REFERENCES linggan_media_blob(sha256),
    derivative_kind text NOT NULL CHECK (derivative_kind IN ('thumbnail', 'ocr_text', 'audio', 'asr_text', 'frame_ocr_text')),
    content_hash text NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    storage_key text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE OR REPLACE FUNCTION linggan_plugin_runtime_forbid_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN RAISE EXCEPTION 'Linggan Browser Producer Runtime facts are append-only'; END $$;

CREATE TRIGGER linggan_runtime_task_is_append_only BEFORE UPDATE OR DELETE ON linggan_runtime_task FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_runtime_attempt_is_append_only BEFORE UPDATE OR DELETE ON linggan_runtime_attempt FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_runtime_package_is_append_only BEFORE UPDATE OR DELETE ON linggan_runtime_capture_package FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_runtime_receipt_is_append_only BEFORE UPDATE OR DELETE ON linggan_runtime_submission_receipt FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_runtime_record_disposition_is_append_only BEFORE UPDATE OR DELETE ON linggan_runtime_record_disposition FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_slot_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_slot FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_observation_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_observation FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_download_attempt_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_download_attempt FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_blob_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_blob FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_materialization_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_materialization FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
-- `linggan_media_upload_session` deliberately has no append-only trigger: it is delivery
-- control state and must advance offsets/retry state. It never acts as Evidence or a UI source.
CREATE TRIGGER linggan_media_processing_job_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_processing_job FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_processing_job_event_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_processing_job_event FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER linggan_media_derivative_is_append_only BEFORE UPDATE OR DELETE ON linggan_media_derivative FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
