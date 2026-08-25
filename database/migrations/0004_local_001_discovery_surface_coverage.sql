-- REAL-XHS-SEARCH-DISCOVERY-001 records bounded current-surface processing counters separately
-- from accepted card count. Existing accepted packages predate these columns, so NULL means
-- UNKNOWN and is never rewritten as a fabricated zero.

ALTER TABLE local_discovery_coverage
    ADD COLUMN discovered_cards integer CHECK (discovered_cards >= 0),
    ADD COLUMN emitted_cards integer CHECK (emitted_cards >= 0),
    ADD COLUMN failed_cards integer CHECK (failed_cards >= 0),
    ADD COLUMN not_attempted_cards integer CHECK (not_attempted_cards >= 0);
