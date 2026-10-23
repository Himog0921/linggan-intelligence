-- OPS-PUBLICATION-SYNC-001 S04: an exact Operations publication has no
-- Material identity, but a rejected/missing execution input must still stop
-- its one authorized detail task instead of repeatedly leasing it.

DO $$
DECLARE
    object_kind_constraint record;
BEGIN
    FOR object_kind_constraint IN
        SELECT constraint_row.conname
        FROM pg_constraint constraint_row
        WHERE constraint_row.conrelid='collection_execution_input_eligibility'::regclass
          AND constraint_row.contype='c'
          AND pg_get_constraintdef(constraint_row.oid) LIKE '%object_kind%'
    LOOP
        EXECUTE format(
            'ALTER TABLE collection_execution_input_eligibility DROP CONSTRAINT %I',
            object_kind_constraint.conname
        );
    END LOOP;
END $$;

ALTER TABLE collection_execution_input_eligibility
    ADD CONSTRAINT collection_execution_input_eligibility_object_kind_check
    CHECK (object_kind IN ('material_content','cross_industry_sample','ops_publication'));

COMMENT ON COLUMN collection_execution_input_eligibility.object_ref IS
    'Material, cross-industry sample, or exact Operations-publication identity. No polymorphic foreign key is implied.';
