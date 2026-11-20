-- Keep immutable definitions and redirects for audit. Qualify their text at read/dispatch time.
CREATE VIEW linggan_comment_problem_withdrawn_source AS
SELECT definition.problem_ref, definition.definition_ref
FROM linggan_comment_problem_definition definition
JOIN linggan_comment_research_request request ON request.request_ref = definition.source_request_ref
JOIN linggan_material_comment source ON source.material_ref = request.source_ref
WHERE definition.origin_invocation_ref IS NOT NULL
  AND EXISTS (
      SELECT 1 FROM linggan_material_comment_restriction restriction
      WHERE restriction.content_public_ref = source.content_public_ref
        AND restriction.comment_external_id = source.comment_external_id
  );
