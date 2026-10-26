-- OPS-PUBLICATION-SYNC-001 S08/S11: every displayed detail field is selected from an
-- append-only adoption decision. A qualified Receipt is one source of a decision; imported
-- and native registration values establish their own traceable baseline instead of a UI fallback.

CREATE TABLE linggan_ops_publication_field_adoption (
    adoption_ref uuid PRIMARY KEY,
    projection_ref uuid REFERENCES linggan_ops_publication_material_projection(projection_ref),
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    publication_version bigint,
    field_name text NOT NULL CHECK (field_name IN (
        'title', 'body', 'published_at', 'like_count', 'collect_count', 'comment_count'
    )),
    disposition text NOT NULL CHECK (disposition IN ('adopted', 'not_provided', 'conflict')),
    source_kind text NOT NULL CHECK (source_kind IN (
        'workbench_import', 'native_registration', 'receipt_detail'
    )),
    source_batch_ref uuid REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    source_request_ref uuid REFERENCES linggan_ops_native_write_request(request_ref),
    value_text text,
    value_count bigint CHECK (value_count IS NULL OR value_count >= 0),
    value_instant timestamptz,
    observed_at text NOT NULL CHECK (length(btrim(observed_at)) > 0),
    source_package_ref uuid REFERENCES linggan_runtime_capture_package(package_ref),
    source_record_ordinal integer CHECK (source_record_ordinal IS NULL OR source_record_ordinal >= 0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (projection_ref, field_name),
    UNIQUE (source_batch_ref, publication_ref, field_name),
    UNIQUE (source_request_ref, publication_ref, field_name),
    FOREIGN KEY (source_package_ref, source_record_ordinal)
        REFERENCES linggan_material_content_detail(package_ref, record_ordinal),
    CHECK (
        (field_name IN ('title', 'body')
         AND value_count IS NULL AND value_instant IS NULL)
        OR (field_name IN ('like_count', 'collect_count', 'comment_count')
            AND value_text IS NULL AND value_instant IS NULL)
        OR (field_name = 'published_at'
            AND value_text IS NULL AND value_count IS NULL)
    ),
    CHECK (
        (disposition = 'not_provided'
         AND value_text IS NULL AND value_count IS NULL AND value_instant IS NULL)
        OR (disposition IN ('adopted', 'conflict')
            AND (value_text IS NOT NULL OR value_count IS NOT NULL OR value_instant IS NOT NULL))
    ),
    CHECK (
        (source_kind = 'receipt_detail'
         AND projection_ref IS NOT NULL AND publication_version > 0
         AND source_batch_ref IS NULL AND source_request_ref IS NULL
         AND source_package_ref IS NOT NULL AND source_record_ordinal IS NOT NULL)
        OR (source_kind = 'workbench_import'
            AND projection_ref IS NULL AND publication_version IS NULL
            AND source_batch_ref IS NOT NULL AND source_request_ref IS NULL
            AND source_package_ref IS NULL AND source_record_ordinal IS NULL)
        OR (source_kind = 'native_registration'
            AND projection_ref IS NULL AND publication_version > 0
            AND source_batch_ref IS NULL AND source_request_ref IS NOT NULL
            AND source_package_ref IS NULL AND source_record_ordinal IS NULL)
    )
);

-- Existing imported current values become explicit baseline facts before readers switch from the
-- registration columns. A later import adds a new batch-scoped decision through import_store.
INSERT INTO linggan_ops_publication_field_adoption
    (adoption_ref,publication_ref,field_name,disposition,source_kind,source_batch_ref,
     value_text,value_count,value_instant,observed_at)
SELECT gen_random_uuid(),p.publication_ref,candidate.field_name,candidate.disposition,
       'workbench_import',p.last_changed_batch_ref,candidate.value_text,candidate.value_count,
       candidate.value_instant,p.source_updated_at::text
  FROM linggan_ops_publication p
 CROSS JOIN LATERAL (VALUES
    ('title'::text, CASE WHEN p.title IS NULL THEN 'not_provided' ELSE 'adopted' END,
        p.title, NULL::bigint, NULL::timestamptz),
    ('body'::text, CASE WHEN p.body IS NULL THEN 'not_provided' ELSE 'adopted' END,
        p.body, NULL::bigint, NULL::timestamptz),
    ('like_count'::text, CASE WHEN p.like_count IS NULL THEN 'not_provided' ELSE 'adopted' END,
        NULL::text, p.like_count, NULL::timestamptz),
    ('collect_count'::text, CASE WHEN p.collect_count IS NULL THEN 'not_provided' ELSE 'adopted' END,
        NULL::text, p.collect_count, NULL::timestamptz),
    ('comment_count'::text, CASE WHEN p.comment_count IS NULL THEN 'not_provided' ELSE 'adopted' END,
        NULL::text, p.comment_count, NULL::timestamptz),
    ('published_at'::text, CASE WHEN p.published_at IS NULL THEN 'not_provided' ELSE 'adopted' END,
        NULL::text, NULL::bigint, p.published_at)
  ) AS candidate(field_name,disposition,value_text,value_count,value_instant)
 WHERE p.source_system='content_workbench'
ON CONFLICT (source_batch_ref, publication_ref, field_name) DO NOTHING;

CREATE INDEX linggan_ops_publication_field_adoption_current_idx
    ON linggan_ops_publication_field_adoption
       (publication_ref, field_name, observed_at DESC, created_at DESC);

CREATE TRIGGER linggan_ops_publication_field_adoption_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_field_adoption
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_field_adoption IS
    'Current detail fields are selected from append-only import, native-registration, or receipt-qualified adoption facts; the registration row is never used as a hidden read fallback.';
