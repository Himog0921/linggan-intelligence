-- Writing styles: a named way of writing, kept apart from any single manuscript. Revisions are
-- immutable; the account's style is the newest revision of its newest style.
ALTER TABLE linggan_creation_assets_request DROP CONSTRAINT linggan_creation_assets_request_kind_check;
ALTER TABLE linggan_creation_assets_request ADD CONSTRAINT linggan_creation_assets_request_kind_check CHECK (kind IN (
 'identity','fact_card','fact_card_confirm','sentence_create','sentence_edit','sentence_review',
 'copy_prepare','copy_confirm','publication_link','publication_unlink','style_create','style_revise'));
CREATE SEQUENCE linggan_creation_style_code_seq;
CREATE TABLE linggan_creation_style (
 style_ref uuid PRIMARY KEY,
 code text NOT NULL CHECK (code ~ '^FG-[0-9]{4,}$'),
 account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
 revision_number integer NOT NULL CHECK (revision_number > 0),
 request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
 name text NOT NULL CHECK (length(name) BETWEEN 1 AND 80 AND name !~ '[[:cntrl:]]'),
 source_note text NOT NULL CHECK (length(source_note) <= 200),
 structure text[] NOT NULL CHECK (cardinality(structure) <= 12),
 rhythm text[] NOT NULL CHECK (cardinality(rhythm) <= 12),
 wording text[] NOT NULL CHECK (cardinality(wording) <= 12),
 skip text[] NOT NULL CHECK (cardinality(skip) <= 12),
 saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
 saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE (code, revision_number)
);
CREATE INDEX linggan_creation_style_account ON linggan_creation_style(account_ref, saved_at DESC);
CREATE TRIGGER creation_style_immutable BEFORE UPDATE OR DELETE ON linggan_creation_style
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
