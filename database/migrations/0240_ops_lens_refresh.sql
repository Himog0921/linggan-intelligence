-- A publication and its refresh request commit together. The generation records which
-- changes have reached the board; a report arriving during calculation remains pending.
CREATE TABLE linggan_ops_lens_refresh (
    publication_ref uuid PRIMARY KEY REFERENCES linggan_ops_publication(publication_ref),
    generation bigint NOT NULL DEFAULT 1,
    snapshot_generation bigint NOT NULL DEFAULT 0,
    label_pending boolean NOT NULL DEFAULT false,
    CHECK (snapshot_generation BETWEEN 0 AND generation)
);

CREATE INDEX ops_lens_refresh_pending ON linggan_ops_lens_refresh (publication_ref)
    WHERE snapshot_generation < generation OR label_pending;

CREATE FUNCTION linggan_ops_lens_queue_registration() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO linggan_ops_lens_refresh(publication_ref,label_pending)
    VALUES (NEW.publication_ref,true);
    RETURN NEW;
END $$;

CREATE TRIGGER ops_lens_queue_registration AFTER INSERT ON linggan_ops_publication
    FOR EACH ROW WHEN (NEW.source_system='linggan_native' AND NEW.platform='xhs'
                       AND NEW.native_record_state='active')
    EXECUTE FUNCTION linggan_ops_lens_queue_registration();

CREATE FUNCTION linggan_ops_lens_queue_view_report() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO linggan_ops_lens_refresh(publication_ref)
    SELECT p.publication_ref FROM linggan_ops_publication p
    WHERE p.publication_ref=NEW.publication_ref AND p.platform='xhs'
      AND linggan_ops_publication_visible(p,NULL::linggan_ops_account,NULL,NULL)
    ON CONFLICT (publication_ref) DO UPDATE SET generation=linggan_ops_lens_refresh.generation+1;
    RETURN NEW;
END $$;

CREATE TRIGGER ops_lens_queue_view_report AFTER INSERT ON linggan_ops_view_report
    FOR EACH ROW WHEN (NEW.source_kind='linggan_ops_manual')
    EXECUTE FUNCTION linggan_ops_lens_queue_view_report();

-- Current titles may change after native registration when a qualified observation
-- adopts a title or supersedes one. For imported publications, refresh only metrics.
CREATE FUNCTION linggan_ops_lens_queue_field_adoption() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO linggan_ops_lens_refresh(publication_ref,label_pending)
    SELECT p.publication_ref, NEW.field_name='title' AND p.source_system='linggan_native'
    FROM linggan_ops_publication p
    WHERE p.publication_ref=NEW.publication_ref AND p.platform='xhs'
      AND NEW.publication_version IS NOT DISTINCT FROM p.native_version
      AND linggan_ops_publication_visible(p,NULL::linggan_ops_account,NULL,NULL)
    ON CONFLICT (publication_ref) DO UPDATE
       SET generation=linggan_ops_lens_refresh.generation+1,
           label_pending=linggan_ops_lens_refresh.label_pending OR EXCLUDED.label_pending;
    RETURN NEW;
END $$;

CREATE TRIGGER ops_lens_queue_field_adoption AFTER INSERT ON linggan_ops_publication_field_adoption
    FOR EACH ROW WHEN (NEW.field_name IN ('title','comment_count'))
    EXECUTE FUNCTION linggan_ops_lens_queue_field_adoption();

CREATE FUNCTION linggan_ops_lens_queue_publication_version() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO linggan_ops_lens_refresh(publication_ref,label_pending)
    VALUES (NEW.publication_ref,NEW.native_record_state='active')
    ON CONFLICT (publication_ref) DO UPDATE
       SET generation=linggan_ops_lens_refresh.generation+1,
           label_pending=EXCLUDED.label_pending;
    RETURN NEW;
END $$;

CREATE TRIGGER ops_lens_queue_publication_version
    AFTER UPDATE OF native_version,native_record_state,native_published_on ON linggan_ops_publication
    FOR EACH ROW WHEN (NEW.source_system='linggan_native' AND NEW.platform='xhs'
      AND (OLD.native_version,OLD.native_record_state,OLD.native_published_on)
        IS DISTINCT FROM (NEW.native_version,NEW.native_record_state,NEW.native_published_on))
    EXECUTE FUNCTION linggan_ops_lens_queue_publication_version();

-- Native notes registered before this migration enter the same work stream. Never
-- seed imported titles into the automatic labelling pool.
INSERT INTO linggan_ops_lens_refresh(publication_ref,label_pending)
SELECT p.publication_ref,true
FROM linggan_ops_publication p
WHERE p.source_system='linggan_native' AND p.platform='xhs'
  AND linggan_ops_publication_visible(p,NULL::linggan_ops_account,NULL,NULL);

-- A scope can return to a previously frozen input (for example after a revocation).
-- Keep the current selection independent of the immutable snapshot's build time.
CREATE TABLE linggan_ops_lens_snapshot_current (
    domain_ref uuid,
    published_from date,
    published_to date,
    snapshot_ref uuid NOT NULL REFERENCES linggan_ops_lens_snapshot(snapshot_ref),
    UNIQUE NULLS NOT DISTINCT (domain_ref,published_from,published_to)
);

CREATE INDEX ops_lens_snapshot_latest_open_scope
    ON linggan_ops_lens_snapshot ((COALESCE(analysis_input->>'domainRef','')),as_of DESC,created_at DESC,snapshot_ref DESC)
    WHERE analysis_input->>'from' IS NULL AND analysis_input->>'to' IS NULL;
