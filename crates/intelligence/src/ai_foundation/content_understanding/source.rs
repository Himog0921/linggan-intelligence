use super::*;
use crate::content_understanding::{ImageSource, Snapshot, TextSource};
use std::collections::BTreeSet;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Packet {
    pub snapshot: Snapshot,
    pub inventory: Value,
    pub coverage: Value,
}
impl Packet {
    pub fn hash(&self) -> String {
        crate::research_text::content_hash(
            &serde_json::to_string(self).expect("packet serialization"),
        )
    }
}
pub(crate) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    cutoff: &str,
) -> Result<Packet, FoundationError> {
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2 AND created_at<=$3::timestamptz)")
        .bind(domain).bind(work).bind(cutoff).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::NotFound);
    }
    let d = linggan_evidence::research_dossier::read_in(tx, &[work], cutoff)
        .await?
        .remove(&work)
        .ok_or(FoundationError::NotFound)?;
    let texts: Vec<_> = d
        .fragments
        .iter()
        .filter(|f| {
            [
                "title",
                "native_title",
                "body",
                "tags",
                "transcript",
                "asr_text",
                "frame_text",
                "frame_ocr_text",
            ]
            .contains(&f.field.as_str())
                && !f.text.trim().is_empty()
        })
        .map(|f| TextSource {
            citation_id: f.fragment_id.clone(),
            source_ref: f.source_ref,
            source_version: f.source_version.clone(),
            start: f.start,
            text: f.text.clone(),
            speaker: f.speaker.clone(),
            structure: {
                let mut structure = f.structure.clone();
                if let Some(fields) = structure.as_object_mut() {
                    fields.remove("text");
                }
                structure
            },
            issues: f.issues.clone(),
            kind: match f.field.as_str() {
                "native_title" => "title",
                "asr_text" => "transcript",
                "frame_ocr_text" => "frame_text",
                v => v,
            }
            .into(),
        })
        .collect();
    let inventory: Vec<Value> = d.media_inventory.iter().map(|s| json!({
        "slotKey":s["slotKey"],"purpose":s["purpose"],"component":s["component"],
        "page":s["page"],"producerOrdinal":s["producerOrdinal"],"displayOrderState":s["displayOrderState"],
        "sourceRef":s["sourceRef"],"sourceVersion":s["sourceVersion"],"blobSha256":s["blobSha256"],
        "sourceState":s["sourceState"],"available":s["originalUrl"].is_string(),
        "sameBytesSources":s["sameBytesSources"]
    })).collect();
    let mut seen = BTreeSet::new();
    let mut images = Vec::new();
    for s in &inventory {
        if s["component"] != "image" || s["available"] != true {
            continue;
        }
        let hash = s["blobSha256"].as_str().ok_or(FoundationError::Invalid)?;
        if !seen.insert(hash.to_owned()) {
            continue;
        }
        let source_ref = s["sourceRef"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or(FoundationError::Invalid)?;
        images.push(ImageSource {
            citation_id: format!("image_{}", images.len() + 1),
            source_ref,
            source_version: s["sourceVersion"]
                .as_str()
                .ok_or(FoundationError::Invalid)?
                .into(),
            blob_sha256: hash.into(),
        });
    }
    let mut kinds: BTreeSet<String> = texts.iter().map(|s| s.kind.clone()).collect();
    if !images.is_empty() {
        kinds.insert("image".into());
    }
    let image_slots = inventory
        .iter()
        .filter(|s| s["component"] == "image")
        .count() as u64;
    let complete = d.coverage["imageInventory"]["expectedPages"].as_u64() == Some(image_slots)
        && inventory
            .iter()
            .all(|s| s["available"] == true && s["displayOrderState"] != "unknown")
        && d.materials
            .iter()
            .filter(|m| m["kind"] != "image")
            .all(|m| {
                if m["kind"] == "audio" || m["kind"] == "video_frame" {
                    m["sourceState"] == "not_applicable"
                        || m["sourceState"] == "known_empty"
                        || (m["sourceState"] == "present" && m["currentRevisionRef"].is_string())
                } else {
                    !["not_acquired", "restricted", "unavailable"]
                        .iter()
                        .any(|s| m["sourceState"] == *s)
                }
            });
    let coverage_mode = if complete { "complete" } else { "partial" };
    let identity = crate::research_text::content_hash(
        &json!([
            "content-understanding.material.v1",
            work,
            texts,
            inventory,
            d.coverage["imageInventory"]["expectedPages"],
            coverage_mode
        ])
        .to_string(),
    );
    Ok(Packet {
        snapshot: Snapshot {
            work_ref: work,
            material_snapshot_identity: identity,
            material_as_of: cutoff.into(),
            included_kinds: kinds.into_iter().collect(),
            coverage_mode: coverage_mode.into(),
            texts,
            images,
        },
        inventory: json!(inventory),
        coverage: d.coverage,
    })
}
pub(crate) async fn frozen_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Packet, FoundationError> {
    let row = sqlx::query("SELECT domain_ref,input_manifest,input_hash FROM linggan_content_understanding_request WHERE request_ref=$1")
        .bind(request).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let saved: Packet =
        serde_json::from_value(row.get("input_manifest")).map_err(|_| FoundationError::Invalid)?;
    let now = linggan_evidence::research_dossier::cutoff_in(tx, None).await?;
    let current = load(tx, row.get("domain_ref"), saved.snapshot.work_ref, &now).await?;
    if current.snapshot.material_snapshot_identity != saved.snapshot.material_snapshot_identity
        || saved.hash() != row.get::<String, _>("input_hash")
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(saved)
}
pub(crate) async fn images_in(
    tx: &mut Transaction<'_, Postgres>,
    packet: &Packet,
) -> Result<Vec<crate::pi_adapter::PiImage>, super::super::executor::ExecutorError> {
    let mut images = Vec::new();
    for image in &packet.snapshot.images {
        let r = sqlx::query("SELECT b.storage_key,b.mime_type,b.byte_size FROM linggan_media_materialization m JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 WHERE m.materialization_ref=$1 AND b.sha256=$2")
            .bind(image.source_ref).bind(&image.blob_sha256).fetch_one(&mut **tx).await?;
        images.push(
            super::super::executor::ocr_recognition::image(
                &r.get::<String, _>("storage_key"),
                &r.get::<String, _>("mime_type"),
                r.get("byte_size"),
                &image.blob_sha256,
            )?
            .0,
        );
    }
    Ok(images)
}
