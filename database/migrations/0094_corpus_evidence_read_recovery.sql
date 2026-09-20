-- CORPUS-EVIDENCE-READ-RECOVERY-001
--
-- COMMENT-STUDY-001 retired the V1 research projection, but the Evidence
-- comment channel is a raw-Evidence read boundary.  Its restriction facts
-- therefore belong to the current material contract, not to a V1 view.

CREATE TABLE IF NOT EXISTS linggan_material_comment_restriction (
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    comment_external_id text NOT NULL,
    restricted_at timestamptz NOT NULL DEFAULT scope_001_now(),
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 1 AND 500),
    PRIMARY KEY(content_public_ref,comment_external_id)
);

DO $$
BEGIN
    IF to_regclass('linggan_comment_research_restriction') IS NOT NULL THEN
        INSERT INTO linggan_material_comment_restriction(
            content_public_ref,comment_external_id,restricted_at,reason
        )
        SELECT content_public_ref,comment_external_id,restricted_at,reason
        FROM linggan_comment_research_restriction
        ON CONFLICT(content_public_ref,comment_external_id) DO NOTHING;
    END IF;
END $$;

-- The Evidence Library reads a bounded page, then enriches its media state.
-- These lookup indexes cover the exact equality/order predicates in that
-- bounded read; they do not create a second Current projection.
CREATE INDEX IF NOT EXISTS linggan_material_media_origin_content_slot_idx
    ON linggan_material_media_origin(content_public_ref,slot_key);
CREATE INDEX IF NOT EXISTS linggan_media_processing_job_slot_created_idx
    ON linggan_media_processing_job(slot_key,created_at);
CREATE INDEX IF NOT EXISTS linggan_media_derivative_job_created_idx
    ON linggan_media_derivative(job_ref,created_at);
CREATE INDEX IF NOT EXISTS linggan_media_processing_job_event_job_occurred_idx
    ON linggan_media_processing_job_event(job_ref,occurred_at DESC);

-- `IF NOT EXISTS` must not turn a same-named, incompatible relation into a
-- permanently recorded no-op.  The local migration ledger is append-only, so
-- fail here rather than let a later install claim that the bounded read was
-- indexed when it was not.
DO $$
DECLARE
    expected record;
    index_oid regclass;
BEGIN
    FOR expected IN
        SELECT *
        FROM (VALUES
            ('linggan_material_media_origin_content_slot_idx',
             'linggan_material_media_origin'::regclass,
             'content_public_ref', 'slot_key', false),
            ('linggan_media_processing_job_slot_created_idx',
             'linggan_media_processing_job'::regclass,
             'slot_key', 'created_at', false),
            ('linggan_media_derivative_job_created_idx',
             'linggan_media_derivative'::regclass,
             'job_ref', 'created_at', false),
            ('linggan_media_processing_job_event_job_occurred_idx',
             'linggan_media_processing_job_event'::regclass,
             'job_ref', 'occurred_at', true)
        ) AS expected(index_name,owner_table,first_key,second_key,second_key_desc)
    LOOP
        index_oid := to_regclass(expected.index_name);
        IF index_oid IS NULL OR NOT EXISTS (
            SELECT 1
            FROM pg_index indexed
            JOIN pg_class relation ON relation.oid=indexed.indexrelid
            JOIN pg_am access_method ON access_method.oid=relation.relam
            WHERE indexed.indexrelid=index_oid
              AND relation.relkind='i'
              AND indexed.indrelid=expected.owner_table
              AND access_method.amname='btree'
              AND NOT indexed.indisunique
              AND indexed.indnkeyatts=2
              AND indexed.indnatts=2
              AND indexed.indpred IS NULL
              AND indexed.indexprs IS NULL
              AND indexed.indisvalid
              AND indexed.indisready
              AND pg_get_indexdef(indexed.indexrelid,1,true)=expected.first_key
              AND pg_get_indexdef(indexed.indexrelid,2,true)=expected.second_key
              AND pg_index_column_has_property(indexed.indexrelid,1,'asc')
              AND pg_index_column_has_property(
                    indexed.indexrelid,2,'desc'
                  )=expected.second_key_desc
        ) THEN
            RAISE EXCEPTION
                '0094 requires compatible index % on % (% , %)',
                expected.index_name, expected.owner_table,
                expected.first_key,
                CASE WHEN expected.second_key_desc
                     THEN expected.second_key || ' DESC'
                     ELSE expected.second_key END;
        END IF;
    END LOOP;
END $$;
