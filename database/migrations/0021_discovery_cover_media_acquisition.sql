-- MEDIA-ACQUISITION-001
--
-- Discovery already observes a cover URL, but a URL is only a short-lived source fact.  This
-- expand-only migration adds the small mutable control record needed to acquire those bytes.
-- Media identity, observations, download attempts, blobs and materializations remain in the
-- append-only tables introduced by 0004/0017.

CREATE TABLE linggan_discovery_cover_media_link (
    material_ref uuid PRIMARY KEY
        REFERENCES linggan_material_discovery_finding(material_ref),
    observation_ref uuid NOT NULL UNIQUE
        REFERENCES linggan_material_media_origin(observation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_media_acquisition_work (
    work_ref uuid PRIMARY KEY,
    observation_ref uuid NOT NULL UNIQUE
        REFERENCES linggan_material_media_origin(observation_ref),
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending','leased','retry_wait','completed','terminal')),
    attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count BETWEEN 0 AND 3),
    claim_generation integer NOT NULL DEFAULT 0 CHECK (claim_generation >= 0),
    claimed_by_installation_ref uuid REFERENCES plugin_installation(installation_ref),
    lease_expires_at timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_error text,
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((state = 'leased' AND claimed_by_installation_ref IS NOT NULL AND lease_expires_at IS NOT NULL)
        OR (state <> 'leased' AND claimed_by_installation_ref IS NULL AND lease_expires_at IS NULL)),
    CHECK ((state = 'completed') = (completed_at IS NOT NULL)),
    CHECK (state <> 'terminal' OR attempt_count = 3)
);

CREATE INDEX linggan_media_acquisition_due_idx
    ON linggan_media_acquisition_work(next_attempt_at, created_at)
    WHERE state IN ('pending','retry_wait');

CREATE INDEX linggan_media_acquisition_lease_idx
    ON linggan_media_acquisition_work(lease_expires_at)
    WHERE state = 'leased';

COMMENT ON TABLE linggan_media_acquisition_work IS
    'Mutable, bounded delivery control for already-authorized media observations; never Evidence.';
COMMENT ON COLUMN linggan_media_acquisition_work.attempt_count IS
    'Incremented when an installation receives a lease; three unsuccessful generations are terminal.';
