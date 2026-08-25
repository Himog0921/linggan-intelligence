-- LOCAL-TRUSTED-001 / loopback browser producer control and receipt facts.
--
-- These rows describe a local plugin task/attempt/delivery relationship. The accepted discovery
-- package remains in LOCAL-001's immutable discovery tables; this migration does not create
-- detail Evidence, media, Observation, Topic, Claim, or a scheduler.

CREATE TABLE local_trusted_task (
    task_id uuid PRIMARY KEY,
    task_spec_hash text NOT NULL UNIQUE CHECK (task_spec_hash ~ '^[0-9a-f]{64}$'),
    task_spec jsonb NOT NULL,
    source text NOT NULL CHECK (source = 'manual'),
    platform text NOT NULL CHECK (platform = 'xhs'),
    page_type text NOT NULL CHECK (page_type = 'search_results'),
    maximum_quota integer NOT NULL CHECK (maximum_quota = 20),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE local_trusted_attempt (
    attempt_id uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES local_trusted_task(task_id),
    producer_instance_id uuid NOT NULL,
    execution_state text NOT NULL CHECK (execution_state = 'started'),
    started_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (attempt_id, task_id)
);

CREATE TABLE local_trusted_submission (
    submission_id uuid PRIMARY KEY,
    task_id uuid NOT NULL,
    attempt_id uuid NOT NULL,
    producer_instance_id uuid NOT NULL,
    package_hash text NOT NULL CHECK (package_hash ~ '^[0-9a-f]{64}$'),
    receipt_ref uuid NOT NULL UNIQUE,
    discovery_package_ref uuid NOT NULL,
    discovery_receipt_ref uuid NOT NULL,
    discovery_admission text NOT NULL CHECK (discovery_admission IN ('accepted', 'replay')),
    delivery_state text NOT NULL CHECK (delivery_state IN ('acknowledged')),
    received_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (attempt_id, task_id) REFERENCES local_trusted_attempt(attempt_id, task_id)
);

CREATE OR REPLACE FUNCTION local_003_forbid_trusted_producer_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN RAISE EXCEPTION 'LOCAL-TRUSTED producer facts are append-only'; END $$;

CREATE TRIGGER local_trusted_task_is_append_only
    BEFORE UPDATE OR DELETE ON local_trusted_task
    FOR EACH ROW EXECUTE FUNCTION local_003_forbid_trusted_producer_mutation();

CREATE TRIGGER local_trusted_attempt_is_append_only
    BEFORE UPDATE OR DELETE ON local_trusted_attempt
    FOR EACH ROW EXECUTE FUNCTION local_003_forbid_trusted_producer_mutation();

CREATE TRIGGER local_trusted_submission_is_append_only
    BEFORE UPDATE OR DELETE ON local_trusted_submission
    FOR EACH ROW EXECUTE FUNCTION local_003_forbid_trusted_producer_mutation();
