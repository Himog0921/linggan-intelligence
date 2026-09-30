-- A creator's patrol schedule can explicitly request bounded detail follow-up.
-- Existing rules remain discovery-only until a person saves an opted-in revision.
ALTER TABLE collection_monitor_rule_revision
    ADD COLUMN creator_follow_details boolean NOT NULL DEFAULT false;

ALTER TABLE collection_monitor_rule_revision
    ADD CONSTRAINT collection_monitor_rule_creator_follow_surface_check
    CHECK (NOT creator_follow_details OR surface_key='creator_profile');
