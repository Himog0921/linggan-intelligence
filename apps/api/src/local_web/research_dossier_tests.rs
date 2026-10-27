//! Actual local HTTP handlers over isolated synthetic PostgreSQL.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_evidence::{material_reading as reading, research_dossier as dossier};
use tower::ServiceExt;
use uuid::Uuid;
#[path = "research_dossier_browser_proof.rs"]
mod browser;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;
#[path = "../../../../crates/evidence/tests/support/standard_dossier_fixture.rs"]
mod standard_fixture;

async fn call(app: Router, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().uri(uri).header("Host", "127.0.0.1:3000");
    if body.is_some() {
        builder = builder
            .method("POST")
            .header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
    )
}
async fn counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_runtime_capture_package),(SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_material_reading),(SELECT count(*) FROM linggan_material_reading_selection),(SELECT count(*) FROM linggan_material_reading_command),(SELECT count(*) FROM linggan_dossier_snapshot),(SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_single_contract_exports_selection_metadata_and_read_only_guards() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_read").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::current_image_reading(&db).await;
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources/{work}/research-dossier");
    let initial = counts(&db).await;
    let (status, value) = call(app.clone(), &base, None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["dossier"]["contract"], "research-dossier.v2");
    assert_eq!(value["dossier"]["selection"]["mode"], "full");
    assert!(value.get("inputs").is_none());
    assert_eq!(value.to_string().matches("结尾不能丢失🙂").count(), 1);
    let (status, markdown) = call(app.clone(), &format!("{base}?format=markdown"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        markdown.as_str().unwrap().matches("结尾不能丢失🙂").count(),
        1
    );
    let (_, selected) = call(app.clone(), &format!("{base}?kinds=body"), None).await;
    assert_eq!(
        selected["dossier"]["materials"].as_array().unwrap().len(),
        1
    );
    assert_eq!(selected["dossier"]["selection"]["mode"], "explicit");
    let (_, metadata) = call(
        app.clone(),
        &format!("{base}/metadata?sections=interaction"),
        None,
    )
    .await;
    assert_eq!(metadata["metadata"]["interaction"]["likes"]["value"], 0);
    assert!(metadata["metadata"].get("author").is_none());
    let (status, publication) = call(
        app.clone(),
        &format!("{base}/metadata?sections=publication"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(publication["metadata"]["publication"]["observedAt"].is_string());
    for key in [
        "publishedAt",
        "localDate",
        "sourceText",
        "sourceKind",
        "precision",
        "referenceObservedAt",
        "sourceObservedAt",
        "parserVersion",
    ] {
        assert!(
            publication["metadata"]["publication"].get(key).is_some(),
            "publication preserves {key}"
        );
    }
    assert!(publication["metadata"].get("author").is_none());

    for query in [
        "?kinds=unknown",
        "?budget=500",
        "?format=unknown",
        "?asOf=not-a-time",
        "?asOf=2999-01-01T00:00:00Z",
    ] {
        assert_eq!(
            call(app.clone(), &format!("{base}{query}"), None).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(
            app.clone(),
            &format!("{base}?domain={}", Uuid::new_v4()),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for retired in [
        "note-material",
        "creator-preview",
        "candidates",
        "relayer-plan",
    ] {
        assert_eq!(
            call(app.clone(), &format!("{base}/{retired}"), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri(&base)
                .header("Host", "127.0.0.1:3000")
                .header("Origin", "https://untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(initial, counts(&db).await);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_current_edit_replays_conflicts_and_honors_source_restriction() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_edit").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::current_image_reading(&db).await;
    let before = dossier::read(&db, work).await.unwrap().unwrap();
    let image = before
        .materials
        .iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap();
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources/{work}/research-dossier");
    let endpoint = format!(
        "{base}/materials/{}/review?domain={}",
        image["ref"].as_str().unwrap(),
        dossier_fixture::DOMAIN
    );
    let mut blocks = image["blocks"].clone();
    blocks[0]["text"] = json!("人工修正，保留否定与0.5 mg。");
    let command = json!({"commandRef":Uuid::new_v4(),"unitRef":image["ref"],"sourceVersion":image["sourceVersion"],"expectedCurrentRevisionRef":image["currentRevisionRef"],"action":"edit","editedBlocks":blocks,"actor":"synthetic-http-reviewer","reason":"Compare the complete original image","verified":true,"resolvedIssueKeys":[]});
    let (status, receipt) = call(app.clone(), &endpoint, Some(command.clone())).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_ne!(receipt["currentRevisionRef"], image["currentRevisionRef"]);
    assert_eq!(receipt["adoption"], "human_edited");
    assert_eq!(receipt["unitRef"], image["ref"]);
    let after = counts(&db).await;
    assert_eq!(
        call(app.clone(), &endpoint, Some(command.clone())).await,
        (StatusCode::OK, receipt.clone())
    );
    assert_eq!(counts(&db).await, after);
    let mut stale = command.clone();
    stale["commandRef"] = json!(Uuid::new_v4());
    assert_eq!(
        call(app.clone(), &endpoint, Some(stale)).await.0,
        StatusCode::CONFLICT
    );
    let mut changed = command.clone();
    changed["reason"] = json!("Different request under the original UUID");
    assert_eq!(
        call(app.clone(), &endpoint, Some(changed)).await.0,
        StatusCode::CONFLICT
    );
    let mut old_action = command.clone();
    old_action["commandRef"] = json!(Uuid::new_v4());
    old_action["action"] = json!("adopt");
    assert!(
        call(app.clone(), &endpoint, Some(old_action))
            .await
            .0
            .is_client_error()
    );
    assert_eq!(
        counts(&db).await,
        after,
        "rejected and replayed commands cannot append state"
    );
    let source: Uuid = serde_json::from_value(image["sourceVersion"].clone()).unwrap();
    linggan_evidence::record_materialization_disposition(
        &db,
        source,
        linggan_evidence::MaterialMediaDisposition::BytesCleaned,
        "synthetic",
        "synthetic bytes cleanup",
    )
    .await
    .unwrap();
    let mut next = command.clone();
    next["commandRef"] = json!(Uuid::new_v4());
    next["expectedCurrentRevisionRef"] = receipt["currentRevisionRef"].clone();
    assert_eq!(
        call(app.clone(), &endpoint, Some(next)).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(app.clone(), &endpoint, Some(command.clone())).await.0,
        StatusCode::OK,
        "an existing receipt is not a new edit"
    );
    let kept = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        kept.materials
            .iter()
            .find(|u| u["kind"] == "image")
            .unwrap()["blocks"][0]["text"],
        "人工修正，保留否定与0.5 mg。"
    );
    linggan_evidence::record_materialization_disposition(
        &db,
        source,
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic",
        "source withdrawn",
    )
    .await
    .unwrap();
    assert_eq!(
        call(app.clone(), &endpoint, Some(command)).await.0,
        StatusCode::CONFLICT
    );
    let hidden = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        hidden
            .materials
            .iter()
            .find(|u| u["kind"] == "image")
            .unwrap()["blocks"],
        json!([])
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_batch_preserves_order_common_cutoff_and_individual_not_found() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_batch").await;
    let work = dossier_fixture::work(&db).await;
    let missing = Uuid::new_v4();
    let app = app_with_database(db.clone());
    let initial = counts(&db).await;
    let input = json!({"domain":dossier_fixture::DOMAIN,"workRefs":[missing,work],"selection":{"kinds":["body"]}});
    let (status, value) = call(
        app.clone(),
        "/api/local/research-dossiers/read",
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["results"][0]["state"], "not_found");
    assert_eq!(value["results"][1]["state"], "available");
    assert_eq!(value["results"][1]["dossier"]["asOf"], value["asOf"]);
    assert_eq!(
        value["results"][1]["dossier"]["materials"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(initial, counts(&db).await);
    let duplicate = json!({"domain":dossier_fixture::DOMAIN,"workRefs":[work,work]});
    assert_eq!(
        call(app, "/api/local/research-dossiers/read", Some(duplicate))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

async fn seed_comments(db: &Database, count: usize) {
    for index in 0..count {
        let reply = index == 1;
        let body = if index == 0 {
            format!("{}末尾原文", "合成评论。".repeat(900))
        } else {
            format!("合成评论 {index}，用于检查回复关系和分页。")
        };
        let mut payload = json!({"commentId":format!("c{index}"),"noteId":"dossier-note","text":body,"authorId":"synthetic-reader","likes":7});
        if reply {
            payload["parentCommentId"] = json!("c0");
            payload["rootCommentId"] = json!("c0");
        }
        dossier_fixture::fixture::submit_package(db,if reply{"replies"}else{"comments"},json!({"contentExternalId":"dossier-note"}),json!({"kind":if reply{"reply"}else{"comment"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":payload})).await;
    }
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_comments_keep_full_text_likes_parent_and_page_boundaries() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_comments").await;
    let work = dossier_fixture::work(&db).await;
    seed_comments(&db, 31).await;
    let app = app_with_database(db);
    let base = format!(
        "/api/local/work-resources/{work}/research-dossier/comments?domain={}",
        dossier_fixture::DOMAIN
    );
    let (status, first) = call(app.clone(), &base, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["total"], 31);
    assert_eq!(first["items"].as_array().unwrap().len(), 30);
    let (_, second) = call(
        app,
        &format!("{base}&cursor={}", first["nextCursor"].as_str().unwrap()),
        None,
    )
    .await;
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert!(second["nextCursor"].is_null());
    let items: Vec<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .collect();
    let root = items
        .iter()
        .find(|v| v["body"].as_str().unwrap().ends_with("末尾原文"))
        .unwrap();
    assert!(root["body"].as_str().unwrap().chars().count() > 4000);
    assert_eq!(root["bodyTruncated"], false);
    let reply = items.iter().find(|v| v["relation"] == "REPLY").unwrap();
    assert_eq!(reply["parentSourceRef"], root["sourceRef"]);
    assert_eq!(reply["likeCount"], 7);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_standard_continuous_segments_pin_revision_and_cover_all_blocks() {
    let db = dossier_fixture::fixture::proof_database("standard_segments").await;
    let work = standard_fixture::scenario(
        &db,
        "segment-note",
        Some(
            &((0..456)
                .map(|i| format!("第{i}段完整原文🙂"))
                .collect::<Vec<_>>()
                .join("\n\n")),
        ),
        3,
        false,
        false,
        false,
    )
    .await;
    dossier_fixture::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"segment-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"segment-note"},"payload":{"hashtags":["#冻结顺序"]}})).await;
    let app = app_with_database(db.clone());
    let base = format!(
        "/api/local/work-resources/{work}/research-dossier?domain={}",
        standard_fixture::DOMAIN
    );
    let (_, full) = call(app.clone(), &base, None).await;
    let expected = section_refs(&full["dossier"]);
    let mut offset = 0;
    let mut received = vec![];
    let mut pinned = None::<(String, String)>;
    loop {
        let suffix = pinned
            .as_ref()
            .map(|(as_of, rev)| format!("&asOf={as_of}&expectedRevision={rev}"))
            .unwrap_or_default();
        let (status, page) = call(
            app.clone(),
            &format!("{base}&segmentSize=38&segmentOffset={offset}{suffix}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        let segment = &page["segment"];
        if pinned.is_none() {
            pinned = Some((
                segment["asOf"].as_str().unwrap().into(),
                segment["completeRevision"].as_str().unwrap().into(),
            ));
        }
        let refs = section_refs(&page["dossier"]);
        assert_eq!(
            refs.len(),
            segment["end"].as_u64().unwrap() as usize - offset
        );
        received.extend(refs);
        if let Some(next) = segment["nextOffset"].as_u64() {
            offset = next as usize;
        } else {
            break;
        }
        if offset == 38 {
            dossier_fixture::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"segment-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"segment-note"},"payload":{"bodyText":"研究期间新正文，不得混入旧研究"}})).await;
        }
    }
    assert_eq!(received, expected);
    assert_eq!(received.len(), 463);
    let (as_of, revision) = pinned.as_ref().unwrap();
    let (status, empty) = call(
        app.clone(),
        &format!(
            "{base}&segmentSize=38&asOf={as_of}&expectedRevision={revision}&segmentOffset=463"
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["segment"]["scope"], "full");
    assert!(empty["segment"]["nextOffset"].is_null());
    assert!(empty["dossier"]["materials"].as_array().unwrap().is_empty());
    assert!(
        empty["dossier"]["note"]["titleBlockRefs"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let (status, _) = call(
        app.clone(),
        &format!("{base}&segmentSize=38&segmentOffset=38"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (as_of, revision) = pinned.unwrap();
    let (status, _) = call(
        app.clone(),
        &format!("{base}&segmentSize=38&asOf={as_of}&expectedRevision=wrong"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        app,
        &format!(
            "{base}&segmentSize=38&asOf={as_of}&expectedRevision={revision}&segmentOffset=9999"
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

fn section_refs(dossier: &Value) -> Vec<Value> {
    dossier["readingView"]["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["groups"].as_array().unwrap())
        .flat_map(|g| g["blockRefs"].as_array().unwrap())
        .cloned()
        .collect()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_nonsegmented_exports_pin_the_selected_snapshot_revision() {
    let db = dossier_fixture::fixture::proof_database("dossier_export_pin").await;
    let work = standard_fixture::scenario(
        &db,
        "export-pin",
        Some("发布文案不可丢失。"),
        3,
        false,
        false,
        false,
    )
    .await;
    let app = app_with_database(db.clone());
    let base = format!(
        "/api/local/work-resources/{work}/research-dossier?domain={}",
        standard_fixture::DOMAIN
    );
    let before = counts(&db).await;
    let (status, full) = call(app.clone(), &base, None).await;
    assert_eq!(status, StatusCode::OK);
    let as_of = full["dossier"]["asOf"].as_str().unwrap();
    let full_revision = full["dossier"]["revision"].as_str().unwrap();
    let selected_uri = format!("{base}&asOf={as_of}&kinds=image");
    let (status, selected) = call(app.clone(), &selected_uri, None).await;
    assert_eq!(status, StatusCode::OK);
    let revision = selected["dossier"]["revision"].as_str().unwrap();
    assert_ne!(revision, full_revision);
    assert_eq!(
        selected["dossier"]["selection"]["completeRevision"],
        full_revision
    );
    for (uri, expected) in [(&base, full_revision), (&selected_uri, revision)] {
        for format in ["json", "markdown"] {
            let (status, result) = call(
                app.clone(),
                &format!("{uri}&format={format}&expectedRevision={expected}"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{result}");
            if format == "markdown" {
                let text = result.as_str().unwrap();
                assert!(text.contains("第3页完整文字"));
                for hidden in [
                    "来源块",
                    "sourceRef",
                    "readingView",
                    "查看原件",
                    "覆盖与来源",
                ] {
                    assert!(!text.contains(hidden), "{text}");
                }
                if uri == &selected_uri {
                    assert!(!text.contains("发布文案不可丢失"));
                }
            }
            let (status, result) = call(
                app.clone(),
                &format!("{uri}&format={format}&expectedRevision=wrong"),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT);
            assert_eq!(result["code"], "dossier_revision_changed");
        }
    }
    let (status, _) = call(
        app.clone(),
        &format!("{selected_uri}&format=markdown&expectedRevision={full_revision}"),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "compare selected revision, not selection.completeRevision"
    );
    let (status, result) = call(
        app.clone(),
        &format!("{selected_uri}&segmentSize=2&expectedRevision={revision}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["segment"]["completeRevision"], revision);
    let (status, result) = call(
        app,
        &format!("{base}&segmentOffset=0&expectedRevision={full_revision}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(result["code"], "segment_size_required");
    assert_eq!(counts(&db).await, before);
}
