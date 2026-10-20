-- Imported packages are immutable data. They grant neither tools nor execution permission.
CREATE FUNCTION linggan_utf16_length(value text) RETURNS bigint LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT COALESCE(sum(CASE WHEN ascii(ch)>65535 THEN 2 ELSE 1 END),0)
 FROM regexp_split_to_table(value,'')ch
$$;
CREATE TABLE linggan_skill (
 skill_ref uuid PRIMARY KEY,
 name text NOT NULL UNIQUE CHECK(length(name)<=64 AND name ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(skill_ref,name)
);
CREATE TABLE linggan_skill_version (
 version_ref uuid PRIMARY KEY,
 skill_ref uuid NOT NULL,
 directory_name text NOT NULL,
 version_no integer NOT NULL CHECK(version_no>0),
 description text NOT NULL CHECK(length(btrim(description))>0 AND linggan_utf16_length(description)<=1024),
 body text NOT NULL CHECK(length(btrim(body))>0 AND octet_length(body)<=262144),
 frontmatter text NOT NULL CHECK(octet_length(frontmatter)<=262144),
 disable_model_invocation boolean NOT NULL,
 invocation text NOT NULL CHECK(octet_length(invocation)<=2097152),
 loader_version text NOT NULL CHECK(loader_version='pi-agent-core/0.85.1'),
 normalization_version text NOT NULL CHECK(normalization_version='ai-skill-package.v1'),
 package_sha256 text NOT NULL CHECK(package_sha256 ~ '^[0-9a-f]{64}$'),
 source_sha256 text NOT NULL CHECK(source_sha256 ~ '^[0-9a-f]{64}$'),
 ingest_transaction xid8 NOT NULL DEFAULT pg_current_xact_id(),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(skill_ref,directory_name) REFERENCES linggan_skill(skill_ref,name),
 UNIQUE(skill_ref,version_no),
 UNIQUE(skill_ref,package_sha256)
);
CREATE TABLE linggan_skill_file (
 version_ref uuid NOT NULL REFERENCES linggan_skill_version,
 path text NOT NULL CHECK(octet_length(path) BETWEEN 1 AND 1024 AND normalize(path,NFC)=path
  AND path !~ '(^/|/$|\\|:|(^|/)\.\.?(/|$)|//|[[:cntrl:]]|(^|/)[sS][cC][rR][iI][pP][tT][sS](/|$))'),
 bytes bytea NOT NULL CHECK(octet_length(bytes)<=262144),
 text_content text CHECK(text_content IS NULL OR (normalize(text_content,NFC)=text_content
  AND position(chr(13) IN text_content)=0
  AND translate(text_content,chr(9)||chr(10),'') !~ '[[:cntrl:]]'
  AND convert_to(text_content,'UTF8')=bytes)),
 sha256 text NOT NULL CHECK(sha256 ~ '^[0-9a-f]{64}$' AND sha256=encode(sha256(bytes),'hex')),
 PRIMARY KEY(version_ref,path),
 CHECK(path<>'SKILL.md' OR text_content IS NOT NULL)
);
CREATE TABLE linggan_skill_import (
 command_ref uuid PRIMARY KEY REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 version_ref uuid NOT NULL REFERENCES linggan_skill_version,
 source_sha256 text NOT NULL CHECK(source_sha256 ~ '^[0-9a-f]{64}$'),
 source_manifest jsonb NOT NULL CHECK(jsonb_typeof(source_manifest)='array' AND jsonb_array_length(source_manifest) BETWEEN 1 AND 64),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE FUNCTION linggan_guard_skill_file_insert() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM linggan_skill_version WHERE version_ref=NEW.version_ref
  AND ingest_transaction=pg_current_xact_id()) THEN RAISE EXCEPTION 'skill_version_sealed'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER skill_file_sealed BEFORE INSERT ON linggan_skill_file
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_skill_file_insert();
CREATE FUNCTION linggan_guard_skill_version_files() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE file_count bigint; total_bytes bigint;
BEGIN
 SELECT count(*),sum(octet_length(bytes)) INTO file_count,total_bytes FROM linggan_skill_file WHERE version_ref=NEW.version_ref;
 IF file_count NOT BETWEEN 1 AND 64 OR total_bytes>1048576 OR NOT EXISTS(
  SELECT 1 FROM linggan_skill_file WHERE version_ref=NEW.version_ref AND path='SKILL.md' AND text_content IS NOT NULL
 ) THEN RAISE EXCEPTION 'skill_package_incomplete'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER skill_version_files_complete AFTER INSERT ON linggan_skill_version
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_guard_skill_version_files();
CREATE TRIGGER skill_identity_immutable BEFORE UPDATE OR DELETE ON linggan_skill FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER skill_version_immutable BEFORE UPDATE OR DELETE ON linggan_skill_version FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER skill_file_immutable BEFORE UPDATE OR DELETE ON linggan_skill_file FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER skill_import_immutable BEFORE UPDATE OR DELETE ON linggan_skill_import FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
