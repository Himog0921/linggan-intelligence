-- OPS-PUBLICATION-SYNC-001 S04: a detail-page session is an authorization
-- context for one frozen source object.  Extend the existing material/sample
-- identity choice with the exact Operations-publication scope without
-- inventing a Material identity for it.

ALTER TABLE collection_detail_page_session
    ADD COLUMN ops_publication_ref uuid REFERENCES linggan_ops_publication(publication_ref);

DO $$
DECLARE
    source_identity_constraint record;
BEGIN
    FOR source_identity_constraint IN
        SELECT constraint_row.conname
        FROM pg_constraint constraint_row
        WHERE constraint_row.conrelid='collection_detail_page_session'::regclass
          AND constraint_row.contype='c'
          AND pg_get_constraintdef(constraint_row.oid) LIKE '%content_public_ref%'
          AND pg_get_constraintdef(constraint_row.oid) LIKE '%cross_industry_sample_ref%'
    LOOP
        EXECUTE format(
            'ALTER TABLE collection_detail_page_session DROP CONSTRAINT %I',
            source_identity_constraint.conname
        );
    END LOOP;
END $$;

-- `0104` removes the cross-industry table and its session column in the unified final
-- schema.  Keep this bridge valid for both a database upgraded from the earlier shape and a
-- fresh final-schema install; otherwise the final fixture cannot even establish the table.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_schema=current_schema()
           AND table_name='collection_detail_page_session'
           AND column_name='cross_industry_sample_ref'
    ) THEN
        EXECUTE 'ALTER TABLE collection_detail_page_session
                 ADD CONSTRAINT collection_detail_page_session_one_source_object
                 CHECK (num_nonnulls(content_public_ref,cross_industry_sample_ref,ops_publication_ref)=1)';
    ELSE
        EXECUTE 'ALTER TABLE collection_detail_page_session
                 ADD CONSTRAINT collection_detail_page_session_one_source_object
                 CHECK (num_nonnulls(content_public_ref,ops_publication_ref)=1)';
    END IF;
END $$;

ALTER TABLE collection_detail_page_session
    ADD CONSTRAINT collection_detail_page_session_ops_publication_unique
    UNIQUE (work_order_ref,ops_publication_ref);

COMMENT ON COLUMN collection_detail_page_session.ops_publication_ref IS
    'Exact Operations publication source for this session. It is mutually exclusive with Material and cross-industry sources.';
