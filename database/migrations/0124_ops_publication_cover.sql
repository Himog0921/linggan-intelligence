-- OPS-MIGRATION-001 D2 / Issue #405: historical publication covers copied from the content
-- workbench machine by Mog. Each row ties one publication registration to one content-addressed
-- file under the local media root; the read route serves only files registered here.
-- Contract: docs/data-contracts/ops-http-001.md.

CREATE TABLE linggan_ops_publication_cover (
    publication_ref uuid PRIMARY KEY REFERENCES linggan_ops_publication(publication_ref),
    -- The workbench media path the export named for this cover; kept as the provenance anchor.
    source_path text NOT NULL CHECK (length(source_path) BETWEEN 1 AND 1024),
    content_sha256 text NOT NULL CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    byte_size bigint NOT NULL CHECK (byte_size > 0 AND byte_size <= 20971520),
    mime_type text NOT NULL CHECK (mime_type IN ('image/webp', 'image/jpeg', 'image/png')),
    storage_key text NOT NULL,
    import_ref uuid NOT NULL,
    imported_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_publication_cover_storage_key_check CHECK (
        storage_key = 'ops-covers/' || content_sha256 || CASE mime_type
            WHEN 'image/webp' THEN '.webp'
            WHEN 'image/jpeg' THEN '.jpg'
            ELSE '.png'
        END
    )
);

CREATE INDEX linggan_ops_publication_cover_import_idx
    ON linggan_ops_publication_cover (import_ref);

-- A registered cover is a fact about one import; a different file later is a conflict to report,
-- never a silent overwrite.
CREATE TRIGGER linggan_ops_publication_cover_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_cover
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_cover IS
    'Historical workbench covers registered to publication registrations; files live under the local media root.';
