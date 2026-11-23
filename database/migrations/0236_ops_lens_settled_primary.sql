-- A settled primary contributes only accepted labels. The audit must still be complete.
CREATE OR REPLACE VIEW linggan_ops_lens_effective_label AS
WITH completed AS (
    SELECT r.*, b.finished_at
    FROM linggan_ops_lens_label_run r
    JOIN linggan_ai_batch_control b ON b.batch_ref = r.foundation_batch_ref AND b.state = 'completed'
    WHERE r.run_kind = 'primary' OR r.title_count = (SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref = r.run_ref)
), eligible AS (
    SELECT p.* FROM completed p WHERE p.run_kind = 'primary' AND EXISTS (
        SELECT 1 FROM completed audit
        JOIN linggan_ops_lens_agreement a ON a.audit_run_ref = audit.run_ref
        WHERE audit.primary_run_ref = p.run_ref
        GROUP BY audit.run_ref HAVING count(*) = 4
    )
), latest AS (
    SELECT DISTINCT ON (r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.title_key)
           r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.*
    FROM eligible r JOIN linggan_ops_lens_label l USING (run_ref)
    ORDER BY r.taxonomy_ref, r.task_config_ref, r.prompt_version_ref, l.title_key,
             r.finished_at DESC, r.run_ref DESC
)
SELECT l.taxonomy_ref, l.task_config_ref, l.prompt_version_ref, l.title_key, field.dimension,
       coalesce(h.value, field.value) AS value, l.run_ref, h.adjudication_ref
FROM latest l
CROSS JOIN LATERAL (VALUES ('moment',l.moment), ('offer',l.offer),
                           ('situation',l.situation), ('audience',l.audience)) field(dimension,value)
LEFT JOIN LATERAL (
    SELECT a.value, a.adjudication_ref FROM linggan_ops_lens_adjudication a
    WHERE a.taxonomy_ref = l.taxonomy_ref AND a.title_key = l.title_key AND a.dimension = field.dimension
    ORDER BY a.created_at DESC, a.adjudication_ref DESC LIMIT 1
) h ON true;
