-- OPS-PUBLICATION-SYNC-001 S16: new exact Operations scopes collect the media
-- slots required by the selected detail page.  Earlier immutable scopes retain
-- their detail-only fact; changing them in place would turn an old consent into
-- a broader one.

DO $$
DECLARE
    acquire_media_constraint text;
BEGIN
    -- PostgreSQL truncates automatically generated check names at 63 bytes.
    -- Locate the old immutable-false guard by definition rather than guessing
    -- which truncation the database used when 0155 was applied.
    SELECT conname INTO acquire_media_constraint
      FROM pg_constraint
     WHERE conrelid = 'collection_work_order_ops_publication_target'::regclass
       AND contype = 'c'
       AND pg_get_constraintdef(oid) LIKE '%acquire_media = false%';
    IF acquire_media_constraint IS NULL THEN
        RAISE EXCEPTION '0155 acquire_media=false guard is missing';
    END IF;
    EXECUTE format(
        'ALTER TABLE collection_work_order_ops_publication_target DROP CONSTRAINT %I',
        acquire_media_constraint
    );
END $$;

ALTER TABLE collection_work_order_ops_publication_target
    ALTER COLUMN acquire_media SET DEFAULT true;

-- A qualified media-slot Receipt is linked to the exact Operations publication
-- separately from the detail/field projection.  It records an observed source
-- slot, not a remote URL and not a claim that bytes are locally available.
CREATE TABLE linggan_ops_publication_media_projection (
    media_projection_ref uuid PRIMARY KEY,
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    publication_version bigint NOT NULL CHECK (publication_version > 0),
    request_ref uuid NOT NULL REFERENCES collection_acquisition_request(request_ref),
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    attempt_id uuid NOT NULL REFERENCES linggan_runtime_attempt(attempt_id),
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    receipt_ref uuid NOT NULL REFERENCES linggan_runtime_submission_receipt(receipt_ref),
    record_ordinal integer NOT NULL CHECK (record_ordinal >= 0),
    observation_ref uuid NOT NULL REFERENCES linggan_material_media_origin(observation_ref),
    slot_key text NOT NULL REFERENCES linggan_media_slot(slot_key),
    observed_at text NOT NULL CHECK (length(btrim(observed_at)) > 0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    UNIQUE (publication_ref, publication_version, observation_ref),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_material_media_origin(package_ref, record_ordinal)
);

CREATE INDEX linggan_ops_publication_media_projection_current_idx
    ON linggan_ops_publication_media_projection
       (publication_ref, publication_version, created_at DESC);

CREATE TRIGGER linggan_ops_publication_media_projection_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_media_projection
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_media_projection IS
    'Receipt-qualified exact Ops publication to observed media-slot link; availability is derived only from the shared local materialization chain.';
