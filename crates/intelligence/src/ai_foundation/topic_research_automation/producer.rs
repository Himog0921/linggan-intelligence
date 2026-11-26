use super::FoundationError;
use crate::ai_foundation::{batch_store::BatchKind, selected_collection_scope, topic_research};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// A scan position only, not a second task ledger. Restarting safely rescans existing command IDs.
#[derive(Default)]
pub struct CursorState {
    collection_run_ref: Option<Uuid>,
    recent_cores: Option<(Uuid, Uuid)>,
    historical_cores: Option<(Uuid, Uuid)>,
    definitions: Option<(Uuid, Uuid)>,
    comments: Option<(Uuid, Uuid)>,
    retries: Option<Uuid>,
}
pub type Cursor = Option<CursorState>;
/// Queue a bounded page of existing Core versions; no model or collection call happens here.
pub async fn tick(db: &Database, cursor: &mut Cursor) -> Result<usize, FoundationError> {
    let Some(selected) = selected_collection_scope::filter(db).await? else {
        *cursor = None;
        return Ok(0);
    };
    let position = cursor.get_or_insert_with(CursorState::default);
    if position.collection_run_ref != selected.collection_run_ref {
        *position = CursorState {
            collection_run_ref: selected.collection_run_ref,
            ..CursorState::default()
        };
    }
    let repaired = if selected.collection_run_ref.is_none() {
        super::definition_repair::tick(db, &mut position.definitions).await?
            + super::comments::tick(db, &mut position.comments).await?
    } else {
        0
    };
    let repaired =
        repaired + retry_confirmed_invalid_outputs(db, &mut position.retries, &selected).await?;
    let rows = sqlx::query(include_str!("candidates.sql"))
        .bind(position.recent_cores.map(|v| v.0))
        .bind(position.recent_cores.map(|v| v.1))
        .bind(crate::topic_map_research_analysis::METHOD_VERSION)
        .bind(selected.domain)
        .bind(selected.works.as_deref())
        .bind(selected.collection_run_ref)
        .bind(position.historical_cores.map(|v| v.0))
        .bind(position.historical_cores.map(|v| v.1))
        .fetch_all(db.pool())
        .await?;
    // Empty lanes wrap independently. A stopped worker can safely rescan both.
    let mut recent_seen = false;
    let mut historical_seen = false;
    let mut queued = repaired;
    for row in rows {
        let domain: Uuid = row.get("domain_ref");
        let core: Uuid = row.get("request_ref");
        let recent: bool = row.get("recent");
        if recent {
            recent_seen = true;
        } else {
            historical_seen = true;
        }
        let request = topic_research::Start {
            command_ref: row.get("command_ref"),
            domain_ref: domain,
            work_refs: vec![row.get("work_ref")],
        };
        match topic_research::start_with_kind(db, &request, BatchKind::Automatic, Some(core)).await
        {
            Ok(_) => queued += 1,
            // A full queue must not move the scan past work it could not admit.
            Err(FoundationError::AdmissionDenied(reasons))
                if reasons
                    .iter()
                    .any(|reason| reason == "topic_automatic_queue_full") =>
            {
                continue;
            }
            Err(FoundationError::Database(error)) => return Err(error.into()),
            Err(_) => {}
        }
        if recent {
            position.recent_cores = Some((domain, core));
        } else {
            position.historical_cores = Some((domain, core));
        }
    }
    if !recent_seen {
        position.recent_cores = None;
    }
    if !historical_seen {
        position.historical_cores = None;
    }
    Ok(queued)
}

async fn retry_confirmed_invalid_outputs(
    db: &Database,
    position: &mut Option<Uuid>,
    selected: &selected_collection_scope::Filter,
) -> Result<usize, FoundationError> {
    let repairs = sqlx::query(super::RETRY_CANDIDATES)
        .bind(Option::<Uuid>::None)
        .bind(crate::topic_map_research_analysis::METHOD_VERSION)
        .bind(*position)
        .bind(selected.collection_run_ref)
        .bind(selected.domain)
        .bind(selected.works.as_deref())
        .fetch_all(db.pool())
        .await?;
    if repairs.is_empty() {
        *position = None;
    }
    let mut queued = 0;
    for row in repairs {
        let window = row.get("window_ref");
        *position = Some(window);
        let request = topic_research::Start {
            command_ref: row.get("command_ref"),
            domain_ref: row.get("domain_ref"),
            work_refs: vec![row.get("work_ref")],
        };
        match topic_research::retry_automatic(db, &request, row.get("core_ref"), window).await {
            Ok(_) => queued += 1,
            Err(FoundationError::Database(error)) => return Err(error.into()),
            Err(_) => {}
        }
    }
    Ok(queued)
}
