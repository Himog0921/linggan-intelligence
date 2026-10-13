//! Formal router + disposable PostgreSQL + fresh browser. No worker or platform is started.
use super::*;
#[allow(dead_code)]
#[path = "../../../../crates/evidence/tests/collection_single_run_postgres.rs"]
mod run_fixture;
use linggan_evidence::collection_run::{CommentMode, CreateRun, RunSource, create_run};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL and bundled Playwright"]
async fn collection_single_run_browser_with_persisted_data() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "single_run_formal_ui",
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let domain = uuid::Uuid::new_v4();
    let other = uuid::Uuid::new_v4();
    for (id, name) in [(domain, "合成家庭支持"), (other, "合成学习方法")] {
        sqlx::query("INSERT INTO observation_domain(domain_ref,name,description,status) VALUES($1,$2,'隔离合成数据，仅供页面验证','active')").bind(id).bind(name).execute(db.pool()).await.unwrap();
    }
    let (install, credential) = run_fixture::station(&db).await;
    let result = create_run(
        &db,
        &CreateRun {
            idempotency_key: uuid::Uuid::new_v4(),
            domain_ref: domain,
            source: RunSource::TemporaryKeyword {
                query: "合成有成果任务".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 5,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let first = Some(result["runRef"].clone());
    let search = run_fixture::next(&db, install, &credential).await;
    let records=(1..=5).map(|n|json!({"kind":"discovery_card","resultPosition":n,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("uifixture{n}")},"payload":{"noteId":format!("uifixture{n}"),"url":format!("https://www.xiaohongshu.com/explore/uifixture{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("合成作品 {n}")}})).collect();
    run_fixture::deliver(
        &db,
        install,
        &search,
        "discovery_search",
        records,
        json!({"basis":"maximum_quota","maximumQuota":5,"query":"合成有成果任务"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    for _ in 0..5 {
        run_fixture::deliver_work_segment(&db, install, &credential, 0, 30, false, true).await;
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
    }
    let selection = create_run(
        &db,
        &CreateRun {
            idempotency_key: uuid::Uuid::new_v4(),
            domain_ref: domain,
            source: RunSource::TemporaryKeyword {
                query: "合成跨页选择".into(),
            },
            ranking: Some("comprehensive".into()),
            work_limit: 51,
            comment_mode: CommentMode::Sample30,
        },
    )
    .await
    .unwrap();
    let selection_ref: uuid::Uuid = serde_json::from_value(selection["runRef"].clone()).unwrap();
    let search = run_fixture::next(&db, install, &credential).await;
    let records=(1..=51).map(|n|json!({"kind":"discovery_card","resultPosition":n,"sourceObject":{"platform":"xhs","type":"content","externalId":format!("uiselection{n}")},"payload":{"noteId":format!("uiselection{n}"),"url":format!("https://www.xiaohongshu.com/explore/uiselection{n}?xsec_token=SYNTHETIC_FIXTURE&xsec_source=pc_user"),"title":format!("跨页合成作品 {n}"),"authorName":"合成作者"}})).collect();
    run_fixture::deliver(
        &db,
        install,
        &search,
        "discovery_search",
        records,
        json!({"basis":"maximum_quota","maximumQuota":51,"query":"合成跨页选择"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    linggan_evidence::collection_run::command_run(
        &db,
        selection_ref,
        &linggan_evidence::collection_run::RunCommand {
            command_ref: uuid::Uuid::new_v4(),
            domain_ref: domain,
            expected_revision: 1,
            action: "stop".into(),
        },
    )
    .await
    .unwrap();
    for n in 0..25 {
        let receipt = create_run(
            &db,
            &CreateRun {
                idempotency_key: uuid::Uuid::new_v4(),
                domain_ref: domain,
                source: RunSource::TemporaryKeyword {
                    query: if n == 0 {
                        "超长合成任务标题用于检查截断与完整查看".repeat(3)
                    } else {
                        format!("合成采集 {n:02}")
                    },
                },
                ranking: Some("comprehensive".into()),
                work_limit: 50,
                comment_mode: if n % 2 == 0 {
                    CommentMode::AllPublic
                } else {
                    CommentMode::Sample30
                },
            },
        )
        .await
        .unwrap();
        let run: uuid::Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
        if n == 0 || n == 2 {
            linggan_evidence::collection_run::command_run(
                &db,
                run,
                &linggan_evidence::collection_run::RunCommand {
                    command_ref: uuid::Uuid::new_v4(),
                    domain_ref: domain,
                    expected_revision: 1,
                    action: if n == 0 { "pause" } else { "stop" }.into(),
                },
            )
            .await
            .unwrap();
        }
        if n == 1 {
            sqlx::query("UPDATE collection_work_order SET queue_state='cancelled' WHERE work_order_ref IN(SELECT work_order_ref FROM collection_run_order WHERE run_ref=$1)").bind(run).execute(db.pool()).await.unwrap();
            linggan_evidence::collection_run_progress::advance_runs(&db)
                .await
                .unwrap();
        }
    }
    for n in 0..25 {
        let target = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','creator',$2,$3,'manual')").bind(target).bind(format!("syntheticcreator{n}")).bind(format!("合成创作者 {n:02}")).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')").bind(domain).bind(target).execute(db.pool()).await.unwrap();
    }
    let keyword_target = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','keyword','合成关键词::most_liked','合成关键词','manual')").bind(keyword_target).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')",
    )
    .bind(domain)
    .bind(keyword_target)
    .execute(db.pool())
    .await
    .unwrap();
    for n in 0..2 {
        sqlx::query("INSERT INTO execution_station(station_ref,display_name,daily_work_quota) VALUES($1,$2,200)").bind(uuid::Uuid::new_v4()).bind(format!("合成工位 {n}")).execute(db.pool()).await.unwrap();
    }
    let unclaimed = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO plugin_installation(installation_ref,install_key,plugin_version,capabilities) VALUES($1,$2,'0.8.61','[]'::jsonb)").bind(unclaimed).bind(unclaimed.to_string()).execute(db.pool()).await.unwrap();
    let changed = linggan_evidence::report_account_eligibility(
        &db,
        install,
        &credential,
        linggan_evidence::AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "collection-ui-changed-synthetic-account",
        },
        Some(b"collection-single-run-fixture-digest-32-bytes"),
    )
    .await
    .unwrap();
    let active_station: uuid::Uuid =
        sqlx::query_scalar("SELECT station_ref FROM plugin_installation WHERE installation_ref=$1")
            .bind(install)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let domain_count: i64 = sqlx::query_scalar("SELECT count(*) FROM observation_domain")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let application = router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(db)),
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: Some(Arc::new(
            b"collection-single-run-fixture-digest-32-bytes".to_vec(),
        )),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = std::env::var("COLLECTION_PROOF_DIR").unwrap();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(std::path::Path::new(&directory).join("fixture.json"),json!({"origin":origin,"domainRef":domain,"otherDomainRef":other,"runRef":first,"selectionRunRef":selection_ref,"domainCount":domain_count,"activeStationRef":active_station,"changedAccountRef":changed.account_ref,"unclaimedRef":unclaimed}).to_string()).unwrap();
    let node = std::env::var("COLLECTION_PROOF_NODE").unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/test-collection-single-run-ui.cjs");
    let result = std::process::Command::new(node)
        .arg(script)
        .env("COLLECTION_PROOF_DIR", &directory)
        .status()
        .unwrap();
    server.abort();
    assert!(
        result.success(),
        "formal populated browser proof failed; see isolated evidence directory"
    );
}
