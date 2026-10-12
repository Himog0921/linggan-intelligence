-- TOPIC-MAP-WORKFLOW-382: preserve source-backed human ideas before classification.
-- Existing saved records and their source/definition lineage remain unchanged.
ALTER TABLE linggan_topic_map_alternative
  ALTER COLUMN topic_ref DROP NOT NULL,
  ALTER COLUMN definition_ref DROP NOT NULL;
ALTER TABLE linggan_topic_map_alternative
  ADD CONSTRAINT topic_map_alternative_definition_pair
    CHECK ((topic_ref IS NULL) = (definition_ref IS NULL)),
  ADD CONSTRAINT topic_map_alternative_manual_without_definition
    CHECK (topic_ref IS NOT NULL OR (
      kind = 'angle' AND method_version = 'topic-map.manual.v1'
      AND research_manifest->>'resultRef' IS NULL
      AND research_manifest->>'angleIndex' IS NULL
      AND research_manifest->>'opportunityIndex' IS NULL
    ));
