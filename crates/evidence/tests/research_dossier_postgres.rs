//! Synthetic PostgreSQL proofs for the sole full-note dossier contract.
use linggan_evidence::{research_dossier as dossier, *};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/research_dossier_fixture.rs"]
mod support;
use support::{DOMAIN, fixture};

async fn counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_media_ocr_layout),(SELECT count(*) FROM linggan_media_ocr_line),(SELECT count(*) FROM linggan_runtime_capture_package),(SELECT count(*) FROM linggan_material_reading),(SELECT count(*) FROM linggan_material_reading_selection),(SELECT count(*) FROM linggan_model_invocation))")
        .fetch_one(db.pool()).await.unwrap()
}

async fn detail(db: &Database, id: &str, payload: Value) -> Uuid {
    fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":payload})).await;
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

fn material<'a>(read: &'a dossier::ResearchDossier, kind: &str) -> &'a Value {
    read.materials.iter().find(|m| m["kind"] == kind).unwrap()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn full_dossier_is_read_only_and_all_consumers_receive_complete_blocks() {
    let db = fixture::proof_database("dossier_v2_full").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let before = counts(&db).await;
    let read = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(read.contract, "research-dossier.v2");
    assert!(
        read.fragments
            .iter()
            .any(|f| f.field == "body" && f.text.ends_with("结尾不能丢失🙂"))
    );
    assert!(
        read.fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text == "用途不清的背景文字")
    );
    assert_eq!(material(&read, "image")["page"], 1);
    assert_eq!(
        material(&read, "image")["blocks"].as_array().unwrap().len(),
        6
    );
    assert_eq!(read.coverage["imageInventory"]["expectedPages"], 1);
    assert_eq!(read.coverage["imageInventory"]["processedPages"], 1);
    let public = serde_json::to_value(&read).unwrap();
    for field in [
        "fragments",
        "media",
        "metadata",
        "inputs",
        "markdown",
        "sections",
    ] {
        assert!(public.get(field).is_none(), "no duplicated public {field}");
    }
    let text = public.to_string();
    assert_eq!(text.matches("结尾不能丢失🙂").count(), 1);
    assert!(!text.contains("rawText") && !text.contains("storageKey"));
    let markdown = dossier::markdown(&read);
    assert_eq!(markdown.matches("结尾不能丢失🙂").count(), 1);
    assert!(markdown.contains("用途不清的背景文字"));
    let loaded = creator_discovery::load_topic_materials_for_works(&db, DOMAIN, &[work])
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&loaded.works[0].fragments).unwrap(),
        serde_json::to_value(&read.fragments).unwrap()
    );
    let scope = serde_json::from_value(json!({"domain":DOMAIN})).unwrap();
    let creators = creator_discovery::load(&db, &scope).await.unwrap();
    assert_eq!(
        serde_json::to_value(&creators.works[0].fragments).unwrap(),
        serde_json::to_value(&read.fragments).unwrap()
    );
    assert_eq!(
        before,
        counts(&db).await,
        "reads cannot create recognition, normalization, selection or provider work"
    );
    assert_eq!(
        read.revision,
        dossier::read(&db, work).await.unwrap().unwrap().revision
    );
    assert!(dossier::relayer_work(&db, work, "stale").await.is_err());
    let receipt = dossier::relayer_work(&db, work, &read.revision)
        .await
        .unwrap();
    assert_eq!(
        receipt["updated"], 1,
        "explicit relayer upgrades the fixture's older normalization rules"
    );
    let after = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        dossier::relayer_work(&db, work, &after.revision)
            .await
            .unwrap()["alreadyCurrent"],
        1
    );
    let final_counts = counts(&db).await;
    for index in [0, 1, 2, 3, 6] {
        assert_eq!(
            before[index], final_counts[index],
            "normalization cannot dispatch OCR or copy source facts"
        );
    }
    assert_eq!(
        dossier::read_at(&db, work, Some(&read.as_of))
            .await
            .unwrap()
            .unwrap()
            .materials,
        read.materials
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn native_text_has_no_size_ceiling_and_history_keeps_original_structure() {
    let db = fixture::proof_database("dossier_v2_native").await;
    let long = format!("{}不得遗漏尾部🙂", "不是结论。".repeat(25000));
    let body = format!(
        "# 原文小标题\n\n{long}\n\n- 第一个条件\n- 第一个条件\n\n| 文件 | 判断 |\n| --- | --- |\n| C:\\notes | 不成立 |\n\n> 他人引述，不代表作者意见"
    );
    let work = detail(
        &db,
        "native",
        json!({"title":"平台标题","bodyText":body,"hashtags":["#A","#B","#A"]}),
    )
    .await;
    let first = dossier::read(&db, work).await.unwrap().unwrap();
    let blocks = material(&first, "body")["blocks"].as_array().unwrap();
    assert_eq!(blocks[1]["text"], long);
    assert_eq!(blocks[2]["items"], json!(["第一个条件", "第一个条件"]));
    assert_eq!(blocks[3]["rows"], json!([["C:\\notes", "不成立"]]));
    assert_eq!(blocks[4]["kind"], "quote");
    assert_eq!(
        material(&first, "tags")["blocks"][0]["items"],
        json!(["#A", "#B", "#A"])
    );
    detail(
        &db,
        "native",
        json!({"title":"平台标题","bodyText":"后续版本，旧段落不再出现"}),
    )
    .await;
    let current = dossier::read(&db, work).await.unwrap().unwrap();
    assert_ne!(first.revision, current.revision);
    assert!(
        !serde_json::to_string(&current)
            .unwrap()
            .contains("不得遗漏尾部")
    );
    let frozen = dossier::read_at(&db, work, Some(&first.as_of))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frozen.materials, first.materials);
    let reading = material(&first, "body")["currentRevisionRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let exact = dossier::read_version(&db, work, reading)
        .await
        .unwrap()
        .unwrap()
        .material;
    assert_eq!(exact["blocks"], material(&first, "body")["blocks"]);
    assert!(
        dossier::read_at(&db, work, Some("2999-01-01T00:00:00Z"))
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn empty_native_source_unknown_inventory_and_numeric_titles_are_distinct() {
    let db = fixture::proof_database("dossier_v2_unknown").await;
    let missing = detail(&db, "missing", json!({"title":"仅有标题"})).await;
    let empty = detail(&db, "empty", json!({"title":"空正文","bodyText":""})).await;
    let missing = dossier::read(&db, missing).await.unwrap().unwrap();
    let empty = dossier::read(&db, empty).await.unwrap().unwrap();
    assert_eq!(material(&missing, "body")["sourceState"], "not_acquired");
    assert_eq!(material(&empty, "body")["sourceState"], "known_empty");
    assert!(missing.coverage["imageInventory"]["expectedPages"].is_null());
    assert_eq!(
        missing.coverage["sources"]["audio"]["state"],
        "not_acquired"
    );
    assert!(missing.metadata["interaction"]["likes"]["value"].is_null());
    for (id, title, likes, native, expected) in [
        ("count", "53", 53, None, None),
        ("native-number", "53", 53, Some("53"), Some("53")),
        ("number-title", "1984", 12, None, Some("1984")),
    ] {
        fixture::submit_package(&db,"profile_discovery",json!({"authorExternalId":"synthetic-author"}),json!({"kind":"profile_discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":title,"likeCount":likes}})).await;
        let work = detail(
            &db,
            id,
            json!({"title":native,"bodyText":"合成正文","likedCount":999}),
        )
        .await;
        let read = dossier::read(&db, work).await.unwrap().unwrap();
        assert_eq!(read.note["platformTitle"].as_str(), expected);
        assert_eq!(read.metadata["interaction"]["likes"]["value"], 999);
    }
}

async fn images(db: &Database, id: &str, total: u32, slots: &[(&str, u32, char)]) -> Vec<Uuid> {
    let target = json!({"contentExternalId":id,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":total}});
    let observations: Vec<_> = slots.iter().map(|_| Uuid::new_v4()).collect();
    let records=slots.iter().zip(&observations).map(|((role,page,_),observation)|{
        let mut slot=json!({"role":role,"ordinal":page});
        if *role=="image" {slot["displayOrdinal"]=json!(page);slot["displayOrderBasis"]=json!("platform_explicit");}
        let uri=format!("https://media.example/{id}/{role}/{page}.jpg");
        json!({"kind":"media_slot","slotKey":format!("xhs:{id}:{role}:{page}"),"observationRef":observation,"slot":slot,"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-14T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":id}})
    }).collect();
    fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":id}),"media_slots","xhs",json!({"target":target,"layers":[fixture::coverage_layer("media_slots",slots.len() as i64)]}),records).await;
    let mut sources = vec![];
    for ((_, _, hash), observation) in slots.iter().zip(observations) {
        let hash = hash.to_string().repeat(64);
        sources.push(
            admit_media_blob(
                db,
                observation,
                &hash,
                "image/jpeg",
                12,
                &format!("blobs/{hash}.jpg"),
            )
            .await
            .unwrap()
            .materialization_ref,
        );
    }
    sources
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn page_holes_repeated_images_and_cover_dedup_require_explicit_first_page_proof() {
    let db = fixture::proof_database("dossier_v2_pages").await;
    let work = detail(&db, "pages", json!({"bodyText":"有缺页的笔记"})).await;
    images(
        &db,
        "pages",
        4,
        &[
            ("cover", 1, 'a'),
            ("image", 1, 'a'),
            ("image", 3, 'a'),
            ("image", 4, 'b'),
        ],
    )
    .await;
    let read = dossier::read(&db, work).await.unwrap().unwrap();
    let pages: Vec<_> = read
        .materials
        .iter()
        .filter(|m| m["kind"] == "image")
        .collect();
    assert_eq!(
        pages.len(),
        4,
        "cover duplicates proven page one; repeated real page three remains"
    );
    assert_eq!(read.coverage["imageInventory"]["missingPages"], json!([2]));
    assert_eq!(read.coverage["imageInventory"]["acquiredPages"], 3);
    assert_eq!(pages[1]["sourceState"], "not_acquired");
    assert_eq!(pages[2]["page"], 3);
    let sparse = detail(&db, "sparse", json!({"bodyText":"首页未取得"})).await;
    images(&db, "sparse", 3, &[("cover", 1, 'c'), ("image", 3, 'c')]).await;
    let read = dossier::read(&db, sparse).await.unwrap().unwrap();
    assert_eq!(
        read.materials
            .iter()
            .filter(|m| m["kind"] == "image")
            .count(),
        4,
        "a matching later page cannot prove cover equals first page"
    );
    assert!(
        read.materials
            .iter()
            .any(|m| m["slotKey"] == "xhs:sparse:cover:1")
    );
    assert_eq!(
        read.coverage["imageInventory"]["missingPages"],
        json!([1, 2])
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn explicit_selection_never_truncates_blocks_and_rejects_unknown_references() {
    let db = fixture::proof_database("dossier_v2_selection").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let full = dossier::read(&db, work).await.unwrap().unwrap();
    let mut selected = full.clone();
    dossier::select(
        &mut selected,
        &dossier::Selection {
            kinds: vec!["body".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(selected.materials.len(), 1);
    assert_eq!(material(&selected, "body"), material(&full, "body"));
    assert_eq!(selected.selection["mode"], "explicit");
    assert_eq!(selected.selection["completeRevision"], full.revision);
    assert!(
        dossier::select(
            &mut full.clone(),
            &dossier::Selection {
                block_refs: vec!["invented".into()],
                ..Default::default()
            }
        )
        .is_err()
    );
    let reference = material(&full, "image")["blocks"][0]["ref"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut block = full.clone();
    dossier::select(
        &mut block,
        &dossier::Selection {
            block_refs: vec![reference],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        material(&block, "image")["blocks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        material(&block, "image")["blocks"][0],
        material(&full, "image")["blocks"][0]
    );
}

async fn publish_image(
    db: &Database,
    work: Uuid,
    slot: &str,
    source: Uuid,
    text: &str,
    publication: material_reading::Publication,
) -> material_reading::PublishedReading {
    let mut tx = db.pool().begin().await.unwrap();
    let result = material_reading::publish_in(
        &mut tx,
        material_reading::NewReading {
            content_public_ref: work,
            unit_key: format!("image:{slot}"),
            kind: "image".into(),
            slot_key: Some(slot.into()),
            page: None,
            source_version: source.to_string(),
            source_state: "present".into(),
            derivative_ref: None,
            producer_ref: Some(Uuid::new_v4()),
            generated_by: "ordinary_ocr".into(),
            rule_version: "synthetic-cover.v2".into(),
            blocks: vec![
                json!({"ref":"source-line","kind":"paragraph","text":text,"speaker":"unconfirmed"}),
            ],
            issues: vec![],
            source_location: json!({}),
            publication,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        result.current,
        matches!(publication, material_reading::Publication::Automatic { .. })
    );
    tx.commit().await.unwrap();
    result
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn cover_dedup_keeps_available_and_human_readings_with_real_page_provenance() {
    let db = fixture::proof_database("dossier_v2_cover").await;
    let work = detail(
        &db,
        "cover-reading",
        json!({"bodyText":"封面来源与页码证据分开"}),
    )
    .await;
    let sources = images(
        &db,
        "cover-reading",
        2,
        &[("cover", 1, 'd'), ("image", 1, 'd'), ("image", 2, 'e')],
    )
    .await;
    let cover = publish_image(
        &db,
        work,
        "xhs:cover-reading:cover:1",
        sources[0],
        "封面已完成全部文字",
        material_reading::Publication::Automatic {
            expected_current: None,
        },
    )
    .await;
    let first = dossier::read(&db, work).await.unwrap().unwrap();
    let kept = first
        .materials
        .iter()
        .find(|m| m["kind"] == "image" && m["page"] == 1)
        .unwrap();
    assert_eq!(kept["currentRevisionRef"], cover.reading_ref.to_string());
    assert_eq!(kept["sourceRef"], sources[0].to_string());
    assert_eq!(kept["pageSource"]["sourceRef"], sources[1].to_string());
    let original_block = kept["blocks"][0].clone();
    let mut edited = original_block.clone();
    edited["text"] = json!("人工核对完整文字，保留 0.5 mg 与否定");
    material_reading::review_reading(
        &db,
        cover.unit_ref,
        &material_reading::ReviewReadingRequest {
            command_ref: Uuid::new_v4(),
            expected_current_revision_ref: Some(cover.reading_ref),
            candidate_revision_ref: cover.reading_ref,
            action: "edit".into(),
            edited_blocks: Some(vec![edited]),
            verified: true,
            resolved_issue_keys: vec![],
            actor: "synthetic-reviewer".into(),
            reason: "Compare the complete synthetic source image".into(),
        },
    )
    .await
    .unwrap();
    publish_image(
        &db,
        work,
        "xhs:cover-reading:image:1",
        sources[1],
        "机器尚未校正的第一页文字",
        material_reading::Publication::Automatic {
            expected_current: None,
        },
    )
    .await;
    let current = dossier::read(&db, work).await.unwrap().unwrap();
    let images: Vec<_> = current
        .materials
        .iter()
        .filter(|m| m["kind"] == "image")
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0]["reading"]["adoption"], "human_edited");
    assert_eq!(
        images[0]["blocks"][0]["text"],
        "人工核对完整文字，保留 0.5 mg 与否定"
    );
    assert_eq!(images[0]["sourceVersion"], sources[0].to_string());
    assert_eq!(
        images[0]["pageSource"]["sourceVersion"],
        sources[1].to_string()
    );
    let frozen = dossier::read_at(&db, work, Some(&first.as_of))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frozen.materials, first.materials);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn cover_candidate_counts_after_dedup_without_reassigning_reading_or_later_page() {
    let db = fixture::proof_database("dossier_v2_cover_candidate").await;
    let work = detail(
        &db,
        "cover-candidate",
        json!({"bodyText":"候选保留真实来源"}),
    )
    .await;
    let sources = images(
        &db,
        "cover-candidate",
        2,
        &[("cover", 1, 'd'), ("image", 1, 'd'), ("image", 2, 'd')],
    )
    .await;
    let candidate = publish_image(
        &db,
        work,
        "xhs:cover-candidate:cover:1",
        sources[0],
        "只有封面来源拥有完整候选",
        material_reading::Publication::Candidate,
    )
    .await;
    let read = dossier::read(&db, work).await.unwrap().unwrap();
    let plan = dossier::relayer_plan(&read);
    assert_eq!(plan["imageSlots"], 2);
    assert_eq!(plan["pendingReviewCount"], 1);
    assert_eq!(
        plan["needsOcr"], 1,
        "the repeated real second page still needs its own reading"
    );
    assert_eq!(plan["layoutCount"], 0);
    let pages: Vec<_> = read
        .materials
        .iter()
        .filter(|m| m["kind"] == "image")
        .collect();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["slotKey"], "xhs:cover-candidate:image:1");
    assert_eq!(pages[1]["slotKey"], "xhs:cover-candidate:image:2");
    assert!(pages.iter().all(|m| m["currentRevisionRef"].is_null()));
    assert!(
        !serde_json::to_string(&read)
            .unwrap()
            .contains("只有封面来源拥有完整候选")
    );
    let candidates = material_reading::read_candidates(&db, work).await.unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["unitRef"], candidate.unit_ref.to_string());
    assert_eq!(candidates[0]["slotKey"], "xhs:cover-candidate:cover:1");
    let exact = dossier::read_version(&db, work, candidate.reading_ref)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.current_revision_ref, None);
    assert_eq!(exact.material["ref"], candidate.unit_ref.to_string());
    assert_eq!(exact.material["sourceVersion"], sources[0].to_string());
    assert_eq!(
        exact.material["blocks"][0]["text"],
        "只有封面来源拥有完整候选"
    );
    assert_ne!(exact.material["ref"], pages[0]["ref"]);
}
