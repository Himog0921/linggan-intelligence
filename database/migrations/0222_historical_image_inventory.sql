-- A recovered image list is derived from the accepted detail package. It does
-- not create a new platform observation or replace a media materialization.
CREATE TABLE linggan_historical_image_inventory (
    detail_ref uuid PRIMARY KEY REFERENCES linggan_material_content_detail(material_ref),
    source_excerpt_sha256 text NOT NULL CHECK (source_excerpt_sha256 ~ '^[0-9a-f]{64}$'),
    observation_refs uuid[] NOT NULL,
    materialization_refs uuid[] NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (cardinality(observation_refs) BETWEEN 1 AND 100),
    CHECK (cardinality(observation_refs) = cardinality(materialization_refs)),
    CHECK (array_position(observation_refs, NULL) IS NULL),
    CHECK (array_position(materialization_refs, NULL) IS NULL)
);

CREATE TRIGGER historical_image_inventory_immutable
BEFORE UPDATE OR DELETE ON linggan_historical_image_inventory
FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
