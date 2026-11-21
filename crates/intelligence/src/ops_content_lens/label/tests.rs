use super::*;
use crate::{
    ai_foundation::{
        capability::ModelLimits,
        control::BudgetPolicy,
        control_store::ControlScope,
        executor::ExecutorSession,
        provider_store::MaterialClass,
        store::FoundationError,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    model_secrets::SyntheticModelSecrets,
    model_worker_drain::ModelWorkerDrain,
    ops_content_lens::{load_taxonomy_file, taxonomy::tests::synthetic_taxonomy},
    ops_ledger::{apply_ops_import, preview_ops_import},
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;
#[path = "../../../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod fixture;
#[path = "../../../tests/support/ops_export_fixture.rs"]
mod ops;
#[path = "../../../tests/support/ai_writing_fixture.rs"]
mod setup;
#[path = "../../../tests/support/ai_writing_endpoint.rs"]
mod wire;

#[test]
fn blind_payload_and_atomic_output_contract() {
    let taxonomy = synthetic_taxonomy();
    let titles = [
        Title {
            title_key: "one".into(),
            title: "标题".into(),
        },
        Title {
            title_key: "two".into(),
            title: "另一个标题".into(),
        },
    ];
    let input = payload(&taxonomy, &titles);
    assert_eq!(input.as_object().unwrap().len(), 2);
    assert_eq!(input["titles"][0], json!({"titleKey":"one","title":"标题"}));
    let row = |k: &str| json!({"titleKey":k,"moment":"other","situation":"other","offer":"other","audience":"other","confidence":"low"});
    let accept = |rows: Value| {
        accepted_labels(&json!({"labels":rows}).to_string(), &taxonomy, &titles).is_some()
    };
    assert!(accept(json!([row("two"), row("one")])));
    for bad in [
        json!([row("one")]),
        json!([row("one"), row("three")]),
        json!([row("one"), row("one")]),
    ] {
        assert!(!accept(bad));
    }
    let mut bad = row("two");
    bad["offer"] = json!("no_such_value");
    assert!(!accept(json!([row("one"), bad])));
}

async fn import(db: &Database, titles: &[&str]) {
    let mut export = ops::first_export();
    export.daily.clear();
    export.publications = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let date = "2026-09-03T05:00:00.000Z";
            let mut p = ops::publication(
                &format!("lens-{i}"),
                Some(("acc-1", "SYNTHETIC 账号甲")),
                "xhs",
                &ops::xhs_note(&format!("{:024x}", i + 1)),
                date,
                Some(987654),
            );
            p["title"] = json!(title);
            p
        })
        .collect();
    let request = export.request("2026-10-12T22:43:00+08:00");
    let preview = preview_ops_import(db, &request).await.unwrap();
    apply_ops_import(db, &request, &preview.preview_hash)
        .await
        .unwrap();
}

async fn taxonomy(db: &Database) {
    let mut body = synthetic_taxonomy();
    body["moment"][0]["example_ids"] = json!(["T0001"]);
    let dir = std::env::temp_dir().join(format!("ops-lens-example-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("taxonomy.json");
    std::fs::write(&path, body.to_string()).unwrap();
    std::fs::write(
        dir.join("titles.jsonl"),
        json!({"id":"T0001","title":"样例 A!"}).to_string(),
    )
    .unwrap();
    let receipt = load_taxonomy_file(db, &path).await.unwrap();
    let stored:bool=sqlx::query_scalar("SELECT body->'moment'->0->'example_ids'->>0=linggan_ops_lens_title_key('样例A') FROM linggan_ops_lens_definition WHERE definition_ref=$1")
        .bind(receipt.definition_ref).fetch_one(db.pool()).await.unwrap();
    assert!(stored);
    std::fs::remove_dir_all(dir).unwrap();
}

async fn wait_for(db: &Database, sql: &'static str, arg: Uuid) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if sqlx::query_scalar::<_, bool>(sql)
                .bind(arg)
                .fetch_one(db.pool())
                .await
                .unwrap()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("synthetic processing timed out");
}

#[tokio::test]
#[ignore = "owned PostgreSQL and local loopback provider"]
async fn synthetic_executor_runs_primary_audit_and_withholds_invalid_shard() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = isolated_proof_schema(&url, "ops_lens_label", fixture::FULL_MIGRATIONS)
        .await
        .unwrap();
    taxonomy(&db).await;
    // Two-title shards exercise multiple requests without changing the v1 seed.
    sqlx::raw_sql("WITH v AS (INSERT INTO linggan_ops_lens_definition(definition_ref,kind,version,body,created_by)
      SELECT gen_random_uuid(),'metric',2,body || '{\"label_batch_size\":2}'::jsonb,'synthetic' FROM linggan_ops_lens_definition WHERE kind='metric' AND version=1 RETURNING definition_ref)
      UPDATE linggan_ops_lens_definition_current SET definition_ref=v.definition_ref FROM v WHERE kind='metric'").execute(db.pool()).await.unwrap();
    let initial = ["样例 A!", "样例A", "合成乙", "合成丙"];
    import(&db, &initial).await;
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ExecutorSession::acquire(&db, "synthetic-label-configuration", 4)
        .await
        .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='ops_title_label'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TASK,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: ModelLimits {
                    input_tokens: 24576,
                    ..model.limits
                },
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    setup::enable(&db, ControlScope::Task(TASK)).await;
    let plan = plan(&db).await.unwrap();
    assert_eq!(
        (plan.total_titles, plan.pending_titles, plan.shards),
        (3, 3, 2)
    );
    assert!(matches!(
        start(&db, Uuid::new_v4()).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    setup::allow(&db, model.endpoint, TASK, MaterialClass::OwnPublishedTitle).await;
    let command = Uuid::new_v4();
    let (a, b) = tokio::join!(start(&db, command), start(&db, Uuid::new_v4()));
    let first = a.unwrap();
    let primary = first.run_ref.unwrap();
    assert_eq!(Some(primary), b.unwrap().run_ref);
    assert_eq!(Some(primary), start(&db, command).await.unwrap().run_ref);
    let drain = ModelWorkerDrain::new();
    let handle = tokio::spawn(crate::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-title-worker",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    wait_for(
        &db,
        "SELECT count(*)=4 FROM linggan_ops_lens_agreement WHERE primary_run_ref=$1",
        primary,
    )
    .await;
    let results:Vec<(i32,i32,f64,f64)>=sqlx::query_as("SELECT sample_count,excluded_example_count,agreement::float8,kappa::float8 FROM linggan_ops_lens_agreement WHERE primary_run_ref=$1")
        .bind(primary).fetch_all(db.pool()).await.unwrap();
    assert_eq!(results, vec![(2, 1, 1.0, 1.0); 4]);
    let labels:Vec<(String,i64)>=sqlx::query_as("SELECT r.run_kind,count(*) FROM linggan_ops_lens_label l JOIN linggan_ops_lens_label_run r USING(run_ref) GROUP BY r.run_kind ORDER BY 1")
        .fetch_all(db.pool()).await.unwrap();
    assert_eq!(labels, vec![("audit".into(), 3), ("primary".into(), 3)]);
    let snapshot = crate::ops_content_lens::build_snapshot(&db, Default::default())
        .await
        .unwrap();
    let frozen: Value = sqlx::query_scalar(
        "SELECT analysis_input FROM linggan_ops_lens_snapshot WHERE snapshot_ref=$1",
    )
    .bind(snapshot.snapshot_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(frozen["publications"].as_object().unwrap().len(), 4);
    let audit: Uuid = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_ops_lens_label_run WHERE primary_run_ref=$1",
    )
    .bind(primary)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        frozen["agreementSources"],
        json!({primary.to_string():audit})
    );
    let quality: Value = sqlx::query_scalar(
        "SELECT body->'quality'->'agreement' FROM linggan_ops_lens_snapshot WHERE snapshot_ref=$1",
    )
    .bind(snapshot.snapshot_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(quality.as_array().unwrap().len(), 4);
    assert!(
        quality
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["runRef"] == json!(primary) && a["auditRunRef"] == json!(audit))
    );
    let plan = super::plan(&db).await.unwrap();
    assert_eq!((plan.reused_titles, plan.pending_titles), (3, 0));
    let mut more = initial.to_vec();
    more.extend(["合成丁", "合成戊", "缺失行"]);
    import(&db, &more).await;
    let second = start(&db, Uuid::new_v4()).await.unwrap();
    assert_eq!(second.plan.shards, 2);
    let run = second.run_ref.unwrap();
    wait_for(&db,"SELECT count(*)=2 FROM linggan_ops_lens_label_run r JOIN linggan_ai_batch_item i ON i.batch_ref=r.foundation_batch_ref JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.run_ref=$1 AND c.state IN ('succeeded','failed')",run).await;
    let states:Vec<(String,i64)>=sqlx::query_as("SELECT c.state,count(*) FROM linggan_ops_lens_label_run r JOIN linggan_ai_batch_item i ON i.batch_ref=r.foundation_batch_ref JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.run_ref=$1 GROUP BY c.state ORDER BY c.state")
        .bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(states, vec![("failed".into(), 1), ("succeeded".into(), 1)]);
    let earlier = read_run(&db, primary).await.unwrap();
    assert_eq!(earlier.primary.succeeded_shards, 2);
    assert_eq!(earlier.audit.unwrap().succeeded_shards, 2);
    let partial = read_run(&db, run).await.unwrap();
    assert_eq!(
        (
            partial.primary.succeeded_shards,
            partial.primary.failed_shards
        ),
        (1, 1)
    );
    assert!(partial.audit.is_none());
    let invalid_labels:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_ops_lens_label l JOIN linggan_ai_batch_item_control c USING(invocation_ref) WHERE l.run_ref=$1 AND c.state='failed'")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    assert_eq!(invalid_labels, 0);
    drain.request();
    handle.await.unwrap().unwrap();
    let sent: Vec<Value> = endpoint
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|r| serde_json::from_str::<Value>(&wire::text(r, "user")).ok())
        .filter(|v| v.get("titles").is_some())
        .collect();
    assert_eq!(sent.len(), 6);
    for body in sent {
        let object = body.as_object().unwrap();
        assert_eq!(object.len(), 2);
        assert!(object.contains_key("taxonomy"));
        for title in body["titles"].as_array().unwrap() {
            assert_eq!(title.as_object().unwrap().len(), 2);
            assert!(title["titleKey"].is_string() && title["title"].is_string());
        }
    }
    endpoint.finish().await;
}
