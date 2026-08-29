-- OBSERVATION-RUNTIME-001 · worker heartbeat and discovery evidence fields
--
-- This migration is expand-only. Existing discovery rows remain UNKNOWN; it deliberately does
-- not scan immutable package payloads to make an old library look populated.

CREATE TABLE collection_scheduler_heartbeat (
    scheduler_key text PRIMARY KEY CHECK (scheduler_key = 'patrol'),
    worker_instance_ref uuid,
    worker_started_at timestamptz,
    last_tick_started_at timestamptz,
    last_tick_completed_at timestamptz,
    last_outcome text NOT NULL DEFAULT 'unknown'
        CHECK (last_outcome IN ('unknown', 'idle', 'dispatched', 'partial', 'failed')),
    dispatched_count integer NOT NULL DEFAULT 0 CHECK (dispatched_count >= 0),
    skipped_count integer NOT NULL DEFAULT 0 CHECK (skipped_count >= 0),
    last_error text,
    CHECK (last_tick_completed_at IS NULL OR last_tick_started_at IS NOT NULL),
    CHECK ((last_outcome = 'failed') = (last_error IS NOT NULL))
);

ALTER TABLE linggan_material_discovery_finding
    ADD COLUMN cover_source_url text,
    ADD COLUMN cover_source_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (cover_source_state IN ('KNOWN','UNKNOWN')),
    ADD COLUMN like_count bigint,
    ADD COLUMN like_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (like_count_state IN ('KNOWN','UNKNOWN')),
    ADD COLUMN comment_count bigint,
    ADD COLUMN comment_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (comment_count_state IN ('KNOWN','UNKNOWN')),
    ADD COLUMN collect_count bigint,
    ADD COLUMN collect_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (collect_count_state IN ('KNOWN','UNKNOWN')),
    ADD COLUMN share_count bigint,
    ADD COLUMN share_count_state text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (share_count_state IN ('KNOWN','UNKNOWN')),
    ADD CONSTRAINT linggan_material_discovery_cover_state_check
        CHECK ((cover_source_state='KNOWN')=(cover_source_url IS NOT NULL)),
    ADD CONSTRAINT linggan_material_discovery_like_state_check
        CHECK ((like_count_state='KNOWN')=(like_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_discovery_comment_state_check
        CHECK ((comment_count_state='KNOWN')=(comment_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_discovery_collect_state_check
        CHECK ((collect_count_state='KNOWN')=(collect_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_discovery_share_state_check
        CHECK ((share_count_state='KNOWN')=(share_count IS NOT NULL)),
    ADD CONSTRAINT linggan_material_discovery_nonnegative_counts_check
        CHECK (like_count >= 0 AND comment_count >= 0 AND collect_count >= 0 AND share_count >= 0);

COMMENT ON COLUMN linggan_material_discovery_finding.cover_source_url IS
    'Observed remote cover candidate from this discovery record; not a verified local replica.';

-- A stale scheduled executor may no longer change Work Order state, but bytes it already paid
-- to observe are not thrown away. These two axes keep control authority separate from material
-- admission. Existing receipts remain UNKNOWN rather than being rewritten from incomplete history.
ALTER TABLE linggan_runtime_submission_receipt
    ADD COLUMN execution_effect text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (execution_effect IN ('UNKNOWN','NOT_APPLICABLE','COMPLETED_LIVE_STEP','LOST_AUTHORITY')),
    ADD COLUMN material_admission text NOT NULL DEFAULT 'UNKNOWN'
        CHECK (material_admission IN ('UNKNOWN','ACCEPTED'));

COMMENT ON COLUMN linggan_runtime_submission_receipt.execution_effect IS
    'Whether this submission could advance scheduled execution state; independent from material admission.';
COMMENT ON COLUMN linggan_runtime_submission_receipt.material_admission IS
    'Package-level admission only; per-record disposition remains authoritative for typed material use.';
