//! Disposable PostgreSQL + synthetic adapter proof. No model or platform calls.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
    topic_map_research::{ResearchCommand, apply_research_command, read_research_progress},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
async fn setup(name: &str) -> (Database, Uuid, PiAdapter) {
    let db = fixture::proof_database(name).await;
    let conn = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision)VALUES($1,1)")
        .bind(conn)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref)VALUES($1,$2,1,'Synthetic','openai-completions','http://127.0.0.1:18080',true,$3)").bind(version).bind(conn).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)VALUES($1,$2,'synthetic-topic-map','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts)VALUES($1,$2,8192,1000,30,$3)").bind(config).bind(model).bind(if name=="topic_map_research_unsent"{1}else{3}).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result)VALUES($1,$2,$3,'probe','synthetic','succeeded',0,0,$4)").bind(Uuid::new_v4()).bind(version).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    let node = std::env::var_os("CREATOR_PROOF_NODE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let output = std::process::Command::new("which")
                .arg("node")
                .output()
                .expect("locate synthetic proof Node executable");
            assert!(
                output.status.success(),
                "set CREATOR_PROOF_NODE to a local Node executable"
            );
            std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
        });
    assert!(node.is_file(), "synthetic proof Node executable must exist");
    let adapter = PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/topic_map_research_adapter.mjs"),
    );
    (db, config, adapter)
}
async fn work(db: &Database, id: &str, body: &str) -> Uuid {
    let package=fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"SYNTHETIC 合成练习","bodyText":body,"authorId":"synthetic-creator","likes":10}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)VALUES($1,$2,$3,'primary','legacy_domain_migration',$4) ON CONFLICT DO NOTHING").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}
fn configure(config: Uuid, daily: i64, auto: bool) -> ResearchCommand {
    ResearchCommand::Configure {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        model_config_ref: config,
        daily_token_limit: daily,
        run_token_limit: 100000,
        automatic_enabled: auto,
        collection_enabled: false,
    }
}
fn start(works: Vec<Uuid>) -> ResearchCommand {
    ResearchCommand::Start {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        trigger: "on_demand".into(),
        work_refs: works,
        topic_ref: None,
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn exact_input_pipeline_idempotency_budget_and_stop() {
    let (db, config, adapter) = setup("topic_map_research_pipeline").await;
    let w = work(
        &db,
        "research-one",
        "SYNTHETIC 家庭实践：每次只做一步，并记录反馈。",
    )
    .await;
    let command = configure(config, 100000, false);
    let a = apply_research_command(&db, &command).await.unwrap();
    assert_eq!(a, apply_research_command(&db, &command).await.unwrap());
    read_research_progress(&db, D).await.unwrap();
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let c = start(vec![w]);
    let r = apply_research_command(&db, &c).await.unwrap();
    assert_eq!(r, apply_research_command(&db, &c).await.unwrap());
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_work_annotation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap(),399);
    assert_eq!(
        apply_research_command(&db, &start(vec![w])).await.unwrap()["state"],
        "no_new_input"
    );
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    // A repeated observation and counters-only refresh are not substantive new research.
    work(
        &db,
        "research-one",
        "SYNTHETIC 家庭实践：每次只做一步，并记录反馈。",
    )
    .await;
    assert_eq!(
        apply_research_command(&db, &start(vec![w])).await.unwrap()["state"],
        "no_new_input"
    );
    let w2 = work(&db, "research-two", "SYNTHETIC 下班后安排一次练习。").await;
    apply_research_command(&db, &configure(config, 1024, false))
        .await
        .unwrap();
    let second = apply_research_command(&db, &start(vec![w2])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(second["runRef"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "daily_budget_paused"
    );
    let stop = ResearchCommand::Stop {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        run_ref: Some(second["runRef"].as_str().unwrap().parse().unwrap()),
    };
    apply_research_command(&db, &stop).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_run SET updated_at=scope_001_now()-interval '2 days'WHERE run_ref=$1").bind(second["runRef"].as_str().unwrap().parse::<Uuid>().unwrap()).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn rejects_foreign_citation_and_preserves_daily_pause_without_call() {
    let (db, config, adapter) = setup("topic_map_research_attack").await;
    let w = work(&db, "research-attack", "SYNTHETIC ATTACK 仅测试越界引用。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        399
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn atomic_daily_reservation_and_unknown_dispatch_do_not_resend() {
    let (db, config, adapter) = setup("topic_map_research_reservation").await;
    let a = work(&db, "slow-one", "SYNTHETIC SLOW 练习过程合成输入。").await;
    let b = work(&db, "later-two", "SYNTHETIC 下一个实践输入。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![a])).await.unwrap();
    let clone_db = db.clone();
    let clone_adapter = adapter.clone();
    let first = tokio::spawn(async move {
        run_once(&clone_db, &SyntheticModelSecrets, &clone_adapter)
            .await
            .unwrap()
    });
    let mut reserved = 0;
    for _ in 0..100 {
        let running:Option<i64>=sqlx::query_scalar("SELECT i.reserved_tokens FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)WHERE q.dispatch_started_at IS NOT NULL AND i.state='running'LIMIT 1").fetch_optional(db.pool()).await.unwrap();
        if let Some(n) = running {
            reserved = n;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(reserved > 0);
    apply_research_command(&db, &configure(config, reserved + 1, false))
        .await
        .unwrap();
    let r = apply_research_command(&db, &start(vec![b])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(r["runRef"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "daily_budget_paused"
    );
    assert!(first.await.unwrap());
    // A dispatched request whose process died is terminal unknown, retaining reserved charge.
    let c = work(&db, "unknown-three", "SYNTHETIC 未知发送状态输入。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let u = apply_research_command(&db, &start(vec![c])).await.unwrap();
    let run = u["runRef"].as_str().unwrap().parse::<Uuid>().unwrap();
    let task: Uuid =
        sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens)SELECT $1,m.connection_version_ref,c.model_ref,c.config_ref,'analyze',$3,'running',2048,2048 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE config_ref=$2").bind(invocation).bind(config).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,dispatch_started_at,deadline_at)VALUES($1,$2,$3,1,$4,'{}',(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()-interval '2 minutes',scope_001_now()-interval '1 minute')").bind(invocation).bind(task).bind(run).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=1,lease_token=$2 WHERE task_ref=$1").bind(task).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "unknown_dispatch"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(invocation)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2048
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_topic_map_research_request WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn comparison_and_comment_citations_are_visible_then_restriction_invalidates() {
    let (db, config, adapter) = setup("topic_map_research_comparison").await;
    let a = work(
        &db,
        "comparison-a",
        "SYNTHETIC COMPARE 家庭练习每次只做一步。",
    )
    .await;
    let b = work(
        &db,
        "comparison-b",
        "SYNTHETIC 外部作者讨论练习先观察反馈。",
    )
    .await;
    let comment = research_fixture::comment_with_author(
        &db,
        "comparison-b",
        "reader-q",
        "孩子每天练习都要催，不催就不开始。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let topic=linggan_intelligence::import_topic_workspace(&db,&serde_json::from_value(json!({"idempotencyKey":"synthetic-comparison-topic","domainKey":"adhd-family","canonicalKey":"synthetic-comparison","displayName":"合成实践主题","definitionText":"合成两侧实践边界","expectedVersion":null,"adjudicationNote":"合成测试","sourceBoundary":"SYNTHETIC","members":[{"workPublicRef":a,"role":"support","rationale":"合成支持"},{"workPublicRef":b,"role":"boundary","rationale":"合成边界"}]})).unwrap()).await.unwrap();
    linggan_intelligence::topic_map::save_topic_map_command(
        &db,
        &linggan_intelligence::topic_map::TopicMapCommand::BindTopic {
            idempotency_key: "synthetic-comparison-bind".into(),
            domain_ref: D,
            topic_ref: topic.topic_ref,
            parent_topic_ref: None,
            expected_version: None,
        },
    )
    .await
    .unwrap();
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![a, b]))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let query = linggan_intelligence::topic_map::TopicMapQuery {
        domain_ref: Some(D),
        reference_window_days: Some(0),
        ..Default::default()
    };
    let snapshot = linggan_intelligence::topic_map::read_topic_map(&db, &query)
        .await
        .unwrap();
    let research = snapshot
        .works
        .iter()
        .find(|w| w.work_ref == a)
        .unwrap()
        .research
        .as_ref()
        .expect("accepted two-work/comment analysis visible");
    assert_eq!(
        research["output"]["responseMatches"][0]["status"],
        "partial"
    );
    assert!(
        research["output"]["angles"][0]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["fragmentId"].as_str().unwrap().contains(".comment."))
    );
    assert_eq!(
        apply_research_command(&db, &start(vec![a, b]))
            .await
            .unwrap()["state"],
        "no_new_input"
    );
    let result: Uuid = research["resultRef"].as_str().unwrap().parse().unwrap();
    linggan_intelligence::topic_map::save_topic_map_command(
        &db,
        &linggan_intelligence::topic_map::TopicMapCommand::SaveAlternative {
            idempotency_key: "synthetic-comment-angle".into(),
            domain_ref: D,
            topic_ref: topic.topic_ref,
            definition_ref: topic.definition_ref,
            title: "怎样开始练习".into(),
            angle: "合成评论练习角度".into(),
            rationale: "合成引用测试".into(),
            evidence_work_refs: vec![a, b],
            method_version: "topic-map.research.v1.1".into(),
            research_result_ref: Some(result),
            research_angle_index: Some(0),
            research_opportunity_index: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        linggan_intelligence::topic_map::read_topic_map(&db, &query)
            .await
            .unwrap()
            .alternatives[0]["sourceState"],
        "available"
    );
    let product_command = linggan_intelligence::topic_map::TopicMapCommand::SaveAlternative {
        idempotency_key: "synthetic-product-research".into(),
        domain_ref: D,
        topic_ref: topic.topic_ref,
        definition_ref: topic.definition_ref,
        title: "修改后的合成产品研究说明".into(),
        angle: "分步支持的待验证假设".into(),
        rationale: "这是待验证说明，不是市场结论".into(),
        evidence_work_refs: vec![a, b],
        method_version: "topic-map.research.v1.1".into(),
        research_result_ref: Some(result),
        research_angle_index: None,
        research_opportunity_index: Some(0),
    };
    let product = linggan_intelligence::topic_map::save_topic_map_command(&db, &product_command)
        .await
        .unwrap();
    let original = linggan_intelligence::topic_map::read_saved_alternative(
        &db,
        D,
        product.subject_ref.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(original.kind, "product_research");
    assert_eq!(original.title.as_deref(), Some("修改后的合成产品研究说明"));
    assert_eq!(original.research_manifest["opportunityIndex"], 0);
    assert!(
        original
            .fragments
            .iter()
            .any(|f| f.field.ends_with("comment") && !f.cited_ranges.is_empty())
    );
    let mut invalid = product_command.clone();
    if let linggan_intelligence::topic_map::TopicMapCommand::SaveAlternative {
        idempotency_key,
        research_angle_index,
        ..
    } = &mut invalid
    {
        *idempotency_key = "synthetic-product-invalid-both".into();
        *research_angle_index = Some(0);
    }
    assert!(matches!(
        linggan_intelligence::topic_map::save_topic_map_command(&db, &invalid).await,
        Err(linggan_intelligence::topic_map::TopicMapError::Invalid(_))
    ));
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic restriction'FROM linggan_material_comment WHERE material_ref=$1").bind(comment).execute(db.pool()).await.unwrap();
    let snapshot = linggan_intelligence::topic_map::read_topic_map(&db, &query)
        .await
        .unwrap();
    assert!(
        snapshot
            .works
            .iter()
            .find(|w| w.work_ref == a)
            .unwrap()
            .research
            .is_none()
    );
    assert_eq!(
        snapshot.alternatives[0]["sourceState"],
        "source_unavailable"
    );
    let alternative: Uuid = snapshot.alternatives[0]["alternativeRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let original = linggan_intelligence::topic_map::read_saved_alternative(&db, D, alternative)
        .await
        .unwrap();
    assert_eq!(original.source_state, "source_unavailable");
    assert!(
        original.fragments.is_empty()
            && original.angle.is_none()
            && original.original_definition.is_none()
    );
    assert!(snapshot.alternatives[0]["angle"].is_null());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn unsent_recovery_respects_one_attempt_and_platform_own_identity() {
    let (db, config, adapter) = setup("topic_map_research_unsent").await;
    let w = work(&db, "unsent-one", "SYNTHETIC 待恢复练习。").await;
    // Same external ID on another platform is not our creator identity.
    linggan_intelligence::topic_map::save_topic_map_command(
        &db,
        &linggan_intelligence::topic_map::TopicMapCommand::OwnCreator {
            idempotency_key: Uuid::new_v4().to_string(),
            domain_ref: D,
            platform: "douyin".into(),
            author_external_id: "synthetic-creator".into(),
            active: true,
        },
    )
    .await
    .unwrap();
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![w])).await.unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    let row = sqlx::query(
        "SELECT task_ref,input_refs FROM linggan_topic_map_research_task WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    use sqlx::Row;
    let task: Uuid = row.get("task_ref");
    let refs: serde_json::Value = row.get("input_refs");
    assert_eq!(refs["roleMetadata"]["works"][0]["own"], false);
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens)SELECT $1,m.connection_version_ref,c.model_ref,c.config_ref,'analyze',$3,'running',2048,2048 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE config_ref=$2").bind(invocation).bind(config).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,deadline_at)VALUES($1,$2,$3,1,$4,'{}',(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()-interval '1 minute')").bind(invocation).bind(task).bind(run).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=1,lease_token=$2 WHERE task_ref=$1").bind(task).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "completed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(invocation)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_topic_map_research_request WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}
