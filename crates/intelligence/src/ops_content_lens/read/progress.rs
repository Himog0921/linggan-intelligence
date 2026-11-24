use super::super::{
    LensDefinitionError,
    label::{RunProgress, read_run},
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardStatus {
    pub enabled: bool,
    pub run: Option<BoardRun>,
    pub latest_snapshot_ref: Option<Uuid>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardRun {
    #[serde(flatten)]
    pub batches: RunProgress,
    pub in_progress: bool,
    /// Succeeded fraction of the current phase (primary or independent audit).
    pub progress: f64,
}

pub async fn board_status(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<BoardStatus, LensDefinitionError> {
    let latest_snapshot_ref = super::latest_snapshot_ref(
        db,
        &super::SnapshotScope {
            domain_ref: domain,
            ..Default::default()
        },
    )
    .await?;
    // A completed primary can still be waiting for its independent audit; settlement can
    // likewise lag the final audit shard. Prefer such a run over a newer terminal one.
    let run:Option<(Uuid,bool)>=sqlx::query_as("SELECT p.run_ref,
        (b.state='open' OR a.state='open' OR
          (b.state='completed' AND a.run_ref IS NULL AND p.title_count=(SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref=p.run_ref)) OR
          (a.state='completed' AND a.title_count=(SELECT count(*) FROM linggan_ops_lens_label l WHERE l.run_ref=a.run_ref)
           AND NOT EXISTS(SELECT FROM linggan_ops_lens_agreement g WHERE g.audit_run_ref=a.run_ref))) IS TRUE AS active
        FROM linggan_ops_lens_label_run p JOIN linggan_ai_batch_control b ON b.batch_ref=p.foundation_batch_ref
        LEFT JOIN LATERAL (SELECT r.run_ref,r.title_count,c.state FROM linggan_ops_lens_label_run r
            JOIN linggan_ai_batch_control c ON c.batch_ref=r.foundation_batch_ref WHERE r.primary_run_ref=p.run_ref
            ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 1) a ON true
        WHERE p.run_kind='primary' ORDER BY active DESC,p.created_at DESC,p.run_ref DESC LIMIT 1")
        .fetch_optional(db.pool()).await?;
    let run = if let Some((id, in_progress)) = run {
        let batches = read_run(db, id)
            .await
            .map_err(|_| LensDefinitionError::RunUnavailable)?;
        let phase = batches.audit.as_ref().unwrap_or(&batches.primary);
        let progress = if phase.total_shards > 0 {
            phase.succeeded_shards as f64 / phase.total_shards as f64
        } else {
            0.0
        };
        Some(BoardRun {
            batches,
            in_progress,
            progress,
        })
    } else {
        None
    };
    Ok(BoardStatus {
        enabled: true,
        run,
        latest_snapshot_ref,
    })
}
