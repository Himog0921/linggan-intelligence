-- MATERIAL-DEEPENING-001 · explicit, bounded work-level material scope
--
-- A discovery quota is not permission to deepen every discovered item.  This relation freezes
-- the exact material identities a Work Order may expand, plus the limits that apply to each one.
-- Runtime tasks and lease-task rows remain the execution truth; this table is only scope.

CREATE TABLE collection_work_order_material_target (
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
    ordinal integer NOT NULL CHECK (ordinal > 0),
    comment_limit integer NOT NULL DEFAULT 30 CHECK (comment_limit BETWEEN 1 AND 30),
    reply_expand_limit integer NOT NULL DEFAULT 2 CHECK (reply_expand_limit BETWEEN 0 AND 2),
    acquire_media boolean NOT NULL DEFAULT true,
    allow_ocr boolean NOT NULL DEFAULT true,
    allow_asr boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (work_order_ref, content_public_ref),
    UNIQUE (work_order_ref, ordinal)
);

CREATE INDEX collection_work_order_material_target_content_idx
    ON collection_work_order_material_target(content_public_ref, created_at DESC);

COMMENT ON TABLE collection_work_order_material_target IS
    'The exact, person-authorized content set a Work Order may deepen; never a discovery remainder or task result.';
