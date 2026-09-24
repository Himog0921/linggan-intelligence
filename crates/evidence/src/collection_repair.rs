//! Bounded operator preview. Historical evidence is never rewritten by this tool.
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    #[error("repair preview identity or hash mismatch")]
    InvalidPreview,
    #[error("repair database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("repair preview could not be encoded")]
    Encoding(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairItem {
    pub task_id: Uuid,
    pub work_order_ref: Uuid,
    pub lease_ref: Uuid,
    pub installation_ref: Option<Uuid>,
    pub execution_state: String,
    pub snapshot_hash: String,
    pub proposed_action: String,
    pub reason: String,
    pub affected_task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairPreview {
    pub contract: String,
    pub batch_id: Uuid,
    pub source: String,
    pub concurrency_precondition: String,
    pub items: Vec<RepairItem>,
    pub truncated: bool,
    pub preview_hash: String,
}

impl RepairPreview {
    fn hash(&self) -> Result<String, RepairError> {
        let mut value = self.clone();
        value.preview_hash.clear();
        Ok(Sha256::digest(serde_json::to_vec(&value)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

#[derive(Debug, Serialize)]
pub struct RepairOutcome {
    pub task_id: Uuid,
    pub outcome: &'static str,
}

async fn read_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Option<Uuid>,
) -> Result<Vec<RepairItem>, RepairError> {
    let locator = crate::execution_input_eligibility::candidate_locator_sql(
        "runtime.task_spec #>> '{target,contentExternalId}'",
    );
    let query = format!(
        r#"
        SELECT task.task_id, lease.work_order_ref, lease.lease_ref, work.installation_ref,
               task.execution_state,
               encode(sha256(convert_to(jsonb_build_array(to_jsonb(task),to_jsonb(lease),
                   runtime.task_spec,to_jsonb(work),locator.source_ref,siblings.facts)::text,'UTF8')),'hex') AS snapshot_hash,
               lease.released_at IS NULL AND lease.expires_at>scope_001_now() AS live,
               locator.url IS NOT NULL AS has_input, siblings.unstarted, siblings.count,
               EXISTS(SELECT 1 FROM collection_detail_page_session session
                      WHERE session.work_order_ref=lease.work_order_ref) AS has_session,
               EXISTS(SELECT 1 FROM collection_work_order_material_target target
                      JOIN linggan_material_content content ON content.public_ref=target.content_public_ref
                      WHERE target.work_order_ref=lease.work_order_ref
                        AND content.content_external_id=runtime.task_spec #>> '{{target,contentExternalId}}') AS has_scope
        FROM collection_work_order_lease_task task
        JOIN collection_work_order_lease lease USING(lease_ref)
        JOIN collection_work_order work USING(work_order_ref)
        JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id
        LEFT JOIN LATERAL ({locator}) locator ON true
        CROSS JOIN LATERAL (
            SELECT count(*) AS count,
                bool_and(sibling.execution_state='pending' AND NOT EXISTS(
                    SELECT 1 FROM linggan_runtime_attempt attempt WHERE attempt.task_id=sibling.task_id)) AS unstarted,
                jsonb_agg(to_jsonb(sibling) ORDER BY sibling.task_id) AS facts
            FROM collection_work_order_lease_task sibling
            JOIN linggan_runtime_task sibling_runtime ON sibling_runtime.task_id=sibling.task_id
            WHERE sibling.lease_ref=task.lease_ref AND sibling_runtime.task_spec #>> '{{target,contentExternalId}}'
                =runtime.task_spec #>> '{{target,contentExternalId}}'
        ) siblings
        WHERE runtime.task_spec->>'platform'='xhs'
          AND runtime.task_spec #>> '{{capabilitiesRequested,0}}'='content_detail'
          AND ($1::uuid IS NULL OR task.task_id=$1)
        ORDER BY lease.issued_at DESC,task.task_id LIMIT 101
    "#
    );
    let rows = sqlx::query(AssertSqlSafe(query.as_str()))
        .bind(task_id)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let reason = if !row.get::<bool, _>("live") {
                "historical_lease_not_rewritten"
            } else if row.get::<bool, _>("has_input") {
                "execution_input_available"
            } else if !row.get::<bool, _>("unstarted") {
                "started_or_terminal_lane_preserved"
            } else if row.get::<bool, _>("has_session") {
                "prepared_delivery_preserved"
            } else if !row.get::<bool, _>("has_scope") {
                "scope_unverified"
            } else if row.get::<Option<Uuid>, _>("installation_ref").is_none() {
                "installation_unverified"
            } else {
                "missing_input_unstarted"
            };
            RepairItem {
                task_id: row.get("task_id"),
                work_order_ref: row.get("work_order_ref"),
                lease_ref: row.get("lease_ref"),
                installation_ref: row.get("installation_ref"),
                execution_state: row.get("execution_state"),
                snapshot_hash: row.get("snapshot_hash"),
                proposed_action: if reason == "missing_input_unstarted" {
                    "stop_missing_input"
                } else {
                    "none"
                }
                .into(),
                reason: reason.into(),
                affected_task_count: if reason == "missing_input_unstarted" {
                    row.get("count")
                } else {
                    0
                },
            }
        })
        .collect())
}

/// Default mode: only SELECTs in a read-only transaction; no repair facts are created.
pub async fn preview_collection_repair(database: &Database) -> Result<RepairPreview, RepairError> {
    preview_collection_repair_for_task(database, None).await
}

/// Explicit task selection reaches older rows even when the default preview is truncated.
pub async fn preview_collection_repair_for_task(
    database: &Database,
    task_id: Option<Uuid>,
) -> Result<RepairPreview, RepairError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let mut items = read_items(&mut tx, task_id).await?;
    let truncated = items.len() > 100;
    items.truncate(100);
    tx.commit().await?;
    let mut preview = RepairPreview {
        contract: "linggan.collection-repair.v1".into(), batch_id: Uuid::new_v4(),
        source: "scheduled_task_lease_and_accepted_discovery; browser_outbox=NOT_OBSERVED".into(),
        concurrency_precondition: "unchanged snapshot; no new lease, input, session, attempt or receipt; per-item serializable transaction".into(),
        items, truncated, preview_hash: String::new(),
    };
    preview.preview_hash = preview.hash()?;
    Ok(preview)
}

/// Explicit selection only. Revalidation and the existing domain stop are atomic per item.
pub async fn apply_collection_repair(
    database: &Database,
    preview: &RepairPreview,
    batch_id: Uuid,
    preview_hash: &str,
) -> Result<Vec<RepairOutcome>, RepairError> {
    if preview.contract != "linggan.collection-repair.v1"
        || preview.items.len() > 100
        || preview.batch_id != batch_id
        || preview.preview_hash != preview_hash
        || preview.hash()? != preview_hash
    {
        return Err(RepairError::InvalidPreview);
    }
    let mut outcomes = Vec::new();
    for expected in &preview.items {
        if expected.proposed_action != "stop_missing_input" {
            outcomes.push(RepairOutcome {
                task_id: expected.task_id,
                outcome: "excluded",
            });
            continue;
        }
        let result = apply_one(database, expected).await;
        let outcome = match result {
            Ok(true) => "stopped_missing_input",
            Ok(false) => "skipped_changed",
            // A contending writer wins. No forced overwrite and no automatic replay of a stale preview.
            Err(RepairError::Database(sqlx::Error::Database(error)))
                if matches!(
                    error.code().as_deref(),
                    Some("40001" | "40P01" | "55P03" | "57014")
                ) =>
            {
                "skipped_concurrent_change"
            }
            Err(_) => "failed_database",
        };
        outcomes.push(RepairOutcome {
            task_id: expected.task_id,
            outcome,
        });
    }
    Ok(outcomes)
}

async fn apply_one(database: &Database, expected: &RepairItem) -> Result<bool, RepairError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "SELECT installation_ref FROM plugin_installation WHERE installation_ref=$1 FOR UPDATE",
    )
    .bind(expected.installation_ref)
    .fetch_optional(&mut *tx)
    .await?;
    sqlx::query(
        "SELECT work_order_ref FROM collection_work_order WHERE work_order_ref=$1 FOR UPDATE",
    )
    .bind(expected.work_order_ref)
    .fetch_optional(&mut *tx)
    .await?;
    sqlx::query("SELECT lease_ref FROM collection_work_order_lease WHERE lease_ref=$1 FOR UPDATE")
        .bind(expected.lease_ref)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query("SELECT task_id FROM collection_work_order_lease_task WHERE lease_ref=$1 ORDER BY sequence_no FOR UPDATE")
        .bind(expected.lease_ref).fetch_all(&mut *tx).await?;
    let current = read_items(&mut tx, Some(expected.task_id)).await?;
    if current.first() != Some(expected) {
        tx.rollback().await?;
        return Ok(false);
    }
    let installation = expected
        .installation_ref
        .ok_or(RepairError::InvalidPreview)?;
    crate::dispatch::stop_member_for_missing_execution_input(
        &mut tx,
        installation,
        expected.task_id,
    )
    .await?;
    tx.commit().await?;
    Ok(true)
}
