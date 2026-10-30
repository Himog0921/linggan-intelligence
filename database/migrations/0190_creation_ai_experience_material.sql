-- The preview manifest and generated text of an experience rewrite are erasable. The public
-- preview and request retain only an opaque marker and a salted fingerprint after withdrawal.
CREATE TABLE linggan_creation_ai_experience_material (
 preview_ref uuid PRIMARY KEY REFERENCES linggan_creation_ai_preview(preview_ref),
 experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
 manifest jsonb NOT NULL CHECK (jsonb_typeof(manifest)='object'),
 hash_salt uuid NOT NULL,
 suggestion jsonb CHECK (suggestion IS NULL OR jsonb_typeof(suggestion)='object')
);
CREATE INDEX creation_ai_experience_material_owner ON linggan_creation_ai_experience_material(experience_ref);

-- A prior deployment with experience-bound source uses needs a separate audited migration:
-- those immutable historical documents already contain hashes of the private paragraph.
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM linggan_creation_revision r
  CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.document->'experienceSlots','[]'::jsonb)) slot
  CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.document->'uses','[]'::jsonb)) usage
  WHERE usage->'target'->>'kind'='paragraph' AND usage->'target'->>'paragraphId'=slot->>'paragraphId')
 THEN RAISE EXCEPTION 'experience_source_use_migration_requires_remediation'; END IF;
END $$;
CREATE TABLE linggan_creation_experience_use_fragment (
 revision_ref uuid NOT NULL,
 paragraph_ref uuid NOT NULL,
 ordinal integer NOT NULL CHECK(ordinal>=0),
 use_value jsonb NOT NULL CHECK(jsonb_typeof(use_value)='object'),
 PRIMARY KEY(revision_ref,ordinal),
 FOREIGN KEY(revision_ref,paragraph_ref) REFERENCES linggan_creation_experience_fragment(revision_ref,paragraph_ref) ON DELETE CASCADE
);

-- A work command remains as an audit receipt, but an experience-derived fingerprint must be
-- erased on withdrawal. The link contains identity only; revision text stays in fragment storage.
CREATE TABLE linggan_creation_experience_command_link (
 request_ref uuid NOT NULL REFERENCES linggan_creation_request(request_ref),
 experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
 PRIMARY KEY(request_ref, experience_ref)
);
INSERT INTO linggan_creation_experience_command_link(request_ref,experience_ref)
SELECT DISTINCT request.request_ref, experience.experience_ref
FROM linggan_creation_request request
CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(request.receipt->'revision'->'document'->'experienceSlots')='array'
 THEN request.receipt->'revision'->'document'->'experienceSlots' ELSE '[]'::jsonb END) slot(value)
JOIN linggan_creation_experience experience ON experience.experience_ref::text=slot.value->>'experienceRef'
ON CONFLICT DO NOTHING;
-- Old stale receipts contain the server revision, not the submitted document. The submitted
-- text cannot be reconstructed from a hash, so conservatively link them to all experiences
-- known in that work's history; withdrawal will invalidate their old replay fingerprint.
INSERT INTO linggan_creation_experience_command_link(request_ref,experience_ref)
SELECT DISTINCT request.request_ref, experience.experience_ref
FROM linggan_creation_request request
JOIN linggan_creation_revision revision ON revision.work_ref=request.work_ref
CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(revision.document->'experienceSlots')='array'
 THEN revision.document->'experienceSlots' ELSE '[]'::jsonb END) slot(value)
JOIN linggan_creation_experience experience ON experience.experience_ref::text=slot.value->>'experienceRef'
WHERE request.receipt->>'outcome'='stale'
ON CONFLICT DO NOTHING;
DROP TRIGGER linggan_creation_request_immutable ON linggan_creation_request;
CREATE FUNCTION linggan_creation_request_erasure_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND current_setting('linggan.experience_erasure',true)='on'
    AND NEW.command_sha256=repeat('0',64)
    AND ROW(NEW.request_ref,NEW.work_ref,NEW.receipt,NEW.received_at)
        IS NOT DISTINCT FROM ROW(OLD.request_ref,OLD.work_ref,OLD.receipt,OLD.received_at)
    AND EXISTS(SELECT 1 FROM linggan_creation_experience_command_link l WHERE l.request_ref=OLD.request_ref)
 THEN RETURN NEW; END IF;
 RAISE EXCEPTION 'creation_request_immutable';
END $$;
CREATE TRIGGER linggan_creation_request_immutable BEFORE UPDATE OR DELETE ON linggan_creation_request
 FOR EACH ROW EXECUTE FUNCTION linggan_creation_request_erasure_guard();

-- Copy receipts retain their audit identity, not an experience-derived full-text digest.
CREATE TABLE linggan_creation_experience_copy_link (
 receipt_ref uuid NOT NULL REFERENCES linggan_creation_copy_receipt(receipt_ref),
 experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
 PRIMARY KEY(receipt_ref,experience_ref)
);
INSERT INTO linggan_creation_experience_copy_link(receipt_ref,experience_ref)
SELECT DISTINCT receipt.receipt_ref, experience.experience_ref
FROM linggan_creation_copy_receipt receipt JOIN linggan_creation_revision revision ON revision.revision_ref=receipt.revision_ref
CROSS JOIN LATERAL jsonb_array_elements(COALESCE(revision.document->'experienceSlots','[]'::jsonb)) slot(value)
JOIN linggan_creation_experience experience ON experience.experience_ref::text=slot.value->>'experienceRef'
ON CONFLICT DO NOTHING;
DROP TRIGGER linggan_creation_copy_receipt_immutable ON linggan_creation_copy_receipt;
CREATE FUNCTION linggan_creation_copy_receipt_erasure_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND current_setting('linggan.experience_erasure',true)='on'
    AND NEW.text_sha256=repeat('0',64)
    AND (to_jsonb(NEW)-'text_sha256') IS NOT DISTINCT FROM (to_jsonb(OLD)-'text_sha256')
    AND EXISTS(SELECT 1 FROM linggan_creation_experience_copy_link l WHERE l.receipt_ref=OLD.receipt_ref)
 THEN RETURN NEW; END IF;
 RAISE EXCEPTION 'creation_copy_receipt_immutable';
END $$;
CREATE TRIGGER linggan_creation_copy_receipt_immutable BEFORE UPDATE OR DELETE ON linggan_creation_copy_receipt
 FOR EACH ROW EXECUTE FUNCTION linggan_creation_copy_receipt_erasure_guard();
