CREATE TABLE linggan_opportunity_metric_definition (
 version int PRIMARY KEY CHECK (version > 0),
 params jsonb NOT NULL CHECK (jsonb_typeof(params) = 'object'),
 created_at timestamptz NOT NULL DEFAULT now(),
 created_by text NOT NULL CHECK (btrim(created_by) <> ''),
 note text
);

CREATE TABLE linggan_opportunity_metric_definition_current (
 singleton boolean PRIMARY KEY DEFAULT TRUE CHECK (singleton),
 version int NOT NULL REFERENCES linggan_opportunity_metric_definition(version),
 switched_at timestamptz NOT NULL
);

-- Production v1. Every threshold has exactly one source here (R3); later changes add a version.
INSERT INTO linggan_opportunity_metric_definition(version, params, created_by, note)
VALUES (1, $params${
  "z": 1.959963984540054,
  "period": "week",
  "period_basis": "work_published_cohort",
  "period_tz": "Asia/Shanghai",
  "week_start": "monday",
  "baseline_periods": 4,
  "series_periods": 8,
  "min_k_significant": 3,
  "min_n_period": 500,
  "min_total_for_significant": 5,
  "creator_window_days": 90,
  "creator_min_n": 50,
  "supply_min_coverage": 0.6,
  "supply_max_answers_for_gap": 2,
  "supply_engagement_ref": "domain_median_likes_90d",
  "supply_max_unjudged_ratio": 0.2,
  "gate_min_labeled": 250,
  "gate_min_kappa": 0.70,
  "gate_min_precision": 0.85,
  "gate_min_recall": 0.80,
  "gate_classes": ["answers", "mentions"],
  "collection_volume_shift_alert": 0.30,
  "max_unanchored_share": 0.20,
  "max_author_unknown_share": 0.20,
  "collection_target_per_period": 700,
  "comment_time_switch_coverage": 0.90,
  "card_rebuild_debounce_seconds": 300
}$params$::jsonb, 'opportunity-card-seed', 'T0.3 production v1; values match the fixed test parameter set');

INSERT INTO linggan_opportunity_metric_definition_current(singleton, version, switched_at)
VALUES (TRUE, 1, now());

-- PostgreSQL's week starts on Monday, and the caller supplies the time zone from params.
CREATE FUNCTION linggan_opportunity_period_start(ts timestamptz, tz text)
RETURNS date LANGUAGE sql STABLE STRICT AS $$
 SELECT date_trunc('week', ts AT TIME ZONE tz)::date
$$;

CREATE FUNCTION linggan_opportunity_period_bounds(p date, tz text)
RETURNS TABLE(start_utc timestamptz, end_utc timestamptz)
LANGUAGE sql STABLE STRICT AS $$
 SELECT (p::timestamp AT TIME ZONE tz), ((p + 7)::timestamp AT TIME ZONE tz)
$$;
