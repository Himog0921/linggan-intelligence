//! One source inventory shared by full reading, coverage and explicit local rebuilding.
use crate::work_resource_current::WorkResourceCurrent;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct Inventory {
    pub slots: Vec<Value>,
    pub manifest: Option<Value>,
    pub tags: BTreeMap<Uuid, Value>,
}

pub(super) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    currents: &[&WorkResourceCurrent],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Inventory>, sqlx::Error> {
    let works: Vec<_> = currents.iter().map(|work| work.public_ref).collect();
    let versions = read_image_versions(tx, &works, as_of).await?;
    let mut result = BTreeMap::<Uuid, Inventory>::new();
    for version in &versions {
        if let Some(manifest) = &version.manifest {
            result.entry(version.work_ref).or_default().manifest = Some(manifest.clone());
        }
    }
    let rows = sqlx::query(r#"
WITH image_versions AS (
 SELECT work_ref, package_ref, legacy_cutoff
 FROM jsonb_to_recordset($3) AS version(work_ref uuid, package_ref uuid, legacy_cutoff timestamptz)
), origins AS (
 SELECT DISTINCT ON (o.slot_key) o.*
 FROM linggan_material_media_origin o JOIN linggan_runtime_capture_package p USING(package_ref)
 JOIN linggan_runtime_record_disposition d ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal
 LEFT JOIN image_versions v ON v.work_ref=o.content_public_ref AND o.purpose IN ('body_image','live_photo')
 WHERE o.content_public_ref=ANY($1) AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 AND o.purpose NOT IN ('comment_image','author_avatar')
 AND d.disposition='accepted_for_media_identity'
 AND (v.work_ref IS NULL OR o.package_ref=v.package_ref OR p.accepted_at<v.legacy_cutoff)
 ORDER BY o.slot_key,o.source_generation DESC
)
SELECT o.content_public_ref,o.slot_key,o.purpose,o.display_ordinal,o.producer_ordinal,o.display_order_state,component.kind AS component_kind,
 o.package_ref,o.observation_ref,o.source_generation,m.materialization_ref,m.blob_sha256,m.storage_key,m.byte_size,
 (SELECT jsonb_build_object('state',CASE WHEN a.ended_at IS NULL OR a.ended_at>$2::timestamptz THEN 'running' ELSE a.terminal_reason END)
  FROM linggan_media_download_attempt a WHERE a.media_observation_ref=o.observation_ref AND a.started_at<=$2::timestamptz
  ORDER BY a.started_at DESC,a.download_attempt_ref DESC LIMIT 1) AS download,
 (SELECT jsonb_object_agg(latest.processor_kind,jsonb_build_object('state',latest.state,'reason',latest.reason)) FROM (
   SELECT DISTINCT ON(j.processor_kind) j.processor_kind,e.state,e.reason
   FROM linggan_media_processing_job j
   LEFT JOIN LATERAL(SELECT CASE state WHEN 'succeeded' THEN 'completed' ELSE state END AS state,reason FROM linggan_media_processing_job_event e WHERE e.job_ref=j.job_ref AND e.occurred_at<=$2::timestamptz ORDER BY e.occurred_at DESC LIMIT 1) e ON true
   WHERE j.slot_key=o.slot_key AND j.blob_sha256=m.blob_sha256 AND j.created_at<=$2::timestamptz
   ORDER BY j.processor_kind,j.created_at DESC,j.job_ref DESC
 ) latest) AS processing,
 EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
 WHERE d.state='WITHDRAWN_OR_RESTRICTED' AND (d.slot_key=o.slot_key OR d.blob_sha256=m.blob_sha256
 OR d.materialization_ref=m.materialization_ref)) AS restricted,
 EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
 WHERE d.materialization_ref=m.materialization_ref) AS bytes_unavailable
FROM origins o CROSS JOIN LATERAL (
 SELECT kind FROM (VALUES ('image'),('motion')) component(kind)
 WHERE (kind='image' AND o.purpose<>'video') OR (kind='motion' AND o.purpose IN ('video','live_photo'))
) component LEFT JOIN LATERAL (
 SELECT m.*,b.storage_key,b.byte_size FROM linggan_media_materialization m JOIN linggan_media_download_attempt a USING(download_attempt_ref)
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
 WHERE a.media_observation_ref=o.observation_ref AND m.verified_at<=$2::timestamptz
 AND ((component.kind='image' AND b.mime_type LIKE 'image/%') OR
 (component.kind='motion' AND (b.mime_type LIKE 'video/%' OR (o.purpose<>'live_photo' AND b.mime_type LIKE 'audio/%'))))
 ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1
) m ON true
ORDER BY o.content_public_ref,o.display_ordinal NULLS LAST,o.producer_ordinal NULLS LAST,o.slot_key,component.kind
"#).bind(&works).bind(as_of).bind(sqlx::types::Json(&versions)).fetch_all(&mut **tx).await?;
    for row in rows {
        let work: Uuid = row.get("content_public_ref");
        let source: Option<Uuid> = row.get("materialization_ref");
        let sha: Option<String> = row.get("blob_sha256");
        let restricted: bool = row.get("restricted");
        let unavailable: bool = row.get("bytes_unavailable");
        let original = if restricted || unavailable {
            None
        } else {
            source
                .zip(sha.as_deref())
                .map(|(reference, hash)| format!("/api/local/media/{reference}/{hash}"))
        };
        let readable = original.is_some()
            && local_readable(
                row.get::<Option<String>, _>("storage_key").as_deref(),
                row.get::<Option<i64>, _>("byte_size"),
            );
        result.entry(work).or_default().slots.push(json!({
            "slotKey":row.get::<String,_>("slot_key"),"purpose":row.get::<String,_>("purpose"),
            "component":row.get::<String,_>("component_kind"),"processing":row.get::<Option<Value>,_>("processing"),
            "page":row.get::<Option<i32>,_>("display_ordinal"),
            "producerOrdinal":row.get::<Option<i32>,_>("producer_ordinal"),
            "displayOrderState":row.get::<String,_>("display_order_state"),
            "packageRef":row.get::<Uuid,_>("package_ref"),"observationRef":row.get::<Uuid,_>("observation_ref"),
            "sourceGeneration":row.get::<i32,_>("source_generation"),"sourceRef":source,
            "sourceVersion":source.map(|r|r.to_string()),"blobSha256":sha,"originalUrl":original,
            "localReadable":readable,"download":row.get::<Option<Value>,_>("download"),
            "sourceState":if restricted {"restricted"} else if source.is_some() {"present"} else {"not_acquired"}
        }));
    }
    read_historical_manifests(tx, currents, as_of, &mut result).await?;
    read_tags(tx, &works, as_of, &mut result).await?;
    Ok(result)
}

fn local_readable(storage: Option<&str>, size: Option<i64>) -> bool {
    use std::path::{Component, Path};
    let Some(storage) = storage.map(Path::new) else {
        return false;
    };
    if storage
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return false;
    }
    let Some(root) = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
        .and_then(|root| std::fs::canonicalize(root).ok())
    else {
        return false;
    };
    let Ok(path) = std::fs::canonicalize(root.join(storage)) else {
        return false;
    };
    if !path.starts_with(&root) {
        return false;
    }
    std::fs::File::open(path)
        .and_then(|file| file.metadata())
        .is_ok_and(|metadata| {
            metadata.is_file() && size.is_some_and(|size| size > 0 && metadata.len() == size as u64)
        })
}

#[derive(Serialize)]
struct ImageVersion {
    work_ref: Uuid,
    package_ref: Option<Uuid>,
    legacy_cutoff: Option<String>,
    #[serde(skip)]
    manifest: Option<Value>,
}

async fn read_image_versions(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<Vec<ImageVersion>, sqlx::Error> {
    // A verified receipt is created only after byte integrity admission. Select
    // one complete package so observations from unfinished captures cannot be
    // mixed into the displayed image set. Dispositions still apply at read time.
    sqlx::query(
        r#"
WITH manifests AS MATERIALIZED (
 SELECT lane.content_public_ref AS work_ref,p.package_ref,p.accepted_at,lane.created_at,
        p.payload #> '{coverage,target,imageInventory}' AS inventory,
        (p.payload #>> '{coverage,target,imageInventory,expectedPages}')::numeric AS expected_pages
 FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package p USING(package_ref)
 WHERE lane.content_public_ref=ANY($1) AND lane.lane='media_slots'
 AND lane.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 AND p.payload #>> '{coverage,target,imageInventory,state}'='complete'
 AND p.payload #>> '{coverage,target,imageInventory,source}'='platform_image_list'
 AND jsonb_typeof(p.payload #> '{coverage,target,imageInventory,expectedPages}')='number'
), ready AS (
 SELECT DISTINCT ON (m.work_ref) m.*
 FROM manifests m
 WHERE (
   SELECT count(*)=m.expected_pages AND count(DISTINCT o.display_ordinal)=m.expected_pages
      AND count(*) FILTER (WHERE o.display_ordinal BETWEEN 1 AND m.expected_pages AND EXISTS (
          SELECT 1 FROM linggan_media_download_attempt a
          JOIN linggan_media_materialization f USING(download_attempt_ref)
          JOIN linggan_media_blob b ON b.sha256=f.blob_sha256
          WHERE a.media_observation_ref=o.observation_ref AND a.terminal_reason='acquired'
          AND f.verified_at<=$2::timestamptz AND b.mime_type LIKE 'image/%' AND b.byte_size>0
      ))=m.expected_pages
   FROM linggan_material_media_origin o
   JOIN linggan_runtime_record_disposition d ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal
   WHERE o.content_public_ref=m.work_ref AND o.package_ref=m.package_ref
   AND o.created_at<=$2::timestamptz AND o.purpose IN ('body_image','live_photo')
   AND d.disposition='accepted_for_media_identity'
 )
 ORDER BY m.work_ref,m.accepted_at DESC,m.created_at DESC,m.package_ref DESC
), first_manifest AS (
 SELECT DISTINCT ON (work_ref) * FROM manifests
 ORDER BY work_ref,accepted_at,created_at,package_ref
), selected AS (
 SELECT first.work_ref,
        COALESCE(ready.package_ref,CASE WHEN NOT legacy.exists THEN first.package_ref END) AS package_ref,
        CASE WHEN ready.package_ref IS NULL AND legacy.exists THEN first.accepted_at::text END AS legacy_cutoff,
        COALESCE(ready.inventory,CASE WHEN NOT legacy.exists THEN first.inventory END) AS inventory
 FROM first_manifest first LEFT JOIN ready USING(work_ref)
 CROSS JOIN LATERAL (
   SELECT EXISTS (
     SELECT 1 FROM linggan_material_media_origin o
     JOIN linggan_runtime_capture_package p USING(package_ref)
     JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref
     JOIN linggan_media_materialization f USING(download_attempt_ref)
     JOIN linggan_media_blob b ON b.sha256=f.blob_sha256
     WHERE o.content_public_ref=first.work_ref AND o.purpose IN ('body_image','live_photo')
     AND p.accepted_at<first.accepted_at AND o.created_at<=$2::timestamptz AND f.verified_at<=$2::timestamptz
     AND a.terminal_reason='acquired' AND b.mime_type LIKE 'image/%' AND b.byte_size>0
   ) AS exists
 ) legacy
)
SELECT work_ref,package_ref,legacy_cutoff,
       inventory || jsonb_build_object('packageRef',package_ref) AS manifest
FROM selected
"#,
    )
    .bind(works)
    .bind(as_of)
    .try_map(|row| {
        Ok(ImageVersion {
            work_ref: row.try_get("work_ref")?,
            package_ref: row.try_get("package_ref")?,
            legacy_cutoff: row.try_get("legacy_cutoff")?,
            manifest: row.try_get("manifest")?,
        })
    })
    .fetch_all(&mut **tx)
    .await
}

async fn read_historical_manifests(
    tx: &mut Transaction<'_, Postgres>,
    currents: &[&WorkResourceCurrent],
    as_of: &str,
    inventories: &mut BTreeMap<Uuid, Inventory>,
) -> Result<(), sqlx::Error> {
    let missing: Vec<_> = currents
        .iter()
        .filter(|work| {
            inventories
                .get(&work.public_ref)
                .is_some_and(|i| i.manifest.is_none())
        })
        .filter_map(|work| work.detail_material_ref)
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query(
        r#"
SELECT d.content_public_ref, d.package_ref, d.material_ref,
       h.observation_refs, h.materialization_refs
FROM linggan_historical_image_inventory h
JOIN linggan_material_content_detail d ON d.material_ref=h.detail_ref
JOIN linggan_runtime_capture_package p USING(package_ref)
JOIN linggan_runtime_record_disposition disposition USING(package_ref,record_ordinal)
WHERE h.detail_ref=ANY($1) AND p.platform='xhs'
  AND disposition.disposition='accepted_for_library_content'
  AND h.source_excerpt_sha256=encode(sha256(convert_to(
    p.payload->'records'->d.record_ordinal->'payload'->>'rawPayload','UTF8'
  )),'hex')
  AND h.recorded_at<=$2::timestamptz AND d.created_at<=$2::timestamptz
  AND p.accepted_at<=$2::timestamptz
"#,
    )
    .bind(missing)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    for row in rows {
        let work: Uuid = row.get("content_public_ref");
        let inventory = inventories.entry(work).or_default();
        let observed: Vec<Uuid> = row.get("observation_refs");
        let materialized: Vec<Uuid> = row.get("materialization_refs");
        if apply_historical_pages(&mut inventory.slots, &observed, &materialized) {
            inventory
                .slots
                .sort_by_key(|slot| slot["page"].as_u64().unwrap_or(u64::MAX));
            inventory.manifest = Some(json!({
                "state":"complete",
                "source":"platform_image_list",
                "expectedPages":observed.len(),
                "packageRef":row.get::<Uuid,_>("package_ref"),
                "historicalDetailRef":row.get::<Uuid,_>("material_ref")
            }));
        }
    }
    Ok(())
}

fn apply_historical_pages(slots: &mut [Value], observed: &[Uuid], materialized: &[Uuid]) -> bool {
    if observed.is_empty()
        || observed.len() != materialized.len()
        || slots
            .iter()
            .any(|s| matches!(s["purpose"].as_str(), Some("video" | "live_photo")))
    {
        return false;
    }
    let mut body: Vec<_> = slots
        .iter_mut()
        .filter(|s| s["purpose"] == "body_image" && s["component"] == "image")
        .collect();
    body.sort_by_key(|s| s["producerOrdinal"].as_u64());
    let matches = body.len() == observed.len()
        && body
            .iter()
            .zip(observed.iter().zip(materialized))
            .enumerate()
            .all(|(index, (slot, (origin, file)))| {
                let page = json!(index + 1);
                slot["producerOrdinal"] == page
                    && (slot["page"].is_null() || slot["page"] == page)
                    && slot["observationRef"] == json!(origin)
                    && slot["sourceRef"] == json!(file)
                    && slot["sourceState"] == "present"
                    && slot["localReadable"] == true
            });
    if matches {
        for (index, slot) in body.into_iter().enumerate() {
            slot["page"] = json!(index + 1);
            slot["displayOrderState"] = json!("KNOWN");
        }
    }
    matches
}

async fn read_tags(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
    result: &mut BTreeMap<Uuid, Inventory>,
) -> Result<(), sqlx::Error> {
    let rows=sqlx::query(r#"
SELECT d.content_public_ref,d.material_ref,p.state,p.projection_contract,
 COALESCE(jsonb_agg(h.tag_text ORDER BY h.ordinal) FILTER (WHERE h.ordinal IS NOT NULL),'[]'::jsonb) AS tags
FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package capture USING(package_ref)
JOIN linggan_material_hashtag_projection p ON p.material_ref=d.material_ref
LEFT JOIN linggan_material_content_hashtag h ON h.material_ref=p.material_ref AND h.projection_contract=p.projection_contract
WHERE d.content_public_ref=ANY($1) AND p.created_at<=$2::timestamptz
 AND d.created_at<=$2::timestamptz AND capture.accepted_at<=$2::timestamptz
 AND p.projection_contract=$3
GROUP BY d.content_public_ref,d.material_ref,p.state,p.projection_contract
"#).bind(works).bind(as_of).bind(crate::material_hashtag::VERSION).fetch_all(&mut **tx).await?;
    for row in rows {
        let work: Uuid = row.get("content_public_ref");
        let source: Uuid = row.get("material_ref");
        result.entry(work).or_default().tags.insert(source,json!({"state":row.get::<String,_>("state"),
            "ruleVersion":row.get::<String,_>("projection_contract"),"values":row.get::<Value,_>("tags")}));
    }
    Ok(())
}
