-- MATERIAL-DEEPENING-001 · bounded local processing control and searchable derived text
--
-- Processing jobs/events/derivatives remain append-only facts from 0004.  This mutable work row
-- only controls who may run the job now and guarantees that pending cannot become an infinite
-- retry strategy.

CREATE TABLE linggan_media_processing_work (
    work_ref uuid PRIMARY KEY,
    job_ref uuid NOT NULL UNIQUE REFERENCES linggan_media_processing_job(job_ref),
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending','leased','retry_wait','completed','terminal','not_applicable')),
    attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 3),
    claim_generation integer NOT NULL DEFAULT 0 CHECK (claim_generation >= 0),
    worker_instance_ref uuid,
    lease_expires_at timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_error text,
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((state='leased' AND worker_instance_ref IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR (state<>'leased' AND worker_instance_ref IS NULL AND lease_expires_at IS NULL)),
    CHECK ((state IN ('completed','not_applicable'))=(completed_at IS NOT NULL)),
    CHECK (state<>'terminal' OR attempt_count=3)
);

CREATE INDEX linggan_media_processing_work_due_idx
    ON linggan_media_processing_work(next_attempt_at,created_at)
    WHERE state IN ('pending','retry_wait');
CREATE INDEX linggan_media_processing_work_lease_idx
    ON linggan_media_processing_work(lease_expires_at)
    WHERE state='leased';

-- Existing jobs become runnable without revisiting the platform.  Their historical pending event
-- remains unchanged; the worker appends the actual running/succeeded/failed event.
INSERT INTO linggan_media_processing_work(work_ref,job_ref)
SELECT gen_random_uuid(), job.job_ref
FROM linggan_media_processing_job job
LEFT JOIN linggan_media_processing_work work USING(job_ref)
WHERE work.job_ref IS NULL;

CREATE TABLE linggan_material_derived_text (
    derivative_ref uuid PRIMARY KEY REFERENCES linggan_media_derivative(derivative_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    kind text NOT NULL CHECK (kind IN ('ocr_text','asr_text','frame_ocr_text')),
    text_content text NOT NULL CHECK (length(text_content) > 0),
    display_text text NOT NULL CHECK (length(display_text) > 0),
    language_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (language_state IN ('KNOWN','UNKNOWN')),
    language_tag text,
    source_location jsonb NOT NULL DEFAULT '{}'::jsonb,
    observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((language_state='KNOWN')=(language_tag IS NOT NULL))
);

CREATE INDEX linggan_material_derived_text_content_idx
    ON linggan_material_derived_text(content_public_ref,created_at DESC);
CREATE INDEX linggan_material_derived_text_search_idx
    ON linggan_material_derived_text
    USING gin (to_tsvector('simple', text_content));
CREATE INDEX linggan_material_comment_body_search_idx
    ON linggan_material_comment
    USING gin (to_tsvector('simple', coalesce(body_text,'')));

CREATE TRIGGER linggan_material_derived_text_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_material_derived_text
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE linggan_media_processing_work IS
    'Mutable bounded local worker control; it is not Evidence and does not replace append-only processing events.';
COMMENT ON COLUMN linggan_material_derived_text.display_text IS
    'Locally displayable, bounded text projection; ordinary UI must not expose the full raw text by default.';
