use super::FoundationError;
use crate::ai_foundation::{batch_store::BatchKind, topic_research};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// A scan position only, not a second task ledger. Restarting safely rescans existing command IDs.
#[derive(Default)]
pub struct CursorState {
    cores: Option<(Uuid, Uuid)>,
    definitions: Option<(Uuid, Uuid)>,
    comments: Option<(Uuid, Uuid)>,
}
pub type Cursor = Option<CursorState>;
/// Queue a bounded page of existing Core versions; no model or collection call happens here.
pub async fn tick(db: &Database, cursor: &mut Cursor) -> Result<usize, FoundationError> {
    let position = cursor.get_or_insert_with(CursorState::default);
    let repaired = super::definition_repair::tick(db, &mut position.definitions).await?
        + super::comments::tick(db, &mut position.comments).await?;
    let rows = sqlx::query(include_str!("candidates.sql"))
        .bind(position.cores.map(|v| v.0))
        .bind(position.cores.map(|v| v.1))
        .bind(crate::topic_map_research_analysis::METHOD_VERSION)
        .fetch_all(db.pool())
        .await?;
    if rows.is_empty() {
        position.cores = None;
        return Ok(repaired);
    }
    let mut queued = repaired;
    for row in rows {
        let domain: Uuid = row.get("domain_ref");
        let core: Uuid = row.get("request_ref");
        position.cores = Some((domain, core));
        let request = topic_research::Start {
            command_ref: row.get("command_ref"),
            domain_ref: domain,
            work_refs: vec![row.get("work_ref")],
        };
        match topic_research::start_with_kind(db, &request, BatchKind::Automatic, Some(core)).await
        {
            Ok(_) => queued += 1,
            // One unavailable source/configuration must not drain the shared AI executor.
            Err(FoundationError::Database(error)) => return Err(error.into()),
            Err(_) => continue,
        }
    }
    Ok(queued)
}
