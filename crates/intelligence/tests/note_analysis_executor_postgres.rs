//! Four sample shapes through the real queue and isolated PostgreSQL with a loopback fixture.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
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
#[tokio::test]
#[ignore = "owned PostgreSQL, loopback adapter; optional private samples remain outside Git"]
async fn four_notes_complete_and_preserve_boundaries() {
    let db = fixture::proof_database("note_analysis_a").await;
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
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
    let mut jobs = Vec::new();
    // Cargo runs integration tests from the crate, so artifact destinations must be absolute.
    if let Ok(dir) = std::env::var("NOTE_ANALYSIS_PROOF_OUTPUT") {
        assert!(
            std::path::Path::new(&dir).is_absolute(),
            "private proof path must be absolute"
        );
    }
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
            if let Some(parent) = parent {
                comments::reply_with_author(
                    &db,
                    &external,
                    id,
                    parent,
                    text,
                    Some(if *author {
                        "test-author"
                    } else {
                        "test-reader"
                    }),
                    "2026-10-14T00:00:00Z",
                )
                .await;
            } else {
                comments::comment_with_author(
                    &db,
                    &external,
                    id,
                    text,
                    Some(if *author {
                        "test-author"
                    } else {
                        "test-reader"
                    }),
                    "2026-10-14T00:00:00Z",
                )
                .await;
            }
        }
        if index == 0 {
            let denied = owner::preview(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
                .await
                .unwrap();
            assert_eq!(denied["canStart"], false);
            assert!(
                denied["reasons"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r == "comment_egress_not_approved")
            );
            setup::allow(
                &db,
                model.endpoint,
                TaskKey::NoteInterpretation,
                MaterialClass::CommentMaterial,
            )
            .await;
        }
        let p = owner::preview(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
            .await
            .unwrap();
        assert_eq!(p["canStart"], true, "preview reasons: {}", p["reasons"]);
        assert_eq!(p["coverage"]["includedCommentCount"], voices.len());
        let r = owner::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            scope: Scope::NoteWithComments,
            knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
            input_hash: p["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
            task_revision: p["taskRevision"].as_i64().unwrap(),
        };
        let receipt = owner::start(&db, &r).await.unwrap();
        assert_eq!(
            owner::start(&db, &r).await.unwrap().request_ref,
            receipt.request_ref
        );
        jobs.push((sid.to_string(), work, receipt.request_ref));
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let task = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "note-analysis-four-proof",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let mut receipts = Vec::new();
    for (sid, work, id) in &jobs {
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let v = owner::read(&db, dossier::DOMAIN, *id).await.unwrap();
                if v["run"]["state"] == "succeeded" {
                    break v;
                }
                assert!(
                    ![json!("failed"), json!("skipped")].contains(&v["run"]["state"]),
                    "run failed: {}",
                    v["run"]
                );
                assert!(!task.is_finished(), "executor exited");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        let latest = owner::latest(&db, dossier::DOMAIN, *work, Scope::NoteWithComments)
            .await
            .unwrap();
        assert_eq!(latest["latestSuccessful"]["requestRef"], json!(id));
        assert_eq!(result["freshness"], "current");
        let material:String=sqlx::query_scalar("SELECT input_manifest->>'materialSnapshotIdentity' FROM linggan_note_analysis_request WHERE request_ref=$1").bind(id).fetch_one(db.pool()).await.unwrap();
        for purpose in ["topic", "comment"] {
            let v = owner::read_context(&db, dossier::DOMAIN, *work, &material, purpose)
                .await
                .unwrap()
                .unwrap();
            assert!(v.get("feedback").is_none());
            assert!(
                v["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["kind"] == "material")
            );
        }
        if sid == "N08" {
            assert!(result["result"]["feedback"].as_array().unwrap().is_empty());
        }
        receipts.push(json!({"sample":sid,"requestRef":id,"state":"succeeded","coverage":result["coverage"],"syntheticProvider":true,"realModelQualityVerified":false}));
        if let Ok(dir) = std::env::var("NOTE_ANALYSIS_PROOF_OUTPUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{sid}.read.json")),
                serde_json::to_vec_pretty(&result).unwrap(),
            )
            .unwrap();
        }
    }
    assert!(owner::read(&db, Uuid::new_v4(), jobs[0].2).await.is_err());
    // A new comment invalidates the preview, but never rewrites an already frozen result.
    let work = jobs[3].1;
    let before = owner::preview(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
        .await
        .unwrap();
    comments::comment_with_author(
        &db,
        "note-analysis-N08",
        "new-voice",
        "SYNTHETIC 新评论",
        Some("test-reader"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    assert!(matches!(
        owner::start(&db, &start_from(&before, work)).await,
        Err(linggan_intelligence::ai_foundation::store::FoundationError::Conflict)
    ));
    assert_eq!(
        owner::read(&db, dossier::DOMAIN, jobs[3].2).await.unwrap()["freshness"],
        "source_changed"
    );
    let newer = owner::preview(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
        .await
        .unwrap();
    let receipt = owner::start(&db, &start_from(&newer, work)).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let v = owner::read(&db, dossier::DOMAIN, receipt.request_ref)
                .await
                .unwrap();
            if v["run"]["state"] == "succeeded" {
                break;
            }
            assert_ne!(v["run"]["state"], "failed");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'new-voice','synthetic withdrawal')").bind(work).execute(db.pool()).await.unwrap();
    assert!(
        owner::read(&db, dossier::DOMAIN, receipt.request_ref)
            .await
            .is_err()
    );
    let fallback = owner::latest(&db, dossier::DOMAIN, work, Scope::NoteWithComments)
        .await
        .unwrap();
    assert_eq!(fallback["latestSuccessful"]["requestRef"], json!(jobs[3].2));
    // Later attribution must not reclassify the speakers in an old request.
    let before = owner::read(&db, dossier::DOMAIN, jobs[0].2).await.unwrap();
    comments::detail_with_author(
        &db,
        "note-analysis-N02",
        "SYNTHETIC later attribution",
        Some("different-author"),
    )
    .await;
    let after = owner::read(&db, dossier::DOMAIN, jobs[0].2).await.unwrap();
    assert_eq!(before["result"], after["result"]);
    assert_eq!(before["coverage"], after["coverage"]);
    let fresh = owner::preview(&db, dossier::DOMAIN, jobs[0].1, Scope::NoteWithComments)
        .await
        .unwrap();
    assert_eq!(fresh["coverage"]["authorCommentCount"], 0);

    drain.request();
    task.await.unwrap().unwrap();
    endpoint.finish().await;
    if let Ok(dir) = std::env::var("NOTE_ANALYSIS_PROOF_OUTPUT") {
        std::fs::write(
            std::path::Path::new(&dir).join("receipt.json"),
            serde_json::to_vec_pretty(&receipts).unwrap(),
        )
        .unwrap();
    }
}

fn start_from(p: &Value, work: Uuid) -> owner::Start {
    owner::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        scope: Scope::NoteWithComments,
        knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
        input_hash: p["inputHash"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
        task_revision: p["taskRevision"].as_i64().unwrap(),
    }
}
