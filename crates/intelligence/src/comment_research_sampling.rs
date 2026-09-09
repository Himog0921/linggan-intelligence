//! Deterministic, bounded samples of currently readable research material. This function only
//! selects references; the replay creator freezes source/context hashes before any execution.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use uuid::Uuid;

pub async fn replay_sources(db: &Database, seed: i64) -> Result<Vec<Uuid>, ModelError> {
    Ok(sqlx::query_scalar(r#"
      WITH eligible AS MATERIALIZED (
        SELECT s.source_ref,s.work_ref,
          concat_ws(':',CASE WHEN char_length(s.body)<=12 THEN 'short' WHEN char_length(s.body)>=200 THEN 'long' ELSE 'medium' END,
            CASE WHEN s.is_reply THEN 'reply' ELSE 'root' END,c.state,
            CASE WHEN e.result_state='no_signal' THEN 'no_signal'
                 WHEN e.execution_state='failed' THEN 'failed'
                 WHEN a.result#>>'{semantic,acceptance}'='partial' THEN 'partial'
                 WHEN jsonb_array_length(COALESCE(a.result#>'{semantic,atoms}','[]'))>1 THEN 'multiple'
                 ELSE 'other' END) AS stratum
        FROM linggan_ci_source s JOIN observation_domain d USING(domain_ref)
        JOIN linggan_comment_clean c ON c.source_ref=s.source_ref AND c.source_sha256=s.source_sha256 AND c.cleaner_version='comment-clean.v2'
        LEFT JOIN linggan_comment_research_eligibility_current e ON e.source_ref=s.source_ref
        LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=e.current_analysis_ref
        WHERE d.is_own_domain AND c.state IN('direct','context') AND s.body IS NOT NULL
      ), work_ranked AS (
        SELECT *,row_number() OVER(PARTITION BY stratum,work_ref ORDER BY md5(source_ref::text||$1::text)) AS within_work
        FROM eligible
      ), stratified AS (
        SELECT *,row_number() OVER(PARTITION BY stratum ORDER BY within_work,md5(work_ref::text||$1::text),source_ref) AS within_stratum
        FROM work_ranked
      ) SELECT source_ref FROM stratified ORDER BY within_stratum,stratum,work_ref,source_ref LIMIT 120
    "#).bind(seed).fetch_all(db.pool()).await?)
}
