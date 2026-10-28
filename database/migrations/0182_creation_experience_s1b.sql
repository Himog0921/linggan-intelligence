-- CREATION-ASSETS-001 S1b. Experience payloads must be removable when their subject withdraws.
-- This candidate is not a complete draft integration and must not be deployed on its own.
CREATE SEQUENCE linggan_creation_experience_code_seq;

CREATE TABLE linggan_creation_experience (
    experience_ref uuid PRIMARY KEY,
    code text NOT NULL UNIQUE CHECK (code ~ '^JL-[0-9]{4,}$'),
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    state text NOT NULL CHECK (state IN ('awaiting_subject', 'confirmed', 'disabled', 'withdrawn')),
    current_revision integer NOT NULL CHECK (current_revision > 0),
    created_by text NOT NULL CHECK (created_by = 'local_machine'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    withdrawn_at timestamptz,
    CHECK ((state = 'withdrawn') = (withdrawn_at IS NOT NULL))
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
    subject_role text,
    confirmation_method text NOT NULL CHECK (confirmation_method IN ('subject_present', 'subject_statement')),
    recorded_by text NOT NULL CHECK (recorded_by = 'local_machine'),
    confirmed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (experience_ref, revision),
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
