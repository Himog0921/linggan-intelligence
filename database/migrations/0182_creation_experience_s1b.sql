-- CREATION-ASSETS-001 S1b. Experience payloads must be removable when their subject withdraws.
-- This candidate is not a complete draft integration and must not be deployed on its own.
CREATE SEQUENCE linggan_creation_experience_code_seq;

-- A pseudonymous subject identifies the person behind an account, not their legal identity.
-- Corrections keep it; a confirmed replacement creates a new subject.
CREATE TABLE linggan_creation_person_subject (
    subject_ref uuid PRIMARY KEY,
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (subject_ref, account_ref)
);
CREATE TABLE linggan_creation_account_person (
    account_ref uuid PRIMARY KEY REFERENCES linggan_ops_account(account_ref),
    subject_ref uuid NOT NULL,
    FOREIGN KEY (subject_ref, account_ref) REFERENCES linggan_creation_person_subject(subject_ref, account_ref)
);
CREATE TABLE linggan_creation_person_assignment (
    identity_ref uuid PRIMARY KEY REFERENCES linggan_creation_account_identity(identity_ref),
    account_ref uuid NOT NULL,
    subject_ref uuid NOT NULL,
    action text NOT NULL CHECK (action IN ('bootstrap', 'created', 'unchanged', 'corrected', 'replaced', 'unavailable')),
    recorded_by text NOT NULL CHECK (recorded_by = 'local_machine'),
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (subject_ref, account_ref) REFERENCES linggan_creation_person_subject(subject_ref, account_ref)
);
CREATE TRIGGER linggan_creation_person_subject_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_person_subject
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_creation_person_assignment_immutable
    BEFORE UPDATE OR DELETE ON linggan_creation_person_assignment
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

-- S1a had no personal experience objects: seed one pseudonym for each current known person.
WITH latest AS (
    SELECT DISTINCT ON (account_ref) account_ref, has_real_person
    FROM linggan_creation_account_identity ORDER BY account_ref, revision_number DESC
), seeded AS (
    INSERT INTO linggan_creation_person_subject(subject_ref,account_ref)
    SELECT gen_random_uuid(), account_ref FROM latest WHERE has_real_person IS TRUE
    RETURNING subject_ref,account_ref
)
INSERT INTO linggan_creation_account_person(account_ref,subject_ref)
SELECT account_ref,subject_ref FROM seeded;
INSERT INTO linggan_creation_person_assignment(identity_ref,account_ref,subject_ref,action,recorded_by)
SELECT i.identity_ref, p.account_ref, p.subject_ref, 'bootstrap', 'local_machine'
FROM linggan_creation_account_person p
JOIN LATERAL (SELECT identity_ref FROM linggan_creation_account_identity
              WHERE account_ref=p.account_ref ORDER BY revision_number DESC LIMIT 1) i ON true;

CREATE TABLE linggan_creation_experience (
    experience_ref uuid PRIMARY KEY,
    code text NOT NULL UNIQUE CHECK (code ~ '^JL-[0-9]{4,}$'),
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    subject_ref uuid NOT NULL,
    state text NOT NULL CHECK (state IN ('awaiting_subject', 'confirmed', 'disabled', 'withdrawn')),
    current_revision integer NOT NULL CHECK (current_revision > 0),
    created_by text NOT NULL CHECK (created_by = 'local_machine'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    withdrawn_at timestamptz,
    CHECK ((state = 'withdrawn') = (withdrawn_at IS NOT NULL)),
    UNIQUE (experience_ref, subject_ref),
    FOREIGN KEY (subject_ref, account_ref) REFERENCES linggan_creation_person_subject(subject_ref, account_ref)
);

-- Unlike ordinary creation revisions this payload is deliberately erasable. Consent is tied to
-- one exact revision; editing produces another revision requiring fresh confirmation.
CREATE TABLE linggan_creation_experience_revision (
    experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
    revision integer NOT NULL CHECK (revision > 0),
    title text NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    body text NOT NULL CHECK (length(body) BETWEEN 1 AND 10000),
    tags text[] NOT NULL CHECK (cardinality(tags) <= 20),
    origin text NOT NULL CHECK (origin IN ('oral', 'manuscript', 'anonymized_coaching')),
    saved_by text NOT NULL CHECK (saved_by = 'local_machine'),
    saved_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (experience_ref, revision)
);
CREATE TABLE linggan_creation_experience_consent (
    experience_ref uuid NOT NULL,
    revision integer NOT NULL,
    draft_use boolean NOT NULL CHECK (draft_use),
    send_to_model boolean,
    subject_account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    subject_ref uuid NOT NULL,
    subject_role text NOT NULL CHECK (length(subject_role) BETWEEN 1 AND 40),
    confirmation_method text NOT NULL CHECK (confirmation_method IN ('subject_present', 'subject_statement')),
    recorded_by text NOT NULL CHECK (recorded_by = 'local_machine'),
    confirmed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (experience_ref, revision),
    FOREIGN KEY (subject_ref, subject_account_ref) REFERENCES linggan_creation_person_subject(subject_ref, account_ref),
    FOREIGN KEY (experience_ref, subject_ref) REFERENCES linggan_creation_experience(experience_ref, subject_ref),
    FOREIGN KEY (experience_ref, revision)
        REFERENCES linggan_creation_experience_revision(experience_ref, revision) ON DELETE CASCADE
);

-- Payload fingerprints are erasable along with the content. After withdrawal old create/edit
-- commands no longer replay the material, but request identity and the disposition remain.
CREATE TABLE linggan_creation_experience_request (
    request_ref uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('create', 'edit', 'confirm', 'withdraw')),
    experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
    result_revision integer NOT NULL CHECK (result_revision > 0),
    result_state text NOT NULL CHECK (result_state IN ('awaiting_subject', 'confirmed', 'withdrawn')),
    command_sha256 text CHECK (command_sha256 IS NULL OR command_sha256 ~ '^[0-9a-f]{64}$'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

-- Draft revisions carry references only; text associated with an experience stays in the
-- erasable fragment store. This table is not an authorization to expose the feature yet.
CREATE TABLE linggan_creation_experience_fragment (
    revision_ref uuid NOT NULL REFERENCES linggan_creation_revision(revision_ref),
    paragraph_ref uuid NOT NULL,
    experience_ref uuid NOT NULL REFERENCES linggan_creation_experience(experience_ref),
    experience_revision integer NOT NULL,
    body text NOT NULL,
    PRIMARY KEY (revision_ref, paragraph_ref)
);
CREATE INDEX linggan_creation_experience_fragment_owner ON linggan_creation_experience_fragment(experience_ref);
