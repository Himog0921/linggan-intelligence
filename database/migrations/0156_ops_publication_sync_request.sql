-- OPS-LAUNCH-001 S01-S20 foundation: a sync click is a durable Operations
-- command, never a second queue.  Execution facts continue to live in the
-- collection Request -> Admission -> Work Order chain once an item is admitted.

CREATE TABLE linggan_ops_publication_sync_request (
    request_ref uuid PRIMARY KEY,
    actor_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
    selected_count integer NOT NULL CHECK (selected_count BETWEEN 1 AND 200),
    purpose text NOT NULL CHECK (purpose = 'ops_publication_sync'),
    submitted_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX linggan_ops_publication_sync_request_actor_idx
    ON linggan_ops_publication_sync_request (actor_member_ref, submitted_at DESC);

CREATE TRIGGER linggan_ops_publication_sync_request_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_sync_request
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

CREATE TABLE linggan_ops_publication_sync_item (
    sync_item_ref uuid PRIMARY KEY,
    request_ref uuid NOT NULL REFERENCES linggan_ops_publication_sync_request(request_ref),
    ordinal integer NOT NULL CHECK (ordinal BETWEEN 1 AND 200),
    requested_publication_ref uuid NOT NULL,
    publication_ref uuid REFERENCES linggan_ops_publication(publication_ref),
    account_ref uuid REFERENCES linggan_ops_account(account_ref),
    domain_ref uuid REFERENCES observation_domain(domain_ref),
    platform text CHECK (platform IN ('xhs', 'douyin')),
    expected_content_id text,
    expected_author_id text,
    publication_version bigint CHECK (publication_version IS NULL OR publication_version > 0),
    locator_sha256 bytea CHECK (locator_sha256 IS NULL OR octet_length(locator_sha256) = 32),
    decision text NOT NULL CHECK (decision IN (
        'accepted', 'existing', 'blocked', 'forbidden', 'conflict', 'invalid'
    )),
    decision_code text NOT NULL CHECK (length(btrim(decision_code)) BETWEEN 1 AND 120),
    acquisition_request_ref uuid REFERENCES collection_acquisition_request(request_ref),
    work_order_ref uuid REFERENCES collection_work_order(work_order_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (request_ref, ordinal),
    UNIQUE (request_ref, requested_publication_ref),
    CONSTRAINT linggan_ops_publication_sync_item_execution_link_check CHECK (
        (
            decision IN ('accepted', 'existing')
            AND publication_ref IS NOT NULL AND account_ref IS NOT NULL AND domain_ref IS NOT NULL
            AND platform IS NOT NULL AND expected_content_id IS NOT NULL AND expected_author_id IS NOT NULL
            AND publication_version IS NOT NULL AND locator_sha256 IS NOT NULL
            AND acquisition_request_ref IS NOT NULL AND work_order_ref IS NOT NULL
        )
        OR (
            decision NOT IN ('accepted', 'existing')
            AND acquisition_request_ref IS NULL AND work_order_ref IS NULL
        )
    )
);

CREATE INDEX linggan_ops_publication_sync_item_publication_idx
    ON linggan_ops_publication_sync_item (publication_ref, created_at DESC)
    WHERE publication_ref IS NOT NULL;

CREATE TRIGGER linggan_ops_publication_sync_item_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_sync_item
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_sync_request IS
    'One idempotent Operations sync command. It records a user decision, not queue or execution success.';
COMMENT ON TABLE linggan_ops_publication_sync_item IS
    'Frozen per-publication decision and, only after real admission, links to the existing acquisition chain.';
