-- An observation target's transition rows are a history: a target's current
-- lifecycle_state is a readable summary, and how it got there is the record.
--
-- `0005` described this as append-only, but nothing enforced it. The product does
-- delete a target's whole history when the target itself is deleted
-- (`crates/evidence/src/collection_target.rs`), so DELETE must stay allowed here.
-- Rewriting an existing row is never a product action, so only UPDATE is refused.
CREATE OR REPLACE FUNCTION collection_target_transition_forbid_rewrite() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN RAISE EXCEPTION 'observation target transitions cannot be rewritten; delete the target to discard its history'; END $$;

CREATE TRIGGER collection_observation_target_transition_is_immutable
    BEFORE UPDATE ON collection_observation_target_transition
    FOR EACH ROW EXECUTE FUNCTION collection_target_transition_forbid_rewrite();
