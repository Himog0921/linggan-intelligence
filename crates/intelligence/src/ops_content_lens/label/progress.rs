//! Read the two foundation batches behind one labeling run. No separate execution state.
use crate::ai_foundation::store::FoundationError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub run_ref: Uuid,
    pub batch_ref: Uuid,
    pub state: String,
    pub total_shards: i64,
    pub succeeded_shards: i64,
    pub failed_shards: i64,
    pub skipped_shards: i64,
    pub cancelled_shards: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgress {
    pub primary: BatchProgress,
    pub audit: Option<BatchProgress>,
}

pub async fn read_run(db: &Database, primary: Uuid) -> Result<RunProgress, FoundationError> {
    let rows=sqlx::query(
        "SELECT r.run_ref,b.batch_ref,b.state,count(*) AS total_shards,
            count(*) FILTER(WHERE c.state='succeeded') AS succeeded_shards,
            count(*) FILTER(WHERE c.state='failed') AS failed_shards,
            count(*) FILTER(WHERE c.state='skipped') AS skipped_shards,
            count(*) FILTER(WHERE c.state='cancelled') AS cancelled_shards
         FROM linggan_ops_lens_label_run r JOIN linggan_ai_batch_control b ON b.batch_ref=r.foundation_batch_ref
         JOIN linggan_ai_batch_item i ON i.batch_ref=b.batch_ref JOIN linggan_ai_batch_item_control c USING(item_ref)
         WHERE (r.run_kind='primary' AND r.run_ref=$1) OR r.primary_run_ref=$1
         GROUP BY r.run_ref,b.batch_ref ORDER BY (r.run_kind='primary'),r.created_at,r.run_ref")
        .bind(primary).fetch_all(db.pool()).await?;
    let mut batches = rows
        .iter()
        .map(|r| BatchProgress {
            run_ref: r.get("run_ref"),
            batch_ref: r.get("batch_ref"),
            state: r.get("state"),
            total_shards: r.get("total_shards"),
            succeeded_shards: r.get("succeeded_shards"),
            failed_shards: r.get("failed_shards"),
            skipped_shards: r.get("skipped_shards"),
            cancelled_shards: r.get("cancelled_shards"),
        })
        .collect::<Vec<_>>();
    let primary = batches.pop().ok_or(FoundationError::NotFound)?;
    Ok(RunProgress {
        primary,
        audit: batches.pop(),
    })
}
