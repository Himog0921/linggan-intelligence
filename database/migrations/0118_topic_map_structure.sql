-- Confirmed structural revisions retain all old definitions and classify explicit new sets.
CREATE TABLE linggan_topic_map_structure_receipt (
 request_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 preview_hash text NOT NULL CHECK(preview_hash ~ '^[0-9a-f]{64}$'),
 kind text NOT NULL CHECK(kind IN ('merge','split')),
 receipt jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_topic_map_structure_source (
 request_ref uuid NOT NULL REFERENCES linggan_topic_map_structure_receipt,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 PRIMARY KEY(request_ref,topic_ref), UNIQUE(definition_ref)
);
CREATE TABLE linggan_topic_map_structure_destination (
 request_ref uuid NOT NULL REFERENCES linggan_topic_map_structure_receipt,
 topic_ref uuid NOT NULL REFERENCES linggan_topic_workspace,
 definition_ref uuid NOT NULL REFERENCES linggan_topic_definition,
 PRIMARY KEY(request_ref,topic_ref)
);
CREATE TRIGGER topic_map_structure_receipt_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_structure_receipt FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_structure_source_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_structure_source FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER topic_map_structure_destination_immutable BEFORE UPDATE OR DELETE ON linggan_topic_map_structure_destination FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
