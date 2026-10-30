-- Structural templates store only a closed set of generic step kinds, never manuscript text.
CREATE FUNCTION linggan_creation_template_valid_steps(steps jsonb) RETURNS boolean
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
BEGIN
 IF jsonb_typeof(steps) <> 'array' THEN RETURN false; END IF;
 RETURN jsonb_array_length(steps) BETWEEN 1 AND 12 AND NOT EXISTS (
   SELECT 1 FROM jsonb_array_elements(steps) AS e(value)
   WHERE jsonb_typeof(e.value) <> 'string'
      OR e.value #>> '{}' NOT IN ('opening','scene','experience','feeling','reflection','action','ending')
 );
END $$;
CREATE SEQUENCE linggan_creation_template_code_seq;
CREATE TABLE linggan_creation_template (
 template_ref uuid PRIMARY KEY,
 code text NOT NULL UNIQUE CHECK(code ~ '^YB-[0-9]{4,}$'),
 work_ref uuid NOT NULL REFERENCES linggan_creation_work,
 revision_ref uuid NOT NULL,
 request_ref uuid NOT NULL UNIQUE,
 request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(work_ref,revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref)
);
CREATE TABLE linggan_creation_template_revision (
 template_ref uuid NOT NULL REFERENCES linggan_creation_template,
 revision integer NOT NULL CHECK(revision>0),
 steps jsonb NOT NULL CHECK(linggan_creation_template_valid_steps(steps)),
 request_ref uuid UNIQUE,
 request_hash text CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 CHECK ((request_ref IS NULL) = (request_hash IS NULL)),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(template_ref,revision)
);
CREATE TABLE linggan_creation_template_status (
 event_ref uuid PRIMARY KEY,
 template_ref uuid NOT NULL,
 revision integer NOT NULL,
 sequence integer NOT NULL CHECK(sequence>0),
 request_ref uuid NOT NULL UNIQUE,
 request_hash text NOT NULL CHECK(request_hash ~ '^[0-9a-f]{64}$'),
 status text NOT NULL CHECK(status IN ('review','ok','off')),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(template_ref,revision) REFERENCES linggan_creation_template_revision(template_ref,revision),
 UNIQUE(template_ref,revision,sequence)
);
CREATE TRIGGER creation_template_immutable BEFORE UPDATE OR DELETE ON linggan_creation_template FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER creation_template_revision_immutable BEFORE UPDATE OR DELETE ON linggan_creation_template_revision FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER creation_template_status_immutable BEFORE UPDATE OR DELETE ON linggan_creation_template_status FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
