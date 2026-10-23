use super::{Error, METHOD_VERSION, Metric, Metrics, PAGE_SIZE, Scope, TraceEntry, TracePage, scope};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

pub async fn read_trace(
    database: &Database, request: &Scope, work: Uuid, offset: usize,
) -> Result<TracePage, Error> {
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    scope::require_work(&mut tx, request, work, &cutoff).await?;
    let mut observations = read_in(&mut tx, request, work, &cutoff, PAGE_SIZE + 1, offset).await?;
    let next_offset = (observations.len() > PAGE_SIZE).then_some(offset + PAGE_SIZE);
    observations.truncate(PAGE_SIZE);
    tx.commit().await?;
    Ok(TracePage { work_ref: work, knowledge_cutoff: cutoff, observations,
        next_offset, method_version: METHOD_VERSION })
}

pub(crate) async fn read_in(
    tx: &mut Transaction<'_, Postgres>, request: &Scope, work: Uuid,
    cutoff: &str, limit: usize, offset: usize,
) -> Result<Vec<TraceEntry>, Error> {
    if limit > 2001 { return Err(Error::Invalid("trace_limit_out_of_range")); }
    let rows = sqlx::query(include_str!("observations.sql"))
        .bind(request.target_ref).bind(request.domain_ref).bind(work).bind(cutoff)
        .bind(i64::try_from(limit).unwrap_or(51)).bind(scope::check_offset(offset)?)
        .fetch_all(&mut **tx).await?;
    rows.into_iter().map(|row| {
        let source: String = row.try_get("source_kind")?;
        let observed: String = row.try_get("observed_text")?;
        let epoch: i64 = row.try_get("observed_epoch_ms")?;
        let reference: Uuid = row.try_get("material_ref")?;
        let metrics = metrics_from_row(&row, &reference.to_string(), &observed, epoch, &source)?;
        Ok(TraceEntry { observation_ref: reference.to_string(), package_ref: row.try_get("package_ref")?,
            round_ref: row.try_get("attempt_id")?, target_ref: row.try_get("target_ref")?,
            target_name: row.try_get("target_name")?, rule_slot: row.try_get("slot_key")?,
            source_kind: source, observed_at: observed, observed_epoch_ms: epoch,
            accepted_at: row.try_get("accepted_text")?, metrics, position: row.try_get("position")?,
            late_arrival: row.try_get("late_arrival")? })
    }).collect()
}

pub(crate) fn metrics_from_row(
    row: &PgRow, reference: &str, observed: &str, epoch: i64, source: &str,
) -> Result<Metrics, Error> {
    let raw: Option<Value> = row.try_get("raw")?;
    let raw = raw.unwrap_or(Value::Null);
    let value = |column: &str, keys: &[&str], text_keys: &[&str]| -> Result<Metric, Error> {
        let count: Option<i64> = row.try_get(column)?;
        let state: String = row.try_get(format!("{column}_state").as_str())?;
        let known = state == "KNOWN" && count.is_some_and(|v| v >= 0);
        Ok(Metric { value: if known { count } else { None },
            state: if known { "known" } else { "unknown" }.into(),
            observation_ref: Some(reference.into()), observed_at: Some(observed.into()),
            observed_epoch_ms: Some(epoch), source_kind: Some(source.into()),
            precision: if known { precision(&raw, keys, text_keys) } else { "unknown" }.into() })
    };
    Ok(Metrics {
        likes: value("like_count", &["likeCount","likes","likedCount"], &["likeText","likeCountText"] )?,
        comments: value("comment_count", &["publicCommentCount","commentCount","comments"], &["commentText","commentCountText"] )?,
        collects: value("collect_count", &["collectCount","collects","collectedCount"], &["collectText","collectCountText"] )?,
        shares: value("share_count", &["shareCount","shares"], &["shareText","shareCountText"] )?,
    })
}

/// Rounded visible counts remain readable, but are never subtraction-grade facts.
fn precision(raw: &Value, keys: &[&str], text_keys: &[&str]) -> &'static str {
    for key in text_keys {
        if let Some(text) = raw.get(*key).and_then(Value::as_str).map(str::trim) {
            if !text.is_empty() && !text.bytes().all(|b| b.is_ascii_digit()) { return "reported"; }
        }
    }
    for key in keys {
        if let Some(value) = raw.get(*key) {
            if value.as_i64().is_some_and(|v| v >= 0) { return "exact"; }
            if let Some(text) = value.as_str().map(str::trim) {
                if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) { return "exact"; }
                return "reported";
            }
        }
    }
    "unknown"
}
