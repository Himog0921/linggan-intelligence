//! Qualification of accepted producer records into typed material observations.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::{ProducerCapturePackage, parse_producer_capture_package};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

/// One-time operator projection of original accepted packages for a known Target/Domain pair.
/// It writes no new CapturePackage, Receipt or disposition, and never reads retired cross tables.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetMaterialReprojection {
    pub target_ref: Uuid,
    pub domain_ref: Uuid,
    pub selected_packages: usize,
    pub already_projected_packages: usize,
    pub projected_packages: usize,
    pub eligible_by_kind: BTreeMap<String, usize>,
}

pub async fn reproject_accepted_target_materials(
    database: &Database,
    target_ref: Uuid,
    domain_ref: Uuid,
    apply: bool,
) -> Result<TargetMaterialReprojection, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let rows = sqlx::query(
        "SELECT package.package_ref,package.package_kind,package.payload \
         FROM linggan_runtime_capture_package package \
         JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
         WHERE receipt.material_admission='ACCEPTED' \
           AND package.package_kind IN ('profile_discovery','content_detail','comments','author_profile') \
           AND EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                       WHERE disposition.package_ref=package.package_ref \
                         AND disposition.disposition IN \
                           ('accepted_for_library_discovery','accepted_for_library_content')) \
           AND EXISTS (SELECT 1 FROM collection_work_order_lease_task lease_task \
                       JOIN collection_work_order_lease lease USING(lease_ref) \
                       JOIN collection_work_order work USING(work_order_ref) \
                       JOIN collection_work_order_domain_usage usage USING(work_order_ref) \
                       WHERE lease_task.task_id=package.task_id \
                         AND work.target_ref=$1 AND usage.domain_ref=$2) \
         ORDER BY CASE package.package_kind \
                    WHEN 'profile_discovery' THEN 0 WHEN 'author_profile' THEN 1 \
                    WHEN 'content_detail' THEN 2 ELSE 3 END,package.accepted_at,package.package_ref",
    )
    .bind(target_ref)
    .bind(domain_ref)
    .fetch_all(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let mut report = TargetMaterialReprojection {
        target_ref,
        domain_ref,
        selected_packages: rows.len(),
        already_projected_packages: 0,
        projected_packages: 0,
        eligible_by_kind: BTreeMap::new(),
    };
    for row in rows {
        let package_ref: Uuid = row.get("package_ref");
        let package_kind: String = row.get("package_kind");
        let (projected_rows, completed_lane, accepted_rows): (i64, bool, i64) = sqlx::query_as(
            "SELECT CASE $2::text \
                      WHEN 'profile_discovery' THEN (SELECT count(*) FROM linggan_material_discovery_finding WHERE package_ref=$1) \
                      WHEN 'content_detail' THEN (SELECT count(*) FROM linggan_material_content_detail WHERE package_ref=$1) \
                      WHEN 'comments' THEN (SELECT count(*) FROM linggan_material_comment WHERE package_ref=$1) \
                      ELSE (SELECT count(*) FROM linggan_material_author_profile WHERE package_ref=$1) END, \
                    EXISTS(SELECT 1 FROM linggan_material_lane_observation WHERE package_ref=$1), \
                    (SELECT count(*) FROM linggan_runtime_record_disposition WHERE package_ref=$1 \
                       AND disposition IN ('accepted_for_library_discovery','accepted_for_library_content'))",
        )
        .bind(package_ref)
        .bind(&package_kind)
        .fetch_one(&mut *tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
        let uninterpreted: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_runtime_record_disposition \
             WHERE package_ref=$1 AND disposition='retained_uninterpreted')",
        )
        .bind(package_ref)
        .fetch_one(&mut *tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
        if uninterpreted {
            return Err(ProducerRuntimeError::MaterialIdentityConflict);
        }
        if projected_rows > 0 || completed_lane {
            if projected_rows != accepted_rows
                || (package_kind != "profile_discovery" && !completed_lane)
            {
                return Err(ProducerRuntimeError::MaterialIdentityConflict);
            }
            let missing_typed_ordinals: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                 WHERE disposition.package_ref=$1 \
                   AND disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content') \
                   AND NOT CASE $2::text \
                     WHEN 'profile_discovery' THEN EXISTS (SELECT 1 FROM linggan_material_discovery_finding finding \
                       WHERE finding.package_ref=disposition.package_ref AND finding.record_ordinal=disposition.record_ordinal) \
                     WHEN 'content_detail' THEN EXISTS (SELECT 1 FROM linggan_material_content_detail detail \
                       WHERE detail.package_ref=disposition.package_ref AND detail.record_ordinal=disposition.record_ordinal) \
                     WHEN 'comments' THEN EXISTS (SELECT 1 FROM linggan_material_comment comment \
                       WHERE comment.package_ref=disposition.package_ref AND comment.record_ordinal=disposition.record_ordinal) \
                     ELSE EXISTS (SELECT 1 FROM linggan_material_author_profile author \
                       WHERE author.package_ref=disposition.package_ref AND author.record_ordinal=disposition.record_ordinal) \
                   END",
            )
            .bind(package_ref)
            .bind(&package_kind)
            .fetch_one(&mut *tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
            if missing_typed_ordinals > 0 {
                return Err(ProducerRuntimeError::MaterialIdentityConflict);
            }
            if package_kind == "profile_discovery" {
                let missing_domain_usages: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM linggan_material_discovery_finding finding \
                     JOIN linggan_runtime_record_disposition disposition \
                       ON disposition.package_ref=finding.package_ref \
                      AND disposition.record_ordinal=finding.record_ordinal \
                     WHERE finding.package_ref=$1 \
                       AND disposition.disposition='accepted_for_library_discovery' \
                       AND NOT EXISTS (SELECT 1 FROM linggan_material_domain_usage usage \
                         WHERE usage.package_ref=finding.package_ref \
                           AND usage.record_ordinal=finding.record_ordinal \
                           AND usage.content_public_ref=finding.content_public_ref \
                           AND usage.domain_ref=$2 AND usage.basis_kind='accepted_discovery')",
                )
                .bind(package_ref)
                .bind(domain_ref)
                .fetch_one(&mut *tx)
                .await
                .map_err(ProducerRuntimeError::Internal)?;
                if missing_domain_usages > 0 {
                    return Err(ProducerRuntimeError::MaterialIdentityConflict);
                }
            }
            report.already_projected_packages += 1;
            continue;
        }
        *report
            .eligible_by_kind
            .entry(package_kind.clone())
            .or_default() += 1;
        if !apply {
            continue;
        }
        let raw: Value = row.get("payload");
        let package = parse_producer_capture_package(&raw.to_string())?;
        if package.package_ref() != package_ref || package.package_kind() != package_kind {
            return Err(ProducerRuntimeError::MaterialIdentityConflict);
        }
        insert_typed_materials(&mut tx, &package).await?;
        report.projected_packages += 1;
    }
    if apply {
        tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    } else {
        tx.rollback()
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(report)
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
        insert_material_domain_usage(
            tx,
            package.package_ref(),
            content_public_ref,
            i32::try_from(ordinal).expect("record count is bounded"),
        )
        .await?;
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

async fn insert_material_domain_usage(
    tx: &mut Transaction<'_, Postgres>,
    package_ref: Uuid,
    content_public_ref: Uuid,
    record_ordinal: i32,
) -> Result<(), ProducerRuntimeError> {
    // This typed projection runs before submit_producer_package writes its receipt, inside the
    // same transaction. The receipt is inserted after all typed rows succeed, so joining it here
    // would silently suppress every Domain usage while still accepting the discovery itself.
    let rows: Vec<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT usage.work_order_ref,usage.request_ref,usage.domain_ref,usage.role \
         FROM linggan_runtime_capture_package package \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         JOIN collection_work_order_domain_usage usage USING(work_order_ref) \
         JOIN linggan_runtime_record_disposition disposition \
           ON disposition.package_ref=package.package_ref \
          AND disposition.record_ordinal=$2 \
          AND disposition.disposition='accepted_for_library_discovery' \
         WHERE package.package_ref=$1",
    )
    .bind(package_ref)
    .bind(record_ordinal)
    .fetch_all(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    for (work_order_ref, request_ref, domain_ref, role) in rows {
        sqlx::query(
            "INSERT INTO linggan_material_domain_usage \
             (usage_ref,content_public_ref,domain_ref,role,basis_kind,request_ref,work_order_ref,package_ref,record_ordinal) \
             VALUES($1,$2,$3,$4,'accepted_discovery',$5,$6,$7,$8) \
             ON CONFLICT (content_public_ref,domain_ref,package_ref,record_ordinal,request_ref) \
             WHERE basis_kind='accepted_discovery' DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(content_public_ref)
        .bind(domain_ref)
        .bind(role)
        .bind(request_ref)
        .bind(work_order_ref)
        .bind(package_ref)
        .bind(record_ordinal)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
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

pub(crate) fn target_string<'a>(
    package: &'a ProducerCapturePackage,
    field: &str,
) -> Option<&'a str> {
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
    // Content 是跨 Domain 的同一份来源身份；用途由 discovery 的 MaterialDomainUsage 记录。
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

pub(crate) fn exact_scalar_text(
    payload: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<String> {
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

/// 取平台给出的互动计数。
///
/// **列表面的计数是文本，不是数字**：插件从搜索/主页卡片上读到的原样是 `"2704"`、
/// `"2.9万"`，偶尔还有 `"赞"` 这种没有数字的占位。此前这里只接 `Value::as_i64`，
/// 于是 1100 多条发现记录的点赞、评论、收藏**全部落空**，`*_state` 一律 `UNKNOWN`。
/// 详情面走 API、给的是数字，所以只有列表面受影响——而列表面正是判断「哪篇值得
/// 深挖」的地方。
///
/// `"2.9万"` 按 29000 记。**这是平台自己就只显示到这个精度**，不是我们丢了精度：
/// 原文 `"2.9万"` 不可变地留在 `linggan_runtime_capture_package.payload` 里，任何
/// 时候都能回查这个数是从什么文本来的，所以投影侧不再复制一份原文。
///
/// 认不出的一概返回 `None`（→ `UNKNOWN`），**绝不当作 0**：`"赞"` 表示这张卡片上
/// 没有数字可读，不表示没有人点赞。
pub(crate) fn exact_nonnegative_count(
    payload: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<i64> {
    // 非负判定留在**每个候选键各自的闭包里**，不要提到 `find_map` 外面：提出去之后
    // 第一个能解析的键一旦为负就直接判定整体未知，后面的候选键再也轮不到。多候选
    // 存在的意义就是「这个键不可用时换下一个」。
    keys.iter().find_map(|key| {
        match payload.get(*key) {
            Some(Value::Number(_)) => payload.get(*key).and_then(Value::as_i64),
            Some(Value::String(text)) => platform_count_text(text),
            _ => None,
        }
        .filter(|value| *value >= 0)
    })
}

/// 把平台卡片上的计数文本解析成整数。
///
/// 只认已经在真实回传里见过的写法：纯数字与「万」。`亿` 是同一套中文计数写法里
/// 「万」的上一级，一并展开。**没见过的写法一概不认**——多认一种没有证据的后缀
/// （`w`、`k`、`K`…）不会让数据更全，只会在某天把一个不是计数的文本读成数字。
fn platform_count_text(text: &str) -> Option<i64> {
    let text = text.trim();
    let (digits, scale) = match text.strip_suffix('万') {
        Some(head) => (head, 10_000_i64),
        None => match text.strip_suffix('亿') {
            Some(head) => (head, 100_000_000_i64),
            None => (text, 1_i64),
        },
    };
    scaled_decimal(digits.trim(), scale)
}

/// `"2.9"` × 10000 → 29000。
///
/// 走定点而不是浮点：`2.9_f64 * 10000.0` 得到 28999.999999999996，取整就少 1 个赞。
fn scaled_decimal(digits: &str, scale: i64) -> Option<i64> {
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let mut total = whole.parse::<i64>().ok()?.checked_mul(scale)?;
    if !fraction.is_empty() {
        let divisor = 10_i64.checked_pow(u32::try_from(fraction.len()).ok()?)?;
        // 小数位比单位还细时（`1.23456万`）截断，不四舍五入：平台没显示的位数我们不猜。
        total = total.checked_add(fraction.parse::<i64>().ok()?.checked_mul(scale)? / divisor)?;
    }
    Some(total)
}

pub(crate) fn observed_cover_url(payload: &serde_json::Map<String, Value>) -> Option<&str> {
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

#[cfg(test)]
mod platform_count_tests {
    use super::*;
    use serde_json::json;

    fn count(value: Value) -> Option<i64> {
        let payload = json!({ "likes": value });
        exact_nonnegative_count(
            payload.as_object().expect("object"),
            &["likeCount", "likes"],
        )
    }

    /// 列表面给的是文本。此前只接数字，1100+ 条发现记录的互动数因此全部落空。
    #[test]
    fn a_plain_digit_text_from_a_card_is_read() {
        assert_eq!(count(json!("2704")), Some(2704));
        assert_eq!(count(json!("0")), Some(0));
    }

    /// 「万」正是高赞笔记的写法——恰恰是最该被读到的那批。
    #[test]
    fn a_chinese_ten_thousand_unit_expands() {
        assert_eq!(count(json!("1万")), Some(10_000));
        assert_eq!(count(json!("2.9万")), Some(29_000));
        assert_eq!(count(json!("1.25万")), Some(12_500));
        assert_eq!(count(json!("5.3万")), Some(53_000));
    }

    /// 定点而非浮点：`2.9 * 10000.0` 在 f64 下是 28999.999999999996。
    #[test]
    fn the_expansion_does_not_lose_a_unit_to_floating_point() {
        for (text, expected) in [("2.9万", 29_000), ("8.7万", 87_000), ("1.1万", 11_000)] {
            assert_eq!(count(json!(text)), Some(expected), "{text}");
        }
    }

    /// `"赞"` 是「这张卡片上没有数字」，不是「没有人点赞」——必须是未知，不能是 0。
    #[test]
    fn a_placeholder_without_digits_stays_unknown() {
        assert_eq!(count(json!("赞")), None);
        assert_eq!(count(json!("")), None);
        assert_eq!(count(json!("万")), None);
        assert_eq!(count(json!("很多")), None);
    }

    /// 详情面走 API 给的是数字，原路径不能被这次改动动到。
    #[test]
    fn a_numeric_count_keeps_its_previous_behaviour() {
        assert_eq!(count(json!(321)), Some(321));
        assert_eq!(count(json!(-1)), None);
        assert_eq!(count(json!(null)), None);
        assert_eq!(count(json!(true)), None);
    }

    /// 负号、空白、千分位这些没在平台上出现过的写法，一律不猜。
    #[test]
    fn unobserved_shapes_are_refused_rather_than_guessed() {
        assert_eq!(count(json!("-5")), None);
        assert_eq!(count(json!("1,234")), None);
        assert_eq!(count(json!("2.9 万")), Some(29_000), "单位前的空白应被容忍");
    }

    /// 只认真实见过的「万」「亿」。`w`/`k` 这类后缀没有任何观测证据，多认一种就是
    /// 多一条把非计数文本读成数字的路。
    #[test]
    fn only_units_actually_seen_on_the_platform_are_expanded() {
        assert_eq!(count(json!("1亿")), Some(100_000_000));
        for guessed in ["1w", "1W", "1k", "1K", "1万万"] {
            assert_eq!(
                count(json!(guessed)),
                None,
                "{guessed} 没有观测依据，不该被认出"
            );
        }
    }

    /// 多候选键的意义是「这个键不可用就换下一个」。非负判定必须留在每个候选各自的
    /// 判断里；提到最外面会让第一个能解析的键一旦为负就直接判整体未知，后备键再也
    /// 轮不到——那是一次静默的能力收窄。
    #[test]
    fn a_negative_first_candidate_falls_back_to_the_next_key() {
        let payload = json!({ "likeCount": -1, "likes": 42 });
        assert_eq!(
            exact_nonnegative_count(
                payload.as_object().expect("object"),
                &["likeCount", "likes"]
            ),
            Some(42)
        );

        let all_negative = json!({ "likeCount": -1, "likes": -2 });
        assert_eq!(
            exact_nonnegative_count(
                all_negative.as_object().expect("object"),
                &["likeCount", "likes"]
            ),
            None
        );

        // 文本候选同样要能被跳过，而不是让整串候选就此打住。
        let text_first = json!({ "likeCount": "赞", "likes": "2.9万" });
        assert_eq!(
            exact_nonnegative_count(
                text_first.as_object().expect("object"),
                &["likeCount", "likes"]
            ),
            Some(29_000)
        );
    }
}
