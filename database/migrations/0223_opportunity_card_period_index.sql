-- Keep the historical period lookup as a snapshot of the existing card, not new metrics.
CREATE TABLE linggan_opportunity_card_period_index (
 problem_ref uuid PRIMARY KEY REFERENCES linggan_opportunity_opportunity_card(problem_ref) ON DELETE CASCADE,
 taxonomy_version bigint NOT NULL,
 metric_def_version integer NOT NULL,
 card_fingerprint text NOT NULL,
 period_index jsonb NOT NULL CHECK (jsonb_typeof(period_index) = 'array')
);

-- Existing cards stay readable; their period lookup is unknown until a normal rebuild.
INSERT INTO linggan_opportunity_card_change(problem_ref, changed_at)
SELECT problem_ref, clock_timestamp() FROM linggan_opportunity_opportunity_card WHERE true
ON CONFLICT(problem_ref) DO UPDATE
 SET changed_at=EXCLUDED.changed_at,
     revision=linggan_opportunity_card_change.revision+1;
