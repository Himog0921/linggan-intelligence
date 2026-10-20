-- No source text or serving/AI execution state. A frozen observation and content identity differ.
CREATE TABLE linggan_note_material_snapshot (
 snapshot_ref uuid PRIMARY KEY,
 domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
 content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 snapshot_identity text NOT NULL CHECK(snapshot_identity ~ '^[0-9a-f]{64}$'),
 material_fingerprint text NOT NULL CHECK(material_fingerprint ~ '^[0-9a-f]{64}$'),
 provenance_hash text NOT NULL CHECK(provenance_hash ~ '^[0-9a-f]{64}$'),
 input_contract text NOT NULL CHECK(input_contract='note-material-input.v1'),
 fingerprint_contract text NOT NULL CHECK(fingerprint_contract='note-material-fingerprint.v1'),
 dossier_contract text NOT NULL,
 layer_version text NOT NULL,
 as_of timestamptz NOT NULL,
 ready boolean NOT NULL,
 not_ready_reasons jsonb NOT NULL CHECK(jsonb_typeof(not_ready_reasons)='array'),
 columns jsonb NOT NULL CHECK(jsonb_typeof(columns)='array'),
 fragment_count integer NOT NULL CHECK(fragment_count>=0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(domain_ref,content_public_ref,snapshot_identity),
 CHECK(as_of<=created_at),
 CHECK(ready=(jsonb_array_length(not_ready_reasons)=0))
);
CREATE TABLE linggan_note_material_fragment (
 snapshot_ref uuid NOT NULL REFERENCES linggan_note_material_snapshot(snapshot_ref),
 ordinal integer NOT NULL CHECK(ordinal>=0),
 citation_id text NOT NULL CHECK(citation_id ~ '^S[1-9][0-9]*$'),
 field text NOT NULL CHECK(field IN ('title','cover_headline','body','tags','ocr','transcript')),
 source_ref uuid NOT NULL,
 source_version text NOT NULL,
 logical_position text NOT NULL,
 text_hash text NOT NULL CHECK(text_hash ~ '^[0-9a-f]{64}$'),
 char_start integer NOT NULL CHECK(char_start>=0),
 char_end integer NOT NULL,
 PRIMARY KEY(snapshot_ref,ordinal),
 UNIQUE(snapshot_ref,citation_id),
 CHECK(char_end>char_start)
);
CREATE INDEX note_material_fragment_source ON linggan_note_material_fragment(source_ref);
CREATE FUNCTION linggan_validate_note_material_fragments() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected integer; actual integer; max_ordinal integer; ref uuid;
BEGIN
 ref:=NEW.snapshot_ref;
 SELECT fragment_count INTO expected FROM linggan_note_material_snapshot WHERE snapshot_ref=ref;
 SELECT count(*),max(ordinal) INTO actual,max_ordinal FROM linggan_note_material_fragment WHERE snapshot_ref=ref;
 IF actual<>expected OR (actual>0 AND max_ordinal<>actual-1) THEN
  RAISE EXCEPTION 'note_material_fragments_incomplete';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER note_snapshot_complete AFTER INSERT ON linggan_note_material_snapshot
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_note_material_fragments();
CREATE CONSTRAINT TRIGGER note_fragment_complete AFTER INSERT ON linggan_note_material_fragment
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_note_material_fragments();
CREATE TRIGGER note_snapshot_immutable BEFORE UPDATE OR DELETE ON linggan_note_material_snapshot
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER note_fragment_immutable BEFORE UPDATE OR DELETE ON linggan_note_material_fragment
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
