-- OPS-TEAM-ADMIN: retain old receipts and distinguish access-only emergency stops.
ALTER TABLE linggan_ops_member_deactivation_request
    ALTER COLUMN unassigned_account_count DROP NOT NULL,
    ADD COLUMN mode text NOT NULL DEFAULT 'legacy'
        CHECK (mode IN ('legacy', 'immediate', 'planned')),
    ADD COLUMN group_vacancy_ref uuid REFERENCES linggan_ops_group(group_ref);

ALTER TABLE linggan_ops_account_responsibility_change
    ADD COLUMN deactivation_request_ref uuid REFERENCES linggan_ops_member_deactivation_request(request_ref);

CREATE INDEX linggan_ops_account_responsibility_deactivation_idx
    ON linggan_ops_account_responsibility_change (deactivation_request_ref)
    WHERE deactivation_request_ref IS NOT NULL;

-- Use the same record eligibility for lists, exports, covers and manual reports.
CREATE FUNCTION linggan_ops_publication_visible(
    p linggan_ops_publication,
    a linggan_ops_account,
    visible_members uuid[],
    viewer uuid
) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT p.merged_into_ref IS NULL
       AND p.missing_since_batch_ref IS NULL
       AND (p.source_system <> 'linggan_native' OR p.native_record_state = 'active')
       AND CASE
           WHEN visible_members IS NULL THEN true
           WHEN p.source_system = 'linggan_native' THEN
               p.native_publisher_member_ref = ANY(visible_members)
               OR a.current_responsible_member_ref = ANY(visible_members)
           WHEN p.source_system = 'content_workbench' THEN EXISTS (
               SELECT 1 FROM (
                   SELECT resolution.state, resolution.resolved_member_ref
                     FROM linggan_ops_historical_owner_resolution resolution
                    WHERE resolution.publication_ref = p.publication_ref
                    ORDER BY resolution.decided_at DESC, resolution.resolution_ref DESC
                    LIMIT 1
               ) latest
               WHERE latest.state = 'matched' AND (
                   latest.resolved_member_ref = ANY(visible_members)
                   OR (viewer = a.current_responsible_member_ref AND (
                       (a.source_system = 'linggan_native' AND a.native_record_state = 'active')
                       OR (a.source_system = 'content_workbench' AND a.ledger_state = 'in_ledger')
                   ))
               )
           )
           ELSE false
       END
$$;
