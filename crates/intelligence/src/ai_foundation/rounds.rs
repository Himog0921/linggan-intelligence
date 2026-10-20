//! Evaluate settled item groups in order. The database commits facts and a system-off event together.
use super::store::FoundationError;
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn evaluate_in(
    tx: &mut Transaction<'_, Postgres>,
    batch: Uuid,
) -> Result<Vec<Uuid>, FoundationError> {
    let mut evaluated = Vec::new();
    let maximum = AI_LIMITS.max_batch_items.div_ceil(AI_LIMITS.round_size);
    for _ in 0..maximum {
        let next: Option<Uuid> = sqlx::query_scalar("SELECT linggan_ai_evaluate_next_round($1)")
            .bind(batch)
            .fetch_one(&mut **tx)
            .await?;
        let Some(next) = next else {
            break;
        };
        evaluated.push(next);
    }
    Ok(evaluated)
}
pub async fn evaluate_ready_rounds(
    db: &Database,
    batch: Uuid,
) -> Result<Vec<Uuid>, FoundationError> {
    let mut tx = db.pool().begin().await?;
    let result = evaluate_in(&mut tx, batch).await?;
    tx.commit().await?;
    Ok(result)
}
