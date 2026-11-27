-- Updates advance changed_at and revision. Keep the start of the pending window
-- through subsequent source events until the worker consumes the queue row.
ALTER TABLE linggan_opportunity_card_change ADD COLUMN queued_at timestamptz;
UPDATE linggan_opportunity_card_change SET queued_at = changed_at;
ALTER TABLE linggan_opportunity_card_change
    ALTER COLUMN queued_at SET DEFAULT statement_timestamp(),
    ALTER COLUMN queued_at SET NOT NULL;
