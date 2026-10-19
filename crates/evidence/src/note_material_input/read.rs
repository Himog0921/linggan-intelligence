use super::{
    Column, ColumnState, Error, FINGERPRINT_VERSION, Material, VERSION, cutoff, identity,
    readiness, validate_works,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub async fn read(database: &Database, domain: Uuid, work: Uuid) -> Result<Material, Error> {
    Ok(read_at(database, domain, &[work], None).await?.remove(0))
}

/// Frozen source selection, with current restriction checks inherited from the dossier.
pub async fn read_at(
    database: &Database,
    domain: Uuid,
    works: &[Uuid],
    as_of: Option<&str>,
) -> Result<Vec<Material>, Error> {
    validate_works(works)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of = cutoff(&mut tx, as_of).await?;
    let result = read_in(&mut tx, domain, works, &as_of).await?;
    tx.commit().await?;
    Ok(result)
}

pub(super) async fn scope(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    works: &[Uuid],
    as_of: &str,
) -> Result<Vec<(Uuid, bool)>, Error> {
    validate_works(works)?;
    let rows = sqlx::query(r#"
SELECT u.content_public_ref, min(u.created_at) + interval '24 hours' <= $3::timestamptz AS grace_elapsed
FROM linggan_material_domain_usage u JOIN observation_domain d USING(domain_ref)
WHERE u.domain_ref=$1 AND d.name='ADHD'
AND u.content_public_ref=ANY($2) AND u.created_at<=$3::timestamptz
GROUP BY u.content_public_ref
"#).bind(domain).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    if rows.len() != works.len() {
        return Err(Error::NotFound);
    }
    Ok(rows
        .into_iter()
        .map(|r| (r.get("content_public_ref"), r.get("grace_elapsed")))
        .collect())
}

pub(super) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    works: &[Uuid],
    as_of: &str,
) -> Result<Vec<Material>, Error> {
    let grace: std::collections::BTreeMap<_, _> =
        scope(tx, domain, works, as_of).await?.into_iter().collect();
    let currents =
        crate::work_resource_current::read_work_resource_currents(tx, works, as_of).await?;
    let mut dossiers = crate::research_dossier::read_in(tx, works, as_of).await?;
    let statuses = readiness::read_in(tx, works, as_of).await?;
    // One batched optional-field read; receipts created after the cutoff remain invisible.
    let rows = sqlx::query(r#"
SELECT d.material_ref,p.state,p.created_at::text AS projected_at,h.ordinal,h.tag_text
FROM linggan_material_content_detail d
JOIN linggan_material_hashtag_projection p ON p.material_ref=d.material_ref AND p.projection_contract=$3
LEFT JOIN linggan_material_content_hashtag h ON h.material_ref=p.material_ref AND h.projection_contract=p.projection_contract
WHERE d.content_public_ref=ANY($1) AND p.created_at<=$2::timestamptz
ORDER BY d.material_ref,h.ordinal
"#).bind(works).bind(as_of).bind(crate::material_hashtag::VERSION).fetch_all(&mut **tx).await?;
    let mut tags = std::collections::BTreeMap::<Uuid, (String, String, Vec<(i32, String)>)>::new();
    for row in rows {
        let item = tags
            .entry(row.get("material_ref"))
            .or_insert_with(|| (row.get("state"), row.get("projected_at"), vec![]));
        if let Some(text) = row.get::<Option<String>, _>("tag_text") {
            item.2.push((row.get("ordinal"), text));
        }
    }
    let mut output = std::collections::BTreeMap::new();
    for current in currents {
        let dossier = dossiers
            .remove(&current.public_ref)
            .ok_or(Error::NotFound)?;
        let mut fragments = vec![];
        for f in &dossier.fragments {
            let position = match f.field.as_str() {
                "title" | "body" => "native".into(),
                _ => {
                    let media = dossier
                        .media
                        .iter()
                        .find(|m| {
                            m["derivativeRef"]
                                .as_str()
                                .and_then(|v| v.parse::<Uuid>().ok())
                                == Some(f.source_ref)
                        })
                        .ok_or(Error::Invalid("fragment_source_unavailable"))?;
                    let mut position = media["semanticMediaSource"]["position"].clone();
                    if let Some(p) = position.as_object_mut() {
                        for key in ["processorVersion", "layoutRef", "blobSha256"] {
                            p.remove(key);
                        }
                    }
                    json!([media["slotOrdinal"], media["sourceScope"], position]).to_string()
                }
            };
            fragments.push(identity::fragment(
                &f.field,
                position,
                f.source_ref,
                f.source_version.clone(),
                f.start,
                f.text.clone(),
            ));
        }
        let available = |field: &str| fragments.iter().any(|f| f.field == field);
        let mut columns: Vec<Column> = ["title", "body", "cover_headline"]
            .into_iter()
            .map(|field| Column {
                field: field.into(),
                state: if available(field) {
                    ColumnState::Available
                } else {
                    ColumnState::NotAcquired
                },
                reasons: if available(field) {
                    vec![]
                } else {
                    vec!["qualified_text_unavailable".into()]
                },
            })
            .collect();
        columns.extend(readiness::columns(
            statuses
                .get(&current.public_ref)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &dossier.media,
            available("ocr"),
            available("transcript"),
        ));
        let mut reasons = vec![];
        if current.detail_material_ref.is_none() && !grace[&current.public_ref] {
            reasons.push("waiting_detail_within_24h".into());
        }
        for column in &columns {
            if matches!(
                column.state,
                ColumnState::Waiting | ColumnState::NeedsRelayer
            ) {
                reasons.push(format!(
                    "{}_{}",
                    column.field,
                    if column.state == ColumnState::Waiting {
                        "processing_pending"
                    } else {
                        "needs_relayer"
                    }
                ));
            }
        }
        let (tag_state, tag_reason) = match current.detail_material_ref {
            None => (ColumnState::NotAcquired, Some("detail_not_acquired")),
            Some(source) => match tags.get(&source) {
                None => {
                    reasons.push("tags_projection_pending".into());
                    (ColumnState::Waiting, Some("tags_projection_pending"))
                }
                Some((state, version, values)) => {
                    for (ordinal, text) in values {
                        fragments.push(identity::fragment(
                            "tags",
                            format!("{ordinal:08}"),
                            source,
                            crate::creator_discovery::hash(
                                &json!([crate::material_hashtag::VERSION, version]).to_string(),
                            ),
                            0,
                            text.clone(),
                        ));
                    }
                    match state.as_str() {
                        "not_provided" => (ColumnState::NotProvided, Some("source_field_absent")),
                        "invalid" => (ColumnState::InvalidSource, Some("source_field_invalid")),
                        _ if values.is_empty() => {
                            (ColumnState::KnownEmpty, Some("source_provided_empty_array"))
                        }
                        _ => (ColumnState::Available, None),
                    }
                }
            },
        };
        columns.push(Column {
            field: "tags".into(),
            state: tag_state,
            reasons: tag_reason.into_iter().map(str::to_owned).collect(),
        });
        let mut material = Material {
            contract: VERSION.into(),
            fingerprint_contract: FINGERPRINT_VERSION.into(),
            domain_ref: domain,
            work_ref: current.public_ref,
            as_of: as_of.into(),
            dossier_revision: dossier.revision,
            material_fingerprint: String::new(),
            provenance_hash: String::new(),
            snapshot_identity: String::new(),
            ready: false,
            not_ready_reasons: reasons,
            columns,
            fragments,
            layer_version: crate::ocr_layering::VERSION.into(),
            processing_scope: super::ProcessingScope {
                included: vec!["image_ocr".into(), "asr".into()],
                context_only: vec![
                    "comment_image".into(),
                    "author_avatar".into(),
                    "thumbnail".into(),
                    "audio_extract".into(),
                ],
                not_connected: vec!["video_frame_ocr".into()],
            },
        };
        identity::finish(&mut material);
        output.insert(current.public_ref, material);
    }
    works
        .iter()
        .map(|w| output.remove(w).ok_or(Error::NotFound))
        .collect()
}

pub(super) async fn lock_work(tx: &mut Transaction<'_, Postgres>, work: Uuid) -> Result<(), Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,420))")
        .bind(work.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn check_revision(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    expected: &str,
) -> Result<(), Error> {
    let now = cutoff(tx, None).await?;
    let value = crate::research_dossier::read_in(tx, &[work], &now)
        .await?
        .remove(&work)
        .ok_or(Error::NotFound)?;
    if value.revision != expected {
        return Err(Error::Conflict);
    }
    Ok(())
}
