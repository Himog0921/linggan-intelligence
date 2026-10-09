-- Temporary search scopes reuse Collection targets without becoming monitoring decisions.
ALTER TABLE collection_observation_target ADD COLUMN purpose_kind text NOT NULL DEFAULT 'long_term' CHECK(purpose_kind IN('long_term','research_round'));
ALTER TABLE collection_observation_target ADD COLUMN purpose_round_ref uuid REFERENCES linggan_topic_map_collection_round;
ALTER TABLE collection_observation_target ADD CONSTRAINT topic_map_temporary_target_round CHECK((purpose_kind='research_round')=(purpose_round_ref IS NOT NULL));
CREATE TABLE linggan_topic_map_search_target (
 round_ref uuid NOT NULL REFERENCES linggan_topic_map_collection_round,target_ref uuid NOT NULL UNIQUE REFERENCES collection_observation_target,
 keyword text NOT NULL,ordinal integer NOT NULL CHECK(ordinal BETWEEN 1 AND 3),candidate_quota integer NOT NULL CHECK(candidate_quota BETWEEN 1 AND 200),
 authorization_ref uuid NOT NULL REFERENCES collection_acquisition_authorization,collection_request_ref uuid,work_order_ref uuid REFERENCES collection_work_order,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),PRIMARY KEY(round_ref,ordinal)
);
CREATE TABLE linggan_topic_map_search_freeze (
 round_ref uuid PRIMARY KEY REFERENCES linggan_topic_map_collection_round,request_ref uuid NOT NULL UNIQUE,
 work_refs uuid[] NOT NULL CHECK(cardinality(work_refs) BETWEEN 1 AND 10),created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE FUNCTION topic_map_search_target_bound() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM 1 FROM linggan_topic_map_collection_round WHERE round_ref=NEW.round_ref FOR UPDATE;
 IF (SELECT COALESCE(sum(candidate_quota),0) FROM linggan_topic_map_search_target WHERE round_ref=NEW.round_ref)+NEW.candidate_quota>200 THEN RAISE EXCEPTION 'topic map candidate quota exceeds 200' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER topic_map_search_target_bound BEFORE INSERT ON linggan_topic_map_search_target FOR EACH ROW EXECUTE FUNCTION topic_map_search_target_bound();
CREATE FUNCTION topic_map_temporary_target_no_monitor() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.purpose_kind='research_round' AND (NEW.monitoring_enabled OR NEW.lifecycle_state='monitoring') THEN RAISE EXCEPTION 'temporary research targets cannot be monitored' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER topic_map_temporary_target_no_monitor BEFORE INSERT OR UPDATE ON collection_observation_target FOR EACH ROW EXECUTE FUNCTION topic_map_temporary_target_no_monitor();
CREATE FUNCTION topic_map_temporary_target_no_rule() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM collection_observation_target WHERE target_ref=NEW.target_ref AND purpose_kind='research_round') THEN RAISE EXCEPTION 'temporary research targets cannot have monitoring rules' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER topic_map_temporary_target_no_rule BEFORE INSERT OR UPDATE ON collection_monitor_rule_revision FOR EACH ROW EXECUTE FUNCTION topic_map_temporary_target_no_rule();
CREATE TRIGGER topic_map_search_freeze_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_search_freeze FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE FUNCTION topic_map_collection_slot_bound() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE maximum integer;
BEGIN
 SELECT detail_limit INTO maximum FROM linggan_topic_map_collection_round WHERE round_ref=NEW.round_ref FOR UPDATE;
 IF (SELECT count(*) FROM linggan_topic_map_collection_slot WHERE round_ref=NEW.round_ref)>=maximum THEN RAISE EXCEPTION 'topic map detail slots exhausted' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER topic_map_collection_slot_bound BEFORE INSERT ON linggan_topic_map_collection_slot FOR EACH ROW EXECUTE FUNCTION topic_map_collection_slot_bound();
CREATE FUNCTION topic_map_temporary_target_identity_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.purpose_kind='research_round' AND (NEW.purpose_kind<>OLD.purpose_kind OR NEW.purpose_round_ref<>OLD.purpose_round_ref OR NEW.identity_key<>OLD.identity_key) THEN RAISE EXCEPTION 'temporary search identity is frozen' USING ERRCODE='23514';END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER topic_map_temporary_target_identity_immutable BEFORE UPDATE ON collection_observation_target FOR EACH ROW EXECUTE FUNCTION topic_map_temporary_target_identity_immutable();
