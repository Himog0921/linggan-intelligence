-- RESEARCH-DOSSIER-V2: one immutable reading history and one explicit current pointer.
-- The application is in development: obsolete derived snapshots/review overlays are
-- removed. Original capture packages, media bytes, OCR observations and jobs remain.
CREATE TABLE linggan_material_unit (
    unit_ref uuid PRIMARY KEY,
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    unit_key text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('body','image','audio','video_frame')),
    slot_key text REFERENCES linggan_media_slot(slot_key),
    page_ordinal integer CHECK (page_ordinal > 0),
    source_version text NOT NULL CHECK (length(source_version) > 0),
    source_state text NOT NULL CHECK (source_state IN ('present','known_empty','not_acquired','not_applicable','restricted')),
    current_reading_ref uuid,
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    adoption text NOT NULL DEFAULT 'automatic' CHECK (adoption IN ('automatic','human_adopted','human_edited')),
    verification text NOT NULL DEFAULT 'unchecked' CHECK (verification IN ('unchecked','verified')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (content_public_ref,unit_key),
    UNIQUE (unit_ref,content_public_ref),
    CHECK ((kind='body' AND slot_key IS NULL AND unit_key='body')
        OR (kind<>'body' AND slot_key IS NOT NULL AND unit_key=kind||':'||slot_key))
);
CREATE UNIQUE INDEX material_unit_slot ON linggan_material_unit(content_public_ref,kind,slot_key) WHERE slot_key IS NOT NULL;
CREATE UNIQUE INDEX material_unit_native ON linggan_material_unit(content_public_ref,kind) WHERE slot_key IS NULL;

CREATE TABLE linggan_material_reading (
    reading_ref uuid PRIMARY KEY,
    unit_ref uuid NOT NULL,
    content_public_ref uuid NOT NULL,
    source_version text NOT NULL CHECK (length(source_version)>0),
    source_state text NOT NULL CHECK (source_state IN ('present','known_empty','not_acquired','not_applicable','restricted')),
    page_ordinal integer CHECK (page_ordinal>0),
    derivative_ref uuid REFERENCES linggan_media_derivative(derivative_ref),
    producer_ref uuid,
    producer_kind text NOT NULL CHECK (producer_kind IN ('platform','ordinary_ocr','local_vision','cloud_vision','asr','frame_ocr','human_edit')),
    rule_version text NOT NULL CHECK (length(rule_version)>0),
    blocks jsonb NOT NULL CHECK (jsonb_typeof(blocks)='array'),
    issues jsonb NOT NULL CHECK (jsonb_typeof(issues)='array'),
    source_location jsonb NOT NULL CHECK (jsonb_typeof(source_location)='object'),
    based_on_reading_ref uuid,
    publication text NOT NULL CHECK (publication IN ('automatic','candidate','human_edit')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (unit_ref,content_public_ref) REFERENCES linggan_material_unit(unit_ref,content_public_ref),
    UNIQUE (unit_ref,source_version,reading_ref),
    UNIQUE (unit_ref,reading_ref),
    FOREIGN KEY (unit_ref,source_version,based_on_reading_ref)
        REFERENCES linggan_material_reading(unit_ref,source_version,reading_ref),
    CHECK ((publication='human_edit')=(based_on_reading_ref IS NOT NULL)),
    CHECK ((publication='human_edit')=(producer_kind='human_edit'))
);
CREATE UNIQUE INDEX material_reading_producer ON linggan_material_reading(unit_ref,source_version,producer_ref,rule_version)
    WHERE producer_ref IS NOT NULL AND publication<>'human_edit';
CREATE INDEX material_reading_unit_time ON linggan_material_reading(unit_ref,created_at,reading_ref);
ALTER TABLE linggan_material_unit ADD CONSTRAINT material_unit_current_source
    FOREIGN KEY (unit_ref,source_version,current_reading_ref)
    REFERENCES linggan_material_reading(unit_ref,source_version,reading_ref) DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE linggan_material_reading_selection (
    selection_ref uuid PRIMARY KEY,
    unit_ref uuid NOT NULL REFERENCES linggan_material_unit(unit_ref),
    revision bigint NOT NULL CHECK (revision>=0),
    event text NOT NULL CHECK (event IN ('initialization','automatic','adopt','edit','keep','source_changed')),
    source_version text NOT NULL,
    source_state text NOT NULL CHECK (source_state IN ('present','known_empty','not_acquired','not_applicable','restricted')),
    reading_ref uuid,
    page_ordinal integer CHECK (page_ordinal>0),
    adoption text NOT NULL CHECK (adoption IN ('automatic','human_adopted','human_edited')),
    verification text NOT NULL CHECK (verification IN ('unchecked','verified')),
    command_ref uuid,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (unit_ref,revision),
    CHECK ((event IN ('adopt','edit','keep'))=(command_ref IS NOT NULL)),
    FOREIGN KEY (unit_ref,source_version,reading_ref)
        REFERENCES linggan_material_reading(unit_ref,source_version,reading_ref) DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX material_reading_selection_time ON linggan_material_reading_selection(unit_ref,created_at,revision);
CREATE TABLE linggan_material_reading_command (
    command_ref uuid PRIMARY KEY,
    request_hash text NOT NULL CHECK (request_hash~'^[0-9a-f]{64}$'),
    unit_ref uuid NOT NULL REFERENCES linggan_material_unit(unit_ref),
    candidate_reading_ref uuid NOT NULL REFERENCES linggan_material_reading(reading_ref),
    expected_reading_ref uuid REFERENCES linggan_material_reading(reading_ref),
    resulting_reading_ref uuid REFERENCES linggan_material_reading(reading_ref),
    action text NOT NULL CHECK (action IN ('adopt','edit','keep')),
    actor text CHECK (actor IS NULL OR length(btrim(actor))>0),
    verified boolean NOT NULL,
    response jsonb NOT NULL CHECK (jsonb_typeof(response)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (unit_ref,candidate_reading_ref) REFERENCES linggan_material_reading(unit_ref,reading_ref),
    FOREIGN KEY (unit_ref,expected_reading_ref) REFERENCES linggan_material_reading(unit_ref,reading_ref),
    FOREIGN KEY (unit_ref,resulting_reading_ref) REFERENCES linggan_material_reading(unit_ref,reading_ref)
);
ALTER TABLE linggan_material_reading_selection ADD CONSTRAINT material_selection_command
    FOREIGN KEY (command_ref) REFERENCES linggan_material_reading_command(command_ref) DEFERRABLE INITIALLY DEFERRED;

CREATE FUNCTION linggan_material_unit_revision_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.unit_ref<>OLD.unit_ref OR NEW.content_public_ref<>OLD.content_public_ref
        OR NEW.unit_key<>OLD.unit_key OR NEW.kind<>OLD.kind OR NEW.slot_key IS DISTINCT FROM OLD.slot_key
        OR NEW.created_at<>OLD.created_at OR NEW.revision<>OLD.revision+1 THEN
        RAISE EXCEPTION 'material_unit_identity_or_revision_invalid';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER material_unit_revision_guard BEFORE UPDATE ON linggan_material_unit
    FOR EACH ROW EXECUTE FUNCTION linggan_material_unit_revision_guard();
CREATE FUNCTION linggan_material_selection_matches_current() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_unit linggan_material_unit;
BEGIN
    SELECT * INTO current_unit FROM linggan_material_unit WHERE unit_ref=NEW.unit_ref;
    IF NOT EXISTS(SELECT 1 FROM linggan_material_reading_selection s
        WHERE s.unit_ref=current_unit.unit_ref AND s.revision=current_unit.revision
          AND s.source_version=current_unit.source_version AND s.source_state=current_unit.source_state
          AND s.reading_ref IS NOT DISTINCT FROM current_unit.current_reading_ref
          AND s.page_ordinal IS NOT DISTINCT FROM current_unit.page_ordinal
          AND s.adoption=current_unit.adoption AND s.verification=current_unit.verification) THEN
        RAISE EXCEPTION 'material_current_selection_missing';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER material_unit_current_event AFTER INSERT OR UPDATE ON linggan_material_unit
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_material_selection_matches_current();
CREATE CONSTRAINT TRIGGER material_selection_current_event AFTER INSERT ON linggan_material_reading_selection
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_material_selection_matches_current();
DO $$ DECLARE name text; BEGIN
    FOREACH name IN ARRAY ARRAY['linggan_material_reading','linggan_material_reading_selection','linggan_material_reading_command'] LOOP
        EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation()',name||'_immutable',name);
    END LOOP;
END $$;
ALTER TABLE linggan_media_processing_work ADD COLUMN reading_base_ref uuid REFERENCES linggan_material_reading(reading_ref);
ALTER TABLE linggan_media_processing_work ADD COLUMN reading_source_version text;

-- Membership is a source fact shared by every current consumer. A later partial
-- observation can supply its own slot, but cannot resurrect other removed pages.
CREATE FUNCTION linggan_dossier_slot_active(work uuid, slot text, cutoff timestamptz)
RETURNS boolean LANGUAGE sql STABLE AS $$
    WITH origin AS (
        SELECT o.purpose,o.package_ref,p.accepted_at
        FROM linggan_material_media_origin o
        JOIN linggan_runtime_capture_package p USING(package_ref)
        JOIN linggan_runtime_record_disposition d
          ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal
        WHERE o.content_public_ref=work AND o.slot_key=slot
          AND o.created_at<=cutoff AND p.accepted_at<=cutoff
          AND d.disposition='accepted_for_media_identity'
        ORDER BY o.source_generation DESC LIMIT 1
    ), manifest AS (
        SELECT p.package_ref,p.accepted_at
        FROM linggan_material_lane_observation lane
        JOIN linggan_runtime_capture_package p USING(package_ref)
        WHERE lane.content_public_ref=work AND lane.lane='media_slots'
          AND lane.created_at<=cutoff AND p.accepted_at<=cutoff
          AND p.payload #>> '{coverage,target,imageInventory,state}'='complete'
          AND p.payload #>> '{coverage,target,imageInventory,source}'='platform_image_list'
          AND jsonb_typeof(p.payload #> '{coverage,target,imageInventory,expectedPages}')='number'
        ORDER BY p.accepted_at DESC,lane.created_at DESC,p.package_ref DESC LIMIT 1
    )
    SELECT COALESCE((SELECT purpose NOT IN ('body_image','live_photo')
        OR NOT EXISTS(SELECT 1 FROM manifest)
        OR EXISTS(SELECT 1 FROM manifest m
            WHERE m.package_ref=o.package_ref OR o.accepted_at>m.accepted_at)
        FROM origin o),false)
$$;

-- Direct cutover for development-derived caches. No runtime v1 reader survives.
DROP TABLE IF EXISTS linggan_note_material_fragment;
DROP TABLE IF EXISTS linggan_note_material_snapshot;
DROP FUNCTION IF EXISTS linggan_validate_note_material_fragments();
CREATE TABLE linggan_dossier_snapshot (
    snapshot_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    snapshot_identity text NOT NULL CHECK(snapshot_identity~'^[0-9a-f]{64}$'),
    material_fingerprint text NOT NULL CHECK(material_fingerprint~'^[0-9a-f]{64}$'),
    provenance_hash text NOT NULL CHECK(provenance_hash~'^[0-9a-f]{64}$'),
    input_contract text NOT NULL CHECK(input_contract='research-dossier.v2'),
    fingerprint_contract text NOT NULL CHECK(fingerprint_contract='research-dossier-fingerprint.v2'),
    dossier_contract text NOT NULL CHECK(dossier_contract='research-dossier.v2'),
    layer_version text NOT NULL,
    as_of timestamptz NOT NULL,
    ready boolean NOT NULL,
    not_ready_reasons jsonb NOT NULL CHECK(jsonb_typeof(not_ready_reasons)='array'),
    columns jsonb NOT NULL CHECK(jsonb_typeof(columns)='array'),
    fragment_count integer NOT NULL CHECK(fragment_count>=0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(domain_ref,content_public_ref,snapshot_identity),
    CHECK(as_of<=created_at),CHECK(ready=(jsonb_array_length(not_ready_reasons)=0))
);
CREATE TABLE linggan_dossier_fragment (
    snapshot_ref uuid NOT NULL REFERENCES linggan_dossier_snapshot(snapshot_ref),
    ordinal integer NOT NULL CHECK(ordinal>=0),
    citation_id text NOT NULL CHECK(citation_id~'^S[1-9][0-9]*$'),
    field text NOT NULL CHECK(field IN ('title','cover_headline','body','tags','ocr','transcript','frame_text')),
    source_ref uuid NOT NULL, source_version text NOT NULL, logical_position text NOT NULL,
    text_hash text NOT NULL CHECK(text_hash~'^[0-9a-f]{64}$'),
    char_start integer NOT NULL CHECK(char_start>=0),char_end integer NOT NULL CHECK(char_end>char_start),
    PRIMARY KEY(snapshot_ref,ordinal),UNIQUE(snapshot_ref,citation_id)
);
CREATE INDEX dossier_fragment_source ON linggan_dossier_fragment(source_ref);
CREATE FUNCTION linggan_validate_dossier_fragments() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE expected integer; actual integer; max_ordinal integer;
BEGIN
    SELECT fragment_count INTO expected FROM linggan_dossier_snapshot WHERE snapshot_ref=NEW.snapshot_ref;
    SELECT count(*),max(ordinal) INTO actual,max_ordinal FROM linggan_dossier_fragment WHERE snapshot_ref=NEW.snapshot_ref;
    IF actual<>expected OR (actual>0 AND max_ordinal<>actual-1) THEN RAISE EXCEPTION 'dossier_fragments_incomplete'; END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER dossier_snapshot_complete AFTER INSERT ON linggan_dossier_snapshot
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_dossier_fragments();
CREATE CONSTRAINT TRIGGER dossier_fragment_complete AFTER INSERT ON linggan_dossier_fragment
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_validate_dossier_fragments();
CREATE TRIGGER dossier_snapshot_immutable BEFORE UPDATE OR DELETE ON linggan_dossier_snapshot
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER dossier_fragment_immutable BEFORE UPDATE OR DELETE ON linggan_dossier_fragment
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

DROP VIEW IF EXISTS linggan_ocr_audit_eligible_source;
DROP TABLE IF EXISTS linggan_media_ocr_review;
DO $$ BEGIN
    -- The reusable material-only fixture intentionally omits the AI task schema.
    IF to_regclass('linggan_ocr_recognition_item') IS NOT NULL THEN
        ALTER TABLE linggan_ocr_recognition_item DROP COLUMN adoption_ref;
        ALTER TABLE linggan_ocr_recognition_item ADD COLUMN candidate_reading_ref uuid REFERENCES linggan_material_reading(reading_ref);
        ALTER TABLE linggan_ocr_recognition_item DROP CONSTRAINT linggan_ocr_recognition_item_run_ref_blob_sha256_key;
        ALTER TABLE linggan_ocr_recognition_item ADD UNIQUE(run_ref,slot_key);
    END IF;
    IF to_regclass('linggan_ocr_audit_sample') IS NOT NULL THEN
        -- Old audit samples were keyed to the retired overlay. Development runs are disposable.
        TRUNCATE linggan_ocr_audit_issue,linggan_ocr_audit_observation,linggan_ocr_audit_dispatch,linggan_ocr_audit_sample;
        ALTER TABLE linggan_ocr_audit_sample DROP COLUMN adoption_ref;
        ALTER TABLE linggan_ocr_audit_sample ADD COLUMN reading_ref uuid REFERENCES linggan_material_reading(reading_ref);
        ALTER TABLE linggan_ocr_audit_sample ADD UNIQUE(run_ref,reading_ref);
        ALTER TABLE linggan_ocr_audit_sample ADD CHECK((pool='probe')=(reading_ref IS NULL));
        UPDATE linggan_ocr_audit_settings SET settings=settings-'maxRequests'-'maxTokens'-'maxCostMicroUsd';
    END IF;
END $$;
CREATE TABLE linggan_ocr_audit_review (
    review_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL,
    issue_ref uuid,
    reading_ref uuid NOT NULL REFERENCES linggan_material_reading(reading_ref),
    revision bigint NOT NULL CHECK(revision>0),
    decision text NOT NULL CHECK(decision IN ('confirmed','dismissed','pending')),
    material_review_command_ref uuid REFERENCES linggan_material_reading_command(command_ref),
    actor text NOT NULL CHECK(length(btrim(actor))>0),
    reason text NOT NULL CHECK(length(btrim(reason))>0),
    metrics jsonb NOT NULL CHECK(jsonb_typeof(metrics)='object'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE(sample_ref,revision)
);
CREATE TRIGGER ocr_audit_review_immutable BEFORE UPDATE OR DELETE ON linggan_ocr_audit_review
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
DO $$ BEGIN
    IF to_regclass('linggan_ocr_audit_sample') IS NOT NULL THEN
        ALTER TABLE linggan_ocr_audit_review ADD FOREIGN KEY(sample_ref) REFERENCES linggan_ocr_audit_sample;
        ALTER TABLE linggan_ocr_audit_review ADD FOREIGN KEY(issue_ref) REFERENCES linggan_ocr_audit_issue;
    END IF;
END $$;

CREATE VIEW linggan_ocr_audit_eligible_source AS
SELECT r.*,u.slot_key,m.blob_sha256,b.storage_key,b.mime_type,b.byte_size,
        '/api/local/media/'||m.materialization_ref||'/'||m.blob_sha256 AS original_url,l.image_width,l.image_height,o.purpose
 FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref
 JOIN linggan_media_ocr_layout l ON l.ocr_derivative_ref=r.derivative_ref
 JOIN linggan_media_materialization m ON m.materialization_ref::text=r.source_version
 JOIN linggan_media_download_attempt a USING(download_attempt_ref)
 JOIN linggan_material_media_origin o ON o.observation_ref=a.media_observation_ref AND o.slot_key=u.slot_key
 JOIN linggan_runtime_capture_package p ON p.package_ref=o.package_ref
 JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=o.package_ref AND disposition.record_ordinal=o.record_ordinal
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
 WHERE u.kind='image' AND linggan_dossier_slot_active(u.content_public_ref,u.slot_key,scope_001_now())
   AND disposition.disposition='accepted_for_media_identity' AND p.accepted_at<=scope_001_now() AND o.created_at<=scope_001_now()
   AND o.source_generation=(SELECT max(o2.source_generation) FROM linggan_material_media_origin o2 JOIN linggan_runtime_capture_package p2 USING(package_ref)
       JOIN linggan_runtime_record_disposition d2 ON d2.package_ref=o2.package_ref AND d2.record_ordinal=o2.record_ordinal
       WHERE d2.disposition='accepted_for_media_identity' AND o2.slot_key=u.slot_key AND o2.created_at<=scope_001_now() AND p2.accepted_at<=scope_001_now())
   AND m.materialization_ref=(SELECT m2.materialization_ref FROM linggan_media_materialization m2 JOIN linggan_media_download_attempt a2 USING(download_attempt_ref) JOIN linggan_media_blob b2 ON b2.sha256=m2.blob_sha256
       WHERE b2.mime_type LIKE 'image/%' AND a2.media_observation_ref=o.observation_ref AND m2.verified_at<=scope_001_now() ORDER BY m2.verified_at DESC,m2.materialization_ref DESC LIMIT 1)
   AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition dis WHERE dis.slot_key=u.slot_key OR dis.blob_sha256=m.blob_sha256 OR dis.materialization_ref=m.materialization_ref OR dis.derivative_ref=r.derivative_ref)
   AND NOT EXISTS(SELECT 1 FROM linggan_media_derivative d JOIN linggan_media_ocr_retirement retired ON retired.retired_job_ref=d.job_ref WHERE d.derivative_ref=r.derivative_ref);
