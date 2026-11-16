-- D-050: malformed observation times cannot select a work author. Preserve the existing
-- shared function, as-of bounds and source priority for all parseable observations.
CREATE OR REPLACE FUNCTION linggan_material_content_author_at(p_content_public_ref uuid, p_as_of timestamptz)
RETURNS TABLE(author_external_id text, attribution_source text, observed_at timestamptz)
LANGUAGE sql STABLE AS $$
 SELECT candidate.author_external_id,candidate.attribution_source,candidate.observed_at
 FROM (
   SELECT detail.author_external_id,'content_detail'::text AS attribution_source,
          linggan_opportunity_observed_at(detail.observed_at) AS observed_at,detail.created_at AS recorded_at,
          detail.package_ref,detail.material_ref,2 AS source_priority
   FROM linggan_material_content_detail detail
   JOIN linggan_runtime_capture_package package USING(package_ref)
   WHERE detail.content_public_ref=p_content_public_ref
     AND package.accepted_at<=p_as_of
     AND detail.created_at<=p_as_of
     AND linggan_opportunity_observed_at(detail.observed_at) IS NOT NULL
     AND NULLIF(btrim(detail.author_external_id),'') IS NOT NULL
   UNION ALL
   SELECT task.task_spec #>> '{target,authorExternalId}','profile_discovery'::text,
          linggan_opportunity_observed_at(finding.observed_at),finding.created_at,
          finding.package_ref,finding.material_ref,1
   FROM linggan_material_discovery_finding finding
   JOIN linggan_runtime_capture_package package USING(package_ref)
   JOIN linggan_runtime_task task USING(task_id)
   WHERE finding.content_public_ref=p_content_public_ref
     AND package.package_kind='profile_discovery'
     AND package.accepted_at<=p_as_of
     AND finding.created_at<=p_as_of
     AND linggan_opportunity_observed_at(finding.observed_at) IS NOT NULL
     AND NULLIF(btrim(task.task_spec #>> '{target,authorExternalId}'),'') IS NOT NULL
 ) candidate
 ORDER BY candidate.source_priority DESC,candidate.observed_at DESC,
          candidate.recorded_at DESC,candidate.package_ref DESC,candidate.material_ref DESC
 LIMIT 1
$$;
