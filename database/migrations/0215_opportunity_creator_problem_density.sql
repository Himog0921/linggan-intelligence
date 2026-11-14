-- D-042: creator density is one derived metric with protocol as a dimension.
-- NULL means the protocol is unknown; it is distinct from every known version.
CREATE TABLE linggan_opportunity_creator_problem_density (
 creator_id text NOT NULL,
 problem_ref uuid NOT NULL REFERENCES linggan_opportunity_problem(problem_ref),
 window_days integer NOT NULL CHECK (window_days > 0),
 taxonomy_version bigint NOT NULL CHECK (taxonomy_version > 0),
 metric_def_version integer NOT NULL REFERENCES linggan_opportunity_metric_definition(version),
 protocol_version integer CHECK (protocol_version > 0),
 k integer NOT NULL CHECK (k >= 0),
 n integer NOT NULL CHECK (n >= 0),
 ci_low double precision,
 ci_high double precision,
 sufficient boolean NOT NULL,
 input_fingerprint text NOT NULL CHECK (input_fingerprint ~ '^[0-9a-f]{64}$'),
 computed_as_of timestamptz NOT NULL,
 computed_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE NULLS NOT DISTINCT (creator_id, problem_ref, window_days, taxonomy_version, metric_def_version, protocol_version),
 CHECK (k <= n),
 CHECK ((n = 0 AND ci_low IS NULL AND ci_high IS NULL)
     OR (n > 0 AND ci_low IS NOT NULL AND ci_high IS NOT NULL)),
 CHECK (ci_low IS NULL OR (0 <= ci_low AND ci_low <= ci_high AND ci_high <= 1))
);

CREATE INDEX linggan_opportunity_creator_problem_density_problem_idx
ON linggan_opportunity_creator_problem_density (problem_ref, taxonomy_version, metric_def_version);
