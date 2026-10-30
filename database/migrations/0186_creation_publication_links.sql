-- Explicit human association: a clipboard receipt is not a publication record.
ALTER TABLE linggan_creation_assets_request DROP CONSTRAINT linggan_creation_assets_request_kind_check;
ALTER TABLE linggan_creation_assets_request ADD CONSTRAINT linggan_creation_assets_request_kind_check CHECK (kind IN (
 'identity','fact_card','fact_card_confirm','sentence_create','sentence_edit','sentence_review',
 'copy_prepare','copy_confirm','publication_link','publication_unlink'));
CREATE TABLE linggan_creation_publication_link (
 link_ref uuid PRIMARY KEY,
 request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
 work_ref uuid NOT NULL,
 revision_ref uuid NOT NULL,
 publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
 linked_by text NOT NULL CHECK (linked_by='local_machine'),
 linked_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY (work_ref,revision_ref) REFERENCES linggan_creation_revision(work_ref,revision_ref)
);
CREATE INDEX creation_publication_link_work ON linggan_creation_publication_link(work_ref,linked_at DESC);
CREATE INDEX creation_publication_link_publication ON linggan_creation_publication_link(publication_ref);
CREATE TABLE linggan_creation_publication_unlink (
 link_ref uuid PRIMARY KEY REFERENCES linggan_creation_publication_link(link_ref),
 request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
 unlinked_by text NOT NULL CHECK (unlinked_by='local_machine'),
 unlinked_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER creation_publication_link_immutable BEFORE UPDATE OR DELETE ON linggan_creation_publication_link
 FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER creation_publication_unlink_immutable BEFORE UPDATE OR DELETE ON linggan_creation_publication_unlink
 FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
