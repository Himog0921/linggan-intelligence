-- Period filters use the metric rows captured with the card, never later metric versions.
ALTER TABLE linggan_opportunity_opportunity_card
 ADD COLUMN period_index jsonb
 CHECK (period_index IS NULL OR jsonb_typeof(period_index) = 'array');

-- Existing cards predate the index; their periods remain unknown until a normal rebuild.
INSERT INTO linggan_opportunity_card_change(problem_ref, changed_at)
SELECT problem_ref, clock_timestamp() FROM linggan_opportunity_opportunity_card WHERE true
ON CONFLICT(problem_ref) DO UPDATE
 SET changed_at=EXCLUDED.changed_at,
     revision=linggan_opportunity_card_change.revision+1;
