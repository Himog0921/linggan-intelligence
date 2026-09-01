//! Bounded single-work material detail projection.

use crate::material_projection::{
    MaterialLibraryItem, MaterialReadError, enrich_discovery_material, enrich_media_material,
    material_item,
};
use linggan_storage_postgres::Database;
use serde_json::{Map, Value};
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
    let row = sqlx::query(crate::material_query_sql::MATERIAL_PAGE_SQL)
        .bind(None::<String>)
        .bind(&as_of)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(Some(public_ref))
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let mut item = material_item(row, None);
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
    let engagement_current = read_engagement_current(&mut tx, public_ref, &as_of).await?;
    let detail_current = read_detail_current(&mut tx, public_ref, &as_of).await?;
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
    public_ref: Uuid,
    as_of: &str,
) -> Result<Value, sqlx::Error> {
    Ok(serde_json::json!({
        "likeCount":read_metric_current(tx, public_ref, as_of, EngagementMetric::Like).await?,
        "commentCount":read_metric_current(tx, public_ref, as_of, EngagementMetric::Comment).await?,
        "collectCount":read_metric_current(tx, public_ref, as_of, EngagementMetric::Collect).await?,
        "shareCount":read_metric_current(tx, public_ref, as_of, EngagementMetric::Share).await?
    }))
}

#[derive(Clone, Copy)]
enum EngagementMetric {
    Like,
    Comment,
    Collect,
    Share,
}

async fn read_metric_current(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    public_ref: Uuid,
    as_of: &str,
    metric: EngagementMetric,
) -> Result<Value, sqlx::Error> {
    let query = match metric {
        EngagementMetric::Like => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at, observation.created_at::text AS created_at,observation.like_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.like_count_state='KNOWN' AND observation.like_count IS NOT NULL ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC LIMIT 2"
        }
        EngagementMetric::Comment => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at, observation.created_at::text AS created_at,observation.comment_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.comment_count_state='KNOWN' AND observation.comment_count IS NOT NULL ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC LIMIT 2"
        }
        EngagementMetric::Collect => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at, observation.created_at::text AS created_at,observation.collect_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.collect_count_state='KNOWN' AND observation.collect_count IS NOT NULL ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC LIMIT 2"
        }
        EngagementMetric::Share => {
            "SELECT observation.package_ref,observation.source_lane,observation.observed_at, observation.created_at::text AS created_at,observation.share_count AS value FROM linggan_material_engagement_observation observation JOIN linggan_runtime_capture_package package USING(package_ref) WHERE observation.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz AND observation.share_count_state='KNOWN' AND observation.share_count IS NOT NULL ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC LIMIT 2"
        }
    };
    let known = sqlx::query(query)
        .bind(public_ref)
        .bind(as_of)
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(metric_point_row)
        .collect::<Vec<_>>();
    let Some(current) = known.first() else {
        return Ok(serde_json::json!({
            "state":"UNKNOWN","current":Value::Null,"previous":Value::Null,
            "delta":Value::Null,"deltaState":"UNKNOWN"
        }));
    };
    let previous = known.get(1).cloned().unwrap_or(Value::Null);
    let delta = previous
        .get("value")
        .and_then(Value::as_i64)
        .map(|previous_value| {
            current.get("value").and_then(Value::as_i64).unwrap_or(0) - previous_value
        });
    Ok(serde_json::json!({
        "state":"KNOWN","current":current,"previous":previous,
        "delta":delta,"deltaState":if delta.is_some() { "KNOWN" } else { "UNKNOWN" }
    }))
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

async fn read_detail_current(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    public_ref: Uuid,
    as_of: &str,
) -> Result<Value, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT detail.material_ref,detail.package_ref,detail.observed_at,detail.created_at::text AS created_at, \
                detail.title,detail.title_state,detail.body_text,detail.body_state, \
                detail.creator_display_name,detail.creator_display_name_state, \
                detail.published_at::text AS published_at,detail.published_at_source_text,detail.published_at_source_text_state, \
                detail.published_at_source_field,detail.published_at_source_kind,detail.published_at_precision, \
                detail.published_at_reference_observed_at::text AS published_at_reference_observed_at,detail.published_at_parser_version \
         FROM linggan_material_content_detail detail \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE detail.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz \
         ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC",
    )
    .bind(public_ref)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    let mut fields = Map::new();
    let mut exact_published = None;
    let mut source_text_published = None;
    for row in rows {
        let source = serde_json::json!({
            "materialRef":row.get::<Uuid,_>("material_ref"),
            "packageRef":row.get::<Uuid,_>("package_ref"),
            "observedAt":row.get::<String,_>("observed_at"),
            "recordedAt":row.get::<String,_>("created_at")
        });
        insert_known_field(
            &mut fields,
            "title",
            row.get::<Option<String>, _>("title"),
            row.get::<String, _>("title_state") == "KNOWN",
            &source,
        );
        insert_known_field(
            &mut fields,
            "body",
            row.get::<Option<String>, _>("body_text"),
            row.get::<String, _>("body_state") == "KNOWN",
            &source,
        );
        insert_known_field(
            &mut fields,
            "creator",
            row.get::<Option<String>, _>("creator_display_name"),
            row.get::<String, _>("creator_display_name_state") == "KNOWN",
            &source,
        );
        let published_at: Option<String> = row.get("published_at");
        let source_text: Option<String> = row.get("published_at_source_text");
        let source_text_known = row.get::<String, _>("published_at_source_text_state") == "KNOWN";
        if exact_published.is_none() && published_at.is_some() {
            exact_published = Some(serde_json::json!({
                "state":"KNOWN","value":published_at,"sourceText":source_text.clone(),
                "sourceField":row.get::<Option<String>,_>("published_at_source_field"),
                "sourceKind":row.get::<String,_>("published_at_source_kind"),
                "precision":row.get::<String,_>("published_at_precision"),
                "referenceObservedAt":row.get::<Option<String>,_>("published_at_reference_observed_at"),
                "parserVersion":row.get::<Option<String>,_>("published_at_parser_version"),
                "source":source.clone()
            }));
        }
        if source_text_published.is_none() && source_text_known && source_text.is_some() {
            source_text_published = Some(serde_json::json!({
                "state":"SOURCE_TEXT_ONLY","value":Value::Null,"sourceText":source_text,
                "sourceField":row.get::<Option<String>,_>("published_at_source_field"),
                "sourceKind":row.get::<String,_>("published_at_source_kind"),
                "precision":row.get::<String,_>("published_at_precision"),
                "referenceObservedAt":row.get::<Option<String>,_>("published_at_reference_observed_at"),
                "parserVersion":row.get::<Option<String>,_>("published_at_parser_version"),
                "source":source
            }));
        }
    }
    if let Some(published) = exact_published.or(source_text_published) {
        fields.insert("publishedAt".to_owned(), published);
    }
    for field in ["title", "body", "creator", "publishedAt"] {
        fields
            .entry(field.to_owned())
            .or_insert_with(|| serde_json::json!({"state":"UNKNOWN","value":Value::Null}));
    }
    Ok(Value::Object(fields))
}

fn insert_known_field(
    fields: &mut Map<String, Value>,
    name: &str,
    value: Option<String>,
    known: bool,
    source: &Value,
) {
    if known && value.is_some() && !fields.contains_key(name) {
        fields.insert(
            name.to_owned(),
            serde_json::json!({"state":"KNOWN","value":value,"source":source}),
        );
    }
}
