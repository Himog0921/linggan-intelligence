-- COLLECTION-001 · Observation targets
--
-- The long-lived identity of something we intend to keep watching: a creator, or a keyword.
-- This table holds identity and lifecycle only.  It deliberately carries no schedule, no
-- quota and no execution state, because none of those exist yet — a target being stored here
-- proves that it was saved, and nothing more.
--
-- Per docs/product/collection-monitoring-rules.md §2: a target enters as `pending_decision`
-- and consumes no platform access.  Storing it is not an Acquisition Request, is not an
-- Authorization, and is not an admission that any capture will ever run (INV-36).

CREATE TABLE collection_observation_target (
    target_ref uuid PRIMARY KEY,

    -- Only the platform with a controlled verification base is open.  Others stay closed even
    -- though the column could hold them (Issue #66: a platform field is not an open platform).
    platform text NOT NULL CHECK (platform IN ('xhs')),

    -- Creator and keyword are two distinct kinds, never one polymorphic "target" (Issue #66).
    target_kind text NOT NULL CHECK (target_kind IN ('creator', 'keyword')),

    -- The normalised identity this target is deduplicated by.  For a creator this is the
    -- platform's own stable id — never a URL, because the same creator has several URL forms.
    -- For a keyword it is the term plus its ranking, because the same word under two rankings
    -- is two different observation surfaces.
    identity_key text NOT NULL CHECK (length(btrim(identity_key)) > 0),

    -- What a human calls it.  Display only: it never participates in identity or matching,
    -- because creators rename themselves.
    display_name text,

    -- Identity facts captured when the target entered (avatar, follower count, bio…).  Null
    -- rather than an empty object when nothing was captured: unknown is not "none".
    identity_facts jsonb,

    -- Where the target came from.  Both routes produce the same pending target.
    source text NOT NULL CHECK (source IN ('plugin_push', 'manual')),

    -- The lifecycle from the product rules §2.1.  Deep archiving must complete before
    -- monitoring can start, so `monitoring` is unreachable except through `archived`.
    lifecycle_state text NOT NULL DEFAULT 'pending_decision'
        CHECK (lifecycle_state IN ('pending_decision', 'archiving', 'archived', 'monitoring', 'paused', 'dismissed')),

    first_stored_at timestamptz NOT NULL DEFAULT scope_001_now(),
    lifecycle_changed_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- One row per real-world target.  Without this a target pushed twice from the browser
    -- silently becomes two parallel targets with two independent baselines — the exact
    -- failure the legacy workbench has today, where MonitorConfig carries no unique index.
    UNIQUE (platform, target_kind, identity_key)
);

CREATE INDEX collection_observation_target_lifecycle_idx
    ON collection_observation_target (lifecycle_state, first_stored_at DESC);

-- Every lifecycle move, kept append-only.  A target's current state is a readable summary;
-- how it got there is the record.  Observations are only ever appended (INV: never rewrite
-- history), and the same discipline applies to the decisions taken about a target.
CREATE TABLE collection_observation_target_transition (
    transition_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),

    from_state text,
    to_state text NOT NULL,

    -- Who moved it.  `system` is reserved for transitions a machine may make on its own;
    -- everything that consumes platform access must name a person.
    actor text NOT NULL CHECK (actor IN ('person', 'system')),

    -- Why.  Free text is not enough for later audit, so a code is required and the prose
    -- is optional alongside it.
    reason_code text NOT NULL CHECK (length(btrim(reason_code)) > 0),
    reason text,

    occurred_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX collection_observation_target_transition_target_idx
    ON collection_observation_target_transition (target_ref, occurred_at DESC);
