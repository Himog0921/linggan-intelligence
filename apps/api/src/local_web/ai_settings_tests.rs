use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "../../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod writing_fixture;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod writing_setup;

#[tokio::test]
#[ignore = "disposable PostgreSQL and actual Axum HTTP proof only"]
async fn ai_settings_http_writing_confirmation_is_queued_and_cancelled_without_inline_invocation() {
    use linggan_intelligence::content_workbench_ai as ai;
    let db = writing_fixture::db("ai_writing_http_queue").await;
    let config = writing_setup::configure_fake_prerequisites(&db).await;
    let work = writing_fixture::create(&db, writing_fixture::doc()).await;
    let app = app_with_database(db.clone());
    let preview = ai::PreviewRequest {
        selection: None,
        preview_ref: Uuid::new_v4(),
        work_ref: work.work_id,
        expected_revision: work.revision.id,
        action: ai::WritingAction::Draft,
        config_ref: config,
        citations: vec![],
        paragraph_ids: vec![],
        instruction: "Synthetic local HTTP queue proof".into(),
    };
    let (status, p) = call(
        app.clone(),
        "/ops/api/creation/ai/preview",
        Some(serde_json::to_value(&preview).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    let (status, samples) = call(app.clone(), "/api/local/ai-settings/writing-samples", None).await;
    assert_eq!(status, StatusCode::OK, "{samples}");
    assert_eq!(samples["state"], "connected");
    assert_eq!(samples["samples"].as_array().unwrap().len(), 1);
    assert_eq!(
        samples["samples"][0]["previewRef"],
        json!(preview.preview_ref)
    );
    assert!(!samples.to_string().contains("inputHash"));
    assert!(
        !samples
            .to_string()
            .contains("Synthetic local HTTP queue proof")
    );
    let confirmed = ai::ExecuteRequest {
        request_id: Uuid::new_v4(),
        preview_ref: preview.preview_ref,
        expected_revision: work.revision.id,
        confirmed_input_hash: p["inputHash"].as_str().unwrap().into(),
        budget_tokens: p["reservedTokens"].as_i64().unwrap(),
        confirmed: true,
    };
    for _ in 0..2 {
        let (status, queued) = call(
            app.clone(),
            "/ops/api/creation/ai/requests",
            Some(serde_json::to_value(&confirmed).unwrap()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{queued}");
        assert_eq!(queued["state"], "queued");
        assert!(queued["invocationRef"].is_null());
        assert_eq!(queued["usage"]["state"], "not_reserved");
    }
    let (status, cancelled) = call(
        app.clone(),
        &format!(
            "/ops/api/creation/ai/requests/{}/cancel",
            confirmed.request_id
        ),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["state"], "cancelled");
    assert!(cancelled["invocationRef"].is_null());
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='writing_legacy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(calls, 0);
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creation_ai_request")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(requests, 1);
    let mut document = work.revision.document.clone();
    document.title = "Synthetic later revision".into();
    linggan_intelligence::content_workbench::save(
        &db,
        work.work_id,
        &linggan_intelligence::content_workbench::SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document,
        },
    )
    .await
    .unwrap();
    let (status, stale) = call(app, "/api/local/ai-settings/writing-samples", None).await;
    assert_eq!(status, StatusCode::OK, "{stale}");
    assert_eq!(stale["samples"], json!([]));
}

async fn call(app: Router, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut request = Request::builder().uri(uri).header("Host", "127.0.0.1:3000");
    if body.is_some() {
        request = request
            .method("POST")
            .header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 * 1048576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "owned PostgreSQL and actual Axum HTTP only; no provider dispatch"]
async fn ai_settings_http_batch_cancel_is_cas_idempotent_and_never_revived_by_resume() {
    use linggan_intelligence::ai_foundation::{
        TaskKey, control_store::ControlScope, provider_store::MaterialClass,
    };
    let db = writing_fixture::db("ai_batch_cancel_http").await;
    let declared = writing_setup::declare(&db, "http://127.0.0.1:1/v1").await;
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::ModelProbe),
    ] {
        writing_setup::enable(&db, scope).await;
    }
    writing_setup::allow(
        &db,
        declared.endpoint,
        TaskKey::ModelProbe,
        MaterialClass::ModelCatalog,
    )
    .await;
    let app = app_with_database(db.clone());
    let (status,config)=call(app.clone(),"/api/local/ai-settings/task-configs",Some(json!({"commandRef":Uuid::new_v4(),"taskKey":"model_probe","expectedRevision":0,"model":null,"skillVersionRef":null,"materialColumns":[],"budget":{"mode":"unlimited"}}))).await;
    assert_eq!(status, StatusCode::OK, "{config}");
    let items:Vec<_>=(0..101).map(|n|json!({"itemKey":n.to_string(),"input":{"kind":"probe","sampleRef":Uuid::new_v4(),"connectionVersionRef":declared.version,"modelRef":null,"operation":"connect","profile":null}})).collect();
    let (status,batch)=call(app.clone(),"/api/local/ai-settings/batches",Some(json!({"commandRef":Uuid::new_v4(),"taskKey":"model_probe","expectedTaskRevision":config["revision"],"taskConfigRef":config["taskConfigRef"],"kind":"probe","items":items}))).await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    let batch = batch["batchRef"].as_str().unwrap();
    let url = format!("/api/local/ai-settings/batches/{batch}");
    let (status, first) = call(app.clone(), &url, None).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    assert!(first["items"][0].get("output").is_none());
    let (status, last) = call(app.clone(), &format!("{url}?fromPosition=100"), None).await;
    assert_eq!(status, StatusCode::OK, "{last}");
    assert_eq!(last["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        call(app.clone(), &format!("{url}?fromPosition=10000"), None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, settings) = call(app.clone(), "/api/local/ai-settings", None).await;
    assert_eq!(settings["batches"][0]["pending"], 101);
    let packet = json!({"commandRef":Uuid::new_v4(),"batchRef":batch,"expectedRevision":first["batch"]["revision"],"reason":"Synthetic explicit cancellation"});
    let mut stale = packet.clone();
    stale["expectedRevision"] = json!(999);
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/batches/cancel",
            Some(stale)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/batches/cancel",
        Some(packet.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["state"], "cancelled");
    assert_eq!(receipt["cancelRequested"], true);
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/batches/cancel",
            Some(packet.clone())
        )
        .await
        .1,
        receipt
    );
    let mut conflicting = packet;
    conflicting["reason"] = json!("Changed payload under same command ID");
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/batches/cancel",
            Some(conflicting)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status,resumed)=call(app.clone(),"/api/local/ai-settings/control",Some(json!({"commandRef":Uuid::new_v4(),"scope":{"kind":"task","task":"model_probe"},"expectedRevision":1,"enabled":true,"reason":"Synthetic human resume cannot reopen cancelled work"}))).await;
    assert_eq!(status, StatusCode::OK, "{resumed}");
    let (status, after) = call(app, &url, None).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(after["batch"]["state"], "cancelled");
    assert_eq!(after["rounds"].as_array().unwrap().len(), 3);
    assert!(
        after["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["state"] == "cancelled"
                && i["attemptCount"] == 0
                && i["invocationRef"].is_null())
    );
    assert!(
        after["rounds"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["attempted"] == 0 && r["failed"] == 0 && r["usable"] == 0)
    );
    let invoices: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(invoices, 0);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_settings_http_ocr_prompt_save_switch_and_rollback_do_not_dispatch() {
    let db = fixture::proof_database("ai_ocr_prompt_http").await;
    let app = app_with_database(db.clone());
    let (status, initial) = call(
        app.clone(),
        "/api/local/ai-settings/prompts/ocr_recognition",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{initial}");
    let old = initial["activeVersionRef"].clone();
    let revision = initial["revision"].as_i64().unwrap();
    let save = json!({"commandRef":Uuid::new_v4(),"taskKey":"ocr_recognition","expectedTaskRevision":revision,
        "systemText":"Synthetic OCR system","instructionText":"Synthetic OCR instructions"});
    let (status, version) = call(
        app.clone(),
        "/api/local/ai-settings/prompts/save",
        Some(save.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{version}");
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/prompts/save",
            Some(save)
        )
        .await
        .1,
        version
    );
    let enable = json!({"commandRef":Uuid::new_v4(),"taskKey":"ocr_recognition","expectedTaskRevision":revision,"versionRef":version["versionRef"]});
    let (status, enabled) = call(
        app.clone(),
        "/api/local/ai-settings/prompts/activate",
        Some(enable),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    let (status, current) = call(
        app.clone(),
        "/api/local/ai-settings/prompts/ocr_recognition",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current["activeVersionRef"], version["versionRef"]);
    let rollback = json!({"commandRef":Uuid::new_v4(),"taskKey":"ocr_recognition","expectedTaskRevision":enabled["taskRevision"],"versionRef":old});
    let (status, _) = call(
        app.clone(),
        "/api/local/ai-settings/prompts/activate",
        Some(rollback),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_settings_http_skill_import_handles_large_files_without_enabling_or_running_them() {
    let db = fixture::proof_database("ai_skills_http").await;
    let app = app_with_database(db.clone());
    let manifest = format!(
        "---\nname: example\ndescription: Synthetic skill\n---\n{}",
        "本地方法。".repeat(6000)
    );
    let packet = json!({"commandRef":Uuid::new_v4(),"expectedVersionRef":null,
        "package":{"directory":"example","files":[{"path":"SKILL.md","encoding":"utf8","content":manifest}]}});
    assert!(packet.to_string().len() > 65536);
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/skills/import",
        Some(packet.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["versionNo"], 1);
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/skills/import",
            Some(packet.clone())
        )
        .await
        .1,
        receipt
    );
    let (status, version) = call(
        app.clone(),
        &format!(
            "/api/local/ai-settings/skills/{}",
            receipt["versionRef"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(version["name"], "example");
    assert_eq!(version["files"][0]["content"], manifest);
    let (status, settings) = call(app.clone(), "/api/local/ai-settings", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["skills"].as_array().unwrap().len(), 1);
    assert!(
        settings["controls"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["enabled"] == false)
    );
    let mut invalid = packet;
    invalid["commandRef"] = json!(Uuid::new_v4());
    invalid["package"]["files"][0]["path"] = json!("scripts/runner");
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/skills/import",
            Some(invalid)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_skill_version")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_settings_http_bounds_cover_exact_budgets_large_batches_and_the_maximum_skill_file_count()
 {
    let db = fixture::proof_database("ai_settings_http_bounds").await;
    let app = app_with_database(db.clone());
    let config_body = json!({"commandRef":Uuid::new_v4(),"taskKey":"model_probe","expectedRevision":0,
        "model":null,"skillVersionRef":null,"materialColumns":[],"budget":{"mode":"capped","tokens":"9223372036854775807"}});
    let (status, config) = call(
        app.clone(),
        "/api/local/ai-settings/task-configs",
        Some(config_body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{config}");
    assert_eq!(config["applied"], true);
    let (status, stored) = call(
        app.clone(),
        &format!(
            "/api/local/ai-settings/task-configs/{}",
            config["taskConfigRef"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["budget"]["tokens"], "9223372036854775807");
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref) VALUES($1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic metadata','openai-completions','https://synthetic-never-fetched.example/api',false,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    let endpoint: Uuid = sqlx::query_scalar(
        "SELECT endpoint_ref FROM linggan_ai_connection_endpoint WHERE version_ref=$1",
    )
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let batch = json!({"commandRef":Uuid::new_v4(),"taskKey":"model_probe","expectedTaskRevision":1,
        "taskConfigRef":config["taskConfigRef"],"kind":"probe","items":(0..1000).map(|i|json!({"itemKey":format!("probe-{i}"),
         "input":{"kind":"probe","sampleRef":Uuid::new_v4(),"connectionVersionRef":version,"modelRef":null,"operation":"connect"}})).collect::<Vec<_>>()});
    assert!(batch.to_string().len() > 65536);
    let (status, denied) = call(
        app.clone(),
        "/api/local/ai-settings/batches",
        Some(batch.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(denied["error"], "ai_admission_denied");
    for scope in [
        json!({"kind":"global"}),
        json!({"kind":"executor"}),
        json!({"kind":"task","task":"model_probe"}),
    ] {
        let (status, result) = call(
            app.clone(),
            "/api/local/ai-settings/control",
            Some(json!({"commandRef":Uuid::new_v4(),"scope":scope,
            "expectedRevision":0,"enabled":true,"reason":"synthetic fixture only"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
    }
    let (status,result)=call(app.clone(),"/api/local/ai-settings/egress",Some(json!({"commandRef":Uuid::new_v4(),
        "endpointRef":endpoint,"taskKey":"model_probe","materialClass":"model_catalog","expectedEventRef":null,
        "allowed":true,"reason":"synthetic fixture only"}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/batches",
        Some(batch.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["itemCount"], 1000);
    assert_eq!(receipt["accepted"], true);
    assert_eq!(
        call(app.clone(), "/api/local/ai-settings/batches", Some(batch))
            .await
            .1,
        receipt
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_batch_item")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1000
    );
    let mut files = vec![
        json!({"path":"SKILL.md","encoding":"utf8","content":format!("---\nname: boundary-skill\ndescription: Synthetic boundary package\n---\n{}x","\n".repeat(16000))}),
    ];
    let path_prefix = format!(
        "references/{}/{}/{}/{}/{}/",
        "a".repeat(180),
        "b".repeat(180),
        "c".repeat(180),
        "d".repeat(180),
        "e".repeat(180)
    );
    for i in 0..63 {
        files.push(json!({"path":format!("{path_prefix}part-{i:02}.md"),"encoding":"utf8","content":format!("{}x","\n".repeat(16000))}));
    }
    let package = json!({"commandRef":Uuid::new_v4(),"expectedVersionRef":null,"package":{"directory":"boundary-skill","files":files}});
    assert!(
        package.to_string().len() > 2 * 1048576,
        "a legal normalized package can exceed the default JSON envelope limit"
    );
    let (status, imported) = call(
        app.clone(),
        "/api/local/ai-settings/skills/import",
        Some(package),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_skill_file")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        64
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_skill_binding_event")
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
#[tokio::test]
async fn ai_settings_routes_are_explicitly_unavailable_without_a_database() {
    for route in ["/api/local/ai-settings", "/api/local/ai-settings/control"] {
        let (status, value) = call(
            app(),
            route,
            if route.ends_with("/control") {
                Some(json!({}))
            } else {
                None
            },
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(value["error"], "ai_database_unavailable");
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_settings_http_commands_are_origin_guarded_cas_and_never_call_a_provider() {
    let db = fixture::proof_database("ai_settings_http").await;
    let app = app_with_database(db.clone());
    let (status, value) = call(app.clone(), "/api/local/ai-settings", None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["contract"], "ai-settings.v1");
    assert!(
        value["controls"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["enabled"] == false)
    );
    assert!(
        value["models"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["capability"].is_null() && r["price"].is_null())
    );
    assert!(!value.to_string().contains("secretRef"));
    assert!(!value.to_string().contains("apiKey"));
    let id = Uuid::new_v4();
    let payload = json!({"commandRef":id,"scope":{"kind":"global"},"expectedRevision":0,"enabled":true,"reason":"synthetic switch intent"});
    let hostile = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/ai-settings/control")
                .header("Host", "127.0.0.1:3000")
                .header("Origin", "https://untrusted.example")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hostile.status(), StatusCode::FORBIDDEN);
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/control",
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["revision"], 1);
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/control",
            Some(payload.clone())
        )
        .await
        .1,
        receipt
    );
    let mut stale = payload.clone();
    stale["commandRef"] = json!(Uuid::new_v4());
    assert_eq!(
        call(app.clone(), "/api/local/ai-settings/control", Some(stale))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut forged = payload;
    forged["actor"] = json!("system");
    let (status, rejected) =
        call(app.clone(), "/api/local/ai-settings/control", Some(forged)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected, json!({"error":"ai_invalid_command"}));
    let (status, after) = call(app.clone(), "/api/local/ai-settings", None).await;
    assert_eq!(status, StatusCode::OK);
    let global = after["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["scope"] == "global")
        .unwrap();
    assert_eq!(global["revision"], 1);
    assert_eq!(global["commandRef"], json!(id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control_event")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and fixed offline catalog; no provider"]
async fn ai_settings_http_catalog_presets_are_offline_versioned_and_cannot_forge_capacity_or_price()
{
    let db = fixture::proof_database("ai_catalog_http").await;
    let app = app_with_database(db.clone());
    let (status,page)=call(app.clone(),"/api/local/ai-settings/catalog?api=openai-completions&provider=deepseek&modelId=deepseek-v4-pro",None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    let preset = &page["presets"][0];
    assert_eq!(preset["limits"]["maxTimeoutSeconds"], Value::Null);
    assert_eq!(preset["prices"]["cachedWrite"], Value::Null);
    assert_eq!(preset["prices"]["input"], "435000");
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref) VALUES($1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic catalog metadata','openai-completions','https://catalog.example/api',false,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'deepseek-v4-pro','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    let endpoint: Uuid = sqlx::query_scalar(
        "SELECT endpoint_ref FROM linggan_ai_connection_endpoint WHERE version_ref=$1",
    )
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let mut limits = preset["limits"].clone();
    limits["maxTimeoutSeconds"] = json!(300);
    let capability = json!({"commandRef":Uuid::new_v4(),"identityRef":model,"endpointRef":endpoint,"expectedCapabilityRef":null,
        "source":"pi_catalog","sourceVersion":preset["sourceVersion"],"probeInvocationRef":null,"limits":limits,
        "compat":preset["compat"],"thinkingLevelMap":preset["thinkingLevelMap"],"outputMode":preset["outputMode"],"strictSchema":false});
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/capabilities",
        Some(capability.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let mut forged = capability.clone();
    forged["commandRef"] = json!(Uuid::new_v4());
    forged["expectedCapabilityRef"] = receipt["revisionRef"].clone();
    forged["limits"]["contextTokens"] = json!(1_000_001);
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/capabilities",
            Some(forged)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut forged = capability;
    forged["commandRef"] = json!(Uuid::new_v4());
    forged["expectedCapabilityRef"] = receipt["revisionRef"].clone();
    forged["outputMode"] = json!("json_schema");
    assert_eq!(
        call(
            app.clone(),
            "/api/local/ai-settings/capabilities",
            Some(forged)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let price = json!({"commandRef":Uuid::new_v4(),"identityRef":model,"endpointRef":endpoint,"expectedPriceRef":null,
        "source":"pi_catalog","sourceVersion":preset["sourceVersion"],"prices":preset["prices"]});
    let (status, receipt) = call(
        app.clone(),
        "/api/local/ai-settings/prices",
        Some(price.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let mut forged = price;
    forged["commandRef"] = json!(Uuid::new_v4());
    forged["expectedPriceRef"] = receipt["revisionRef"].clone();
    forged["prices"]["input"] = json!("435001");
    assert_eq!(
        call(app.clone(), "/api/local/ai-settings/prices", Some(forged))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control WHERE enabled")
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_capability")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_price")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and actual Axum HTTP proof only"]
async fn ai_settings_http_legacy_probe_and_default_save_are_retired_before_any_provider_or_write() {
    let db = fixture::proof_database("ai_retired_model_routes").await;
    let app = app_with_database(db.clone());
    let before: Value = sqlx::query_scalar("SELECT to_jsonb(w) FROM linggan_model_workspace w")
        .fetch_one(db.pool())
        .await
        .unwrap();
    for (uri, body) in [
        ("/api/local/model-settings", None),
        (
            "/api/local/model-settings/probes",
            Some(json!({"apiKey":"SYNTHETIC-NEVER-DISPATCH"})),
        ),
        (
            "/api/local/model-settings/config",
            Some(json!({"configRef":Uuid::new_v4()})),
        ),
    ] {
        let (status, value) = call(app.clone(), uri, body).await;
        assert_eq!(status, StatusCode::GONE, "{value}");
        assert_eq!(value["error"], "ai_legacy_endpoint_retired");
        assert!(!value.to_string().contains("SYNTHETIC-NEVER-DISPATCH"));
    }
    let after: Value = sqlx::query_scalar("SELECT to_jsonb(w) FROM linggan_model_workspace w")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
    let (status, page) = call(app, "/api/local/ai-settings/invocations", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"], json!([]));
}

#[tokio::test]
#[ignore = "owned PostgreSQL and actual Axum retirement proof only"]
async fn ai_settings_http_comment_creator_and_topic_execution_cannot_be_reenabled_through_old_routes()
 {
    let db = fixture::proof_database("ai_retired_business_routes").await;
    let app = app_with_database(db.clone());
    for (uri, payload) in [
        ("/api/local/comment-study/runs".to_string(), json!({})),
        ("/api/local/comment-study/policy".to_string(), json!({})),
        (
            "/api/local/creator-discovery-analysis/retry".to_string(),
            json!({"domain":Uuid::new_v4()}),
        ),
        (
            "/api/local/topic-map/research/commands".to_string(),
            json!({"action":"configure","requestRef":Uuid::new_v4(),"domainRef":Uuid::new_v4(),"modelConfigRef":Uuid::new_v4(),"dailyTokenLimit":1024,"runTokenLimit":1024,"automaticEnabled":true,"collectionEnabled":true}),
        ),
    ] {
        let (status, value) = call(app.clone(), &uri, Some(payload)).await;
        assert_eq!(status, StatusCode::GONE, "{uri}: {value}");
        assert_eq!(value["error"], "ai_legacy_execution_retired");
    }
    let response=app.clone().oneshot(Request::builder().method("PUT").uri("/api/local/creator-discovery-policy").header("Host","127.0.0.1:3000").header("Content-Type","application/json")
        .body(Body::from(json!({"domain":Uuid::new_v4(),"revision":0,"likeThreshold":10,"analysisEnabled":true,"configRef":Uuid::new_v4(),"dailyTokenLimit":1024}).to_string())).unwrap()).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "the current threshold DTO rejects retired AI-policy fields"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}
