-- WORK-RESOURCE-READ-001
--
-- A detail collector may observe both a human-readable time label and an exact platform epoch.
-- Keep both assertions and their provenance.  Only a producer-qualified platform epoch may become
-- an exact published_at instant; relative or calendar source text remains display evidence and is
-- never silently promoted by the read model.

ALTER TABLE linggan_material_content_detail
    ADD COLUMN published_at timestamptz,
    ADD COLUMN published_at_source_field text,
    ADD COLUMN published_at_source_kind text NOT NULL DEFAULT 'unknown'
        CHECK (published_at_source_kind IN ('platform_epoch', 'visible_text', 'unknown')),
    ADD COLUMN published_at_precision text NOT NULL DEFAULT 'unknown'
        CHECK (published_at_precision IN ('millisecond', 'second', 'minute', 'day', 'relative', 'unknown')),
    ADD COLUMN published_at_reference_observed_at timestamptz,
    ADD COLUMN published_at_parser_version text,
    ADD CONSTRAINT linggan_material_detail_published_at_qualification_check CHECK (
        published_at IS NULL
        OR (
            published_at_source_kind = 'platform_epoch'
            AND published_at_precision IN ('millisecond', 'second')
            AND published_at_source_field IS NOT NULL
            AND published_at_parser_version IS NOT NULL
        )
    ),
    ADD CONSTRAINT linggan_material_detail_published_reference_check CHECK (
        published_at_reference_observed_at IS NULL
        OR published_at_source_kind = 'visible_text'
    );

COMMENT ON COLUMN linggan_material_content_detail.published_at IS
    'Exact platform publication instant, admitted only from a producer-qualified platform epoch.';
COMMENT ON COLUMN linggan_material_content_detail.published_at_source_text IS
    'Verbatim visible or payload source text; it is evidence but is not necessarily an exact instant.';
COMMENT ON COLUMN linggan_material_content_detail.published_at_reference_observed_at IS
    'Observation-time reference required by relative visible text; never used to rewrite historical rows.';
