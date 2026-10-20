use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;

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
                .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_preview_relayer_comments_and_scope_are_real_routes() {
    let db = dossier_fixture::fixture::proof_database("research_dossier_http").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::legacy_ocr(&db).await;
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources/{work}/research-dossier");
    let query = format!("?domain={}", dossier_fixture::DOMAIN);
    let (status, preview) = call(app.clone(), &format!("{base}{query}"), None).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["relayerPlan"]["pendingLayouts"], 1);
    let (status,receipt)=call(app.clone(),&format!("{base}/relayer"),Some(serde_json::json!({"domain":dossier_fixture::DOMAIN,"expectedRevision":preview["dossier"]["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["updated"], 1);
    let (status, after) = call(app.clone(), &format!("{base}{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(after["markdown"].as_str().unwrap().contains("第3行连续"));
    assert!(
        after["inputs"]["comments"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["kind"] == "image_substantive_text")
    );
    let (status, comments) = call(app.clone(), &format!("{base}/comments{query}"), None).await;
    assert_eq!(status, StatusCode::OK, "{comments}");
    assert_eq!(comments["limit"], 30);
    let (status, creator) =
        call(app.clone(), &format!("{base}/creator-preview{query}"), None).await;
    assert_eq!(status, StatusCode::OK, "{creator}");
    assert_eq!(creator["reason"], "model_not_configured");
    dossier_fixture::model_config(&db).await;
    let (status, creator) =
        call(app.clone(), &format!("{base}/creator-preview{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(creator["available"], true, "{creator}");
    assert!(creator["estimatedInputTokensUpperBound"].as_u64().unwrap() <= 8192);
    let scope =
        serde_json::from_value(json!({"domain":dossier_fixture::DOMAIN,"platform":"xhs"})).unwrap();
    let data =
        linggan_evidence::creator_discovery::load_creator_material_for_work(&db, &scope, work)
            .await
            .unwrap();
    let expected =
        linggan_intelligence::creator_discovery_input::prepare(&data.domain, &data.works[0], 8192)
            .unwrap();
    assert_eq!(creator["input"], expected.0);
    assert_eq!(
        creator["fragments"],
        serde_json::to_value(expected.1).unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let (status, _) = call(
        app.clone(),
        &format!("{base}?domain=00000000-0000-4000-8000-000000000002"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
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
    let db = dossier_fixture::fixture::proof_database("dossier_comment_pages").await;
    let work = dossier_fixture::work(&db).await;
    seed_comments(&db, 31).await;
    let app = app_with_database(db.clone());
    let base = format!(
        "/api/local/work-resources/{work}/research-dossier/comments?domain={}",
        dossier_fixture::DOMAIN
    );
    let (status, first) = call(app.clone(), &base, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["items"].as_array().unwrap().len(), 30);
    assert_eq!(first["total"], 31);
    let cursor = first["nextCursor"].as_str().unwrap();
    let (_, second) = call(app.clone(), &format!("{base}&cursor={cursor}"), None).await;
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
    assert_eq!(
        reply["anonymousAuthorContext"]["platformUserIdReturned"],
        false
    );
}

/// Run only through test-research-dossier-postgres.sh --preview. Uses synthetic data.
#[tokio::test]
#[ignore = "manual isolated frontend preview; runs until interrupted"]
async fn manual_dossier_frontend_preview() {
    assert_eq!(
        std::env::var("DOSSIER_FRONTEND_PREVIEW").as_deref(),
        Ok("1")
    );
    let db = dossier_fixture::fixture::proof_database("dossier_frontend").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::legacy_ocr(&db).await;
    dossier_fixture::model_config(&db).await;
    seed_comments(&db, 3).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!(
        "DOSSIER_PREVIEW_URL=http://{}/corpus/evidence?domain={}&work={}&tab=dossier",
        listener.local_addr().unwrap(),
        dossier_fixture::DOMAIN,
        work
    );
    axum::serve(listener, app_with_database(db)).await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_note_material_is_scoped_read_only_and_origin_guarded() {
    let db = dossier_fixture::fixture::proof_database("note_material_http").await;
    let work = dossier_fixture::work(&db).await;
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources/{work}/research-dossier/note-material");
    let (status, value) = call(
        app.clone(),
        &format!("{base}?domain={}", dossier_fixture::DOMAIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["contract"], "note-material-input.v1");
    assert_eq!(value["ready"], true);
    assert!(
        value["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["text"].as_str().unwrap().ends_with("结尾不能丢失🙂"))
    );
    let (status, _) = call(
        app.clone(),
        &format!("{base}?domain=00000000-0000-4000-8000-000000000002"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&base)
                .header("Host", "127.0.0.1:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let (status, _) = call(
        app.clone(),
        &format!("{base}?domain={}&as_of=2099-01-01", dossier_fixture::DOMAIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("{base}?domain={}", dossier_fixture::DOMAIN))
                .header("Host", "127.0.0.1:3000")
                .header("Origin", "https://untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_note_material_snapshot")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

/// PAGE-NOTE-MATERIAL-001: owned synthetic DB, actual read handler and production React.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL + built React + Chromium; --note-material-ui only"]
async fn dossier_note_material_readonly_browser_proof() {
    assert_eq!(std::env::var("NOTE_MATERIAL_UI_PROOF").as_deref(), Ok("1"));
    let db = dossier_fixture::fixture::proof_database("note_material_ui").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::legacy_ocr(&db).await;
    linggan_evidence::record_slot_disposition(
        &db,
        "xhs:dossier-note:image:1",
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "readonly UI restricted-column fixture",
    )
    .await
    .unwrap();
    let effects = || async {
        sqlx::query_scalar::<_, Value>("SELECT jsonb_build_array((SELECT count(*) FROM linggan_note_material_snapshot),(SELECT count(*) FROM linggan_model_invocation),(SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_media_processing_job_event),(SELECT count(*) FROM linggan_media_ocr_layering_result),(SELECT count(*) FROM linggan_runtime_capture_package),(SELECT count(*) FROM collection_acquisition_request),(SELECT count(*) FROM collection_work_order),(SELECT count(*) FROM collection_work_order_lease))")
            .fetch_one(db.pool()).await.unwrap()
    };
    let before = effects().await;
    let directory = std::env::var("WEB_APP_PROOF_DIR").expect("owned proof output directory");
    std::fs::create_dir_all(&directory).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let application = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    std::fs::write(
        std::path::Path::new(&directory).join("fixture.json"),
        json!({"origin":origin,"domain":dossier_fixture::DOMAIN,"work":work}).to_string(),
    )
    .unwrap();
    let result =
        std::process::Command::new(std::env::var("WEB_APP_PROOF_NODE").expect("Node supplied"))
            .arg(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/note-material-readonly.mjs"),
            )
            .env("WEB_APP_PROOF_DIR", &directory)
            .status();
    server.abort();
    let _ = server.await;
    assert!(
        result.unwrap().success(),
        "readonly browser proof failed: {directory}"
    );
    let report: Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&directory).join("report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["finished"], true);
    assert_eq!(report["pageErrors"], json!([]));
    assert_eq!(report["blockedRequests"], json!([]));
    assert_eq!(
        effects().await,
        before,
        "read path creates no model, snapshot, media, layering, collection, work-order or lease side effects"
    );
}
