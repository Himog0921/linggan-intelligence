use super::*;
#[allow(dead_code)]
#[path = "ai_writing_fixture.rs"]
mod setup;
#[allow(dead_code)]
#[path = "ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::ai_foundation::{
    TaskKey,
    control::BudgetPolicy,
    control_store::ControlScope,
    executor::{ExecutorSession, service},
    operating_direction as directions,
    provider_store::MaterialClass,
    task_config::{MaterialColumn, ModelSelection, SaveTaskConfig, save_task_config},
};

#[tokio::test]
#[ignore = "disposable PostgreSQL and owned local SDK endpoint"]
async fn qualified_works_generate_reviewable_directions() {
    let db = fixture_database("operating_direction_generation").await;
    let works = [
        work(&db, 1, "xhs", Some(203), 3).await,
        work(&db, 2, "xhs", None, 1).await,
        work(&db, 3, "xhs", None, 1).await,
    ];
    for w in works {
        publish_current_body(&db, w).await;
    }
    let (first, _) = topic(&db, "direction-a", "作业分心", None, &[works[0], works[2]]).await;
    let (second, version) = topic(&db, "direction-b", "作业启动", None, &works[1..]).await;
    let snapshot = read_topic_map(
        &db,
        &TopicMapQuery {
            domain_ref: Some(domain()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let existing = Uuid::new_v4();
    let stale = Uuid::new_v4();
    let hidden_name = "历史方向中已撤的文案";
    let history = json!({"benchmarkAuthors":[],"directions":[
        {"directionRef":existing,"label":hidden_name,"definition":"编辑定义不外发","topicRefs":[first]},
        {"directionRef":stale,"label":"已过期来源","definition":"已过期定义","topicRefs":[first],"textSource":[first,Uuid::new_v4()]}]});
    sqlx::query("INSERT INTO linggan_operating_direction_revision(domain_ref,revision,command_ref,state,catalog) VALUES ($1,1,$2,'draft',$3)").bind(domain()).bind(Uuid::new_v4()).bind(&history).execute(db.pool()).await.unwrap();
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ExecutorSession::acquire(&db, "direction-proof-probe", 4)
        .await
        .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let tasks = [TaskKey::DirectionListDraft, TaskKey::DirectionProposal];
    for task in tasks {
        let revision =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_task_setting WHERE task_key=$1")
                .bind(task.as_str())
                .fetch_one(db.pool())
                .await
                .unwrap();
        save_task_config(
            &db,
            &SaveTaskConfig {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_revision: revision,
                model: Some(ModelSelection {
                    model_ref: model.model,
                    capability_ref: model.capability,
                    limits: model.limits.clone(),
                }),
                skill_version_ref: None,
                material_columns: [MaterialColumn::Body].into_iter().collect(),
                budget: BudgetPolicy::Unlimited,
            },
        )
        .await
        .unwrap();
        setup::enable(&db, ControlScope::Task(task)).await;
        let req = directions::Generate {
            command_ref: Uuid::new_v4(),
            task_key: task,
            domain_ref: domain(),
            topic_refs: if task == tasks[0] {
                Default::default()
            } else {
                [second].into_iter().collect()
            },
        };
        assert!(
            matches!(directions::generate(&db,req.clone(),&snapshot).await,Err(linggan_intelligence::ai_foundation::store::FoundationError::AdmissionDenied(r)) if r.iter().any(|x|x=="egress_not_approved"))
        );
        setup::allow(&db, model.endpoint, task, MaterialClass::NoteMaterial).await;
        let receipt = directions::generate(&db, req.clone(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            receipt,
            directions::generate(&db, req, &snapshot).await.unwrap()
        );
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let mut service = tokio::spawn(service::run(
        db.clone(),
        "direction-proof-executor",
        linggan_contracts::ai_limits::AI_LIMITS.default_concurrency,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let result = tokio::time::timeout(std::time::Duration::from_secs(25), async {
        loop {
            let result = directions::latest(&db, domain()).await.unwrap();
            if service.is_finished() {
                panic!(
                    "direction service terminated: {:?}; receipts: {result}",
                    (&mut service).await
                );
            }
            let rows = result["runs"].as_array().unwrap();
            assert!(
                rows.iter().all(|r| matches!(
                    r["state"].as_str(),
                    Some("pending" | "running" | "succeeded")
                )),
                "{result}"
            );
            if rows.iter().all(|r| r["state"] == "succeeded") {
                break result;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    drain.request();
    service.await.unwrap().unwrap();
    for r in result["runs"].as_array().unwrap() {
        let c = &r["candidates"][0];
        assert_eq!(c["existingDirectionRef"], json!(existing));
        assert!(
            !c["evidenceWorkRefs"]
                .as_array()
                .unwrap()
                .contains(&json!(works[2])),
            "a boundary work cannot support a direction"
        );
        assert_eq!(c["topicDefinitionRefs"][second.to_string()], json!(version));
        assert!(
            c["directionRef"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
                .is_some()
        );
        if r["taskKey"] == "direction_list_draft" {
            assert_eq!(c["topicRefs"].as_array().unwrap().len(), 2);
            assert!(c["topicRefs"].as_array().unwrap().contains(&json!(first)));
        } else {
            assert_eq!(c["topicRefs"], json!([second]));
        }
    }
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    for r in &requests[1..] {
        let p: serde_json::Value = serde_json::from_str(&wire::text(r, "user")).unwrap();
        assert!(!p.to_string().contains(hidden_name));
        assert!(
            !p["existing"]
                .as_object()
                .unwrap()
                .contains_key(&stale.to_string())
        );
        for fragments in p["evidence"].as_object().unwrap().values() {
            assert!(
                fragments
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|f| f["field"] == "body" && f["text"].is_string())
            );
        }
    }
    let published: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_operating_direction_revision WHERE state='published'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(published, 0);
    sqlx::query("INSERT INTO linggan_operating_direction_revision(domain_ref,revision,command_ref,state,catalog) VALUES ($1,2,$2,'draft',$3)").bind(domain()).bind(Uuid::new_v4()).bind(json!({"directions":[],"benchmarkAuthors":[]})).execute(db.pool()).await.unwrap();
    assert!(
        directions::latest(&db, domain()).await.unwrap()["runs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["candidates"].as_array().unwrap().len() == 1)
    );
    // Change the source definition after generation. Both candidates point to this version.
    import_topic_workspace(
        &db,
        &TopicWorkspaceImport {
            idempotency_key: "direction-revision-proof".into(),
            domain_key: "adhd-family".into(),
            canonical_key: "direction-b".into(),
            display_name: "另一个启动范围".into(),
            definition_text: "替换后的合成定义".into(),
            expected_version: Some(1),
            adjudication_note: "经人工核验范围变化".into(),
            source_boundary: "合成测试作品".into(),
            members: vec![
                TopicMaterialMemberImport {
                    work_public_ref: works[1],
                    role: TopicMaterialRole::Support,
                    rationale: "支持".into(),
                },
                TopicMaterialMemberImport {
                    work_public_ref: works[2],
                    role: TopicMaterialRole::Boundary,
                    rationale: "边界".into(),
                },
            ],
        },
    )
    .await
    .unwrap();
    assert!(
        directions::latest(&db, domain()).await.unwrap()["runs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["candidates"] == json!([]) && r["reason"] == "direction_source_changed")
    );
    endpoint.finish().await;
}
