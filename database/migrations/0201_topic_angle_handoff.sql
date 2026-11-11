-- Link a saved angle to the existing creation idea and its anchor; no second idea entity.
CREATE TABLE linggan_topic_angle_handoff (
 alternative_ref uuid PRIMARY KEY REFERENCES linggan_topic_map_alternative,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 idea_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_idea_box,
 anchor_work_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_work,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER topic_angle_handoff_immutable BEFORE UPDATE OR DELETE ON linggan_topic_angle_handoff
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
