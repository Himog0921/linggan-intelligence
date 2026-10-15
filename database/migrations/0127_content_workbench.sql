-- CONTENT-WORKBENCH-001 / #425. Local authorship is separate from source facts and publication.
CREATE TABLE linggan_creation_work (
 work_ref uuid PRIMARY KEY, domain_ref uuid NOT NULL REFERENCES observation_domain,
 saved_by text NOT NULL CHECK(saved_by='local_machine'),
 current_revision_ref uuid, created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_creation_revision (
 revision_ref uuid PRIMARY KEY, work_ref uuid NOT NULL REFERENCES linggan_creation_work,
 revision_number bigint NOT NULL CHECK(revision_number>0), previous_revision_ref uuid,
 document jsonb NOT NULL CHECK(jsonb_typeof(document)='object'),
 document_sha256 text NOT NULL CHECK(document_sha256 ~ '^[0-9a-f]{64}$'),
 saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(work_ref,revision_ref), UNIQUE(work_ref,revision_number),
 UNIQUE NULLS NOT DISTINCT(work_ref,previous_revision_ref),
 FOREIGN KEY(work_ref,previous_revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref)
);
ALTER TABLE linggan_creation_work ADD FOREIGN KEY(work_ref,current_revision_ref)
 REFERENCES linggan_creation_revision(work_ref,revision_ref);
CREATE TABLE linggan_creation_request (
 request_ref uuid PRIMARY KEY, work_ref uuid NOT NULL REFERENCES linggan_creation_work,
 command_sha256 text NOT NULL CHECK(command_sha256 ~ '^[0-9a-f]{64}$'),
 receipt jsonb NOT NULL, received_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_creation_adoption (
 revision_ref uuid PRIMARY KEY, work_ref uuid NOT NULL,
 suggestion_ref uuid NOT NULL, action text NOT NULL CHECK(action IN('draft','rewrite_selection','page_copy')),
 input_revision_ref uuid NOT NULL REFERENCES linggan_creation_revision,
 model_invocation_ref uuid NOT NULL REFERENCES linggan_model_invocation(invocation_ref), method_version text NOT NULL,
 FOREIGN KEY(work_ref,revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref),
 FOREIGN KEY(work_ref,input_revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref)
);
CREATE TABLE linggan_creation_asset (
 asset_ref uuid PRIMARY KEY, work_ref uuid NOT NULL REFERENCES linggan_creation_work,
 request_ref uuid NOT NULL UNIQUE, storage_key text NOT NULL UNIQUE,
 sha256 text NOT NULL CHECK(sha256 ~ '^[0-9a-f]{64}$'),
 byte_size bigint NOT NULL CHECK(byte_size BETWEEN 1 AND 20971520),
 mime_type text NOT NULL CHECK(mime_type IN('image/png','image/jpeg','image/webp')),
 width integer NOT NULL CHECK(width BETWEEN 1 AND 12000), height integer NOT NULL CHECK(height BETWEEN 1 AND 12000),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(), UNIQUE(work_ref,asset_ref)
);
CREATE TABLE linggan_creation_export (
 export_ref uuid PRIMARY KEY, work_ref uuid NOT NULL REFERENCES linggan_creation_work,
 revision_ref uuid NOT NULL, request_ref uuid NOT NULL UNIQUE,
 command_sha256 text NOT NULL CHECK(command_sha256 ~ '^[0-9a-f]{64}$'),
 renderer_version text NOT NULL, font_version text NOT NULL,
 state text NOT NULL CHECK(state IN('rendering','failed','complete')), failure_code text,
 package_storage_key text, package_sha256 text, package_byte_size bigint,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(work_ref,revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref),
 CHECK((state='complete')=(package_storage_key IS NOT NULL AND package_sha256 IS NOT NULL AND package_byte_size IS NOT NULL))
);
CREATE TABLE linggan_creation_export_page (
 export_ref uuid NOT NULL REFERENCES linggan_creation_export, page_ref uuid NOT NULL,
 ordinal integer NOT NULL CHECK(ordinal BETWEEN 1 AND 30), storage_key text,
 sha256 text CHECK(sha256 IS NULL OR sha256 ~ '^[0-9a-f]{64}$'), byte_size bigint,
 PRIMARY KEY(export_ref,page_ref), UNIQUE(export_ref,ordinal),
 CHECK((storage_key IS NULL AND sha256 IS NULL AND byte_size IS NULL) OR (storage_key IS NOT NULL AND sha256 IS NOT NULL AND byte_size BETWEEN 1 AND 20971520))
);
CREATE TABLE linggan_creation_export_request (
 request_ref uuid PRIMARY KEY, export_ref uuid NOT NULL REFERENCES linggan_creation_export,
 command_sha256 text NOT NULL CHECK(command_sha256 ~ '^[0-9a-f]{64}$'), receipt jsonb NOT NULL
);
CREATE TRIGGER linggan_creation_export_request_immutable BEFORE UPDATE OR DELETE ON linggan_creation_export_request FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_revision_immutable BEFORE UPDATE OR DELETE ON linggan_creation_revision FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_request_immutable BEFORE UPDATE OR DELETE ON linggan_creation_request FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_asset_immutable BEFORE UPDATE OR DELETE ON linggan_creation_asset FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_adoption_immutable BEFORE UPDATE OR DELETE ON linggan_creation_adoption FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE INDEX linggan_creation_work_domain ON linggan_creation_work(domain_ref,created_at DESC);
CREATE FUNCTION linggan_creation_export_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR OLD.state='complete' OR NEW.work_ref<>OLD.work_ref OR NEW.revision_ref<>OLD.revision_ref OR NEW.renderer_version<>OLD.renderer_version OR NEW.font_version<>OLD.font_version OR NEW.request_ref<>OLD.request_ref OR NEW.command_sha256<>OLD.command_sha256 THEN RAISE EXCEPTION 'creation export is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER linggan_creation_export_guard BEFORE UPDATE OR DELETE ON linggan_creation_export FOR EACH ROW EXECUTE FUNCTION linggan_creation_export_guard();
CREATE FUNCTION linggan_creation_export_page_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR OLD.sha256 IS NOT NULL OR NEW.export_ref<>OLD.export_ref OR NEW.page_ref<>OLD.page_ref OR NEW.ordinal<>OLD.ordinal OR EXISTS(SELECT 1 FROM linggan_creation_export WHERE export_ref=OLD.export_ref AND state='complete') THEN RAISE EXCEPTION 'creation export page is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER linggan_creation_export_page_guard BEFORE UPDATE OR DELETE ON linggan_creation_export_page FOR EACH ROW EXECUTE FUNCTION linggan_creation_export_page_guard();
