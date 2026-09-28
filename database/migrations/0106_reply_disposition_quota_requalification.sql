-- Historical replies above a task's maximumQuota carry the same contract mismatch
-- with its quota suffix. Requalify them through the same canonical disposition.
ALTER TABLE linggan_runtime_record_disposition
    DROP CONSTRAINT linggan_runtime_record_requalification_complete;

ALTER TABLE linggan_runtime_record_disposition
    ADD CONSTRAINT linggan_runtime_record_requalification_complete CHECK (
        (initial_disposition IS NULL AND initial_reason IS NULL AND requalified_at IS NULL AND requalification_basis IS NULL)
        OR (initial_disposition = 'quarantined'
            AND initial_reason IN ('task_package_contract_mismatch',
                                   'task_package_contract_mismatch__beyond_task_maximum_quota')
            AND requalified_at IS NOT NULL
            AND requalification_basis = 'reply_task_identity_v2'
            AND ((disposition = 'accepted_for_library_content'
                  AND reason IN ('typed_reply_relationship_valid', 'typed_reply_relationship_valid__beyond_task_maximum_quota'))
                 OR (disposition = 'quarantined'
                     AND reason IN ('typed_reply_relationship_invalid', 'typed_reply_relationship_invalid__beyond_task_maximum_quota',
                                    'typed_comment_identity_duplicate', 'typed_comment_identity_duplicate__beyond_task_maximum_quota'))))
    );

CREATE OR REPLACE FUNCTION linggan_reply_disposition_requalification_guard() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'Linggan Browser Producer Runtime facts are append-only';
    END IF;
    IF OLD.disposition <> 'quarantined'
       OR OLD.reason NOT IN ('task_package_contract_mismatch',
                             'task_package_contract_mismatch__beyond_task_maximum_quota')
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
