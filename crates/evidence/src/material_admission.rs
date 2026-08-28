//! Qualification of accepted producer records into typed material observations.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) fn record_disposition(
    package: &ProducerCapturePackage,
    record: &Value,
) -> Option<(&'static str, &'static str)> {
    let accepted = "accepted_for_library_content";
    let quarantined = "quarantined";
    match package.package_kind() {
        "discovery_search" | "profile_discovery" => {
            let expected_kind = if package.package_kind() == "profile_discovery" {
                "profile_discovery_card"
            } else {
                "discovery_card"
            };
            if record.get("kind").and_then(Value::as_str) != Some(expected_kind) {
                return None;
            }
            Some(match discovery_record(package, record) {
                Some(_) => (
                    "accepted_for_library_discovery",
                    "typed_discovery_identity_valid",
                ),
                None => (quarantined, "typed_discovery_identity_invalid"),
            })
        }
        "content_detail" => Some(
            match content_detail_record(record).is_some_and(|detail| {
                target_string(package, "contentExternalId") == Some(detail.content_id)
            }) {
                true => (accepted, "typed_detail_identity_valid"),
                false => (quarantined, "typed_detail_identity_invalid"),
            },
        ),
        "comments" | "replies" => {
            let replies = package.package_kind() == "replies";
            let expected_kind = if replies { "reply" } else { "comment" };
            let valid = record.get("kind").and_then(Value::as_str) == Some(expected_kind)
                && record
                    .get("payload")
                    .and_then(Value::as_object)
                    .is_some_and(|payload| {
                        exact_string(payload, "noteId")
                            == target_string(package, "contentExternalId")
                            && exact_string(payload, "commentId").is_some()
                            && if replies {
                                exact_string(payload, "rootCommentId").is_some()
                                    && exact_string(payload, "parentCommentId")
                                        .or_else(|| exact_string(payload, "replyToCommentId"))
                                        .is_some()
                            } else {
                                ["rootCommentId", "parentCommentId", "replyToCommentId"]
                                    .iter()
                                    .all(|key| exact_string(payload, key).is_none())
                            }
                    });
            Some(if valid {
                (
                    accepted,
                    if replies {
                        "typed_reply_relationship_valid"
                    } else {
                        "typed_comment_identity_valid"
                    },
                )
            } else {
                (
                    quarantined,
                    if replies {
                        "typed_reply_relationship_invalid"
                    } else {
                        "typed_comment_identity_invalid"
                    },
                )
            })
        }
        "author_profile" => {
            let author_id = target_string(package, "authorExternalId");
            let valid = record.get("kind").and_then(Value::as_str) == Some("author_profile")
                && record
                    .pointer("/sourceObject/externalId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == author_id
                && record
                    .get("payload")
                    .and_then(Value::as_object)
                    .is_some_and(|payload| {
                        exact_string(payload, "userId").or_else(|| exact_string(payload, "id"))
                            == author_id
                    });
            Some(if valid {
                (accepted, "typed_author_identity_valid")
            } else {
                (quarantined, "typed_author_identity_invalid")
            })
        }
        "media_slots" => crate::material_media::record_disposition(package, record),
        _ => None,
    }
}

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
    match package.package_kind() {
        "discovery_search" | "profile_discovery" => insert_discovery_records(tx, package).await?,
        "content_detail" => insert_content_records(tx, package).await?,
        "comments" => insert_comment_records(tx, package, false).await?,
        "replies" => insert_comment_records(tx, package, true).await?,
        "author_profile" => insert_author_records(tx, package).await?,
        "media_slots" | "media_bytes" => crate::material_media::insert(tx, package).await?,
        _ => {}
    }
    Ok(())
}

async fn insert_discovery_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<(), ProducerRuntimeError> {
    for (ordinal, record) in package.records().iter().enumerate() {
        let Some(finding) = discovery_record(package, record) else {
            continue;
        };
        let content_public_ref = ensure_content(tx, package, finding.content_id).await?;
        let title = exact_string(finding.payload, "title");
        let creator = exact_string(finding.payload, "authorName");
        let published = exact_scalar_text(finding.payload, "publishedAtText");
        sqlx::query("INSERT INTO linggan_material_discovery_finding (material_ref,content_public_ref,package_ref,record_ordinal,discovery_kind,result_position,observed_at,title,title_state,creator_display_name,creator_state,published_at_source_text,published_at_source_text_state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
            .bind(Uuid::new_v4()).bind(content_public_ref).bind(package.package_ref()).bind(i32::try_from(ordinal).expect("package record count is bounded"))
            .bind(package.package_kind()).bind(finding.result_position).bind(package.observed_at()).bind(title).bind(known_state(title))
            .bind(creator).bind(known_state(creator)).bind(published.as_deref()).bind(known_state(published.as_deref()))
            .execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(())
}

async fn insert_content_records(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<(), ProducerRuntimeError> {
    let Some(expected_content_id) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    let content_public_ref = ensure_content(tx, package, expected_content_id).await?;
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
        let Some(detail) = content_detail_record(record) else {
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
) -> Result<(), ProducerRuntimeError> {
    let Some(content_id) = target_string(package, "contentExternalId") else {
        return Ok(());
    };
    let content_public_ref = ensure_content(tx, package, content_id).await?;
    let expected_kind = if replies { "reply" } else { "comment" };
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
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
) -> Result<(), ProducerRuntimeError> {
    let Some(author_id) = target_string(package, "authorExternalId") else {
        return Ok(());
    };
    let mut retained = 0;
    for (ordinal, record) in package.records().iter().enumerate() {
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
    let layer = package.coverage().pointer("/layers/0");
    let count = |field| {
        layer
            .and_then(|value| value.get(field))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
    };
    sqlx::query("INSERT INTO linggan_material_lane_observation (package_ref,lane,content_public_ref,author_external_id,observed,retained,failed,known_unattempted,unknown_count,maximum_quota,stopped_reason,observed_at) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,(task.task_spec->>'maximumQuota')::integer,$10,$11 FROM linggan_runtime_capture_package package JOIN linggan_runtime_task task ON task.task_id = package.task_id WHERE package.package_ref = $1")
        .bind(package.package_ref()).bind(lane).bind(content_public_ref).bind(author_external_id)
        .bind(count("observed")).bind(retained).bind(count("failed")).bind(count("notAttempted")).bind(count("unknown"))
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
    let published_at = exact_scalar_text(detail.payload, "publishedAtText");
    let searchable_text = [title, body]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    sqlx::query("INSERT INTO linggan_material_content_detail (material_ref,content_public_ref,package_ref,record_ordinal,observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,published_at_source_text,published_at_source_text_state,searchable_text,author_external_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)")
        .bind(Uuid::new_v4()).bind(content_public_ref).bind(package.package_ref())
        .bind(i32::try_from(ordinal).expect("package record count is bounded")).bind(package.observed_at())
        .bind(title).bind(known_state(title)).bind(body).bind(known_state(body))
        .bind(creator).bind(known_state(creator)).bind(published_at.as_deref())
        .bind(known_state(published_at.as_deref())).bind(searchable_text).bind(author_external_id)
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
    if record
        .pointer("/sourceObject/platform")
        .and_then(Value::as_str)
        .is_some_and(|value| value != package.platform())
        || record
            .pointer("/sourceObject/type")
            .and_then(Value::as_str)
            .is_some_and(|value| value != "content")
    {
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

fn content_detail_record(record: &Value) -> Option<ContentDetailRecord<'_>> {
    if record.get("kind")?.as_str()? != "content_detail" {
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

pub(crate) fn known_state<T>(value: Option<T>) -> &'static str {
    if value.is_some() { "KNOWN" } else { "UNKNOWN" }
}
