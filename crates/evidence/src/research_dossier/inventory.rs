//! One source inventory shared by full reading, coverage and explicit local rebuilding.
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
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Inventory>, sqlx::Error> {
    let mut result = BTreeMap::<Uuid, Inventory>::new();
    let rows = sqlx::query(r#"
WITH origins AS (
 SELECT DISTINCT ON (o.slot_key) o.*
 FROM linggan_material_media_origin o JOIN linggan_runtime_capture_package p USING(package_ref)
 WHERE o.content_public_ref=ANY($1) AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 AND o.purpose NOT IN ('comment_image','author_avatar')
 ORDER BY o.slot_key,o.source_generation DESC
)
SELECT o.content_public_ref,o.slot_key,o.purpose,o.display_ordinal,o.producer_ordinal,o.display_order_state,component.kind AS component_kind,
 o.package_ref,o.observation_ref,o.source_generation,m.materialization_ref,m.blob_sha256,
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
 SELECT m.* FROM linggan_media_materialization m JOIN linggan_media_download_attempt a USING(download_attempt_ref)
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
 WHERE a.media_observation_ref=o.observation_ref AND m.verified_at<=$2::timestamptz
 AND ((component.kind='image' AND b.mime_type LIKE 'image/%') OR
 (component.kind='motion' AND (b.mime_type LIKE 'video/%' OR (o.purpose<>'live_photo' AND b.mime_type LIKE 'audio/%'))))
 ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1
) m ON true WHERE linggan_dossier_slot_active(o.content_public_ref,o.slot_key,$2::timestamptz)
ORDER BY o.content_public_ref,o.display_ordinal NULLS LAST,o.producer_ordinal NULLS LAST,o.slot_key,component.kind
"#).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
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
        result.entry(work).or_default().slots.push(json!({
            "slotKey":row.get::<String,_>("slot_key"),"purpose":row.get::<String,_>("purpose"),
            "component":row.get::<String,_>("component_kind"),"processing":row.get::<Option<Value>,_>("processing"),
            "page":row.get::<Option<i32>,_>("display_ordinal"),
            "producerOrdinal":row.get::<Option<i32>,_>("producer_ordinal"),
            "displayOrderState":row.get::<String,_>("display_order_state"),
            "packageRef":row.get::<Uuid,_>("package_ref"),"observationRef":row.get::<Uuid,_>("observation_ref"),
            "sourceGeneration":row.get::<i32,_>("source_generation"),"sourceRef":source,
            "sourceVersion":source.map(|r|r.to_string()),"blobSha256":sha,"originalUrl":original,
            "sourceState":if restricted {"restricted"} else if source.is_some() {"present"} else {"not_acquired"}
        }));
    }
    read_manifests(tx, works, as_of, &mut result).await?;
    read_tags(tx, works, as_of, &mut result).await?;
    Ok(result)
}

async fn read_manifests(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
    result: &mut BTreeMap<Uuid, Inventory>,
) -> Result<(), sqlx::Error> {
    let rows=sqlx::query(r#"
SELECT DISTINCT ON (lane.content_public_ref) lane.content_public_ref,package.package_ref,
 package.payload #> '{coverage,target,imageInventory}' AS manifest
FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package package USING(package_ref)
WHERE lane.content_public_ref=ANY($1) AND lane.lane='media_slots'
 AND lane.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz
ORDER BY lane.content_public_ref,package.accepted_at DESC,lane.created_at DESC,package.package_ref DESC
"#).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    for row in rows {
        let work: Uuid = row.get("content_public_ref");
        let manifest: Option<Value> = row.get("manifest");
        if let Some(mut manifest) = manifest.filter(|m| m.is_object()) {
            manifest["packageRef"] = json!(row.get::<Uuid, _>("package_ref"));
            result.entry(work).or_default().manifest = Some(manifest);
        }
    }
    Ok(())
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
