//! Desktop browser proof: real HTTP, queue, adapter and PostgreSQL; owned loopback output only.
use super::*;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        control::BudgetPolicy,
        control_store::ControlScope,
        note_analysis as owner,
        provider_store::MaterialClass,
        skill_binding::{SwitchSkill, switch_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        task_config::{MaterialColumn, ModelSelection, SaveTaskConfig, save_task_config},
    },
    note_analysis::Scope,
};
use serde_json::{Value, json};
use uuid::Uuid;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "owned isolated PostgreSQL and desktop browser; no external model"]
async fn note_analysis_browser_host() {
    assert_eq!(
        std::env::var("NOTE_ANALYSIS_BROWSER_PROOF").as_deref(),
        Ok("1")
    );
    let output = std::path::PathBuf::from(std::env::var_os("NOTE_ANALYSIS_PROOF_OUTPUT").unwrap());
    assert!(output.is_absolute());
    std::fs::create_dir_all(&output).unwrap();
    assert!(
        !output.join("stop").exists(),
        "remove old stop signal before starting"
    );
    let dist = std::env::var_os("LINGGAN_WEB_DIST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist")
        });
    assert!(
        dist.join("index.html").is_file(),
        "build the candidate React app first"
    );
    std::fs::write(
        output.join("response-schema.json"),
        serde_json::to_vec_pretty(&linggan_intelligence::note_analysis::response_schema()).unwrap(),
    )
    .unwrap();
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "note_analysis_browser_a",
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let endpoint = BrowserEndpoint::start(output.clone()).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    );
    let mut session = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "note-analysis-probe",
        4,
    )
    .await
    .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &session, &adapter).await;
    session.request_drain().await.unwrap();
    session.complete_drain().await.unwrap();
    drop(session);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='note_interpretation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let cfg = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: linggan_intelligence::ai_foundation::capability::ModelLimits {
                    input_tokens: 31768,
                    ..model.limits.clone()
                },
            }),
            skill_version_ref: None,
            material_columns: [
                MaterialColumn::Title,
                MaterialColumn::Body,
                MaterialColumn::Ocr,
            ]
            .into_iter()
            .collect(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let skill_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/data-contracts/ai-midterm-quality-002/skill/linggan-note-analysis");
    let files = ["SKILL.md", "references/output-guide.md"]
        .into_iter()
        .map(|path| UploadFile {
            path: path.into(),
            encoding: Encoding::Utf8,
            content: std::fs::read_to_string(skill_root.join(path)).unwrap(),
        })
        .collect();
    let skill = import_skill(
        &db,
        &adapter,
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: None,
            package: SkillPackage {
                directory: "linggan-note-analysis".into(),
                files,
            },
        },
    )
    .await
    .unwrap();
    switch_skill(
        &db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_task_revision: cfg.revision,
            expected_binding_revision: 0,
            version_ref: skill.version_ref,
        },
    )
    .await
    .unwrap();
    setup::enable(&db, ControlScope::Task(TaskKey::NoteInterpretation)).await;
    setup::allow(
        &db,
        model.endpoint,
        TaskKey::NoteInterpretation,
        MaterialClass::NoteMaterial,
    )
    .await;
    let mut scenarios = Vec::new();
    for (index, sid) in ["N02", "N03", "N05", "N08"].iter().enumerate() {
        let external = format!("note-analysis-{sid}");
        let (title, body, voices) = if let Ok(root) = std::env::var("NOTE_ANALYSIS_PRIVATE_SAMPLES")
        {
            let root = std::path::Path::new(&root).join(sid);
            let d: Value =
                serde_json::from_slice(&std::fs::read(root.join("dossier.share.json")).unwrap())
                    .unwrap();
            let c: Value =
                serde_json::from_slice(&std::fs::read(root.join("comments.json")).unwrap())
                    .unwrap();
            let text = d["materials"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|m| m["blocks"].as_array().unwrap())
                .filter(|b| b["kind"] != "gap")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            (
                d["note"]["platformTitle"].as_str().unwrap().to_owned(),
                text,
                c["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| {
                        (
                            c["body"].as_str().unwrap().to_owned(),
                            c["isNoteAuthor"].as_bool().unwrap(),
                            c["parentCommentId"].as_str().map(str::to_owned),
                            c["commentId"].as_str().unwrap().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            (
                format!("合成{sid}"),
                [
                    "坐下未落笔，清桌并读第一题。",
                    "限时从1找到50，准备开始。",
                    "晨起和课后分别列出活动做法。",
                    "先说明自己承担什么，再给老师具体请求。",
                ][index]
                    .to_owned(),
                if index == 3 {
                    vec![]
                } else {
                    vec![
                        ("今晚试试。".into(), false, None, "c1".into()),
                        (
                            "作者回复：可以尝试。".into(),
                            true,
                            Some("c1".into()),
                            "c2".into(),
                        ),
                    ]
                },
            )
        };
        let package=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":external}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"title":title,"bodyText":body,"authorId":"test-author"}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(&external)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        for (text, author, parent, id) in &voices {
            let mut payload = json!({"commentId":id,"noteId":external,"text":text,"authorId":if *author {"test-author"} else {"test-reader"}});
            if let Some(parent) = parent {
                payload["rootCommentId"] = json!(parent);
                payload["parentCommentId"] = json!(parent);
            }
            fixture::submit_package_at(&db, if parent.is_some(){"replies"}else{"comments"},json!({"contentExternalId":external}),json!({"kind":if parent.is_some(){"reply"}else{"comment"},"sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":payload}),"2026-10-14T00:00:00Z").await;
        }
        scenarios.push(json!({"sample":sid,"work":work,"domain":dossier::DOMAIN,"url":format!("http://127.0.0.1:4186/corpus/evidence?domain={}&work={work}",dossier::DOMAIN)}));
    }
    setup::allow(
        &db,
        model.endpoint,
        TaskKey::NoteInterpretation,
        MaterialClass::CommentMaterial,
    )
    .await;
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let executor = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "note-analysis-browser",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4186")
        .await
        .unwrap();
    let app = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    std::fs::write(output.join("fixture.json"),serde_json::to_vec_pretty(&json!({"origin":"http://127.0.0.1:4186","domain":dossier::DOMAIN,"scenarios":scenarios,"provider":"owned loopback synthetic response","realModelQualityVerified":false})).unwrap()).unwrap();
    eprintln!(
        "NOTE_ANALYSIS_BROWSER_READY http://127.0.0.1:4186 manifest={}",
        output.join("fixture.json").display()
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    while !output.join("stop").exists() && std::time::Instant::now() < deadline {
        assert!(!executor.is_finished(), "executor exited");
        // Persist complete frozen inputs privately so budget measurements use the actual packet.
        let rows: Vec<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(r) FROM linggan_note_analysis_request r ORDER BY created_at",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        std::fs::write(
            output.join("requests.json"),
            serde_json::to_vec_pretty(&rows).unwrap(),
        )
        .unwrap();
        for scenario in &scenarios {
            let work: Uuid = serde_json::from_value(scenario["work"].clone()).unwrap();
            let latest = owner::latest(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
                .await
                .unwrap();
            std::fs::write(
                output.join(format!(
                    "{}.latest.json",
                    scenario["sample"].as_str().unwrap()
                )),
                serde_json::to_vec_pretty(&latest).unwrap(),
            )
            .unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    server.abort();
    drain.request();
    executor.await.unwrap().unwrap();
    std::fs::write(
        output.join("loopback-requests.json"),
        serde_json::to_vec_pretty(&*endpoint.requests.lock().unwrap()).unwrap(),
    )
    .unwrap();
    endpoint.finish().await;
    db.pool().close().await;
}

#[path = "../../../../crates/intelligence/tests/support/note_analysis_response.rs"]
mod synthetic_response;

struct BrowserEndpoint {
    url: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}
impl BrowserEndpoint {
    async fn start(output: std::path::PathBuf) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let sample = std::env::var_os("NOTE_ANALYSIS_N02_EXAMPLE").map(|p| {
            serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()["resultRecord"]
                    ["result"]
                    .clone()
        });
        let source_blocks: Vec<Value> = std::env::var_os("NOTE_ANALYSIS_PRIVATE_SAMPLES")
            .map(|root| {
                let dossier: Value = serde_json::from_slice(
                    &std::fs::read(std::path::PathBuf::from(root).join("N02/dossier.share.json"))
                        .unwrap(),
                )
                .unwrap();
                dossier["materials"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|m| m["blocks"].as_array().unwrap().clone())
                    .collect()
            })
            .unwrap_or_default();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = wire::read(&mut socket).await;
                let prompt = wire::text(&request, "user");
                recorded.lock().unwrap().push(request);
                let response = if prompt.contains("ai-probe.synthetic.v1") {
                    json!({"lingganProbe":"ok"})
                } else if output.join("fail-next").exists() {
                    std::fs::remove_file(output.join("fail-next")).unwrap();
                    json!({"syntheticFailure":true})
                } else {
                    let packet: Value = serde_json::from_str(&prompt).unwrap();
                    match sample
                        .as_ref()
                        .and_then(|s| map_saved_example(s, &packet, &source_blocks))
                    {
                        Some(value) => value,
                        None => synthetic_response::response(&packet),
                    }
                };
                wire::respond(&mut socket, &response.to_string()).await;
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }
    async fn finish(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

/// This is human-authored layout material replay, never real model quality evidence.
fn map_saved_example(example: &Value, packet: &Value, source_blocks: &[Value]) -> Option<Value> {
    let sources = packet["material"]["sources"].as_array()?;
    let mut result = example.clone();
    let mut mapped = Vec::new();
    for evidence in example["evidence"].as_array()? {
        let quote = evidence["quote"].as_str()?;
        let mut matches: Vec<_> = sources
            .iter()
            .filter(|s| {
                s["kind"] == evidence["kind"]
                    && s["text"].as_str().is_some_and(|text| text.contains(quote))
            })
            .collect();
        if matches.len() > 1 {
            let original = source_blocks
                .iter()
                .find(|b| b["shareRef"] == evidence["exampleShareRef"])?;
            matches.retain(|source| source["text"] == original["text"]);
        }
        if matches.len() != 1 {
            return None;
        }
        mapped.push(
            json!({"id":evidence["id"],"inputCitationId":matches[0]["citationId"],"quote":quote}),
        );
    }
    result["evidence"] = json!(mapped);
    result["summary"] = json!(format!(
        "人工样稿回放（非模型输出）：{}",
        example["summary"].as_str()?
    ));
    Some(result)
}
