-- OPS-PUBLICATION-SYNC-001 S04: a registered Operations publication is not a
-- Material and must never be represented by a fabricated discovery finding or
-- capture package. This immutable scope is the one legal bridge from an
-- admitted Collection Work Order to the explicitly selected publication.
--
-- It records the exact platform identity and the protected execution-locator
-- reference, not the signed URL. The dispatcher may resolve that reference
-- only for a task belonging to this Work Order.

ALTER TABLE linggan_ops_publication_execution_locator
    ADD CONSTRAINT linggan_ops_publication_execution_locator_identity_key
    UNIQUE (publication_ref, publication_version, locator_ref);

CREATE TABLE collection_work_order_ops_publication_target (
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    publication_version bigint NOT NULL CHECK (publication_version > 0),
    locator_ref uuid NOT NULL,
    content_external_id text NOT NULL CHECK (length(btrim(content_external_id)) > 0),
    author_external_id text NOT NULL CHECK (length(btrim(author_external_id)) > 0),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    -- S04 is an exact detail read. Comments, replies, media, OCR and ASR
    -- require their own explicitly authorized scope; a sync click cannot
    -- silently widen into any of those lanes.
    comment_limit integer NOT NULL DEFAULT 0 CHECK (comment_limit = 0),
    reply_expand_limit integer NOT NULL DEFAULT 0 CHECK (reply_expand_limit = 0),
    acquire_media boolean NOT NULL DEFAULT false CHECK (acquire_media = false),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (work_order_ref, publication_ref),
    UNIQUE (work_order_ref, ordinal),
    CONSTRAINT collection_work_order_ops_publication_target_locator_binding
        FOREIGN KEY (publication_ref, publication_version, locator_ref)
        REFERENCES linggan_ops_publication_execution_locator
            (publication_ref, publication_version, locator_ref)
);

CREATE INDEX collection_work_order_ops_publication_target_locator_idx
    ON collection_work_order_ops_publication_target (locator_ref);

CREATE TRIGGER collection_work_order_ops_publication_target_is_append_only
    BEFORE UPDATE OR DELETE ON collection_work_order_ops_publication_target
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE collection_work_order_ops_publication_target IS
    'Immutable exact Ops-publication scope for an admitted Work Order; never a material, discovery result, package, or second queue.';
