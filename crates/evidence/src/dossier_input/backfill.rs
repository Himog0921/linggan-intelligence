use super::{Error, cutoff, read};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const MAX_DETAILS: usize = 500;

pub(super) async fn inventory(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    as_of: &str,
) -> Result<Vec<(Uuid, Value, bool)>, Error> {
    let rows=sqlx::query(r#"
SELECT d.material_ref,p.payload->'records'->d.record_ordinal->'payload' AS source,
EXISTS(SELECT 1 FROM linggan_material_hashtag_projection h WHERE h.material_ref=d.material_ref AND h.projection_contract=$3) AS projected
FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref)
JOIN linggan_runtime_record_disposition r USING(package_ref,record_ordinal)
WHERE d.content_public_ref=$1 AND d.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
AND r.disposition='accepted_for_library_content'
ORDER BY d.created_at,d.material_ref LIMIT 501
"#).bind(work).bind(as_of).bind(crate::material_hashtag::VERSION).fetch_all(&mut **tx).await?;
    if rows.len() > MAX_DETAILS {
        return Err(Error::Invalid("detail_limit_exceeded"));
    }
    rows.into_iter()
        .map(|r| {
            Ok((
                r.get("material_ref"),
                r.try_get::<Option<Value>, _>("source")?
                    .ok_or(Error::Invalid("source_record_unavailable"))?,
                r.get("projected"),
            ))
        })
        .collect()
}

pub(super) async fn apply(
    database: &Database,
    domain: Uuid,
    work: Uuid,
    as_of: &str,
    expected: &str,
) -> Result<Value, Error> {
    let mut tx = database.pool().begin().await?;
    read::lock_work(&mut tx, work).await?;
    let as_of = cutoff(&mut tx, Some(as_of)).await?;
    read::scope(&mut tx, domain, &[work], &as_of).await?;
    let rows = inventory(&mut tx, work, &as_of).await?;
    let now = cutoff(&mut tx, None).await?;
    let current_sources = inventory(&mut tx, work, &now).await?;
    if rows.iter().map(|(source, _, _)| source).collect::<Vec<_>>()
        != current_sources
            .iter()
            .map(|(source, _, _)| source)
            .collect::<Vec<_>>()
    {
        return Err(Error::Conflict);
    }
    let read_only_replay = rows.iter().all(|(_, _, projected)| *projected);
    // Publishing tags changes the dossier revision. A replay may acknowledge
    // exactly those immutable projections without writing them again; pending
    // writes still require the original complete dossier revision.
    if !read_only_replay {
        read::check_revision(&mut tx, work, expected).await?;
    }
    let (mut inserted, mut current, mut absent, mut invalid, mut nonempty) = (0, 0, 0, 0, 0);
    for (material, payload, _) in &rows {
        let payload = payload
            .as_object()
            .ok_or(Error::Invalid("source_record_invalid"))?;
        let parsed = crate::material_hashtag::parse(payload);
        if parsed.state == "not_provided" {
            absent += 1;
        }
        if parsed.state == "invalid" {
            invalid += 1;
        }
        if !parsed.tags.is_empty() {
            nonempty += 1;
        }
        if crate::material_hashtag::project(&mut tx, *material, payload).await? {
            inserted += 1;
        } else {
            current += 1;
        }
    }
    tx.commit().await?;
    Ok(
        json!({"detailCount":rows.len(),"projected":inserted,"alreadyCurrent":current,"notProvided":absent,"invalid":invalid,"nonempty":nonempty,"readOnlyReplay":read_only_replay}),
    )
}
