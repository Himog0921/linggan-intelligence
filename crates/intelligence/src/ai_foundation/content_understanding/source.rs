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
    let mut d = linggan_evidence::research_dossier::read_in(tx, &[work], cutoff)
        .await?
        .remove(&work)
        .ok_or(FoundationError::NotFound)?;
    // Video identity can be present in the accepted detail even when collection
    // did not produce a video media slot. Consult the captured type as of cutoff.
    let source_type: Option<String> = sqlx::query_scalar(
        "SELECT source_type FROM (SELECT CASE WHEN raw.payload->>'type' = 'video'
            OR (jsonb_typeof(raw.payload->'video') = 'string' AND btrim(raw.payload->>'video') <> '')
            OR (jsonb_typeof(raw.payload->'videoDownloadUrl') = 'string' AND btrim(raw.payload->>'videoDownloadUrl') <> '')
            OR (jsonb_typeof(raw.payload->'videoPlayUrl') = 'string' AND btrim(raw.payload->>'videoPlayUrl') <> '')
            OR EXISTS (SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(raw.payload->'videoStreams')='array' THEN raw.payload->'videoStreams' ELSE '[]'::jsonb END) stream WHERE jsonb_typeof(stream->'url')='string' AND btrim(stream->>'url') <> '')
          THEN 'video' ELSE 'normal' END AS source_type, d.created_at, d.material_ref
         FROM linggan_material_content_detail d
         JOIN linggan_runtime_capture_package p USING(package_ref)
         JOIN linggan_runtime_submission_receipt r ON r.attempt_id=p.attempt_id
         JOIN linggan_runtime_record_disposition x ON x.package_ref=d.package_ref AND x.record_ordinal=d.record_ordinal
         CROSS JOIN LATERAL (SELECT p.payload->'records'->d.record_ordinal->'payload' AS payload) raw
         WHERE d.content_public_ref=$1 AND d.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
          AND r.material_admission='ACCEPTED' AND x.disposition<>'quarantined'
          AND (raw.payload->>'type' IN ('video','normal') OR jsonb_typeof(raw.payload->'video')='string'
            OR jsonb_typeof(raw.payload->'videoDownloadUrl')='string' OR jsonb_typeof(raw.payload->'videoPlayUrl')='string'
            OR jsonb_typeof(raw.payload->'videoStreams')='array')
         ) typed ORDER BY (source_type='video') DESC, created_at DESC, material_ref DESC LIMIT 1",
    ).bind(work).bind(cutoff).fetch_optional(&mut **tx).await?;
    if source_type.as_deref() == Some("video") {
        d.note["sourceContentType"] = json!("video");
    }
    from_dossier(d, cutoff)
}

/// The same material identity for single-work starts and bounded scope qualification.
pub(super) fn from_dossier(
    d: linggan_evidence::research_dossier::ResearchDossier,
    cutoff: &str,
) -> Result<Packet, FoundationError> {
    let work = d.work_ref;
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
                v => v,
            }
            .into(),
        })
        .collect();
    let inventory: Vec<Value> = d.media_inventory.iter().map(|s| json!({
        "slotKey":s["slotKey"],"purpose":s["purpose"],"component":s["component"],
        "page":s["page"],"producerOrdinal":s["producerOrdinal"],"displayOrderState":s["displayOrderState"],
        "sourceRef":s["sourceRef"],"sourceVersion":s["sourceVersion"],"blobSha256":s["blobSha256"],
        "sourceState":s["sourceState"],"available":s["originalUrl"].is_string() && s["localReadable"] == true,
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
    let readiness = linggan_evidence::research_dossier::l1_material_readiness(&d);
    let complete = readiness.kind != linggan_evidence::research_dossier::NoteKind::Video
        && !d.coverage["issues"].as_array().is_some_and(|issues| {
            issues
                .iter()
                .any(|issue| issue["code"] == "image_update_pending")
        })
        && d.coverage["imageInventory"]["expectedPages"].as_u64() == Some(image_slots)
        && inventory
            .iter()
            .all(|s| s["available"] == true && s["displayOrderState"] != "unknown")
        && d.materials
            .iter()
            .filter(|m| m["kind"] != "image")
            .all(|m| {
                if m["kind"] == "audio" {
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
    let mut coverage = d.coverage;
    coverage["materialReadiness"] = json!(readiness);
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
        coverage,
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
    if !video_transcripts_included(&saved, &current)
        || !saved_sources_available(&saved, &current)
        || saved.hash() != row.get::<String, _>("input_hash")
        || current.coverage["materialReadiness"]["state"] != "ready"
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(saved)
}
// A queued request must contain the current video transcripts in its frozen
// input. A transcript added after enqueueing cannot repair that old payload.
fn video_transcripts_included(saved: &Packet, current: &Packet) -> bool {
    if current.coverage["materialReadiness"]["kind"] != "video" {
        return true;
    }
    let video_slots: Vec<_> = saved
        .inventory
        .as_array()
        .into_iter()
        .flatten()
        .filter(|slot| slot["purpose"] == "video" && slot["component"] == "motion")
        .collect();
    !video_slots.is_empty()
        && video_slots.iter().all(|slot| {
            let source_ref = slot["sourceRef"]
                .as_str()
                .and_then(|value| value.parse::<Uuid>().ok());
            saved.snapshot.texts.iter().any(|text| {
                text.kind == "transcript"
                    && !text.text.trim().is_empty()
                    && Some(text.source_ref) == source_ref
            })
        })
}

/// Reconstruct the frozen source at its original material time. The dossier applies
/// current source restrictions even for historical reads, so replacement alone does
/// not hide a saved result, while a subsequent withdrawal still does.
pub(crate) async fn saved_result_available(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    cutoff: &str,
    saved: &Packet,
    current: &Packet,
) -> Result<bool, FoundationError> {
    // DEC-0013 removed video-frame text from the canonical reader at every cutoff.
    // Its old packets can never be requalified by rereading a historical dossier.
    if saved
        .snapshot
        .texts
        .iter()
        .any(|text| text.kind == "frame_text")
    {
        return Ok(false);
    }
    let within_cutoff: bool = sqlx::query_scalar("SELECT $1::timestamptz <= $2::timestamptz")
        .bind(&saved.snapshot.material_as_of)
        .bind(cutoff)
        .fetch_one(&mut **tx)
        .await?;
    if !within_cutoff {
        return Ok(false);
    }
    if saved_sources_available(saved, current) {
        return Ok(true);
    }
    let historical =
        linggan_evidence::research_dossier::read_in(tx, &[work], &saved.snapshot.material_as_of)
            .await?
            .remove(&work);
    match historical {
        Some(dossier) => Ok(saved_sources_available(
            saved,
            &from_dossier(dossier, &saved.snapshot.material_as_of)?,
        )),
        None => Ok(false),
    }
}

pub(crate) fn saved_sources_available(saved: &Packet, current: &Packet) -> bool {
    let saved_text = serde_json::to_value(&saved.snapshot.texts).unwrap_or(Value::Null);
    let current_text = serde_json::to_value(&current.snapshot.texts).unwrap_or(Value::Null);
    let texts_available = saved_text
        .as_array()
        .zip(current_text.as_array())
        .is_some_and(|(old, new)| {
            old.iter()
                .all(|source| new.iter().any(|candidate| candidate == source))
        });
    let images_available = saved.snapshot.images.iter().all(|image| {
        current.snapshot.images.iter().any(|candidate| {
            image.source_ref == candidate.source_ref
                && image.source_version == candidate.source_version
                && image.blob_sha256 == candidate.blob_sha256
        })
    });
    saved.snapshot.work_ref == current.snapshot.work_ref
        && (!saved.snapshot.texts.is_empty() || !saved.snapshot.images.is_empty())
        && texts_available
        && images_available
}

/// Images referenced by a frozen v2 manifest, in manifest order.
pub(crate) async fn images_for_manifest_in(
    tx: &mut Transaction<'_, Postgres>,
    manifest: &Value,
) -> Result<Vec<crate::pi_adapter::PiImage>, super::super::executor::ExecutorError> {
    let mut images = Vec::new();
    for material in manifest.as_array().into_iter().flatten() {
        if !matches!(material["materialKind"].as_str(), Some("cover" | "image")) {
            continue;
        }
        let source_ref: Uuid = serde_json::from_value(material["sourceRef"].clone())
            .map_err(|_| super::super::executor::ExecutorError::Invalid)?;
        let hash = material["mediaSha256"]
            .as_str()
            .ok_or(super::super::executor::ExecutorError::Invalid)?;
        let r = sqlx::query("SELECT b.storage_key,b.mime_type,b.byte_size FROM linggan_media_materialization m JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 WHERE m.materialization_ref=$1 AND b.sha256=$2")
            .bind(source_ref).bind(hash).fetch_one(&mut **tx).await?;
        images.push(
            super::super::executor::ocr_recognition::image(
                &r.get::<String, _>("storage_key"),
                &r.get::<String, _>("mime_type"),
                r.get("byte_size"),
                hash,
            )?
            .0,
        );
    }
    Ok(images)
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

#[cfg(test)]
mod tests {
    use super::*;
    use linggan_evidence::research_dossier::ResearchDossier;

    #[test]
    fn queued_video_input_must_include_current_transcript() {
        let mut saved = Packet {
            snapshot: Snapshot {
                work_ref: Uuid::new_v4(),
                material_snapshot_identity: "old".into(),
                material_as_of: "2026-11-17T00:00:00Z".into(),
                included_kinds: vec![],
                coverage_mode: "partial".into(),
                texts: vec![],
                images: vec![],
            },
            inventory: json!([]),
            coverage: json!({"materialReadiness":{"kind":"video"}}),
        };
        let source_ref = Uuid::new_v4();
        saved.inventory = json!([{"purpose":"video","component":"motion","sourceRef":source_ref}]);
        let mut current = saved.clone();
        current.snapshot.texts.push(TextSource {
            citation_id: "speech".into(),
            source_ref,
            source_version: "1".into(),
            start: 0,
            text: "完整转录".into(),
            kind: "transcript".into(),
            speaker: "author".into(),
            structure: json!({}),
            issues: vec![],
        });
        assert!(!video_transcripts_included(&saved, &current));
        saved.snapshot.texts = current.snapshot.texts.clone();
        assert!(video_transcripts_included(&saved, &current));
        current.snapshot.texts.push(TextSource {
            citation_id: "additional".into(),
            source_ref,
            source_version: "1".into(),
            start: 0,
            text: "调用期间新加入的转录".into(),
            kind: "transcript".into(),
            speaker: "author".into(),
            structure: json!({}),
            issues: vec![],
        });
        assert!(video_transcripts_included(&saved, &current));
        saved.snapshot.texts[0].source_ref = Uuid::new_v4();
        assert!(!video_transcripts_included(&saved, &current));
    }

    #[test]
    fn pending_image_update_cannot_be_reported_as_complete() {
        let source = Uuid::new_v4();
        let mut dossier = ResearchDossier {
            contract: "research-dossier.v2",
            work_ref: Uuid::new_v4(),
            revision: "r".into(),
            material_revision: "m".into(),
            as_of: "2026-11-16T00:00:00Z".into(),
            note: json!({}),
            materials: vec![
                json!({"kind":"body","sourceRef":"native","sourceState":"known_empty","blocks":[]}),
            ],
            reading_view: json!({}),
            coverage: json!({"imageInventory":{"expectedPages":1},"issues":[]}),
            attachments: json!({}),
            selection: json!({}),
            fragments: vec![],
            media: vec![],
            media_inventory: vec![
                json!({"slotKey":"image:1","purpose":"body_image","component":"image","page":1,
                "displayOrderState":"KNOWN","sourceRef":source,"sourceVersion":source.to_string(),"blobSha256":"a".repeat(64),
                "sourceState":"present","originalUrl":"/local/image","localReadable":true}),
            ],
            candidate_statuses: vec![],
            metadata: json!({}),
        };
        let complete = from_dossier(dossier.clone(), "2026-11-16T00:00:00Z").unwrap();
        assert_eq!(complete.snapshot.coverage_mode, "complete");
        dossier.coverage["issues"] = json!([{"code":"image_update_pending"}]);
        let pending = from_dossier(dossier, "2026-11-16T00:00:00Z").unwrap();
        assert_eq!(pending.snapshot.coverage_mode, "partial");
        assert_eq!(pending.coverage["materialReadiness"]["state"], "ready");
        assert!(saved_sources_available(&complete, &pending));
        let mut extra = pending.clone();
        extra.snapshot.images.push(ImageSource {
            citation_id: "image_2".into(),
            source_ref: Uuid::new_v4(),
            source_version: "new".into(),
            blob_sha256: "b".repeat(64),
        });
        assert!(saved_sources_available(&complete, &extra));
        extra.snapshot.images.clear();
        assert!(!saved_sources_available(&complete, &extra));
    }
}
