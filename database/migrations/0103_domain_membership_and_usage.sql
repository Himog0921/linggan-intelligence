-- DOMAIN-UNIFICATION-001 · Domain membership and immutable usage snapshots.
--
-- This is an additive bridge migration. The legacy single-domain columns remain temporarily so
-- existing application versions can still start while the unified read/write paths are cut over.

ALTER TABLE observation_domain
    ADD COLUMN description text,
    ADD COLUMN research_goal text,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT scope_001_now();

-- Keep the deprecated bridge column writable until 0104 removes it. New Domain writers do not
-- express a home/external distinction, so transitional rows receive the neutral legacy value.
ALTER TABLE observation_domain ALTER COLUMN is_own_domain SET DEFAULT false;

CREATE TABLE observation_domain_target (
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    role text NOT NULL CHECK (role IN ('primary', 'reference')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (domain_ref, target_ref)
);

INSERT INTO observation_domain_target (domain_ref, target_ref, role)
SELECT target.domain_ref, target.target_ref, 'primary'
  FROM collection_observation_target target
 WHERE target.domain_ref IS NOT NULL
ON CONFLICT (domain_ref, target_ref) DO NOTHING;

CREATE INDEX observation_domain_target_target_idx
    ON observation_domain_target (target_ref, domain_ref);

ALTER TABLE collection_acquisition_request
    ADD COLUMN domain_ref uuid REFERENCES observation_domain(domain_ref),
    ADD COLUMN observation_role text;

UPDATE collection_acquisition_request request
   SET domain_ref = COALESCE(target.domain_ref, '00000000-0000-4000-8000-000000000001'::uuid),
       observation_role = 'primary'
  FROM collection_observation_target target
 WHERE target.target_ref = request.target_ref;

ALTER TABLE collection_acquisition_request
    ALTER COLUMN domain_ref SET NOT NULL,
    ALTER COLUMN observation_role SET NOT NULL,
    ADD CONSTRAINT collection_acquisition_request_role_check
        CHECK (observation_role IN ('primary', 'reference'));

CREATE INDEX collection_acquisition_request_domain_idx
    ON collection_acquisition_request (domain_ref, requested_at DESC);

CREATE TABLE collection_work_order_domain_usage (
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    request_ref uuid NOT NULL UNIQUE REFERENCES collection_acquisition_request(request_ref),
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    role text NOT NULL CHECK (role IN ('primary', 'reference')),
    basis_kind text NOT NULL CHECK (basis_kind IN ('admitted', 'merged', 'legacy_migration')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (work_order_ref, request_ref)
);

INSERT INTO collection_work_order_domain_usage
    (work_order_ref, request_ref, domain_ref, role, basis_kind)
SELECT work.work_order_ref, request.request_ref, request.domain_ref,
       request.observation_role, 'legacy_migration'
  FROM collection_work_order work
  JOIN collection_admission_decision decision ON decision.decision_ref = work.decision_ref
  JOIN collection_acquisition_request request ON request.request_ref = decision.request_ref
ON CONFLICT (work_order_ref, request_ref) DO NOTHING;

CREATE INDEX collection_work_order_domain_usage_domain_idx
    ON collection_work_order_domain_usage (domain_ref, work_order_ref);

CREATE FUNCTION collection_work_order_domain_usage_require_unclaimed_queue() RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    parent_target_ref uuid;
    parent_queue_state text;
BEGIN
    SELECT target_ref, queue_state
      INTO parent_target_ref, parent_queue_state
      FROM collection_work_order
     WHERE work_order_ref=NEW.work_order_ref;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'domain usage requires an existing Work Order'
            USING ERRCODE='23503';
    END IF;

    -- Match Request and first-claim lock order. The claim path locks Target before Work Order,
    -- so this row lock makes the first claim the serialization point even for direct SQL writes.
    PERFORM 1
      FROM collection_observation_target
     WHERE target_ref=parent_target_ref
       FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'domain usage requires an existing observation Target'
            USING ERRCODE='23503';
    END IF;

    SELECT queue_state
      INTO parent_queue_state
      FROM collection_work_order
     WHERE work_order_ref=NEW.work_order_ref;
    IF parent_queue_state <> 'queued'
       OR EXISTS (
           SELECT 1 FROM collection_work_order_lease
            WHERE work_order_ref=NEW.work_order_ref
       ) THEN
        RAISE EXCEPTION 'Work Order Domain usage is frozen after first claim'
            USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER collection_work_order_domain_usage_only_before_first_claim
    BEFORE INSERT ON collection_work_order_domain_usage
    FOR EACH ROW EXECUTE FUNCTION collection_work_order_domain_usage_require_unclaimed_queue();

CREATE TRIGGER collection_work_order_domain_usage_is_append_only
    BEFORE UPDATE OR DELETE ON collection_work_order_domain_usage
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_material_domain_usage (
    usage_ref uuid PRIMARY KEY,
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    role text NOT NULL CHECK (role IN ('primary', 'reference')),
    basis_kind text NOT NULL CHECK (basis_kind IN (
        'accepted_discovery', 'admission_reuse', 'legacy_domain_migration'
    )),
    request_ref uuid REFERENCES collection_acquisition_request(request_ref),
    work_order_ref uuid REFERENCES collection_work_order(work_order_ref),
    package_ref uuid REFERENCES linggan_runtime_capture_package(package_ref),
    record_ordinal integer,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (
        (basis_kind = 'accepted_discovery'
            AND request_ref IS NOT NULL AND work_order_ref IS NOT NULL
            AND package_ref IS NOT NULL AND record_ordinal IS NOT NULL)
        OR (basis_kind = 'admission_reuse'
            AND request_ref IS NOT NULL AND work_order_ref IS NULL
            AND package_ref IS NOT NULL AND record_ordinal IS NULL)
        OR (basis_kind = 'legacy_domain_migration'
            AND request_ref IS NULL AND work_order_ref IS NULL
            AND package_ref IS NOT NULL AND record_ordinal IS NULL)
    )
);

CREATE UNIQUE INDEX linggan_material_domain_usage_discovery_uq
    ON linggan_material_domain_usage (content_public_ref, domain_ref, package_ref, record_ordinal, request_ref)
    WHERE basis_kind = 'accepted_discovery';
CREATE UNIQUE INDEX linggan_material_domain_usage_reuse_uq
    ON linggan_material_domain_usage (content_public_ref, domain_ref, request_ref)
    WHERE basis_kind = 'admission_reuse';
CREATE UNIQUE INDEX linggan_material_domain_usage_legacy_uq
    ON linggan_material_domain_usage (content_public_ref, domain_ref)
    WHERE basis_kind = 'legacy_domain_migration';
CREATE INDEX linggan_material_domain_usage_read_idx
    ON linggan_material_domain_usage (domain_ref, content_public_ref, role);

CREATE TRIGGER linggan_material_domain_usage_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_material_domain_usage
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

INSERT INTO linggan_material_domain_usage
    (usage_ref, content_public_ref, domain_ref, role, basis_kind, package_ref)
SELECT gen_random_uuid(), content.public_ref, content.domain_ref, 'primary',
       'legacy_domain_migration', content.first_package_ref
  FROM linggan_material_content content
 WHERE content.domain_ref IS NOT NULL
ON CONFLICT (content_public_ref, domain_ref)
    WHERE basis_kind = 'legacy_domain_migration' DO NOTHING;

-- The replacement clean Comment Study bootstrap stores one active policy per Domain. This bridge
-- also upgrades an already initialized local derived layer without touching its immutable runs.
DO $domain_active_policy$
BEGIN
    IF to_regclass('linggan_comment_study_active_policy') IS NOT NULL
       AND EXISTS (
           SELECT 1 FROM information_schema.columns
            WHERE table_schema = current_schema()
              AND table_name = 'linggan_comment_study_active_policy'
              AND column_name = 'singleton'
       ) THEN
        ALTER TABLE linggan_comment_study_active_policy
            ADD COLUMN domain_ref uuid;
        UPDATE linggan_comment_study_active_policy active
           SET domain_ref = policy.domain_ref
          FROM linggan_comment_study_policy policy
         WHERE policy.policy_ref = active.policy_ref;
        ALTER TABLE linggan_comment_study_active_policy
            ALTER COLUMN domain_ref SET NOT NULL,
            ADD CONSTRAINT linggan_comment_study_active_policy_domain_fk
                FOREIGN KEY (domain_ref) REFERENCES observation_domain(domain_ref);
        ALTER TABLE linggan_comment_study_active_policy DROP CONSTRAINT linggan_comment_study_active_policy_pkey;
        ALTER TABLE linggan_comment_study_active_policy DROP COLUMN singleton;
        ALTER TABLE linggan_comment_study_active_policy
            ADD PRIMARY KEY (domain_ref);
    END IF;
END
$domain_active_policy$;

-- Freeze whether each selected Study work entered through this Domain as primary or reference.
-- Existing work rows predate explicit reference selection and therefore remain primary.
DO $domain_comment_study_work_role$
BEGIN
    IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
        ALTER TABLE linggan_comment_study_work
            ADD COLUMN IF NOT EXISTS observation_role text;
        UPDATE linggan_comment_study_work
           SET observation_role='primary'
         WHERE observation_role IS NULL;
        ALTER TABLE linggan_comment_study_work
            ALTER COLUMN observation_role SET NOT NULL;
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint
             WHERE conrelid='linggan_comment_study_work'::regclass
               AND conname='linggan_comment_study_work_observation_role_check'
        ) THEN
            ALTER TABLE linggan_comment_study_work
                ADD CONSTRAINT linggan_comment_study_work_observation_role_check
                    CHECK (observation_role IN ('primary','reference'));
        END IF;
    END IF;
END
$domain_comment_study_work_role$;
