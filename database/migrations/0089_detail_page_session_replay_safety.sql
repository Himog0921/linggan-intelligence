-- DETAIL-PAGE-SESSION-REPLAY-SAFETY-001
--
-- A detail-page session is an execution-access boundary, not a second task
-- ledger and not Evidence.  Task -> Attempt -> Capture Package -> Receipt
-- remains authoritative for each independently accepted lane.  This table
-- records the one server-side authorization context shared by those lanes so
-- a replayed claim cannot silently authorize another browser navigation.

CREATE TABLE collection_detail_page_session (
    session_ref uuid PRIMARY KEY,
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    -- Evidence materials and cross-industry samples deliberately remain in
    -- their own source tables.  One session records exactly one of them.
    -- `content_public_ref` has a direct FK because every collection-control
    -- schema includes the material projection. `cross_industry_sample_ref`
    -- intentionally has no FK: the collection-control-only proof schema does
    -- not install the cross-industry source table. Grant issuance joins and
    -- validates that sample before this row can be written. This boundary does
    -- not make a direct, out-of-contract SQL insert valid source evidence.
    content_public_ref uuid REFERENCES linggan_material_content(public_ref),
    cross_industry_sample_ref uuid,
    owner_installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    grant_request_id uuid NOT NULL,
    initial_lease_ref uuid NOT NULL REFERENCES collection_work_order_lease(lease_ref),
    plan_snapshot jsonb NOT NULL,
    plan_hash text NOT NULL CHECK (plan_hash ~ '^[0-9a-f]{64}$'),
    state text NOT NULL CHECK (state IN (
        'authorized','navigation_started','navigation_committed','delivery_pending','finished','stopped'
    )),
    authorized_at timestamptz NOT NULL DEFAULT scope_001_now(),
    -- This is an observed navigation fact reported by the Browser Producer.
    -- It is deliberately nullable: authorization or local consumption alone
    -- must never be presented as proof that Chrome committed a page load.
    navigation_observed_at timestamptz,
    last_progress_at timestamptz NOT NULL DEFAULT scope_001_now(),
    finished_at timestamptz,
    stop_reason text CHECK (stop_reason IN (
        'navigation_state_unknown','owner_unavailable','page_unavailable','risk_stop','delivery_terminal'
    )),
    UNIQUE (work_order_ref, content_public_ref),
    UNIQUE (work_order_ref, cross_industry_sample_ref),
    UNIQUE (grant_request_id),
    CHECK ((content_public_ref IS NULL) <> (cross_industry_sample_ref IS NULL)),
    CHECK ((finished_at IS NULL) = (state NOT IN ('finished','stopped'))),
    CHECK ((stop_reason IS NULL) = (state <> 'stopped'))
);

CREATE INDEX collection_detail_page_session_owner_progress_idx
    ON collection_detail_page_session (owner_installation_ref,last_progress_at DESC);

COMMENT ON TABLE collection_detail_page_session IS
    'One server authorization context per Work Order/detail target. It prevents implicit repeat navigation; it is not a Task, Attempt, Package, Receipt, or Evidence table.';
COMMENT ON COLUMN collection_detail_page_session.grant_request_id IS
    'Browser-persisted idempotency identity. The same request may retrieve the same grant; a new id never resets an existing session.';
COMMENT ON COLUMN collection_detail_page_session.navigation_observed_at IS
    'Reported after Chrome observes the extension-created detail tab. Null means navigation was not observed, not that zero navigations are proven.';
