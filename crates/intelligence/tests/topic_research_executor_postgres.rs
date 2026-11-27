//! One complete topic workflow, owned PostgreSQL and synthetic loopback calls only.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::ai_foundation::{
    control::BudgetPolicy,
    control_store::ControlScope,
    provider_store::MaterialClass,
    task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    topic_research as research,
};
use uuid::Uuid;
#[path = "support/topic_core_fixture.rs"]
mod core_fixture;
#[path = "support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "support/research_method_fixture.rs"]
mod method;
#[path = "support/late_ai_receipt.rs"]
mod late;

#[tokio::test]
#[ignore = "owned PostgreSQL and gated loopback only"]
async fn returned_topic_response_after_unknown_reuses_original_attempt_and_meter() {
    exercise("late_after_unknown").await;
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_publishes_one_analysis_and_assignment_per_window() {
    exercise("normal").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_invalid_response_writes_no_result_or_assignment() {
    exercise("invalid_output").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_no_signal_publishes_analysis_without_assignment() {
    exercise("no_signal").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_invalid_ancestor_writes_no_result_or_assignment() {
    exercise("invalid_ancestor").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn returned_topic_response_survives_blocked_publication_and_executor_restart() {
    exercise("recover_publication").await;
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn returned_topic_response_can_finish_after_its_original_lease_interval() {
    exercise("slow_publication").await;
}

#[tokio::test]
#[ignore = "isolated postgres and loopback provider"]
async fn partial_window_exposes_one_assignment_and_one_rejection() {
    exercise("partial_assignment").await;
}

async fn exercise(variant: &'static str) {
    let invalid = matches!(variant, "invalid_output" | "invalid_ancestor");
    let db = dossier::fixture::proof_database("topic_execution").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading_with_bytes(&db, Some(core_fixture::bytes())).await;
    let late = variant == "late_after_unknown";
    let mut endpoint = wire::Endpoint::variant(late, variant).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-topic-probe",
        4,
    )
    .await
    .unwrap();
    let mut configured =
        setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    let l1_config = l1_setup::configure_high(&db, &configured).await;
    configured.capability = sqlx::query_scalar("SELECT m.capability_ref FROM linggan_ai_task_config t JOIN linggan_model_config m ON m.config_ref=t.model_config_ref WHERE t.task_config_ref=$1").bind(l1_config).fetch_one(db.pool()).await.unwrap();
    for control in [ControlScope::Global, ControlScope::Executor] {
        setup::enable(&db, control).await;
    }
    core_fixture::prepare(&db, work, &owner, core_fixture::CoreStorage::Direct).await;
    // The second work proves that an accepted topic is matched, not duplicated.
    let second = if variant == "normal" {
        let package = dossier::fixture::submit_package(&db,"content_detail",serde_json::json!({"contentExternalId":"second-topic-note"}),serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"second-topic-note"},"payload":{"title":"另一合格作品","bodyText":"另一作者的正文。结尾不能丢失🙂","authorId":"another-author"}})).await;
        let second:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='second-topic-note'").fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(second).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        core_fixture::image(&db, "second-topic-note").await;
        core_fixture::prepare(&db, second, &owner, core_fixture::CoreStorage::Recovered).await;
        Some(second)
    } else {
        None
    };
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in research::CURRENT_TASKS {
        let revision: i64 =
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
                    model_ref: configured.model,
                    capability_ref: configured.capability,
                    limits: linggan_intelligence::ai_foundation::capability::ModelLimits {
                        input_tokens: 24576,
                        ..configured.limits.clone()
                    },
                }),
                skill_version_ref: None,
                material_columns: Default::default(),
                budget: BudgetPolicy::Unlimited,
            },
        )
        .await
        .unwrap();
        setup::enable(&db, ControlScope::Task(task)).await;
        setup::allow(&db, configured.endpoint, task, MaterialClass::NoteMaterial).await;
        setup::allow(
            &db,
            configured.endpoint,
            task,
            MaterialClass::CommentMaterial,
        )
        .await;
    }
    for control in [ControlScope::Global, ControlScope::Executor] {
        setup::enable(&db, control).await;
    }
    let mut versions = Vec::new();
    for task in research::CURRENT_TASKS {
        versions.push(method::assign(&db, &adapter, task, "V1", None).await);
    }
    let unavailable = Uuid::new_v4();
    let mixed_selection = variant == "normal";
    let request = research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: if mixed_selection { vec![work, unavailable] } else { vec![work] },
    };
    let automatic_retry = variant == "invalid_output";
    use linggan_intelligence::ai_foundation::topic_research_automation as automation;
    let (run_ref, stage_count) = if automatic_retry {
        let settings = automation::read(&db, dossier::DOMAIN).await.unwrap();
        automation::change(&db, &automation::Change {
            command_ref: Uuid::new_v4(), domain_ref: dossier::DOMAIN,
            expected_revision: settings.revision, enabled: true,
        }).await.unwrap();
        assert_eq!(automation::tick(&db, &mut None).await.unwrap(), 1);
        (sqlx::query_scalar("SELECT run_ref FROM linggan_topic_research_run").fetch_one(db.pool()).await.unwrap(), 1)
    } else {
        let receipt = research::start(&db, &request).await.unwrap();
        assert_eq!(research::start(&db, &request).await.unwrap().run_ref, receipt.run_ref);
        if mixed_selection {
            let eligibility = research::read_eligibility(&db, dossier::DOMAIN, &[work, unavailable]).await.unwrap();
            assert!(eligibility[0].reason.is_none());
            assert_eq!(eligibility[1].reason.as_deref(), Some("ai_source_unavailable"));
            assert_eq!(receipt.skipped_works.len(), 1);
            assert_eq!(receipt.skipped_works[0].work_ref, unavailable);
        }
        (receipt.run_ref, receipt.stage_count)
    };
    assert_eq!(stage_count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    for (i, task) in research::CURRENT_TASKS.into_iter().enumerate() {
        method::assign(&db, &adapter, task, "V2", Some(versions[i])).await;
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let mut gate = if matches!(variant, "recover_publication" | "slow_publication") {
        let mut connection = db.pool().acquire().await.unwrap().detach();
        sqlx::raw_sql("CREATE FUNCTION hold_publication() RETURNS trigger LANGUAGE plpgsql AS $$
          BEGIN PERFORM pg_advisory_xact_lock(19198, 247); RETURN NEW; END $$;
          CREATE TRIGGER slow_publication BEFORE INSERT ON linggan_topic_research_stage_result
          FOR EACH ROW EXECUTE FUNCTION hold_publication();
          SELECT pg_advisory_lock(19198,247);").execute(&mut connection).await.unwrap();
        Some(connection)
    } else { None };
    let start_worker = || {
        let future = linggan_intelligence::ai_foundation::executor::service::run(
            db.clone(), "synthetic-topic-service", 4,
            std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
            adapter.clone(), drain.clone(),
        );
        tokio::spawn(async move {
            let outcome = future.await;
            if let Err(error) = &outcome { eprintln!("topic executor stopped: {error:?}"); }
            outcome
        })
    };
    if late { late::short_attempt_lease(&db).await; }
    let mut running = start_worker();
    if late {
        endpoint.seen.take().unwrap().await.unwrap();
        late::wait_unknown(&db).await;
        endpoint.release.take().unwrap().send(()).unwrap();
        late::wait_reconciled(&db).await;
    }
    if let Some(connection) = gate.as_mut() {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_returned_response) AND EXISTS(
                    SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=19198 AND objid=247 AND NOT granted)")
                    .fetch_one(&mut *connection).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            }
        }).await.expect("response committed before the held business projection");
        let restart = variant == "recover_publication";
        let held_seconds = if restart { 16 } else { 32 };
        tokio::time::sleep(std::time::Duration::from_secs(held_seconds)).await;
        assert!(!running.is_finished(), "business projection outlives the transport receipt deadline");
        if restart {
            running.abort();
            assert!(running.await.unwrap_err().is_cancelled());
            sqlx::query("SELECT pg_advisory_unlock(19198,247)").execute(&mut *connection).await.unwrap();
            running = start_worker();
        } else {
            sqlx::query("SELECT pg_advisory_unlock(19198,247)").execute(&mut *connection).await.unwrap();
        }
    }
    let run = wait_for(&db, run_ref, &running).await;
    assert_eq!(run["executionMode"], "single_pass");
    assert_eq!(run["windowCount"], 1);
    assert_eq!(
        run["succeededStageCount"],
        if invalid { 0 } else { 1 }
    );
    assert_eq!(run["failedStageCount"], if invalid { 1 } else { 0 });
    if variant == "invalid_ancestor" {
        let reason: Option<String> = sqlx::query_scalar(
            "SELECT result->>'validationReason' FROM linggan_model_invocation WHERE task_key LIKE 'topic_research%' ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(db.pool())
        .await
        .unwrap()
        .flatten();
        assert_eq!(reason.as_deref(), Some("topic_parent_criteria_repeated"));
    }
    assert_eq!(
        run["stages"].as_array().unwrap().iter().map(|s| s["stageNumber"].as_i64().unwrap()).collect::<Vec<_>>(),
        vec![0]
    );
    let results: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let assignments: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_unit_assignment")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(results, if invalid { 0 } else { 1 });
    if variant == "partial_assignment" { assert_eq!(run["stages"][0]["rejectedUnitCount"], 1); }
    let has_assignment = matches!(variant, "partial_assignment" | "normal" | "recover_publication" | "slow_publication" | "late_after_unknown");
    assert_eq!(assignments, i64::from(has_assignment));
    let query = serde_json::from_value(serde_json::json!({"domainRef":dossier::DOMAIN})).unwrap();
    let graph = linggan_intelligence::topic_map::read_topic_map(&db, &query)
        .await
        .unwrap();
    assert_eq!(!graph.topics.is_empty(), has_assignment);
    if automatic_retry {
        let retry_ref = tokio::time::timeout(std::time::Duration::from_secs(85), async {
            loop {
                let retry: Option<Uuid> = sqlx::query_scalar("SELECT run_ref FROM linggan_topic_research_run WHERE run_ref<>$1")
                    .bind(run_ref).fetch_optional(db.pool()).await.unwrap();
                if let Some(retry) = retry {
                    break retry;
                }
                assert!(!running.is_finished(), "AI worker stopped before the retry cycle");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }).await.expect("the worker should create a retry after the backoff");
        let retry = wait_for(&db, retry_ref, &running).await;
        assert_eq!(retry["failedStageCount"], 1);
        tokio::time::sleep(std::time::Duration::from_secs(66)).await;
        let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE task_key='topic_research'")
            .fetch_one(db.pool()).await.unwrap();
        assert_eq!(invocations, 2, "the invalid output permits a single auditable retry");
    }
    if let Some(second) = second {
        // A recovered-only L1 must pass the automatic scan, source qualification
        // and frozen-window restore using its original analysis identity.
        use linggan_intelligence::ai_foundation::topic_research_automation as automation;
        let settings = automation::read(&db, dossier::DOMAIN).await.unwrap();
        automation::change(&db, &automation::Change {
            domain_ref: dossier::DOMAIN,
            enabled: true,
            expected_revision: settings.revision,
            command_ref: Uuid::new_v4(),
        }).await.unwrap();
        let mut cursor = None;
        automation::tick(&db, &mut cursor).await.unwrap();
        let (second_run, core_ref): (Uuid, String) = sqlx::query_as(
            "SELECT run_ref,input_refs#>>'{coverage,coreRef}' FROM linggan_topic_research_window WHERE work_ref=$1"
        ).bind(second).fetch_one(db.pool()).await.unwrap();
        let recovered: Uuid = sqlx::query_scalar(
            "SELECT request_ref FROM linggan_content_understanding_offline_recovery WHERE work_ref=$1"
        ).bind(second).fetch_one(db.pool()).await.unwrap();
        assert_eq!(core_ref, recovered.to_string());
        assert_eq!(automation::tick(&db, &mut cursor).await.unwrap(), 0);
        let run = wait_for(&db, second_run, &running).await;
        assert_eq!(run["succeededStageCount"], 1);
        assert_eq!(run["failedStageCount"], 0);
        let topics: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_topic_workspace w JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=w.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE b.domain_ref=$1",
        )
        .bind(dossier::DOMAIN)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(topics, 1, "a second work must match the accepted topic");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM linggan_topic_map_unit_assignment"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            2
        );
    }
    if matches!(variant, "recover_publication" | "slow_publication") {
        let charges: Vec<i64> = sqlx::query_scalar("SELECT charged_tokens FROM linggan_model_invocation WHERE task_key='topic_research'")
            .fetch_all(db.pool()).await.unwrap();
        assert_eq!(charges, vec![26], "one returned provider usage record across local persistence retries");
    }
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), if variant == "normal" || automatic_retry { 3 } else { 2 });
    // The first run is pinned to V1 even after the setting advances; the later run starts on V2.
    for request in &requests[1..2] {
        let system = wire::text(request, "system");
        assert!(
            system.contains("METHOD V1") && system.contains("SKILL V1"),
            "{system}"
        );
        assert!(!system.contains("METHOD V2") && !system.contains("SKILL V2"));
    }
    if variant == "normal" || automatic_retry {
        let system = wire::text(&requests[2], "system");
        assert!(
            system.contains("METHOD V2") && system.contains("SKILL V2"),
            "{system}"
        );
        assert!(!system.contains("METHOD V1") && !system.contains("SKILL V1"));
    }
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}

async fn wait_for(
    db: &linggan_storage_postgres::Database,
    run_ref: Uuid,
    running: &tokio::task::JoinHandle<
        Result<linggan_intelligence::ai_foundation::executor::service::DrainReceipt, linggan_intelligence::ai_foundation::executor::ExecutorError>,
    >,
) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(25), async {
        loop {
            let progress = research::read(db, dossier::DOMAIN).await.unwrap();
            let run = progress["runs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|run| run["runRef"] == run_ref.to_string())
                .unwrap()
                .clone();
            if run["pendingStageCount"] == 0 {
                return run;
            }
            assert!(!running.is_finished(), "executor stopped");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[test]
#[ignore = "synthetic endpoint contract fixture only"]
fn synthetic_topic_response_matches_comment_and_reassignment_units() {
    let prompt = |mode: &str, definitions: serde_json::Value, units: serde_json::Value| {
        serde_json::json!({"packet":{"contract":"topic-map.single-pass.v1","input":{
            "mode":mode,"definitions":definitions,"units":units}}})
        .to_string()
    };
    let candidate = serde_json::json!([{"topicRef":"topic-1","definitionRef":"definition-1"}]);
    let units = serde_json::json!([
        {"discussionIndex":3,"discussion":{"evidence":[{"fragmentId":"fragment-1"}]}},
        {"discussionIndex":7,"discussion":{"evidence":[]}}
    ]);
    for mode in ["comment", "reassignment"] {
        let output =
            wire::topic_response(&prompt(mode, candidate.clone(), units.clone())).unwrap();
        assert_eq!(output["contract"], "topic-map.single-pass.v1");
        assert_eq!(output["projection"], serde_json::Value::Null);
        assert_eq!(output["newTopics"], serde_json::json!([]));
        let decisions = output["decisions"].as_array().unwrap();
        assert_eq!(decisions.len(), 2, "{mode}");
        assert_eq!(
            (
                decisions[0]["discussionIndex"].as_i64(),
                decisions[0]["status"].as_str()
            ),
            (Some(3), Some("matched")),
            "{mode}"
        );
        assert_eq!(decisions[0]["matches"][0]["topicRef"], "topic-1");
        assert_eq!(
            (
                decisions[1]["discussionIndex"].as_i64(),
                decisions[1]["status"].as_str()
            ),
            (Some(7), Some("uncertain")),
            "{mode}"
        );
        assert!(decisions
            .iter()
            .all(|decision| decision["newTopicId"].is_null()
                && decision["relations"] == serde_json::json!([])));
    }
    let unmatched =
        wire::topic_response(&prompt("comment", serde_json::json!([]), units)).unwrap();
    assert!(unmatched["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|decision| decision["status"] == "uncertain"
            && decision["matches"] == serde_json::json!([])));
    assert_eq!(
        wire::topic_response(&prompt("author", candidate, serde_json::json!([]))),
        None
    );
}

#[path = "support/topic_resolution_order.rs"]
mod resolution_order;
