-- COLLECTION-REPLIES-CONTRACT-001 · One canonical disposition with preserved first decision.
-- A corrected decision remains on the same record key consumed by every material reader.
-- The first decision is retained as audit evidence, not exposed as a second active disposition.

ALTER TABLE linggan_runtime_record_disposition
    ADD COLUMN initial_disposition text,
    ADD COLUMN initial_reason text,
    ADD COLUMN requalified_at timestamptz,
    ADD COLUMN requalification_basis text,
    ADD CONSTRAINT linggan_runtime_record_requalification_complete CHECK (
        (initial_disposition IS NULL AND initial_reason IS NULL AND requalified_at IS NULL AND requalification_basis IS NULL)
        OR (initial_disposition = 'quarantined'
            AND initial_reason = 'task_package_contract_mismatch'
            AND requalified_at IS NOT NULL
            AND requalification_basis = 'reply_task_identity_v2'
            AND ((disposition = 'accepted_for_library_content'
                  AND reason IN ('typed_reply_relationship_valid', 'typed_reply_relationship_valid__beyond_task_maximum_quota'))
                 OR (disposition = 'quarantined'
                     AND reason IN ('typed_reply_relationship_invalid', 'typed_reply_relationship_invalid__beyond_task_maximum_quota',
                                    'typed_comment_identity_duplicate', 'typed_comment_identity_duplicate__beyond_task_maximum_quota'))))
    );

CREATE FUNCTION linggan_reply_disposition_requalification_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Linggan Browser Producer Runtime facts are append-only';
    END IF;
    IF OLD.disposition <> 'quarantined'
       OR OLD.reason <> 'task_package_contract_mismatch'
       OR OLD.initial_disposition IS NOT NULL
       OR NEW.package_ref IS DISTINCT FROM OLD.package_ref
       OR NEW.record_ordinal IS DISTINCT FROM OLD.record_ordinal
       OR NEW.created_at IS DISTINCT FROM OLD.created_at
       OR NEW.initial_disposition IS DISTINCT FROM OLD.disposition
       OR NEW.initial_reason IS DISTINCT FROM OLD.reason
       OR NEW.requalification_basis IS DISTINCT FROM 'reply_task_identity_v2'
       OR NEW.requalified_at IS NULL
       OR NOT ((NEW.disposition = 'accepted_for_library_content'
                AND NEW.reason IN ('typed_reply_relationship_valid', 'typed_reply_relationship_valid__beyond_task_maximum_quota'))
               OR (NEW.disposition = 'quarantined'
                   AND NEW.reason IN ('typed_reply_relationship_invalid', 'typed_reply_relationship_invalid__beyond_task_maximum_quota',
                                      'typed_comment_identity_duplicate', 'typed_comment_identity_duplicate__beyond_task_maximum_quota')))
       OR NOT EXISTS (SELECT 1 FROM linggan_runtime_capture_package package
                       WHERE package.package_ref=OLD.package_ref AND package.package_kind='replies') THEN
        RAISE EXCEPTION 'reply disposition requalification must preserve the first decision and use a recognized outcome';
    END IF;
    RETURN NEW;
END $$;

DROP TRIGGER linggan_runtime_record_disposition_is_append_only ON linggan_runtime_record_disposition;
CREATE TRIGGER linggan_runtime_record_disposition_requalification_guard
    BEFORE UPDATE OR DELETE ON linggan_runtime_record_disposition
    FOR EACH ROW EXECUTE FUNCTION linggan_reply_disposition_requalification_guard();
