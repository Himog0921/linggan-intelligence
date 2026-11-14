-- D-050: retain one shared current-comment projection. A malformed observation time
-- cannot select its body or displace a parseable observation of the same stable comment.
CREATE OR REPLACE VIEW linggan_material_comment_current AS
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
WHERE linggan_opportunity_observed_at(observed_at) IS NOT NULL
ORDER BY content_public_ref, comment_external_id,
         linggan_opportunity_observed_at(observed_at) DESC, created_at DESC, material_ref DESC;

COMMENT ON VIEW linggan_material_comment_current IS
    'Latest parseable-time observation by stable (content_public_ref, comment_external_id); immutable observations remain in linggan_material_comment.';
