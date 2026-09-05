-- COLLECTION-CONTROL-CLOSURE-001 · an approved detail read need not imply comment collection.
--
-- The frozen material scope is the execution authority. `comment_limit = 0` means exactly that
-- the Work Order may read the note detail, but it may not create a comments/replies task.
-- Existing scopes remain unchanged; this only expands the expressible bounded policy.

ALTER TABLE collection_work_order_material_target
    DROP CONSTRAINT collection_work_order_material_target_comment_limit_check;

ALTER TABLE collection_work_order_material_target
    ADD CONSTRAINT collection_work_order_material_target_comment_limit_check
        CHECK (comment_limit BETWEEN 0 AND 30),
    ADD CONSTRAINT collection_work_order_material_target_reply_requires_comment_check
        CHECK (comment_limit > 0 OR reply_expand_limit = 0);

COMMENT ON COLUMN collection_work_order_material_target.comment_limit IS
    '0 means detail-only; 1..30 authorizes that many top-level comments for this exact material.';
