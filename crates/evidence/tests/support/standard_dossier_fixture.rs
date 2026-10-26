//! Saved-result replay shared by isolated PG and actual React browser proofs.
//! Default scenarios are synthetic; optional private inputs remain local to the owned proof.
#[path = "material_fixture.rs"]
pub mod fixture;
use linggan_evidence::{material_reading as reading, *};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
pub const DOMAIN: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);

pub async fn scenario(
    db: &Database,
    name: &str,
    body: Option<&str>,
    images: u32,
    video: bool,
    partial: bool,
    empty: bool,
) -> Uuid {
    let package = fixture::submit_package(db,"content_detail",json!({"contentExternalId":name}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":name},"payload":{"bodyText":body,"authorId":"synthetic-author"}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(name)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    let mut slots = vec![];
    for page in 1..=images {
        slots.push((if video && page == 1 { "cover" } else { "image" }, page));
    }
    if video {
        slots.push(("video", 1));
    }
    let observations: Vec<_> = slots.iter().map(|_| Uuid::new_v4()).collect();
    let records=slots.iter().zip(&observations).map(|((role,page),observation)|{
        let mut slot=json!({"role":role,"ordinal":page});
        if *role=="image"{slot["displayOrdinal"]=json!(page);slot["displayOrderBasis"]=json!("platform_explicit");}
        let uri=format!("https://media.example/{name}/{role}/{page}");
        json!({"kind":"media_slot","slotKey":format!("xhs:{name}:{role}:{page}"),"observationRef":observation,"slot":slot,"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-14T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":name}})
    }).collect();
    fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":name}),"media_slots","xhs",json!({"target":if video{json!({"contentExternalId":name})}else{json!({"contentExternalId":name,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":images}})},"layers":[fixture::coverage_layer("media_slots",slots.len() as i64)]}),records).await;
    for ((role, page), observation) in slots.iter().zip(observations) {
        if partial && *page == images {
            continue;
        }
        let data = if *role == "video" {
            std::env::var_os("STANDARD_DOSSIER_SYNTHETIC_VIDEO")
                .map(|p| std::fs::read(p).unwrap())
                .unwrap_or_else(|| b"synthetic-video-bytes".to_vec())
        } else {
            vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0,
                1, 8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252,
                255, 31, 0, 3, 3, 2, 0, 239, 154, 237, 127, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
                96, 130,
            ]
        };
        use sha2::{Digest, Sha256};
        let sha = Sha256::digest(&data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let key = format!(
            "standard/{name}/{role}/{page}.{}",
            if *role == "video" { "mp4" } else { "png" }
        );
        if let Some(root) = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT") {
            let path = std::path::PathBuf::from(root).join(&key);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &data).unwrap();
        }
        let source = admit_media_blob(
            db,
            observation,
            &sha,
            if *role == "video" {
                "video/mp4"
            } else {
                "image/png"
            },
            data.len() as i64,
            &key,
        )
        .await
        .unwrap()
        .materialization_ref;
        let slot = format!("xhs:{name}:{role}:{page}");
        if partial && *page == images - 1 {
            let job:Uuid=sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='image_ocr' ORDER BY created_at DESC LIMIT 1").bind(&slot).fetch_one(db.pool()).await.unwrap();
            sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'failed','synthetic_decode_failure')").bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
            continue;
        }
        if empty {
            continue;
        }
        let kinds: &[&str] = if *role == "video" {
            &["audio", "video_frame"]
        } else {
            &["image"]
        };
        for kind in kinds {
            let blocks = if *kind == "image" {
                vec![
                    json!({"ref":"heading","kind":"heading","text":format!("合成第{page}页标题"),"speaker":"unconfirmed"}),
                    json!({"ref":"text","kind":"paragraph","text":format!("{name} 第{page}页完整文字，否定与0.5 mg均保留。"),"speaker":"unconfirmed"}),
                ]
            } else {
                vec![
                    json!({"ref":"timed","kind":"paragraph","text":if *kind=="audio"{"合成口播：先看条件，再做决定。"}else{"合成画面文字：补充图示。"},"speaker":"unconfirmed","time":{"startMs":1000,"endMs":if *kind=="audio"{3000}else{1000}}}),
                ]
            };
            let mut tx = db.pool().begin().await.unwrap();
            reading::publish_in(
                &mut tx,
                reading::NewReading {
                    content_public_ref: work,
                    unit_key: format!("{kind}:{slot}"),
                    kind: (*kind).into(),
                    slot_key: Some(slot.clone()),
                    page: if *kind == "image" {
                        Some(i32::try_from(*page).unwrap())
                    } else {
                        None
                    },
                    source_version: source.to_string(),
                    source_state: "present".into(),
                    derivative_ref: None,
                    producer_ref: Some(Uuid::new_v4()),
                    generated_by: match *kind {
                        "audio" => "asr",
                        "video_frame" => "frame_ocr",
                        _ => "cloud_vision",
                    }
                    .into(),
                    rule_version: "standard-synthetic-replay.v1".into(),
                    blocks,
                    issues: vec![],
                    source_location: json!({"synthetic":true}),
                    publication: reading::Publication::Automatic {
                        expected_current: None,
                    },
                },
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    }
    work
}

/// Private replay only: saved recognizer output is not adopted into the source database.
/// The fresh source/reading identities below belong exclusively to the disposable proof DB.
pub async fn replay_saved(db: &Database, entry: &serde_json::Value) -> Uuid {
    let name = entry["name"].as_str().unwrap();
    let body = entry["body"]["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(linggan_evidence::research_dossier::content::block_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    let images = entry["images"].as_array().unwrap();
    let work = scenario(
        db,
        name,
        (!body.is_empty()).then_some(body.as_str()),
        images.len().try_into().unwrap(),
        false,
        false,
        true,
    )
    .await;
    if let Some(title) = entry["note"]["platformTitle"].as_str() {
        fixture::submit_package(db,"content_detail",json!({"contentExternalId":name}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":name},"payload":{"title":title,"bodyText":body}})).await;
    }
    for (index, image) in images.iter().enumerate() {
        let slot = format!("xhs:{name}:image:{}", index + 1);
        let observation:Uuid=sqlx::query_scalar("SELECT observation_ref FROM linggan_material_media_origin WHERE slot_key=$1 ORDER BY source_generation DESC LIMIT 1").bind(&slot).fetch_one(db.pool()).await.unwrap();
        let bytes = std::fs::read(image["path"].as_str().unwrap()).unwrap();
        use sha2::{Digest, Sha256};
        let sha = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(image["sha256"], sha);
        let key = format!("saved-replay/{name}/{}.bin", index + 1);
        let path = std::path::PathBuf::from(std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT").unwrap())
            .join(&key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &bytes).unwrap();
        let source = admit_media_blob(
            db,
            observation,
            &sha,
            image["mimeType"].as_str().unwrap(),
            bytes.len() as i64,
            &key,
        )
        .await
        .unwrap()
        .materialization_ref;
        let mut tx = db.pool().begin().await.unwrap();
        reading::publish_in(&mut tx,reading::NewReading{content_public_ref:work,unit_key:format!("image:{slot}"),kind:"image".into(),slot_key:Some(slot),page:Some((index+1).try_into().unwrap()),source_version:source.to_string(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:image["generatedBy"].as_str().unwrap().into(),rule_version:"private-saved-result-replay.v1".into(),blocks:serde_json::from_value(image["blocks"].clone()).unwrap(),issues:serde_json::from_value(image["issues"].clone()).unwrap(),source_location:json!({"proofPurpose":"offline_replay_only","originalWorkRef":entry["sourceWorkRef"],"originalReadingRef":image["sourceReadingRef"]}),publication:reading::Publication::Automatic{expected_current:None}}).await.unwrap();
        tx.commit().await.unwrap();
    }
    // Copy exact block structure from the existing platform reading when available.
    if !body.is_empty() {
        let d = linggan_evidence::research_dossier::read(db, work)
            .await
            .unwrap()
            .unwrap();
        let unit = d.materials.iter().find(|m| m["kind"] == "body").unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        reading::publish_in(&mut tx,reading::NewReading{content_public_ref:work,unit_key:"body".into(),kind:"body".into(),slot_key:None,page:None,source_version:unit["sourceVersion"].as_str().unwrap().into(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"platform".into(),rule_version:"private-saved-result-replay.v1".into(),blocks:serde_json::from_value(entry["body"]["blocks"].clone()).unwrap(),issues:vec![],source_location:json!({"proofPurpose":"offline_replay_only","originalWorkRef":entry["sourceWorkRef"]}),publication:reading::Publication::Automatic{expected_current:serde_json::from_value::<Option<Uuid>>(unit["currentRevisionRef"].clone()).unwrap_or(None)}}).await.unwrap();
        tx.commit().await.unwrap();
    }
    work
}
