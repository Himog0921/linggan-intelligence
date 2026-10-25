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
async fn candidate(db: &Database, work: Uuid, image: &Value, index: u32) -> Uuid {
    let slot = image["slotKey"].as_str().unwrap();
    let blocks = if index == 2 {
        vec![
            json!({"ref":"candidate-heading","kind":"heading","level":2,"text":"候选二完整读本","speaker":"unconfirmed"}),
            json!({"ref":"candidate-table","kind":"table","columns":["条件","读数"],"rows":[["不适用","0.5 mg"]],"speaker":"unconfirmed","bbox":[0.1,0.2,0.7,0.2]}),
            json!({"ref":"candidate-list","kind":"list","ordered":true,"start":2,"items":["不可删除否定","条件有先后"],"speaker":"unconfirmed"}),
            json!({"ref":"candidate-dialogue","kind":"dialogue","turns":[{"speaker":"other","text":"他人的经验"},{"speaker":"author","text":"作者补充适用范围"}]}),
        ]
    } else {
        vec![
            json!({"ref":"candidate-first","kind":"paragraph","text":"候选一，尚未采用","speaker":"unconfirmed"}),
        ]
    };
    let issues = if index == 2 {
        vec![
            json!({"key":"candidate-word","code":"glyph_uncertain","message":"表格的一字待核对","resolution":"reading_edit","blockRef":"candidate-table"}),
            json!({"key":"candidate-source","code":"source_boundary","message":"来源范围仍需单独确认","resolution":"source_refresh"}),
        ]
    } else {
        vec![]
    };
    let mut tx = db.pool().begin().await.unwrap();
    let published = reading::publish_in(
        &mut tx,
        reading::NewReading {
            content_public_ref: work,
            unit_key: format!("image:{slot}"),
            kind: "image".into(),
            slot_key: Some(slot.into()),
            page: Some(1),
            source_version: image["sourceVersion"].as_str().unwrap().into(),
            source_state: "present".into(),
            derivative_ref: None,
            producer_ref: Some(Uuid::new_v4()),
            generated_by: "cloud_vision".into(),
            rule_version: "synthetic-provider.v2".into(),
            blocks,
            issues,
            source_location: json!({"fixture":"synthetic-only","originalUrl":image["originalUrl"]}),
            publication: reading::Publication::Candidate,
        },
    )
    .await
    .unwrap();
    assert!(!published.current);
    tx.commit().await.unwrap();
    published.reading_ref
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_http_single_contract_exports_selection_metadata_and_read_only_guards() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_read").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::ordinary_ocr(&db).await;
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
    for retired in ["note-material", "creator-preview"] {
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
async fn dossier_http_candidates_review_replay_conflict_and_restricted_history() {
    let db = dossier_fixture::fixture::proof_database("dossier_http_v2_review").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::ordinary_ocr(&db).await;
    let before = dossier::read(&db, work).await.unwrap().unwrap();
    let image = before
        .materials
        .iter()
        .find(|m| m["kind"] == "image")
        .unwrap();
    let first = candidate(&db, work, image, 1).await;
    let second = candidate(&db, work, image, 2).await;
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources/{work}/research-dossier");
    let (_, choices) = call(app.clone(), &format!("{base}/candidates"), None).await;
    assert_eq!(choices["candidates"].as_array().unwrap().len(), 2);
    let with_candidates = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(with_candidates.material_revision, before.material_revision);
    assert_ne!(
        with_candidates.revision, before.revision,
        "candidate identities affect the review plan, not the current text"
    );
    let endpoint = format!("{base}/materials/{}/review", image["ref"].as_str().unwrap());
    let command = json!({"commandRef":Uuid::new_v4(),"expectedCurrentRevisionRef":image["currentRevisionRef"],"candidateRevisionRef":second,"action":"adopt","actor":"synthetic-http-reviewer","reason":"Compare saved image with candidate","verified":false,"resolvedIssueKeys":[]});
    let (status, receipt) = call(app.clone(), &endpoint, Some(command.clone())).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["currentRevisionRef"], second.to_string());
    let replay = call(app.clone(), &endpoint, Some(command.clone())).await;
    assert_eq!(replay, (StatusCode::OK, receipt));
    let stale = json!({"commandRef":Uuid::new_v4(),"expectedCurrentRevisionRef":image["currentRevisionRef"],"candidateRevisionRef":first,"action":"adopt","actor":"synthetic-http-reviewer","reason":"Compare saved image with candidate","verified":false,"resolvedIssueKeys":[]});
    assert_eq!(
        call(app.clone(), &endpoint, Some(stale)).await.0,
        StatusCode::CONFLICT
    );
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading_command")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        commands, 1,
        "a replay and a stale command cannot append another decision"
    );
    let (_, adopted_version) = call(app.clone(), &format!("{base}/readings/{second}"), None).await;
    assert_eq!(adopted_version["currentRevisionRef"], second.to_string());
    assert_eq!(
        adopted_version["material"]["currentRevisionRef"],
        second.to_string()
    );
    let replacement = json!({"commandRef":Uuid::new_v4(),"expectedCurrentRevisionRef":second,
        "candidateRevisionRef":first,"action":"adopt","actor":"another-http-reviewer",
        "reason":"A later explicit review of the full saved image","verified":true,"resolvedIssueKeys":[]});
    assert_eq!(
        call(app.clone(), &endpoint, Some(replacement)).await.0,
        StatusCode::OK
    );
    let (_, old_version) = call(app.clone(), &format!("{base}/readings/{second}"), None).await;
    assert_eq!(
        old_version["material"], adopted_version["material"],
        "exact immutable content never follows the new current pointer"
    );
    assert_eq!(
        old_version["currentRevisionRef"],
        first.to_string(),
        "old receipts cannot replace the current unit pointer, including deduplicated units"
    );
    let source = image["sourceRef"].as_str().unwrap().parse().unwrap();
    linggan_evidence::record_materialization_disposition(
        &db,
        source,
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "test withdrawal",
    )
    .await
    .unwrap();
    assert_eq!(
        call(app.clone(), &format!("{base}/readings/{second}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, current) = call(app, &base, None).await;
    assert!(!current.to_string().contains("候选二完整读本"));
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
    let expected: Vec<_> = ["mainBlockRefs", "supplementBlockRefs"]
        .iter()
        .flat_map(|k| {
            full["dossier"]["readingView"][k]
                .as_array()
                .unwrap()
                .clone()
        })
        .collect();
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
        let refs: Vec<_> = ["mainBlockRefs", "supplementBlockRefs"]
            .iter()
            .flat_map(|k| {
                page["dossier"]["readingView"][k]
                    .as_array()
                    .unwrap()
                    .clone()
            })
            .collect();
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
