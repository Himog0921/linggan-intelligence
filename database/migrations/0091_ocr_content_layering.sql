-- OCR-CONTENT-LAYERING-001
--
-- A line layout and a semantic text layer are Material Transformations, not a replacement for
-- either the admitted image Blob or the original OCR result.  Every retained phrase therefore
-- remains traceable to an OCR line and to the exact processor version that read the image.

CREATE TABLE linggan_media_ocr_layout (
    layout_ref uuid PRIMARY KEY,
    ocr_derivative_ref uuid NOT NULL UNIQUE REFERENCES linggan_media_derivative(derivative_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    blob_sha256 text NOT NULL REFERENCES linggan_media_blob(sha256),
    engine text NOT NULL CHECK (engine='paddleocr'),
    engine_version text NOT NULL CHECK (char_length(engine_version) BETWEEN 1 AND 120),
    image_width integer NOT NULL CHECK (image_width>0),
    image_height integer NOT NULL CHECK (image_height>0),
    layout_content_hash text NOT NULL CHECK (layout_content_hash ~ '^[0-9a-f]{64}$'),
    layout_byte_size bigint NOT NULL CHECK (layout_byte_size>0),
    layout_storage_key text NOT NULL CHECK (char_length(layout_storage_key)>0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_media_ocr_line (
    layout_ref uuid NOT NULL REFERENCES linggan_media_ocr_layout(layout_ref),
    line_ref uuid NOT NULL,
    ordinal integer NOT NULL CHECK (ordinal>=0),
    text_content text NOT NULL CHECK (char_length(trim(text_content))>0),
    confidence numeric(6,5) NOT NULL CHECK (confidence>=0 AND confidence<=1),
    left_norm numeric(7,6) NOT NULL CHECK (left_norm>=0 AND left_norm<=1),
    top_norm numeric(7,6) NOT NULL CHECK (top_norm>=0 AND top_norm<=1),
    right_norm numeric(7,6) NOT NULL CHECK (right_norm>=0 AND right_norm<=1 AND right_norm>=left_norm),
    bottom_norm numeric(7,6) NOT NULL CHECK (bottom_norm>=0 AND bottom_norm<=1 AND bottom_norm>=top_norm),
    PRIMARY KEY (layout_ref,line_ref),
    UNIQUE (layout_ref,ordinal)
);
CREATE INDEX linggan_media_ocr_line_layout_order_idx
    ON linggan_media_ocr_line(layout_ref,ordinal);

CREATE TABLE linggan_media_ocr_layering_result (
    layering_ref uuid PRIMARY KEY,
    layout_ref uuid NOT NULL REFERENCES linggan_media_ocr_layout(layout_ref),
    layer_version text NOT NULL CHECK (char_length(layer_version) BETWEEN 1 AND 120),
    state text NOT NULL CHECK (state IN ('ACCEPTED','PARTIAL','NEEDS_REVIEW','FAILED')),
    decision_source text NOT NULL CHECK (decision_source IN ('rules','vision')),
    cover_headline text,
    image_substantive_text text,
    retained_line_refs jsonb NOT NULL DEFAULT '[]'::jsonb,
    excluded_lines jsonb NOT NULL DEFAULT '[]'::jsonb,
    model_invocation_ref uuid,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (jsonb_typeof(retained_line_refs)='array'),
    CHECK (jsonb_typeof(excluded_lines)='array'),
    CHECK (state<>'ACCEPTED' OR cover_headline IS NOT NULL OR image_substantive_text IS NOT NULL),
    CHECK (decision_source='rules' OR model_invocation_ref IS NOT NULL),
    UNIQUE(layout_ref,layer_version)
);
CREATE INDEX linggan_media_ocr_layering_read_idx
    ON linggan_media_ocr_layering_result(layout_ref,created_at DESC);

-- Tesseract v1/v2 output is no longer eligible for display, search or title fallback.  The
-- existing media job/event history is retained as the minimal audit record; only derivative
-- readability is retired.  New Paddle jobs are appended through the normal version requeue.
CREATE TABLE linggan_media_ocr_retirement (
    retired_job_ref uuid PRIMARY KEY REFERENCES linggan_media_processing_job(job_ref),
    reason text NOT NULL CHECK (reason='tesseract_replaced_by_paddleocr'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason)
SELECT job.job_ref,'tesseract_replaced_by_paddleocr'
FROM linggan_media_processing_job job
WHERE job.processor_kind IN ('image_ocr','video_frame_ocr')
  AND job.processor_version IN ('local-v1','local-v2')
ON CONFLICT(retired_job_ref) DO NOTHING;
INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason)
SELECT gen_random_uuid(),retired_job_ref,'invalidated','tesseract_replaced_by_paddleocr'
FROM linggan_media_ocr_retirement;
