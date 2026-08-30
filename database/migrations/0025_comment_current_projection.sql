-- COMMENT-CURRENT-001
-- Each producer Attempt remains append-only. This derived current projection selects the latest
-- observation for a stable platform comment identity, so retrying a traversal never makes one
-- real comment appear twice in retrieval or analysis.

CREATE VIEW linggan_material_comment_current AS
SELECT DISTINCT ON (content_public_ref, comment_external_id)
    material_ref,
    content_public_ref,
    package_ref,
    record_ordinal,
    comment_external_id,
    root_comment_external_id,
    parent_comment_external_id,
    parent_identity_source_field,
    is_reply,
    body_text,
    body_state,
    author_external_id,
    author_display_name,
    observed_at,
    created_at
FROM linggan_material_comment
ORDER BY content_public_ref, comment_external_id, observed_at::timestamptz DESC, created_at DESC, material_ref DESC;

COMMENT ON VIEW linggan_material_comment_current IS
    'Latest current projection by stable (content_public_ref, comment_external_id); immutable per-Attempt observations remain in linggan_material_comment.';
