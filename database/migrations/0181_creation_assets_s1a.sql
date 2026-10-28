-- CREATION-ASSETS-001 S1a (DEC-0012): account identity, product fact cards, the bridge sentence
-- library and copy receipts. Every table is append-only; a correction is a new revision.
-- Authorship is the single local machine until member login exists (S0); that slice must widen
-- the saved_by checks below before anyone else can write.

CREATE SEQUENCE linggan_creation_fact_card_code_seq;
CREATE SEQUENCE linggan_creation_bridge_sentence_code_seq;

-- One row per command so a request id belongs to exactly one kind of command and a replay can
-- tell "same command again" from "different command, same id".
CREATE TABLE linggan_creation_assets_request (
    request_ref uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN (
        'identity', 'fact_card', 'fact_card_confirm', 'sentence_create', 'sentence_edit',
        'sentence_review', 'copy_prepare', 'copy_confirm')),
    command_sha256 text NOT NULL CHECK (command_sha256 ~ '^[0-9a-f]{64}$'),
    received_at timestamptz NOT NULL DEFAULT scope_001_now()
);

-- What the operators know about an account. NULL is "not known", never "no".
CREATE TABLE linggan_creation_account_identity (
    identity_ref uuid PRIMARY KEY,
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    revision_number integer NOT NULL CHECK (revision_number > 0),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    org_relation text NOT NULL CHECK (org_relation IN ('parent', 'coach', 'none', 'unknown')),
    bio_discloses_relation boolean,
    bio_checked_at timestamptz,
    bio_checked_by text CHECK (bio_checked_by IS NULL OR bio_checked_by = 'local_machine'),
    has_real_person boolean,
    backing_person_role text
        CHECK (backing_person_role IS NULL OR length(backing_person_role) BETWEEN 1 AND 40),
    can_provide_experience boolean,
    saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
    saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (account_ref, revision_number),
    CHECK (has_real_person IS TRUE OR backing_person_role IS NULL),
    CHECK ((bio_discloses_relation IS NULL) = (bio_checked_at IS NULL)),
    CHECK ((bio_checked_at IS NULL) = (bio_checked_by IS NULL))
);

-- Product fact cards. A revision is content only; it governs the checks once it has a
-- confirmation row, and an older confirmed revision stays in force until a newer one is confirmed.
CREATE TABLE linggan_creation_fact_card (
    card_ref uuid PRIMARY KEY,
    code text NOT NULL CHECK (code ~ '^CP-[0-9]{4,}$'),
    revision_number integer NOT NULL CHECK (revision_number > 0),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    allowed_names text[] NOT NULL CHECK (cardinality(allowed_names) <= 20),
    banned_names text[] NOT NULL CHECK (cardinality(banned_names) <= 20),
    what text NOT NULL CHECK (length(what) <= 2000),
    who text NOT NULL CHECK (length(who) <= 2000),
    can_say text NOT NULL CHECK (length(can_say) <= 2000),
    cannot_say text NOT NULL CHECK (length(cannot_say) <= 2000),
    must_note text NOT NULL CHECK (length(must_note) <= 2000),
    saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
    saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (code, revision_number)
);
CREATE TABLE linggan_creation_fact_card_confirmation (
    card_ref uuid PRIMARY KEY REFERENCES linggan_creation_fact_card(card_ref),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    confirmed_by text NOT NULL CHECK (confirmed_by = 'local_machine'),
    confirmed_at timestamptz NOT NULL DEFAULT scope_001_now()
);

-- Bridge sentence library. Wording lives in revisions; review state lives in status events, and a
-- revision without events is waiting for review.
CREATE TABLE linggan_creation_bridge_sentence (
    sentence_ref uuid PRIMARY KEY,
    code text NOT NULL CHECK (code ~ '^XJ-[0-9]{4,}$'),
    revision_number integer NOT NULL CHECK (revision_number > 0),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    bridge_group text NOT NULL CHECK (bridge_group IN ('parent', 'coach')),
    body text NOT NULL CHECK (length(body) BETWEEN 1 AND 500 AND body !~ '[[:cntrl:]]'),
    origin text NOT NULL CHECK (origin IN ('manual', 'saved_back')),
    entry_findings jsonb NOT NULL CHECK (jsonb_typeof(entry_findings) = 'array'),
    saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
    saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (code, revision_number)
);
CREATE TABLE linggan_creation_bridge_sentence_status (
    status_ref uuid PRIMARY KEY,
    sentence_ref uuid NOT NULL REFERENCES linggan_creation_bridge_sentence(sentence_ref),
    seq integer NOT NULL CHECK (seq > 0),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    status text NOT NULL CHECK (status IN ('review', 'ok', 'off', 'fix')),
    note text CHECK (note IS NULL OR length(note) BETWEEN 1 AND 500),
    set_by text NOT NULL CHECK (set_by = 'local_machine'),
    set_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (sentence_ref, seq)
);

-- "Copied" is not "published": a receipt records which revision was checked under which rules,
-- and a confirmation records that the text reached the clipboard. The text itself is not stored.
CREATE TABLE linggan_creation_copy_receipt (
    receipt_ref uuid PRIMARY KEY,
    work_ref uuid NOT NULL,
    revision_ref uuid NOT NULL,
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    ruleset_version text NOT NULL CHECK (length(ruleset_version) BETWEEN 1 AND 64),
    fact_cards jsonb NOT NULL CHECK (jsonb_typeof(fact_cards) = 'array'),
    warn_rules jsonb NOT NULL CHECK (jsonb_typeof(warn_rules) = 'object'),
    hint_rules jsonb NOT NULL CHECK (jsonb_typeof(hint_rules) = 'object'),
    text_sha256 text NOT NULL CHECK (text_sha256 ~ '^[0-9a-f]{64}$'),
    saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (work_ref, revision_ref)
        REFERENCES linggan_creation_revision(work_ref, revision_ref)
);
CREATE INDEX linggan_creation_copy_receipt_work
    ON linggan_creation_copy_receipt (work_ref, created_at DESC);
CREATE TABLE linggan_creation_copy_confirmation (
    receipt_ref uuid PRIMARY KEY REFERENCES linggan_creation_copy_receipt(receipt_ref),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_creation_assets_request(request_ref),
    copied_by text NOT NULL CHECK (copied_by = 'local_machine'),
    copied_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE TRIGGER linggan_creation_assets_request_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_assets_request
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_account_identity_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_account_identity
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_fact_card_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_fact_card
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_fact_card_confirmation_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_fact_card_confirmation
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_bridge_sentence_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_bridge_sentence
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_bridge_sentence_status_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_bridge_sentence_status
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_copy_receipt_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_copy_receipt
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_copy_confirmation_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_copy_confirmation
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_creation_account_identity IS
    'Append-only account identity revisions (CREATION-ASSETS-001); unknown stays NULL and is never read as no.';
COMMENT ON TABLE linggan_creation_copy_receipt IS
    'Which draft revision was checked under which ruleset when its text was prepared for copying; copying is never publication.';
