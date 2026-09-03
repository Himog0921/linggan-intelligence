//! Bounded single-work material detail projection.

use crate::material_projection::{
    MaterialLibraryItem, MaterialReadError, enrich_discovery_material, enrich_media_material,
    material_item,
};
use crate::work_resource_current::{
    WorkResourceCurrent, WorkResourceCurrentSource, read_work_resource_currents,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub async fn read_material_detail(
    database: &Database,
    public_ref: Uuid,
) -> Result<Option<MaterialLibraryItem>, MaterialReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let current = read_work_resource_currents(&mut tx, &[public_ref], &as_of)
        .await?
        .pop();
    let Some(current) = current else {
        tx.commit().await?;
        return Ok(None);
    };
    let mut item = material_item(&current, None);
    enrich_discovery_material(&mut tx, &mut item, &as_of).await?;
    crate::material_social_read::enrich(&mut tx, &mut item, None, &as_of).await?;
    enrich_media_material(&mut tx, &mut item, &as_of).await?;
    // The timeline is a bounded presentation window. It deliberately takes the most recent
    // rows and is re-ordered below for chronological display; it is never the authority for a
    // metric's current value because a metric can remain UNKNOWN for more than one page.
    let engagement_rows = sqlx::query(
        "SELECT observation.package_ref,observation.source_lane,observation.observed_at,observation.created_at::text AS created_at,observation.like_count,observation.like_count_state,observation.comment_count,observation.comment_count_state, \
                collect_count,collect_count_state,share_count,share_count_state \
         FROM linggan_material_engagement_observation observation \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz \
         ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC LIMIT 100",
    )
    .bind(public_ref)
    .bind(&as_of)
    .fetch_all(&mut *tx)
    .await?;
    let mut engagement_rows = engagement_rows
        .iter()
        .map(engagement_row)
        .collect::<Vec<_>>();
    engagement_rows.reverse();
    let engagement_current = read_engagement_current(&mut tx, &current, &as_of).await?;
    let detail_current = read_detail_current(&current);
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert(
            "engagementTimeline".to_owned(),
            Value::Array(engagement_rows.clone()),
        );
        inspector.insert(
            "engagementCurrent".to_owned(),
            serde_json::json!({
                "selection":"LATEST_KNOWN_OBSERVATION_PER_METRIC",
                "metrics":engagement_current
            }),
        );
        inspector.insert("detailCurrent".to_owned(), detail_current);
    }
    tx.commit().await?;
    Ok(Some(item))
}

fn engagement_row(row: &sqlx::postgres::PgRow) -> Value {
    serde_json::json!({
        "packageRef":row.get::<Uuid,_>("package_ref"),
        "sourceLane":row.get::<String,_>("source_lane"),
        "observedAt":row.get::<String,_>("observed_at"),
        "recordedAt":row.get::<String,_>("created_at"),
        "likeCount":row.get::<Option<i64>,_>("like_count"),
        "likeCountState":row.get::<String,_>("like_count_state"),
        "commentCount":row.get::<Option<i64>,_>("comment_count"),
        "commentCountState":row.get::<String,_>("comment_count_state"),
        "collectCount":row.get::<Option<i64>,_>("collect_count"),
        "collectCountState":row.get::<String,_>("collect_count_state"),
        "shareCount":row.get::<Option<i64>,_>("share_count"),
        "shareCountState":row.get::<String,_>("share_count_state"),
    })
}

/// Current and previous are selected from all accepted history, independently of the bounded
/// timeline. A long run of later UNKNOWN observations must not turn a retained known fact into
/// a stale value or make the latest valid fact disappear after the timeline's display limit.
async fn read_engagement_current(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    current: &WorkResourceCurrent,
    as_of: &str,
) -> Result<Value, sqlx::Error> {
    Ok(serde_json::json!({
        "likeCount":read_metric_history(tx, current.public_ref, as_of, EngagementMetric::Like, current.like_count, &current.like_source).await?,
        "commentCount":read_metric_history(tx, current.public_ref, as_of, EngagementMetric::Comment, current.comment_count, &current.comment_source).await?,
        "collectCount":read_metric_history(tx, current.public_ref, as_of, EngagementMetric::Collect, current.collect_count, &current.collect_source).await?,
        "shareCount":read_metric_history(tx, current.public_ref, as_of, EngagementMetric::Share, current.share_count, &current.share_source).await?
    }))
}

#[derive(Clone, Copy)]
enum EngagementMetric {
    Like,
    Comment,
    Collect,
    Share,
}

async fn read_metric_history(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    public_ref: Uuid,
    as_of: &str,
    metric: EngagementMetric,
    current_value: Option<i64>,
    current_source: &WorkResourceCurrentSource,
) -> Result<Value, sqlx::Error> {
    let Some(current_value) = current_value else {
        return Ok(serde_json::json!({
            "state":"UNKNOWN","current":Value::Null,"previous":Value::Null,
            "delta":Value::Null,"deltaState":"UNKNOWN"
        }));
    };
    let query = match metric {
        EngagementMetric::Like => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at,observation.created_at::text AS created_at,observation.like_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.like_count_state='KNOWN' AND observation.like_count IS NOT NULL AND NOT (observation.package_ref=$3 AND observation.source_lane=$4) ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC LIMIT 1"
        }
        EngagementMetric::Comment => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at,observation.created_at::text AS created_at,observation.comment_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.comment_count_state='KNOWN' AND observation.comment_count IS NOT NULL AND NOT (observation.package_ref=$3 AND observation.source_lane=$4) ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC LIMIT 1"
        }
        EngagementMetric::Collect => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at,observation.created_at::text AS created_at,observation.collect_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.collect_count_state='KNOWN' AND observation.collect_count IS NOT NULL AND NOT (observation.package_ref=$3 AND observation.source_lane=$4) ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC LIMIT 1"
        }
        EngagementMetric::Share => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at,observation.created_at::text AS created_at,observation.share_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.share_count_state='KNOWN' AND observation.share_count IS NOT NULL AND NOT (observation.package_ref=$3 AND observation.source_lane=$4) ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC LIMIT 1"
        }
    };
    let (Some(current_package_ref), Some(current_source_lane)) = (
        current_source.package_ref,
        current_source.source_lane.as_deref(),
    ) else {
        return Err(sqlx::Error::Protocol(
            "Work Resource Current returned a KNOWN engagement value without provenance".into(),
        ));
    };
    let previous = sqlx::query(query)
        .bind(public_ref)
        .bind(as_of)
        .bind(current_package_ref)
        .bind(current_source_lane)
        .fetch_optional(&mut **tx)
        .await?
        .as_ref()
        .map(metric_point_row)
        .unwrap_or(Value::Null);
    let current = metric_point(current_value, current_source);
    let delta = previous
        .get("value")
        .and_then(Value::as_i64)
        .map(|previous_value| current_value - previous_value);
    Ok(serde_json::json!({
        "state":"KNOWN","current":current,"previous":previous,
        "delta":delta,"deltaState":if delta.is_some() { "KNOWN" } else { "UNKNOWN" }
    }))
}

fn metric_point(value: i64, source: &WorkResourceCurrentSource) -> Value {
    serde_json::json!({
        "value":value,
        "packageRef":source.package_ref,
        "sourceLane":source.source_lane,
        "observedAt":source.observed_at,
        "recordedAt":source.recorded_at
    })
}

fn metric_point_row(row: &sqlx::postgres::PgRow) -> Value {
    serde_json::json!({
        "value":row.get::<i64,_>("value"),
        "packageRef":row.get::<Uuid,_>("package_ref"),
        "sourceLane":row.get::<String,_>("source_lane"),
        "observedAt":row.get::<String,_>("observed_at"),
        "recordedAt":row.get::<String,_>("created_at")
    })
}

fn read_detail_current(current: &WorkResourceCurrent) -> Value {
    serde_json::json!({
        "title":text_current(&current.title, &current.title_state, &current.title_source),
        "body":text_current(&current.body_text, &current.body_state, &current.body_source),
        "creator":text_current(&current.creator_display_name, &current.creator_display_name_state, &current.creator_source),
        "publishedAt":published_current(current)
    })
}

fn text_current(value: &Option<String>, state: &str, source: &WorkResourceCurrentSource) -> Value {
    match value {
        Some(value) if state == "KNOWN" => serde_json::json!({
            "state":"KNOWN","value":value,"source":detail_source(source)
        }),
        _ => serde_json::json!({"state":"UNKNOWN","value":Value::Null}),
    }
}

fn published_current(current: &WorkResourceCurrent) -> Value {
    let state = if current.published_at.is_some() {
        "KNOWN"
    } else if current.published_at_source_text.is_some() {
        "SOURCE_TEXT_ONLY"
    } else {
        return serde_json::json!({"state":"UNKNOWN","value":Value::Null});
    };
    serde_json::json!({
        "state":state,"value":current.published_at,
        "sourceText":current.published_at_source_text,
        "sourceField":current.published_at_source_field,
        "sourceKind":current.published_at_source_kind,
        "precision":current.published_at_precision,
        "referenceObservedAt":current.published_at_reference_observed_at,
        "parserVersion":current.published_at_parser_version,
        "source":detail_source(&current.published_source)
    })
}

fn detail_source(source: &WorkResourceCurrentSource) -> Value {
    serde_json::json!({
        "materialRef":source.material_ref,
        "packageRef":source.package_ref,
        "observedAt":source.observed_at,
        "recordedAt":source.recorded_at
    })
}
