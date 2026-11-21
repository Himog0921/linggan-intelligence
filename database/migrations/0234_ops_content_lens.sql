CREATE FUNCTION linggan_ops_lens_title_key(title text) RETURNS text
    LANGUAGE sql IMMUTABLE
    RETURN md5(regexp_replace(lower(title), '[[:space:][:punct:]]', '', 'g'));

CREATE TABLE linggan_ops_lens_definition (
    definition_ref uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('taxonomy', 'metric')),
    version integer NOT NULL CHECK (version > 0),
    body jsonb NOT NULL CHECK (jsonb_typeof(body) = 'object'),
    created_by text NOT NULL CHECK (btrim(created_by) <> ''),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    note text,
    UNIQUE (kind, version),
    UNIQUE (kind, definition_ref)
);

-- 唯一允许更新的表：当前指针。切换是人的动作。
CREATE TABLE linggan_ops_lens_definition_current (
    kind text PRIMARY KEY CHECK (kind IN ('taxonomy', 'metric')),
    definition_ref uuid NOT NULL,
    FOREIGN KEY (kind, definition_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    switched_by text NOT NULL CHECK (btrim(switched_by) <> ''),
    switched_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_ops_lens_label_run (
    run_ref uuid PRIMARY KEY,
    run_kind text NOT NULL CHECK (run_kind IN ('primary', 'audit')),
    taxonomy_ref uuid NOT NULL,
    taxonomy_kind text GENERATED ALWAYS AS ('taxonomy') STORED,
    FOREIGN KEY (taxonomy_kind, taxonomy_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    metric_ref uuid NOT NULL,
    metric_kind text GENERATED ALWAYS AS ('metric') STORED,
    FOREIGN KEY (metric_kind, metric_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    primary_run_ref uuid,
    primary_kind text GENERATED ALWAYS AS ('primary') STORED,
    task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config(task_config_ref),
    prompt_version_ref uuid NOT NULL REFERENCES linggan_ai_prompt_version(version_ref),
    input_hash text NOT NULL CHECK (input_hash ~ '^[0-9a-f]{64}$'),
    run_key text NOT NULL UNIQUE,
    foundation_batch_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_batch(batch_ref),
    frozen_input jsonb NOT NULL CHECK (jsonb_typeof(frozen_input) = 'object'),
    title_count integer NOT NULL CHECK (title_count > 0),
    sample_seed bigint,
    blind boolean NOT NULL CHECK (blind),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((run_kind = 'audit') = (primary_run_ref IS NOT NULL)),
    CHECK ((run_kind = 'audit') = (sample_seed IS NOT NULL)),
    UNIQUE (run_ref, primary_run_ref),
    UNIQUE (run_ref, run_kind, taxonomy_ref, metric_ref, task_config_ref, prompt_version_ref),
    FOREIGN KEY (primary_run_ref, primary_kind, taxonomy_ref, metric_ref, task_config_ref, prompt_version_ref)
        REFERENCES linggan_ops_lens_label_run(run_ref, run_kind, taxonomy_ref, metric_ref, task_config_ref, prompt_version_ref)
);

-- The method belongs to the specific foundation batch that processed this run.
CREATE FUNCTION linggan_ops_lens_check_run_method() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM linggan_ai_batch b JOIN linggan_ai_task_config cfg USING (task_config_ref)
        WHERE b.batch_ref = NEW.foundation_batch_ref AND cfg.task_config_ref = NEW.task_config_ref
          AND cfg.prompt_version_ref = NEW.prompt_version_ref
    ) THEN
        RAISE EXCEPTION 'ops_lens_batch_method_mismatch';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER ops_lens_run_method BEFORE INSERT ON linggan_ops_lens_label_run
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_lens_check_run_method();

CREATE TABLE linggan_ops_lens_label (
    run_ref uuid NOT NULL REFERENCES linggan_ops_lens_label_run(run_ref),
    title_key text NOT NULL CHECK (title_key ~ '^[0-9a-f]{32}$'),
    situation text NOT NULL,
    moment text NOT NULL,
    offer text NOT NULL,
    audience text NOT NULL,
    confidence text NOT NULL CHECK (confidence IN ('high', 'medium', 'low')),
    invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation(invocation_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (run_ref, title_key)
);

CREATE TABLE linggan_ops_lens_agreement (
    primary_run_ref uuid NOT NULL REFERENCES linggan_ops_lens_label_run(run_ref),
    audit_run_ref uuid NOT NULL,
    FOREIGN KEY (audit_run_ref, primary_run_ref) REFERENCES linggan_ops_lens_label_run(run_ref, primary_run_ref),
    dimension text NOT NULL CHECK (dimension IN ('situation', 'moment', 'offer', 'audience')),
    sample_count integer NOT NULL CHECK (sample_count >= 0),
    excluded_example_count integer NOT NULL CHECK (excluded_example_count >= 0),
    agreement numeric CHECK (agreement BETWEEN 0 AND 1),
    kappa numeric CHECK (kappa BETWEEN -1 AND 1),
    reason text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (primary_run_ref, audit_run_ref, dimension)
);

CREATE TABLE linggan_ops_lens_adjudication (
    adjudication_ref uuid PRIMARY KEY,
    title_key text NOT NULL CHECK (title_key ~ '^[0-9a-f]{32}$'),
    taxonomy_ref uuid NOT NULL,
    taxonomy_kind text GENERATED ALWAYS AS ('taxonomy') STORED,
    FOREIGN KEY (taxonomy_kind, taxonomy_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    dimension text NOT NULL CHECK (dimension IN ('situation', 'moment', 'offer', 'audience')),
    value text NOT NULL,
    member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    reason text,
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TABLE linggan_ops_lens_snapshot (
    snapshot_ref uuid PRIMARY KEY,
    as_of timestamptz NOT NULL,
    taxonomy_ref uuid NOT NULL,
    taxonomy_kind text GENERATED ALWAYS AS ('taxonomy') STORED,
    FOREIGN KEY (taxonomy_kind, taxonomy_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    metric_ref uuid NOT NULL,
    metric_kind text GENERATED ALWAYS AS ('metric') STORED,
    FOREIGN KEY (metric_kind, metric_ref) REFERENCES linggan_ops_lens_definition(kind, definition_ref),
    input_hash text NOT NULL CHECK (input_hash ~ '^[0-9a-f]{64}$'),
    analysis_input jsonb NOT NULL CHECK (jsonb_typeof(analysis_input) = 'object'),
    body jsonb NOT NULL CHECK (jsonb_typeof(body) = 'object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (input_hash, metric_ref)
);

-- Dimensions are selected independently because adjudicating one leaves the other three intact.
CREATE VIEW linggan_ops_lens_effective_label AS
WITH completed AS (
    SELECT r.*, b.finished_at
    FROM linggan_ops_lens_label_run r
    JOIN linggan_ai_batch_control b ON b.batch_ref = r.foundation_batch_ref AND b.state = 'completed'
    WHERE r.title_count = (SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref = r.run_ref)
), eligible AS (
    SELECT p.* FROM completed p WHERE p.run_kind = 'primary' AND EXISTS (
        SELECT 1 FROM completed audit
        JOIN linggan_ops_lens_agreement a ON a.audit_run_ref = audit.run_ref
        WHERE audit.primary_run_ref = p.run_ref
        GROUP BY audit.run_ref HAVING count(*) = 4
    )
), latest AS (
    SELECT DISTINCT ON (r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.title_key)
           r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.*
    FROM eligible r JOIN linggan_ops_lens_label l USING (run_ref)
    ORDER BY r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.title_key,
             r.finished_at DESC, r.run_ref DESC
)
SELECT l.taxonomy_ref, l.task_config_ref, l.prompt_version_ref, l.title_key, field.dimension,
       coalesce(h.value, field.value) AS value, l.run_ref, h.adjudication_ref
FROM latest l
CROSS JOIN LATERAL (VALUES ('moment',l.moment), ('offer',l.offer),
                           ('situation',l.situation), ('audience',l.audience)) field(dimension,value)
LEFT JOIN LATERAL (
    SELECT a.value, a.adjudication_ref FROM linggan_ops_lens_adjudication a
    WHERE a.taxonomy_ref = l.taxonomy_ref AND a.title_key = l.title_key AND a.dimension = field.dimension
    ORDER BY a.created_at DESC, a.adjudication_ref DESC LIMIT 1
) h ON true;

CREATE INDEX ops_lens_adjudication_lookup ON linggan_ops_lens_adjudication
    (taxonomy_ref, title_key, dimension, created_at DESC, adjudication_ref DESC);

CREATE TRIGGER ops_lens_definition_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_definition
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER ops_lens_label_run_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_label_run
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER ops_lens_label_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_label
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER ops_lens_agreement_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_agreement
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER ops_lens_adjudication_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_adjudication
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER ops_lens_snapshot_immutable BEFORE UPDATE OR DELETE ON linggan_ops_lens_snapshot
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

WITH seed AS (
    INSERT INTO linggan_ops_lens_definition (definition_ref,kind,version,body,created_by)
    VALUES (gen_random_uuid(),'metric',1,'{
        "views_threshold_high":5000, "views_threshold_mid":1000, "comments_threshold_active":10,
        "min_n_rate":20, "min_account_posts":5, "wilson_z":1.96,
        "permutation_count":2000, "permutation_seed":42, "label_batch_size":50,
        "audit_sample_size":250, "audit_sample_seed":43, "min_kappa":0.75,
        "min_agreement":0.85, "stale_after_days":14
    }','migration:0234') RETURNING definition_ref
)
INSERT INTO linggan_ops_lens_definition_current (kind,definition_ref,switched_by)
SELECT 'metric',definition_ref,'migration:0234' FROM seed;
