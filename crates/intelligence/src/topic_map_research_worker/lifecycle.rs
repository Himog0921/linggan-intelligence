//! A run completes only after its own authorized maintenance has drained.
use super::*;
use sqlx::{Postgres, Transaction};

/// The maintenance lock distinguishes "busy" from "nothing remains". Policy/run
/// locks and subsequent fresh statements serialize completion with queue writers.
pub(super) async fn complete_ready_runs(db: &Database) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    let owner: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('topic-map-backfill',120))",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !owner {
        return Ok(false);
    }
    let rows = sqlx::query("SELECT r.run_ref,r.domain_ref FROM linggan_topic_map_research_run r WHERE r.state IN ('queued','running') AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref AND t.state IN ('queued','running')) ORDER BY r.created_at,r.run_ref LIMIT 16")
        .fetch_all(&mut *tx).await?;
    let mut completed = false;
    for row in rows {
        let domain: Uuid = row.get("domain_ref");
        let run: Uuid = row.get("run_ref");
        let policy = sqlx::query("SELECT p.domain_ref FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 AND p.status='active' AND d.status='active' FOR UPDATE OF p SKIP LOCKED")
            .bind(domain).fetch_optional(&mut *tx).await?;
        if policy.is_none() {
            continue;
        }
        let locked = sqlx::query("SELECT run_ref FROM linggan_topic_map_research_run WHERE run_ref=$1 AND state IN ('queued','running') FOR UPDATE SKIP LOCKED")
            .bind(run).fetch_optional(&mut *tx).await?;
        if locked.is_none() {
            continue;
        }
        // A task may have committed after the candidate scan but before these
        // row locks. Never use that earlier snapshot to make the final decision.
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state IN ('queued','running'))")
            .bind(run).fetch_one(&mut *tx).await?;
        if active
            || pending_definitions(&mut tx, run).await?
            || core::comparison::pending_in(&mut tx, run).await?
        {
            continue;
        }
        completed |= sqlx::query("UPDATE linggan_topic_map_research_run SET state='completed',updated_at=scope_001_now() WHERE run_ref=$1 AND state IN ('queued','running')")
            .bind(run).execute(&mut *tx).await?.rows_affected()>0;
    }
    tx.commit().await?;
    Ok(completed)
}

async fn pending_definitions(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
) -> Result<bool, ModelError> {
    Ok(sqlx::query_scalar(r#"
        SELECT EXISTS(
            SELECT 1 FROM linggan_topic_map_research_run r
            JOIN linggan_topic_map_research_policy p USING(domain_ref)
            JOIN linggan_topic_map_research_task source USING(run_ref)
            JOIN linggan_topic_definition definition ON true
            JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding
                WHERE topic_ref=definition.topic_ref ORDER BY version DESC LIMIT 1) binding ON true
            WHERE r.run_ref=$1 AND source.state='succeeded' AND source.distilled_json IS NOT NULL
                AND source.phase IN ('extract','resolve')
                AND binding.domain_ref=r.domain_ref
                AND definition.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=definition.topic_ref)
                AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=definition.topic_ref)
                AND (p.automatic_enabled
                    OR (r.trigger='on_demand' AND COALESCE(r.input_scope->>'backfillReopened','false')<>'true'
                        AND definition.created_at>=r.created_at)
                    OR EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(r.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                        WHERE permission->'workRefs' ? source.work_public_ref::text
                            AND permission->'definitionRefs' ? definition.definition_ref::text))
                AND (NOT EXISTS(SELECT 1 FROM linggan_topic_map_definition_scan WHERE definition_ref=definition.definition_ref)
                    OR EXISTS(SELECT 1 FROM linggan_topic_map_backfill_queue q
                        WHERE q.source_task_ref=source.task_ref AND q.definition_ref=definition.definition_ref
                            AND q.state='pending'))
        )
    "#).bind(run).fetch_one(&mut **tx).await?)
}
