//! Real guarded HTTP composition of the S1a asset routes against synthetic, isolated PostgreSQL.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "../../../../crates/intelligence/tests/support/ops_export_fixture.rs"]
mod ops;

async fn proof(name: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        name,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap()
}
async fn seed_accounts(db: &Database) -> (Uuid, Uuid) {
    let export = ops::first_export();
    let request = export.request("2026-10-12T22:43:00+08:00");
    let preview = linggan_intelligence::ops_ledger::preview_ops_import(db, &request)
        .await
        .unwrap();
    linggan_intelligence::ops_ledger::apply_ops_import(db, &request, &preview.preview_hash)
        .await
        .unwrap();
    let by = |source: &'static str| async move {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT account_ref FROM linggan_ops_account WHERE source_record_id=$1",
        )
        .bind(source)
        .fetch_one(db.pool())
        .await
        .unwrap()
    };
    (by("acc-1").await, by("acc-2").await)
}
async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Host", "127.0.0.1:4185");
    if body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({"rawLength": bytes.len()})),
    )
}
fn rid() -> serde_json::Value {
    json!(Uuid::new_v4())
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn creation_assets_http_person_correction_and_replacement_require_explicit_choice() {
    let db = proof("assets_http_person").await;
    let (account, _) = seed_accounts(&db).await;
    let app = app_with_database(db);
    let uri = format!("/ops/api/creation/accounts/{account}/identity");
    let mut input = json!({"requestId":rid(), "expectedRevision":0, "orgRelation":"parent",
        "bioDisclosesRelation":null,"hasRealPerson":true,"backingPersonRole":"妈妈","canProvideExperience":null});
    let (status, first) = call(&app, Method::PUT, &uri, Some(input.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["identity"]["hasKnownPerson"], true);
    input["requestId"] = rid();
    input["expectedRevision"] = json!(1);
    input["backingPersonRole"] = json!("爸爸");
    let (status, missing) = call(&app, Method::PUT, &uri, Some(input.clone())).await;
    assert_eq!(
        (status, missing["code"].as_str()),
        (
            StatusCode::BAD_REQUEST,
            Some("creation_person_change_required")
        )
    );
    input["personChange"] = json!("correction");
    let (status, corrected) = call(&app, Method::PUT, &uri, Some(input.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(corrected["identity"]["revision"], 2);
    input["requestId"] = rid();
    input["expectedRevision"] = json!(2);
    input["personChange"] = json!("replacement");
    let (status, replaced) = call(&app, Method::PUT, &uri, Some(input.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replaced["identity"]["revision"], 3);
    let (status, replay) = call(&app, Method::PUT, &uri, Some(input)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn creation_assets_http_identity_cards_and_sentences_follow_the_contract() {
    let db = proof("assets_http_library").await;
    let (xhs, douyin) = seed_accounts(&db).await;
    let app = app_with_database(db);

    let (status, listed) = call(&app, Method::GET, "/ops/api/creation/accounts", None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let refs: Vec<&str> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["accountRef"].as_str().unwrap())
        .collect();
    assert!(refs.contains(&xhs.to_string().as_str()));
    assert!(!refs.contains(&douyin.to_string().as_str()));

    let identity = json!({"requestId": rid(), "expectedRevision": 0, "orgRelation": "parent",
        "bioDisclosesRelation": true, "hasRealPerson": true, "backingPersonRole": "妈妈",
        "canProvideExperience": null});
    let uri = format!("/ops/api/creation/accounts/{xhs}/identity");
    let (status, made) = call(&app, Method::PUT, &uri, Some(identity.clone())).await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["identity"]["revision"], 1);
    assert_eq!(made["identity"]["canProvideExperience"], json!(null));
    let (_, replay) = call(&app, Method::PUT, &uri, Some(identity.clone())).await;
    assert_eq!(replay["replayed"], true);
    let mut stale = identity.clone();
    stale["requestId"] = rid();
    let (status, body) = call(&app, Method::PUT, &uri, Some(stale)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("creation_assets_stale"))
    );
    let mut unknown = identity.clone();
    unknown["requestId"] = rid();
    unknown["unexpected"] = json!(true);
    assert!(
        call(&app, Method::PUT, &uri, Some(unknown))
            .await
            .0
            .is_client_error()
    );
    let (status, _) = call(
        &app,
        Method::PUT,
        &format!("/ops/api/creation/accounts/{}/identity", Uuid::new_v4()),
        Some(
            json!({"requestId": rid(), "expectedRevision": 0, "orgRelation": "none",
            "bioDisclosesRelation": null, "hasRealPerson": null, "backingPersonRole": null,
            "canProvideExperience": null}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Fact card: create, confirm, read back.
    let card = json!({"requestId": rid(), "expectedRevision": 0, "name": "示例营",
        "allowedNames": ["示例营"], "bannedNames": ["示例课"], "what": "合成", "who": "",
        "canSay": "", "cannotSay": "", "mustNote": ""});
    let (status, created) = call(
        &app,
        Method::POST,
        "/ops/api/creation/fact-cards",
        Some(card),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let code = created["card"]["code"].as_str().unwrap().to_string();
    let (status, confirmed) = call(
        &app,
        Method::POST,
        &format!("/ops/api/creation/fact-cards/{code}/confirm"),
        Some(json!({"requestId": rid(), "expectedRevision": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    let (_, cards) = call(&app, Method::GET, "/ops/api/creation/fact-cards", None).await;
    assert_eq!(cards["items"][0]["confirmed"]["revision"], 1);

    // Sentence: a cure promise is refused with the findings, a plain one waits for review.
    let (status, refused) = call(
        &app,
        Method::POST,
        "/ops/api/creation/bridge-sentences",
        Some(json!({"requestId": rid(), "group": "parent", "text": "孩子保证能好", "origin": "manual"})),
    )
    .await;
    assert_eq!(
        (status, refused["code"].as_str()),
        (StatusCode::CONFLICT, Some("creation_assets_blocked"))
    );
    assert_eq!(refused["findings"][0]["rule"], "R-L1");
    let (status, made) = call(
        &app,
        Method::POST,
        "/ops/api/creation/bridge-sentences",
        Some(json!({"requestId": rid(), "group": "parent", "text": "我是示例营的学员家长", "origin": "manual"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["sentence"]["status"], "review");
    let (_, none_yet) = call(
        &app,
        Method::GET,
        &format!("/ops/api/creation/accounts/{xhs}/candidates"),
        None,
    )
    .await;
    assert_eq!(none_yet["state"], "ready");
    assert!(none_yet["items"].as_array().unwrap().is_empty());
    let (status, approved) = call(
        &app,
        Method::POST,
        "/ops/api/creation/bridge-sentences/XJ-0001/review",
        Some(json!({"requestId": rid(), "expectedRevision": 1, "action": "approve", "note": null})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    let (_, served) = call(
        &app,
        Method::GET,
        &format!("/ops/api/creation/accounts/{xhs}/candidates"),
        None,
    )
    .await;
    assert_eq!(served["items"][0]["code"], "XJ-0001");
    let (status, filtered) = call(
        &app,
        Method::GET,
        "/ops/api/creation/bridge-sentences?group=coach&status=ok",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(filtered["items"].as_array().unwrap().is_empty());
    let (status, _) = call(
        &app,
        Method::GET,
        "/ops/api/creation/bridge-sentences?group=nobody",
        None,
    )
    .await;
    assert!(status.is_client_error());
    let (status, check) = call(
        &app,
        Method::POST,
        "/ops/api/creation/bridge-sentences/check",
        Some(json!({"text": "欢迎私信我报名"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(check["blocked"], false);
    assert!(
        check["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["rule"] == "R-L6")
    );

    // The same-origin guard still applies to every new route.
    let evil = Request::builder()
        .uri("/ops/api/creation/fact-cards")
        .header("Host", "127.0.0.1:4185")
        .header("Origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(evil).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn creation_assets_http_checks_and_copy_run_against_the_saved_revision() {
    let db = proof("assets_http_copy").await;
    let (xhs, _) = seed_accounts(&db).await;
    let app = app_with_database(db);
    let (status, work) = call(
        &app,
        Method::POST,
        "/ops/api/creation/works",
        Some(json!({"requestId": rid(), "domainRef": dossier::DOMAIN, "document": {
            "contract": "content-workbench.v1", "title": "合成标题", "accountRef": xhs,
            "topicRef": null, "angle": "", "outline": [], "tags": ["ADHD"],
            "paragraphs": [{"id": Uuid::new_v4(), "text": "第一段。"}, {"id": Uuid::new_v4(), "text": "第二段。"}],
            "excerpts": [], "canvas": [], "pages": [], "uses": []}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{work}");
    let id = work["workId"].as_str().unwrap();
    let revision = work["revision"]["id"].clone();

    // The draft has an account whose identity is not confirmed: the bridge slot must be decided.
    let (status, report) = call(
        &app,
        Method::POST,
        "/ops/api/creation/checks",
        Some(json!({"document": work["revision"]["document"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["bridgeState"], "unconfirmed");
    assert_eq!(report["findings"][0]["rule"], "R-M1");
    assert_eq!(report["findings"][0]["level"], "must");
    assert_eq!(report["rulesetVersion"], "creation-checks.v2");
    let mut repeated = work["revision"]["document"].clone();
    repeated["tags"] = json!(["ADHD", "ADHD", "#adhd"]);
    let (status, checked) = call(
        &app,
        Method::POST,
        "/ops/api/creation/checks",
        Some(json!({"document": repeated})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    let repeats: Vec<_> = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule"] == "R-W7")
        .collect();
    assert_eq!(repeats.len(), 2);
    assert_eq!(repeats[0]["location"]["index"], 1);
    assert_eq!(repeats[1]["location"]["index"], 2);
    let mut unknown_field = work["revision"]["document"].clone();
    unknown_field["serverOverride"] = json!(true);
    assert!(
        call(
            &app,
            Method::POST,
            "/ops/api/creation/checks",
            Some(json!({"document": unknown_field}))
        )
        .await
        .0
        .is_client_error()
    );

    let copy = format!("/ops/api/creation/works/{id}/copy");
    let (status, blocked) = call(
        &app,
        Method::POST,
        &copy,
        Some(json!({"requestId": rid(), "expectedRevision": revision})),
    )
    .await;
    assert_eq!(
        (status, blocked["code"].as_str()),
        (StatusCode::CONFLICT, Some("creation_assets_blocked"))
    );
    assert_eq!(blocked["findings"][0]["rule"], "R-M1");

    // Decide "这篇不植入", save, and copy.
    let mut document = work["revision"]["document"].clone();
    document["bridge"] = json!({"skip": true});
    let (status, saved) = call(
        &app,
        Method::POST,
        &format!("/ops/api/creation/works/{id}/save"),
        Some(json!({"requestId": rid(), "expectedRevision": revision, "document": document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let current = saved["revision"]["id"].clone();
    let (status, stale) = call(
        &app,
        Method::POST,
        &copy,
        Some(json!({"requestId": rid(), "expectedRevision": revision})),
    )
    .await;
    assert_eq!(
        (status, stale["code"].as_str()),
        (StatusCode::CONFLICT, Some("creation_assets_stale"))
    );
    let command = json!({"requestId": rid(), "expectedRevision": current});
    let (status, ready) = call(&app, Method::POST, &copy, Some(command.clone())).await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["text"], "第一段。\n\u{3164}\n第二段。\n#ADHD");
    assert_eq!(ready["title"], "合成标题");
    let (_, again) = call(&app, Method::POST, &copy, Some(command)).await;
    assert_eq!(again["replayed"], true);
    assert_eq!(again["receiptRef"], ready["receiptRef"]);

    let status_uri = format!("/ops/api/creation/works/{id}/copy-status");
    let (_, before) = call(&app, Method::GET, &status_uri, None).await;
    assert!(before["copied"].is_null());
    let receipt = ready["receiptRef"].as_str().unwrap();
    let (status, mark) = call(
        &app,
        Method::POST,
        &format!("{copy}/{receipt}/confirm"),
        Some(json!({"requestId": rid()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{mark}");
    let (_, after) = call(&app, Method::GET, &status_uri, None).await;
    assert_eq!(after["copied"]["revisionId"], current);
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/ops/api/creation/works/{}/copy", Uuid::new_v4()),
        Some(json!({"requestId": rid(), "expectedRevision": Uuid::new_v4()})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
