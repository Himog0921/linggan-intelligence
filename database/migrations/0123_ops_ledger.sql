-- OPS-MIGRATION-001 D1 / Issue #402: the operations ledger migrated from the content workbench.
--
-- Imported values only state what the workbench recorded at export time. They are not Linggan
-- observations and never prove that an account is usable, a post is visible or a number is right.
-- Until the D6 cutover the workbench is the only place people enter data; these tables mirror its
-- exports and are never edited in Linggan. Contract: docs/data-contracts/ops-ledger-001.md.

CREATE FUNCTION linggan_ops_forbid_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN
        RAISE EXCEPTION 'operations ledger: % on % is not allowed', TG_OP, TG_TABLE_NAME;
    END $$;

-- One applied import. The preview hash fixes the three export files, the stated export time,
-- the business domain and the rule version; applying the same preview twice is a replay.
CREATE TABLE linggan_ops_import_batch (
    batch_ref uuid PRIMARY KEY,
    preview_hash text NOT NULL UNIQUE CHECK (preview_hash ~ '^[0-9a-f]{64}$'),
    rule_version text NOT NULL CHECK (rule_version = 'ops-import.v1'),
    source_system text NOT NULL CHECK (source_system = 'content_workbench'),
    source_workspace_id text NOT NULL CHECK (length(source_workspace_id) BETWEEN 1 AND 128),
    exported_at timestamptz NOT NULL,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    accounts_sha256 text NOT NULL CHECK (accounts_sha256 ~ '^[0-9a-f]{64}$'),
    publications_sha256 text NOT NULL CHECK (publications_sha256 ~ '^[0-9a-f]{64}$'),
    daily_sha256 text NOT NULL CHECK (daily_sha256 ~ '^[0-9a-f]{64}$'),
    account_rows integer NOT NULL CHECK (account_rows >= 0),
    publication_rows integer NOT NULL CHECK (publication_rows >= 0),
    daily_rows integer NOT NULL CHECK (daily_rows >= 0),
    accounts_inserted integer NOT NULL CHECK (accounts_inserted >= 0),
    accounts_updated integer NOT NULL CHECK (accounts_updated >= 0),
    accounts_unchanged integer NOT NULL CHECK (accounts_unchanged >= 0),
    publications_inserted integer NOT NULL CHECK (publications_inserted >= 0),
    publications_updated integer NOT NULL CHECK (publications_updated >= 0),
    publications_unchanged integer NOT NULL CHECK (publications_unchanged >= 0),
    publications_marked_missing integer NOT NULL CHECK (publications_marked_missing >= 0),
    daily_inserted integer NOT NULL CHECK (daily_inserted >= 0),
    daily_updated integer NOT NULL CHECK (daily_updated >= 0),
    daily_unchanged integer NOT NULL CHECK (daily_unchanged >= 0),
    daily_marked_missing integer NOT NULL CHECK (daily_marked_missing >= 0),
    view_reports_appended integer NOT NULL CHECK (view_reports_appended >= 0),
    applied_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TRIGGER linggan_ops_import_batch_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_import_batch
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_import_batch IS
    'Applied content-workbench imports with file fingerprints and per-table change counts.';

-- An account we operate. `in_ledger` accounts came from the account export. `absent_from_export`
-- accounts are absent from the latest account export: either deleted in the workbench before the
-- first import and rebuilt from publication records (old status unknown, so NULL), or deleted later
-- (their last known fields stay). Pages show both as「已停用」.
CREATE TABLE linggan_ops_account (
    account_ref uuid PRIMARY KEY,
    source_system text NOT NULL CHECK (source_system = 'content_workbench'),
    source_record_id text NOT NULL CHECK (length(source_record_id) BETWEEN 1 AND 128),
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    ledger_state text NOT NULL CHECK (ledger_state IN ('in_ledger', 'absent_from_export')),
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    account_name text NOT NULL CHECK (length(account_name) BETWEEN 1 AND 512),
    handle text,
    status text CHECK (status IN ('active', 'violation_disabled', 'warming', 'paused', 'archived')),
    is_retained boolean,
    operator_name text,
    responsible_user_source_id text,
    follower_count bigint CHECK (follower_count IS NULL OR follower_count >= 0),
    profile_url text,
    profile_link_state text NOT NULL
        CHECK (profile_link_state IN ('parsed', 'pending_match', 'missing')),
    platform_user_id text,
    device_code text,
    ip_location text,
    notes text,
    review_status text
        CHECK (review_status IN ('confirmed', 'draft', 'pending_review', 'unassigned')),
    created_by_user_source_id text,
    source_created_at timestamptz,
    source_updated_at timestamptz,
    first_batch_ref uuid NOT NULL,
    last_changed_batch_ref uuid NOT NULL,
    CONSTRAINT linggan_ops_account_source_key UNIQUE (source_system, source_record_id),
    CONSTRAINT linggan_ops_account_ledger_fields_check CHECK (
        ledger_state = 'absent_from_export'
        OR (status IS NOT NULL AND is_retained IS NOT NULL AND review_status IS NOT NULL
            AND source_created_at IS NOT NULL AND source_updated_at IS NOT NULL)
    ),
    CONSTRAINT linggan_ops_account_profile_missing_check
        CHECK ((profile_link_state = 'missing') = (profile_url IS NULL)),
    CONSTRAINT linggan_ops_account_profile_parsed_check
        CHECK ((profile_link_state = 'parsed') = (platform_user_id IS NOT NULL)),
    CONSTRAINT linggan_ops_account_platform_user_check
        CHECK (platform_user_id IS NULL
               OR (platform = 'xhs' AND platform_user_id ~ '^[0-9a-f]{24}$'))
);

CREATE INDEX linggan_ops_account_domain_idx
    ON linggan_ops_account (domain_ref, platform, ledger_state);

-- One publication registration as entered in the workbench. View counts are not a column: they
-- live in linggan_ops_view_report. `legacy_*` columns keep the old backflow meaning unchanged.
CREATE TABLE linggan_ops_publication (
    publication_ref uuid PRIMARY KEY,
    source_system text NOT NULL CHECK (source_system = 'content_workbench'),
    source_record_id text NOT NULL CHECK (length(source_record_id) BETWEEN 1 AND 128),
    account_ref uuid REFERENCES linggan_ops_account(account_ref),
    platform text NOT NULL CHECK (platform IN ('xhs', 'douyin')),
    publication_url text NOT NULL CHECK (length(publication_url) BETWEEN 1 AND 4096),
    canonical_url text NOT NULL CHECK (length(canonical_url) BETWEEN 1 AND 4096),
    content_id_state text NOT NULL CHECK (content_id_state IN (
        'parsed_publication_url', 'parsed_canonical_url', 'pending_match', 'conflict'
    )),
    platform_content_id text,
    published_at timestamptz NOT NULL,
    -- Business days follow Beijing time, as in the workbench.
    published_business_date date
        GENERATED ALWAYS AS ((published_at AT TIME ZONE 'Asia/Shanghai')::date) STORED,
    published_date_state text NOT NULL
        CHECK (published_date_state IN ('as_recorded', 'suspect_year')),
    title text,
    body text,
    publisher_name text,
    responsible_user_source_id text,
    created_by_user_source_id text,
    like_count bigint CHECK (like_count IS NULL OR like_count >= 0),
    collect_count bigint CHECK (collect_count IS NULL OR collect_count >= 0),
    comment_count bigint CHECK (comment_count IS NULL OR comment_count >= 0),
    cover_click_rate double precision CHECK (
        cover_click_rate IS NULL OR (cover_click_rate >= 0 AND cover_click_rate < 'Infinity')
    ),
    entry_source text NOT NULL
        CHECK (entry_source IN ('manual', 'csv_import', 'topic_publication_sync')),
    review_status text NOT NULL
        CHECK (review_status IN ('confirmed', 'draft', 'pending_review', 'unassigned')),
    legacy_effective_publication boolean NOT NULL,
    legacy_data_synced_at timestamptz,
    legacy_data_sync_status text NOT NULL CHECK (legacy_data_sync_status IN (
        'syncable', 'synced', 'content_unavailable', 'needs_link', 'manual_fill', 'excluded'
    )),
    -- Duplicate registrations of the same post point at the kept record (rule ops-dedupe.v1).
    merged_into_ref uuid,
    -- NULL while the record is in the latest export. Set when a later export no longer has it;
    -- this states absence from the export, not deletion on the platform.
    missing_since_batch_ref uuid,
    source_created_at timestamptz NOT NULL,
    source_updated_at timestamptz NOT NULL,
    first_batch_ref uuid NOT NULL,
    last_changed_batch_ref uuid NOT NULL,
    CONSTRAINT linggan_ops_publication_source_key UNIQUE (source_system, source_record_id),
    CONSTRAINT linggan_ops_publication_content_id_check CHECK (
        (content_id_state IN ('parsed_publication_url', 'parsed_canonical_url'))
        = (platform_content_id IS NOT NULL)
    ),
    CONSTRAINT linggan_ops_publication_content_id_shape_check CHECK (
        platform_content_id IS NULL
        OR (platform = 'xhs' AND platform_content_id ~ '^[0-9a-f]{24}$')
    ),
    CONSTRAINT linggan_ops_publication_not_self_merged_check
        CHECK (merged_into_ref IS NULL OR merged_into_ref <> publication_ref),
    CONSTRAINT linggan_ops_publication_missing_not_merged_check
        CHECK (missing_since_batch_ref IS NULL OR merged_into_ref IS NULL)
);

ALTER TABLE linggan_ops_publication
    ADD CONSTRAINT linggan_ops_publication_merged_into_fk
    FOREIGN KEY (merged_into_ref) REFERENCES linggan_ops_publication(publication_ref)
    DEFERRABLE INITIALLY DEFERRED;

CREATE INDEX linggan_ops_publication_account_idx
    ON linggan_ops_publication (account_ref, published_at);
CREATE INDEX linggan_ops_publication_business_date_idx
    ON linggan_ops_publication (published_business_date, platform);
CREATE INDEX linggan_ops_publication_content_idx
    ON linggan_ops_publication (platform, platform_content_id)
    WHERE platform_content_id IS NOT NULL;
CREATE INDEX linggan_ops_publication_merged_idx
    ON linggan_ops_publication (merged_into_ref) WHERE merged_into_ref IS NOT NULL;

-- One view-count report. The current value of a publication is its latest report by sequence;
-- a latest `cleared` report, like no report at all, means the count is unknown.
CREATE TABLE linggan_ops_view_report (
    report_ref uuid PRIMARY KEY,
    report_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    report_kind text NOT NULL CHECK (report_kind IN ('value', 'cleared')),
    view_count bigint CHECK (view_count IS NULL OR view_count >= 0),
    -- Imported reports do not know who typed the value in the workbench.
    source_kind text NOT NULL CHECK (source_kind = 'workbench_import'),
    batch_ref uuid NOT NULL,
    as_of timestamptz NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_view_report_value_check
        CHECK ((report_kind = 'value') = (view_count IS NOT NULL))
);

CREATE INDEX linggan_ops_view_report_latest_idx
    ON linggan_ops_view_report (publication_ref, report_sequence DESC);

CREATE TRIGGER linggan_ops_view_report_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_view_report
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

-- One operator's day. In the workbench both counts default to 0, so 0 and "not entered" cannot be
-- told apart; the old values are kept as they were.
CREATE TABLE linggan_ops_daily_report (
    daily_ref uuid PRIMARY KEY,
    source_system text NOT NULL CHECK (source_system = 'content_workbench'),
    source_record_id text NOT NULL CHECK (length(source_record_id) BETWEEN 1 AND 128),
    operator_name text NOT NULL CHECK (length(operator_name) BETWEEN 1 AND 256),
    business_date date NOT NULL,
    links_opened_count integer NOT NULL CHECK (links_opened_count >= 0),
    followers_gained integer NOT NULL CHECK (followers_gained >= 0),
    responsible_user_source_id text,
    created_by_user_source_id text,
    review_status text NOT NULL
        CHECK (review_status IN ('confirmed', 'draft', 'pending_review', 'unassigned')),
    notes text,
    missing_since_batch_ref uuid,
    source_created_at timestamptz NOT NULL,
    source_updated_at timestamptz NOT NULL,
    first_batch_ref uuid NOT NULL,
    last_changed_batch_ref uuid NOT NULL,
    CONSTRAINT linggan_ops_daily_report_source_key UNIQUE (source_system, source_record_id)
);

-- The workbench keeps one live row per operator and day.
CREATE UNIQUE INDEX linggan_ops_daily_report_operator_day_idx
    ON linggan_ops_daily_report (operator_name, business_date)
    WHERE missing_since_batch_ref IS NULL;

-- Batch references are written before the batch row inside the same import transaction.
ALTER TABLE linggan_ops_account
    ADD CONSTRAINT linggan_ops_account_first_batch_fk FOREIGN KEY (first_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT linggan_ops_account_last_batch_fk FOREIGN KEY (last_changed_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE linggan_ops_publication
    ADD CONSTRAINT linggan_ops_publication_first_batch_fk FOREIGN KEY (first_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT linggan_ops_publication_last_batch_fk FOREIGN KEY (last_changed_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT linggan_ops_publication_missing_batch_fk FOREIGN KEY (missing_since_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE linggan_ops_view_report
    ADD CONSTRAINT linggan_ops_view_report_batch_fk FOREIGN KEY (batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE linggan_ops_daily_report
    ADD CONSTRAINT linggan_ops_daily_report_first_batch_fk FOREIGN KEY (first_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT linggan_ops_daily_report_last_batch_fk FOREIGN KEY (last_changed_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED,
    ADD CONSTRAINT linggan_ops_daily_report_missing_batch_fk FOREIGN KEY (missing_since_batch_ref)
        REFERENCES linggan_ops_import_batch(batch_ref) DEFERRABLE INITIALLY DEFERRED;

-- Mirror rows may be refreshed by a later import but are never deleted.
CREATE TRIGGER linggan_ops_account_is_never_deleted
    BEFORE DELETE ON linggan_ops_account
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_ops_publication_is_never_deleted
    BEFORE DELETE ON linggan_ops_publication
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_ops_daily_report_is_never_deleted
    BEFORE DELETE ON linggan_ops_daily_report
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_account IS
    'Own accounts mirrored from the content workbench; every account belongs to one domain.';
COMMENT ON TABLE linggan_ops_publication IS
    'Publication registrations as entered in the content workbench; not Linggan observations.';
COMMENT ON TABLE linggan_ops_view_report IS
    'Append-only view-count reports; the latest report per publication is its current value.';
COMMENT ON TABLE linggan_ops_daily_report IS
    'Operator daily link-open and follower counts as entered in the content workbench.';
