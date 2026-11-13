-- T1.2/D-038: protocol version is a dimension of one problem-metric table.
-- NULL denotes an unknown protocol; it is not version 0 or a known protocol.
CREATE TABLE linggan_opportunity_digest_verifier (
 digest_version text PRIMARY KEY,
 key_verifier text NOT NULL CHECK (key_verifier ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE linggan_opportunity_problem_metric_period (
 problem_ref uuid NOT NULL REFERENCES linggan_opportunity_problem(problem_ref),
 period_start date NOT NULL,
 taxonomy_version bigint NOT NULL CHECK (taxonomy_version > 0),
 metric_def_version integer NOT NULL REFERENCES linggan_opportunity_metric_definition(version),
 protocol_version integer CHECK (protocol_version > 0),
 k integer NOT NULL CHECK (k >= 0),
 n integer NOT NULL CHECK (n >= 0),
 ci_low double precision,
 ci_high double precision,
 empty_reason text CHECK (empty_reason IS NULL OR empty_reason = 'no_eligible_comments'),
 baseline_k integer NOT NULL CHECK (baseline_k >= 0),
 baseline_n integer NOT NULL CHECK (baseline_n >= 0),
 no_baseline boolean NOT NULL,
 mentioners_total integer NOT NULL CHECK (mentioners_total >= 0),
 evidence_tier text NOT NULL CHECK (evidence_tier IN ('candidate','established','significant')),
 input_fingerprint text NOT NULL CHECK (input_fingerprint ~ '^[0-9a-f]{64}$'),
 computed_as_of timestamptz NOT NULL,
 computed_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE NULLS NOT DISTINCT (problem_ref, period_start, taxonomy_version, metric_def_version, protocol_version),
 CHECK (k <= n AND baseline_k <= baseline_n),
 CHECK ((n = 0 AND ci_low IS NULL AND ci_high IS NULL)
     OR (n > 0 AND ci_low IS NOT NULL AND ci_high IS NOT NULL)),
 CHECK ((n = 0) = (empty_reason IS NOT NULL)),
 CHECK (ci_low IS NULL OR (0 <= ci_low AND ci_low <= ci_high AND ci_high <= 1)),
 CHECK (NOT no_baseline OR evidence_tier <> 'significant')
);

-- D-028: the comment-published shadow is a distinct immutable definition. This does not move
-- the current definition pointer or authorize changing the user-visible production basis.
CREATE TABLE linggan_opportunity_metric_shadow (
 current_version integer PRIMARY KEY REFERENCES linggan_opportunity_metric_definition(version),
 shadow_version integer NOT NULL UNIQUE REFERENCES linggan_opportunity_metric_definition(version),
 CHECK (current_version <> shadow_version)
);

CREATE FUNCTION linggan_opportunity_ensure_metric_shadow(p_current_version integer)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE source_params jsonb;
DECLARE next_version integer;
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended('opportunity-metric-shadow-version', 0));
 IF EXISTS (SELECT 1 FROM linggan_opportunity_metric_shadow WHERE current_version=p_current_version) THEN
  RETURN;
 END IF;
 SELECT params INTO STRICT source_params FROM linggan_opportunity_metric_definition
 WHERE version=p_current_version;
 IF source_params->>'period_basis' <> 'work_published_cohort' THEN
  RETURN;
 END IF;
 SELECT max(version) + 1 INTO next_version FROM linggan_opportunity_metric_definition;
 INSERT INTO linggan_opportunity_metric_definition(version, params, created_by, note)
 VALUES (next_version, jsonb_set(source_params, '{period_basis}', '"comment_published"'::jsonb),
         'opportunity-card-seed', 'shadow; current pointer remains unchanged');
 INSERT INTO linggan_opportunity_metric_shadow(current_version, shadow_version)
 VALUES (p_current_version, next_version);
END $$;

CREATE FUNCTION linggan_opportunity_record_current_metric_shadow()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM linggan_opportunity_ensure_metric_shadow(NEW.version);
 RETURN NEW;
END $$;

CREATE TRIGGER linggan_opportunity_record_current_metric_shadow
BEFORE INSERT OR UPDATE OF version ON linggan_opportunity_metric_definition_current
FOR EACH ROW EXECUTE FUNCTION linggan_opportunity_record_current_metric_shadow();

SELECT linggan_opportunity_ensure_metric_shadow(version)
FROM linggan_opportunity_metric_definition_current WHERE singleton;
