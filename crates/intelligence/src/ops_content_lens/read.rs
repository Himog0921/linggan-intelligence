//! Administrator reads of immutable snapshots; only publication display fields are live.
mod members;
mod progress;
mod spot;
pub use members::{PageRequest, publications};
pub use progress::{BoardStatus, board_status};
pub use spot::spot_check;

use super::{AnalysisInput, LensDefinitionError, MetricDefinition, SnapshotScope};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotLookup {
    pub snapshot_ref: Option<Uuid>,
    pub domain_ref: Option<Uuid>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Freshness {
    pub stale_days: Option<i32>,
    pub is_stale: Option<bool>,
}

#[derive(Serialize)]
pub struct SnapshotRead {
    pub snapshot: Option<Value>,
    pub freshness: Option<Freshness>,
}

pub async fn read_snapshot(
    db: &Database,
    request: SnapshotLookup,
) -> Result<SnapshotRead, LensDefinitionError> {
    let scope = SnapshotScope {
        domain_ref: request.domain_ref,
        from: request.from,
        to: request.to,
    };
    if request.snapshot_ref.is_some()
        && (scope.domain_ref.is_some() || scope.from.is_some() || scope.to.is_some())
    {
        return Err(LensDefinitionError::InvalidScope);
    }
    scope.validate()?;
    // Freshness is relative to the day of reading. The recorded body and its historical
    // staleDays remain exactly as generated; the range always matches all three bounds.
    let result:Option<(Value,Option<i32>,i64)>=sqlx::query_as("SELECT s.body,
        (transaction_timestamp() AT TIME ZONE 'Asia/Shanghai')::date - (s.body#>>'{quality,latestPublishedDate}')::date,
        (d.body->>'stale_after_days')::bigint FROM linggan_ops_lens_snapshot s
        JOIN linggan_ops_lens_definition d ON d.definition_ref=s.metric_ref
        WHERE s.snapshot_ref=$1 OR ($1 IS NULL AND s.analysis_input->>'domainRef' IS NOT DISTINCT FROM $2::text
            AND s.analysis_input->>'from' IS NOT DISTINCT FROM $3 AND s.analysis_input->>'to' IS NOT DISTINCT FROM $4)
        ORDER BY s.as_of DESC,s.created_at DESC,s.snapshot_ref DESC LIMIT 1")
        .bind(request.snapshot_ref).bind(scope.domain_ref.map(|d|d.to_string())).bind(scope.from).bind(scope.to)
        .fetch_optional(db.pool()).await?;
    match result {
        Some((body, stale_days, threshold)) => Ok(SnapshotRead {
            snapshot: Some(body),
            freshness: Some(Freshness {
                stale_days,
                is_stale: stale_days.map(|n| i64::from(n) > threshold),
            }),
        }),
        None if request.snapshot_ref.is_some() => Err(LensDefinitionError::SnapshotNotFound),
        None => Ok(SnapshotRead {
            snapshot: None,
            freshness: None,
        }),
    }
}

async fn frozen(
    db: &Database,
    id: Uuid,
) -> Result<(AnalysisInput, MetricDefinition), LensDefinitionError> {
    let (Json(input),Json(metric)) = sqlx::query_as("SELECT s.analysis_input,d.body
        FROM linggan_ops_lens_snapshot s JOIN linggan_ops_lens_definition d ON d.definition_ref=s.metric_ref
        WHERE s.snapshot_ref=$1").bind(id).fetch_optional(db.pool()).await?
        .ok_or(LensDefinitionError::SnapshotNotFound)?;
    Ok((input, metric))
}
