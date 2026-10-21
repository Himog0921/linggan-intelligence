-- Versioned adoption of full local OCR observations, attached to existing media results.
-- No jobs are enqueued, no sources are rewritten, and no runtime is activated here.
CREATE TABLE linggan_media_ocr_adoption (
    adoption_ref uuid PRIMARY KEY,
    derivative_ref uuid NOT NULL UNIQUE REFERENCES linggan_media_derivative(derivative_ref),
    layout_ref uuid NOT NULL UNIQUE REFERENCES linggan_media_ocr_layout(layout_ref),
    rule_version text NOT NULL CHECK (char_length(rule_version) BETWEEN 1 AND 120),
    input_fingerprint text NOT NULL CHECK (input_fingerprint ~ '^[0-9a-f]{64}$'),
    observation jsonb NOT NULL CHECK (jsonb_typeof(observation)='object'),
    assessment jsonb NOT NULL CHECK (jsonb_typeof(assessment)='object'),
    line_ref_map jsonb NOT NULL CHECK (jsonb_typeof(line_ref_map)='array'),
    image_text text NOT NULL,
    cover_title text,
    state text NOT NULL CHECK (state IN ('ACCEPTED','PARTIAL','NEEDS_REVIEW')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (state<>'ACCEPTED' OR length(image_text)>0 OR cover_title IS NOT NULL)
);
CREATE TRIGGER linggan_media_ocr_adoption_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_media_ocr_adoption
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
COMMENT ON TABLE linggan_media_ocr_adoption IS
    'Typed versioned OCR adoption projection; raw bundle and original image remain independently traceable. Not an author or truth claim.';
