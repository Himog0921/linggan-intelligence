-- AI-GEN1 / PR-1A. Independent completion facts distinguish an empty array from no projection.
CREATE TABLE linggan_material_hashtag_projection (
 material_ref uuid NOT NULL REFERENCES linggan_material_content_detail(material_ref),
 projection_contract text NOT NULL CHECK(projection_contract='content-hashtags.v1'),
 state text NOT NULL CHECK(state IN ('provided','not_provided','invalid')),
 tag_count integer NOT NULL CHECK(tag_count>=0),
 source_hash text NOT NULL CHECK(source_hash ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(material_ref,projection_contract),
 CHECK(state='provided' OR tag_count=0)
);
CREATE TABLE linggan_material_content_hashtag (
 material_ref uuid NOT NULL,
 projection_contract text NOT NULL,
 ordinal integer NOT NULL CHECK(ordinal>=0),
 tag_text text NOT NULL CHECK(length(btrim(tag_text))>0),
 PRIMARY KEY(material_ref,projection_contract,ordinal),
 FOREIGN KEY(material_ref,projection_contract)
  REFERENCES linggan_material_hashtag_projection(material_ref,projection_contract)
  DEFERRABLE INITIALLY DEFERRED
);
CREATE FUNCTION linggan_validate_hashtag_projection() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected integer; actual integer; max_ordinal integer;
BEGIN
 SELECT tag_count INTO expected FROM linggan_material_hashtag_projection
 WHERE material_ref=NEW.material_ref AND projection_contract=NEW.projection_contract;
 IF NOT FOUND THEN RAISE EXCEPTION 'hashtag_projection_missing'; END IF;
 SELECT count(*),max(ordinal) INTO actual,max_ordinal FROM linggan_material_content_hashtag
 WHERE material_ref=NEW.material_ref AND projection_contract=NEW.projection_contract;
 IF actual<>expected OR (actual>0 AND max_ordinal<>actual-1) THEN
  RAISE EXCEPTION 'hashtag_projection_incomplete';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER hashtag_projection_complete AFTER INSERT ON linggan_material_hashtag_projection
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_hashtag_projection();
CREATE CONSTRAINT TRIGGER hashtag_rows_complete AFTER INSERT ON linggan_material_content_hashtag
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_hashtag_projection();
CREATE TRIGGER hashtag_projection_immutable BEFORE UPDATE OR DELETE ON linggan_material_hashtag_projection
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER hashtag_rows_immutable BEFORE UPDATE OR DELETE ON linggan_material_content_hashtag
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
