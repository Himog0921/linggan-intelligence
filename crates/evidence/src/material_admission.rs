//! Qualification of accepted producer records into typed material observations.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) async fn insert_typed_materials(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<(), ProducerRuntimeError> {
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_material_lane_observation') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        if package.package_kind() == "media_slots" {
            crate::material_media::insert_legacy_only(tx, package).await?;
        }
        return Ok(());
    }
    let accepted_ordinals = sqlx::query_scalar::<_, i32>(
        "SELECT record_ordinal FROM linggan_runtime_record_disposition \
         WHERE package_ref=$1 AND disposition <> 'quarantined'",
    )
    .bind(package.package_ref())
    .fetch_all(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?
    .into_iter()
    .collect::<HashSet<_>>();
    match package.package_kind() {
        "discovery_search" | "profile_discovery" => {
            insert_discovery_records(tx, package, &accepted_ordinals).await?
        }
        "content_detail" => insert_content_records(tx, package, &accepted_ordinals).await?,
        "comments" => insert_comment_records(tx, package, false, &accepted_ordinals).await?,
        "replies" => insert_comment_records(tx, package, true, &accepted_ordinals).await?,
        "author_profile" => insert_author_records(tx, package, &accepted_ordinals).await?,
        "media_slots" | "media_bytes" => {
            crate::material_media::insert(tx, package, &accepted_ordinals).await?
        }
        _ => {}
    }
    Ok(())
}

async fn insert_discovery_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        let Some(finding) = discovery_record(package, record) else {
            continue;
        };
        let content_public_ref = ensure_content(tx, package, finding.content_id).await?;
        let title = exact_string(finding.payload, "title");
        let creator = exact_string(finding.payload, "authorName");
        let published = exact_scalar_text(finding.payload, "publishedAtText");
        let cover = observed_cover_url(finding.payload);
        let likes = exact_nonnegative_count(finding.payload, &["likeCount", "likes"]);
        let comments = exact_nonnegative_count(finding.payload, &["commentCount", "comments"]);
        let collects = exact_nonnegative_count(finding.payload, &["collectCount", "collects"]);
        let shares = exact_nonnegative_count(finding.payload, &["shareCount", "shares"]);
        let material_ref = Uuid::new_v4();
        let record_ordinal = i32::try_from(ordinal).expect("package record count is bounded");
        sqlx::query("INSERT INTO linggan_material_discovery_finding (material_ref,content_public_ref,package_ref,record_ordinal,discovery_kind,result_position,observed_at,title,title_state,creator_display_name,creator_state,published_at_source_text,published_at_source_text_state,cover_source_url,cover_source_state,like_count,like_count_state,comment_count,comment_count_state,collect_count,collect_count_state,share_count,share_count_state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23)")
            .bind(material_ref).bind(content_public_ref).bind(package.package_ref()).bind(record_ordinal)
            .bind(package.package_kind()).bind(finding.result_position).bind(package.observed_at()).bind(title).bind(known_state(title))
            .bind(creator).bind(known_state(creator)).bind(published.as_deref()).bind(known_state(published.as_deref()))
            .bind(cover).bind(known_state(cover)).bind(likes).bind(known_state(likes))
            .bind(comments).bind(known_state(comments)).bind(collects).bind(known_state(collects))
            .bind(shares).bind(known_state(shares))
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        if let Some(cover_source_url) = cover {
            crate::media_acquisition::project_discovery_cover(
                tx,
                material_ref,
                content_public_ref,
                package.package_ref(),
                record_ordinal,
                package.platform(),
                finding.content_id,
                package.observed_at(),
                cover_source_url,
            )
            .await?;
        }
    }
    Ok(())
}

async fn insert_content_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(expected_content_id) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    let content_public_ref = ensure_content(tx, package, expected_content_id).await?;
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        let Some(detail) = content_detail_record(package, record) else {
            continue;
        };
        if detail.content_id != expected_content_id {
            continue;
        }
        insert_content_detail(tx, package, ordinal, detail, content_public_ref).await?;
        retained += 1;
    }
    insert_lane_observation(
        tx,
        package,
        "detail",
        Some(content_public_ref),
        None,
        retained,
    )
    .await
}

async fn insert_comment_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    replies: bool,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(content_id) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    let content_public_ref = ensure_content(tx, package, content_id).await?;
    let expected_kind = if replies { "reply" } else { "comment" };
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        if record.get("kind").and_then(Value::as_str) != Some(expected_kind) {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        if exact_string(payload, "noteId") != Some(content_id) {
            continue;
        }
        let Some(comment_id) = exact_string(payload, "commentId") else {
            continue;
        };
        let (root_id, parent_id, parent_source) = if replies {
            let Some(root) = exact_string(payload, "rootCommentId") else {
                continue;
            };
            let parent = exact_string(payload, "parentCommentId")
                .map(|value| (value, "parentCommentId"))
                .or_else(|| {
                    exact_string(payload, "replyToCommentId")
                        .map(|value| (value, "replyToCommentId"))
                });
            let Some((parent, source)) = parent else {
                continue;
            };
            (root, Some(parent), Some(source))
        } else {
            if ["rootCommentId", "parentCommentId", "replyToCommentId"]
                .iter()
                .any(|key| exact_string(payload, key).is_some())
            {
                continue;
            }
            (comment_id, None, None)
        };
        let body = exact_string(payload, "text");
        sqlx::query("INSERT INTO linggan_material_comment (material_ref,content_public_ref,package_ref,record_ordinal,comment_external_id,root_comment_external_id,parent_comment_external_id,parent_identity_source_field,is_reply,body_text,body_state,author_external_id,author_display_name,observed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
            .bind(Uuid::new_v4()).bind(content_public_ref).bind(package.package_ref())
            .bind(i32::try_from(ordinal).expect("package record count is bounded"))
            .bind(comment_id).bind(root_id).bind(parent_id).bind(parent_source).bind(replies)
            .bind(body).bind(known_state(body)).bind(exact_string(payload, "authorId"))
            .bind(exact_string(payload, "authorName")).bind(package.observed_at())
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        retained += 1;
    }
    insert_lane_observation(
        tx,
        package,
        if replies { "replies" } else { "comments" },
        Some(content_public_ref),
        None,
        retained,
    )
    .await
}

async fn insert_author_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    accepted_ordinals: &HashSet<i32>,
) -> Result<(), ProducerRuntimeError> {
    let Some(author_id) = target_string(package, "authorExternalId") else {
        return Ok(());
    };
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
        if !accepted_ordinals.contains(&i32::try_from(ordinal).expect("record count is bounded")) {
            continue;
        }
        if record.get("kind").and_then(Value::as_str) != Some("author_profile")
            || record
                .pointer("/sourceObject/externalId")
                .and_then(Value::as_str)
                .map(str::trim)
                != Some(author_id)
        {
            continue;
        }
        let Some(payload) = record.get("payload").and_then(Value::as_object) else {
            continue;
        };
        if exact_string(payload, "userId").or_else(|| exact_string(payload, "id"))
            != Some(author_id)
        {
            continue;
        }
        let display_name = exact_string(payload, "nickname");
        let biography = exact_string(payload, "desc");
        let follower_count = payload
            .get("fans")
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0);
        sqlx::query("INSERT INTO linggan_material_author_profile (material_ref,platform,author_external_id,package_ref,record_ordinal,observed_at,display_name,display_name_state,biography,biography_state,follower_count,follower_count_state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
            .bind(Uuid::new_v4()).bind(package.platform()).bind(author_id).bind(package.package_ref())
            .bind(i32::try_from(ordinal).expect("package record count is bounded")).bind(package.observed_at())
            .bind(display_name).bind(known_state(display_name)).bind(biography).bind(known_state(biography))
            .bind(follower_count).bind(known_state(follower_count))
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
        retained += 1;
    }
    insert_lane_observation(tx, package, "author", None, Some(author_id), retained).await
}

pub(crate) async fn insert_lane_observation(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    lane: &str,
    content_public_ref: Option<Uuid>,
    author_external_id: Option<&str>,
    retained: i32,
) -> Result<(), ProducerRuntimeError> {
    let capability = match lane {
        "detail" => "content_detail",
        other => other,
    };
    let layer =
        crate::material_contract_validation::unique_coverage_layer(package.coverage(), capability);
    let count = |field| {
        layer
            .and_then(|value| value.get(field))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
    };
    sqlx::query("INSERT INTO linggan_material_lane_observation (package_ref,lane,content_public_ref,author_external_id,observed,producer_acquired,retained,failed,known_unattempted,unknown_count,maximum_quota,stopped_reason,observed_at) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,(task.task_spec->>'maximumQuota')::integer,$11,$12 FROM linggan_runtime_capture_package package JOIN linggan_runtime_task task ON task.task_id = package.task_id WHERE package.package_ref = $1")
        .bind(package.package_ref()).bind(lane).bind(content_public_ref).bind(author_external_id)
        .bind(count("observed")).bind(count("acquired")).bind(retained).bind(count("failed")).bind(count("notAttempted")).bind(count("unknown"))
        .bind(layer.and_then(|value| value.get("stoppedReason")).and_then(Value::as_str)).bind(package.observed_at())
        .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

fn target_string<'a>(package: &'a ProducerCapturePackage, field: &str) -> Option<&'a str> {
    package
        .coverage()
        .pointer(&format!("/target/{field}"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) async fn ensure_content(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    content_id: &str,
) -> Result<Uuid, ProducerRuntimeError> {
    sqlx::query("INSERT INTO linggan_material_content (platform,content_external_id,public_ref,first_package_ref) VALUES ($1,$2,$3,$4) ON CONFLICT (platform,content_external_id) DO NOTHING")
        .bind(package.platform()).bind(content_id).bind(Uuid::new_v4()).bind(package.package_ref())
        .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE platform = $1 AND content_external_id = $2")
        .bind(package.platform()).bind(content_id).fetch_one(&mut **tx).await.map_err(ProducerRuntimeError::Internal)
}

async fn insert_content_detail(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    ordinal: usize,
    detail: ContentDetailRecord<'_>,
    content_public_ref: Uuid,
) -> Result<(), ProducerRuntimeError> {
    let title = exact_string(detail.payload, "title");
    let body = exact_string(detail.payload, "bodyText");
    let creator = exact_string(detail.payload, "authorName");
    let author_external_id = exact_string(detail.payload, "authorId");
    let published = published_at_evidence(detail.payload);
    let like_count = exact_nonnegative_count(detail.payload, &["likes", "likeCount", "likedCount"]);
    let comment_count = exact_nonnegative_count(
        detail.payload,
        &["publicCommentCount", "comments", "commentCount"],
    );
    let collect_count = exact_nonnegative_count(
        detail.payload,
        &["collects", "collectCount", "collectedCount"],
    );
    let share_count = exact_nonnegative_count(detail.payload, &["shares", "shareCount"]);
    let searchable_text = [title, body]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    sqlx::query("INSERT INTO linggan_material_content_detail (material_ref,content_public_ref,package_ref,record_ordinal,observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,like_count,like_count_state,comment_count,comment_count_state,collect_count,collect_count_state,share_count,share_count_state,published_at,published_at_source_field,published_at_source_kind,published_at_precision,published_at_reference_observed_at,published_at_parser_version) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,CASE WHEN $24::bigint IS NULL THEN NULL ELSE to_timestamp($24::double precision / 1000.0) END,$25,$26,$27,$28::timestamptz,$29)")
        .bind(Uuid::new_v4()).bind(content_public_ref).bind(package.package_ref())
        .bind(i32::try_from(ordinal).expect("package record count is bounded")).bind(package.observed_at())
        .bind(title).bind(known_state(title)).bind(body).bind(known_state(body))
        .bind(creator).bind(known_state(creator)).bind(published.source_text.as_deref())
        .bind(known_state(published.source_text.as_deref())).bind(searchable_text).bind(author_external_id)
        .bind(like_count).bind(known_state(like_count))
        .bind(comment_count).bind(known_state(comment_count))
        .bind(collect_count).bind(known_state(collect_count))
        .bind(share_count).bind(known_state(share_count))
        .bind(published.exact_millis).bind(published.source_field)
        .bind(published.source_kind).bind(published.precision)
        .bind(published.reference_observed_at).bind(published.parser_version)
        .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

struct ContentDetailRecord<'a> {
    content_id: &'a str,
    payload: &'a serde_json::Map<String, Value>,
}

struct DiscoveryRecord<'a> {
    content_id: &'a str,
    result_position: Option<i32>,
    payload: &'a serde_json::Map<String, Value>,
}

fn discovery_record<'a>(
    package: &ProducerCapturePackage,
    record: &'a Value,
) -> Option<DiscoveryRecord<'a>> {
    let expected_kind = if package.package_kind() == "profile_discovery" {
        "profile_discovery_card"
    } else {
        "discovery_card"
    };
    if record.get("kind")?.as_str()? != expected_kind {
        return None;
    }
    if !source_is(package, record, "content") {
        return None;
    }
    let content_id = record.pointer("/sourceObject/externalId")?.as_str()?.trim();
    let payload = record.get("payload")?.as_object()?;
    if content_id.is_empty() {
        return None;
    }
    let result_position = record
        .get("resultPosition")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .and_then(|value| i32::try_from(value).ok());
    Some(DiscoveryRecord {
        content_id,
        result_position,
        payload,
    })
}

fn content_detail_record<'a>(
    package: &ProducerCapturePackage,
    record: &'a Value,
) -> Option<ContentDetailRecord<'a>> {
    if record.get("kind")?.as_str()? != "content_detail" {
        return None;
    }
    if !source_is(package, record, "content") {
        return None;
    }
    let content_id = record.pointer("/sourceObject/externalId")?.as_str()?.trim();
    let payload = record.get("payload")?.as_object()?;
    if content_id.is_empty() {
        return None;
    }
    Some(ContentDetailRecord {
        content_id,
        payload,
    })
}

fn source_is(package: &ProducerCapturePackage, record: &Value, subject_type: &str) -> bool {
    record
        .pointer("/sourceObject/platform")
        .and_then(Value::as_str)
        == Some(package.platform())
        && record.pointer("/sourceObject/type").and_then(Value::as_str) == Some(subject_type)
}

pub(crate) fn exact_string<'a>(
    payload: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn exact_scalar_text(payload: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    payload.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

struct PublishedAtEvidence<'a> {
    exact_millis: Option<i64>,
    source_text: Option<String>,
    source_field: Option<&'a str>,
    source_kind: &'a str,
    precision: &'a str,
    reference_observed_at: Option<&'a str>,
    parser_version: Option<&'a str>,
}

fn published_at_evidence(payload: &serde_json::Map<String, Value>) -> PublishedAtEvidence<'_> {
    let source_text = exact_scalar_text(payload, "publishedAtText");
    let source_field = exact_string(payload, "publishedAtSourceField");
    let declared_kind = exact_string(payload, "publishedAtSourceKind").unwrap_or("unknown");
    let declared_precision = exact_string(payload, "publishedAtPrecision").unwrap_or("unknown");
    let parser_version = exact_string(payload, "publishedAtParserVersion");
    let normalized_millis = payload
        .get("publishedAt")
        .and_then(|value| {
            value.as_i64().or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim().parse::<i64>().ok())
            })
        })
        .filter(|value| *value > 0);
    let source_field_is_supported = matches!(
        source_field,
        Some("publishTime" | "publishDate" | "publishedAt" | "createTime" | "create_time" | "time")
    );
    let is_qualified_epoch = declared_kind == "platform_epoch"
        && matches!(declared_precision, "second" | "millisecond")
        && source_field_is_supported
        && parser_version == Some("xhs-detail-time-v2")
        && normalized_millis.is_some();
    let source_kind = match declared_kind {
        "platform_epoch" if is_qualified_epoch => "platform_epoch",
        "visible_text" => "visible_text",
        _ => "unknown",
    };
    let precision = match (source_kind, declared_precision) {
        ("platform_epoch", "second") => "second",
        ("platform_epoch", "millisecond") => "millisecond",
        ("visible_text", "minute") => "minute",
        ("visible_text", "day") => "day",
        ("visible_text", "relative") => "relative",
        _ => "unknown",
    };
    PublishedAtEvidence {
        exact_millis: is_qualified_epoch.then_some(normalized_millis).flatten(),
        source_text,
        source_field: (source_kind != "unknown").then_some(source_field).flatten(),
        source_kind,
        precision,
        reference_observed_at: (source_kind == "visible_text")
            .then(|| exact_string(payload, "publishedAtReferenceObservedAt"))
            .flatten(),
        parser_version: (source_kind != "unknown")
            .then_some(parser_version)
            .flatten(),
    }
}

fn exact_nonnegative_count(payload: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        payload
            .get(*key)
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0)
    })
}

fn observed_cover_url(payload: &serde_json::Map<String, Value>) -> Option<&str> {
    let direct = ["cover", "coverImg", "coverUrl", "thumbnail"]
        .into_iter()
        .find_map(|key| exact_string(payload, key));
    let first_image = payload
        .get("images")
        .and_then(Value::as_array)
        .and_then(|images| images.first())
        .and_then(|image| match image {
            Value::String(value) => Some(value.as_str()),
            Value::Object(value) => ["urlDefault", "url", "src"]
                .into_iter()
                .find_map(|key| exact_string(value, key)),
            _ => None,
        });
    direct
        .or(first_image)
        .map(str::trim)
        .filter(|value| observed_xhs_media_url_is_allowed(value))
}

fn observed_xhs_media_url_is_allowed(value: &str) -> bool {
    let Some(authority_and_path) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = authority_and_path.split('/').next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    host == "xhscdn.com" || host.ends_with(".xhscdn.com")
}

pub(crate) fn known_state<T>(value: Option<T>) -> &'static str {
    if value.is_some() { "KNOWN" } else { "UNKNOWN" }
}
