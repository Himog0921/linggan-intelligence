-- OPS-PUBLICATION-SYNC-001 S04: an exact Operations publication becomes a
-- Material only when its scheduled detail task has an accepted, live-authority
-- receipt and a qualified content record. This is an append-only link; it
-- must not rewrite the Operations ledger's manual metrics, responsibility,
-- business assignment, or historical registration.

CREATE TABLE linggan_ops_publication_material_projection (
    projection_ref uuid PRIMARY KEY,
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    publication_version bigint NOT NULL CHECK (publication_version > 0),
    request_ref uuid NOT NULL REFERENCES collection_acquisition_request(request_ref),
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    attempt_id uuid NOT NULL REFERENCES linggan_runtime_attempt(attempt_id),
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    receipt_ref uuid NOT NULL UNIQUE REFERENCES linggan_runtime_submission_receipt(receipt_ref),
    record_ordinal integer NOT NULL CHECK (record_ordinal >= 0),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    observed_at text NOT NULL CHECK (length(btrim(observed_at)) > 0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_material_content_detail(package_ref, record_ordinal)
);

CREATE INDEX linggan_ops_publication_material_projection_publication_idx
    ON linggan_ops_publication_material_projection (publication_ref, created_at DESC);

CREATE INDEX linggan_ops_publication_material_projection_content_idx
    ON linggan_ops_publication_material_projection (content_public_ref, created_at DESC);

CREATE TRIGGER linggan_ops_publication_material_projection_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_material_projection
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_material_projection IS
    'Qualified scheduled detail Receipt to exact Ops publication and typed Material link; append-only and never an Ops-ledger overwrite.';
