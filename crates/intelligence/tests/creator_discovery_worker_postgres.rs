#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use linggan_evidence::creator_discovery::load;
use linggan_intelligence::{
    creator_discovery_worker::{run_once, CreatorDiscoverySchedule}, model_secrets::SyntheticModelSecrets, pi_adapter::PiAdapter,
};
use serde_json::json;
use uuid::Uuid;
const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn worker_admission_budget_fingerprints_and_bounded_recovery() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_worker").await;
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic','openai-completions','http://127.0.0.1:18080',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-creator','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,1000,30,3)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let probe = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,'probe','synthetic','succeeded',0,0,$4)").bind(probe).bind(version).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    let mut refs = Vec::new();
    for id in ["creator-one", "creator-two"] {
        let package=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"实践","bodyText":"家庭实践过程。".repeat(300),"authorId":"worker-author","likes":10}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(work);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,config_ref,daily_token_limit) VALUES($1,'xhs',$2,100000)").bind(D).bind(config).execute(db.pool()).await.unwrap();
    let node = std::env::var_os("CREATOR_PROOF_NODE")
        .map(std::path::PathBuf::from)
        .expect("CREATOR_PROOF_NODE required");
    let adapter = PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/creator_discovery_adapter.mjs"),
    );
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE linggan_creator_discovery_policy SET analysis_enabled=true")
        .execute(db.pool())
        .await
        .unwrap();
    for _ in 0..5 {
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap();
    }
    let count=sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze' AND state='succeeded'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 4, "2 work analyses and 2 incremental author analyses");
    let q = serde_json::from_value(json!({"domain":D})).unwrap();
    let data = load(&db, &q).await.unwrap();
    assert!(data.works.iter().all(|w| w.relevance == "related"));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap(),532);
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    // Same semantic text with fresher metric/source records must not requeue.
    fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"creator-one"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"creator-one"},"payload":{"title":"实践","bodyText":"家庭实践过程。".repeat(300),"authorId":"worker-author","likes":9999}}),"2026-09-28T10:00:00Z").await;
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    assert_eq!(
        load(&db, &q)
            .await
            .unwrap()
            .works
            .iter()
            .find(|w| w.work_ref == refs[0])
            .unwrap()
            .likes,
        Some(9999)
    );
    // New text queues once. A saved manual correction survives every automatic attempt.
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET manual_overrides=$2 WHERE work_public_ref=$1").bind(refs[0]).bind(json!({"personal_experience":{"value":{"value":"no"},"actor":"fixture"}})).execute(db.pool()).await.unwrap();
    fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"creator-one"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"creator-one"},"payload":{"title":"新实践","bodyText":"SYNTHETIC_INVALID","authorId":"worker-author","likes":9999}}),"2026-09-29T10:00:00Z").await;
    for _ in 0..6 {
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE linggan_creator_discovery_work_analysis SET next_attempt_at=scope_001_now()",
        )
        .execute(db.pool())
        .await
        .unwrap();
    }
    let state:(String,i32,serde_json::Value)=sqlx::query_as("SELECT job_state,attempt_count,manual_overrides FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state.0, "failed");
    assert_eq!(state.1, 3);
    assert_eq!(state.2["personal_experience"]["value"]["value"], "no");
    // Qualification failure cannot dispatch; paused attempts do not exhaust retries.
    sqlx::query("UPDATE linggan_model_invocation SET result=$2 WHERE invocation_ref=$1")
        .bind(probe)
        .bind(json!({"ok":false}))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='queued',attempt_count=0 WHERE work_public_ref=$1").bind(refs[0]).execute(db.pool()).await.unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
        .await
        .unwrap();
    let paused:(String,i32)=sqlx::query_as("SELECT job_state,attempt_count FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(paused, ("paused".into(), 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        before
    );
    sqlx::query("UPDATE linggan_model_invocation SET result=$2 WHERE invocation_ref=$1")
        .bind(probe)
        .bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true}))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_creator_discovery_policy SET daily_token_limit=1024")
        .execute(db.pool())
        .await
        .unwrap();
    let expired_lease = Uuid::new_v4();
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='running',attempt_count=1,lease_token=$2,lease_expires_at=scope_001_now()-interval '1 minute',next_attempt_at=scope_001_now() WHERE work_public_ref=$1").bind(refs[0]).bind(expired_lease).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
        .await
        .unwrap();
    let code:String=sqlx::query_scalar("SELECT last_error_code FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(code, "model_budget_exhausted");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        before
    );
    // The expired dispatch consumed attempt 1; the budget pause consumed none.
    assert_eq!(sqlx::query_scalar::<_,i32>("SELECT attempt_count FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap(),1);
    // A 4000-character source must fit a smaller configured input budget through
    // bounded fragments and still produce a usable evidence-linked result.
    sqlx::query("UPDATE linggan_creator_discovery_policy SET daily_token_limit=100000")
        .execute(db.pool())
        .await
        .unwrap();
    let small_config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,2200,1000,30,3)")
        .bind(small_config).bind(model)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_creator_discovery_policy SET config_ref=$1")
        .bind(small_config)
        .execute(db.pool())
        .await
        .unwrap();
    fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"creator-one"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"creator-one"},"payload":{"title":"预算内实践","bodyText":"家庭实践过程。".repeat(900),"authorId":"worker-author","likes":9999}}),"2026-09-30T10:00:00Z").await;
    for _ in 0..8 {
        if !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule).await.unwrap() { break; }
    }
    let clipped:(String,serde_json::Value)=sqlx::query_as("SELECT job_state,result_json FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(clipped.0, "idle");
    assert_eq!(clipped.1["relevance"]["value"], "related");
    let focus_state:String=sqlx::query_scalar("SELECT job_state FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND author_external_id='worker-author'").bind(D).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        focus_state, "idle",
        "small input budget must settle focus with available fragments; diagnostic: {:?}", sqlx::query_as::<_,(Option<String>, i32)>("SELECT last_error_code,attempt_count FROM linggan_creator_discovery_author_analysis WHERE author_external_id='worker-author'").fetch_one(db.pool()).await.unwrap()
    );
    // A timed-out process leaves an invocation with unknown provider outcome. Recovery
    // closes its receipt, keeps the conservative charge, and never asserts no cost.
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='running',attempt_count=1,lease_token=$2,lease_expires_at=scope_001_now()-interval '1 minute' WHERE work_public_ref=$1").bind(refs[0]).bind(expired_lease).execute(db.pool()).await.unwrap();
    let orphan = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze','orphan','running',400,400,$5)").bind(orphan).bind(version).bind(model).bind(config).bind(json!({"purpose":"creator_discovery","domainRef":D,"jobKind":"work","jobKey":refs[0],"jobLease":expired_lease})).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
        .await
        .unwrap();
    let recovered:(String,i64,String)=sqlx::query_as("SELECT state,charged_tokens,failure_code FROM linggan_model_invocation WHERE invocation_ref=$1").bind(orphan).fetch_one(db.pool()).await.unwrap();
    assert_eq!(recovered, ("failed".into(), 400, "lease_expired".into()));
    let superseded = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,created_at) VALUES($1,$2,$3,$4,'analyze','superseded','running',500,500,$5,scope_001_now()-interval '6 minutes')").bind(superseded).bind(version).bind(model).bind(config).bind(json!({"purpose":"creator_discovery","domainRef":D,"jobKind":"work","jobKey":refs[0],"jobLease":Uuid::new_v4()})).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
        .await
        .unwrap();
    let swept:(String,i64,String)=sqlx::query_as("SELECT state,charged_tokens,failure_code FROM linggan_model_invocation WHERE invocation_ref=$1").bind(superseded).fetch_one(db.pool()).await.unwrap();
    assert_eq!(swept, ("failed".into(), 500, "lease_superseded".into()));
    sqlx::query("UPDATE linggan_creator_discovery_policy SET config_ref=$1")
        .bind(config)
        .execute(db.pool())
        .await
        .unwrap();
    // The adapter signals after the actual child process starts, then waits for
    // the test to replace the source. A successful old receipt must not publish.
    let barrier_id = Uuid::new_v4();
    // PiAdapter clears the child environment, so Node resolves os.tmpdir() to /tmp.
    let barrier_dir =
        std::path::PathBuf::from("/tmp").join(format!("creator-discovery-barrier-{barrier_id}"));
    std::fs::create_dir(&barrier_dir).unwrap();
    fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"creator-one"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"creator-one"},"payload":{"title":"并发实践","bodyText":format!("SYNTHETIC_BARRIER:{barrier_id} 家庭实践过程。"),"authorId":"worker-author","likes":9999}}),"2026-10-01T10:00:00Z").await;
    let current = load(&db, &q)
        .await
        .unwrap()
        .works
        .into_iter()
        .find(|work| work.work_ref == refs[0])
        .unwrap();
    assert!(
        current
            .fragments
            .iter()
            .any(|fragment| fragment.text.contains(&barrier_id.to_string()))
    );
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET requested_fingerprint=$2,job_state='queued',attempt_count=0,next_attempt_at=scope_001_now()-interval '1 hour',lease_token=NULL,lease_expires_at=NULL WHERE work_public_ref=$1").bind(refs[0]).bind(&current.fingerprint).execute(db.pool()).await.unwrap();
    let secrets = SyntheticModelSecrets;
    let mut barrier_schedule = CreatorDiscoverySchedule::default();
    let work_future = run_once(&db, &secrets, &adapter, &mut barrier_schedule);
    let replace_future = async {
        for _ in 0..500 {
            if barrier_dir.join("ready").is_file() {
                let invocation:Uuid=sqlx::query_scalar("SELECT invocation_ref FROM linggan_model_invocation WHERE state='running' AND result->>'purpose'='creator_discovery' AND result->>'jobKind'='work' AND result->>'jobKey'=$1::text ORDER BY created_at DESC LIMIT 1").bind(refs[0].to_string()).fetch_one(db.pool()).await.unwrap();
                let admitted:Option<String>=sqlx::query_scalar("SELECT result->>'dispatchAdmittedAt' FROM linggan_model_invocation WHERE invocation_ref=$1").bind(invocation).fetch_one(db.pool()).await.unwrap();
                assert!(
                    admitted.is_some(),
                    "adapter cannot start before durable dispatch admission"
                );
                // This must commit while the adapter waits: admission is finished,
                // so disabling blocks future calls without cancelling this one.
                sqlx::query("UPDATE linggan_creator_discovery_policy SET analysis_enabled=false WHERE domain_ref=$1").bind(D).execute(db.pool()).await.unwrap();
                let updated=sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET requested_fingerprint='superseded-by-new-source',job_state='queued',lease_token=NULL,lease_expires_at=NULL WHERE work_public_ref=$1 AND job_state='running'").bind(refs[0]).execute(db.pool()).await.unwrap();
                assert_eq!(updated.rows_affected(), 1);
                std::fs::write(barrier_dir.join("release"), "continue").unwrap();
                return invocation;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let job:(String,i32,Option<String>)=sqlx::query_as("SELECT job_state,attempt_count,last_error_code FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
        let invocation:Option<(String,Option<String>)>=sqlx::query_as("SELECT state,failure_code FROM linggan_model_invocation WHERE result->>'jobKey'=$1::text ORDER BY created_at DESC LIMIT 1").bind(refs[0].to_string()).fetch_optional(db.pool()).await.unwrap();
        panic!(
            "synthetic adapter never reached the ready barrier: job={job:?}, invocation={invocation:?}"
        );
    };
    let (completed, invocation) = tokio::join!(work_future, replace_future);
    assert!(completed.unwrap());
    std::fs::remove_dir_all(&barrier_dir).unwrap();
    let receipt:(String,Option<i64>,Option<i64>,i64,Option<String>)=sqlx::query_as("SELECT state,input_tokens,output_tokens,charged_tokens,result->>'dispatchAdmittedAt' FROM linggan_model_invocation WHERE invocation_ref=$1").bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(receipt.0, "succeeded");
    assert_eq!(
        (receipt.1, receipt.2, receipt.3),
        (Some(111), Some(22), 133)
    );
    assert!(receipt.4.is_some());
    let stale:(String,String,Option<String>)=sqlx::query_as("SELECT job_state,requested_fingerprint,result_fingerprint FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stale.0, "queued");
    assert_eq!(stale.1, "superseded-by-new-source");
    assert_ne!(stale.2.as_deref(), Some(stale.1.as_str()));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn author_analysis_progresses_with_work_backlog_and_one_call_per_visit() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_fairness").await;
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic','openai-completions','http://127.0.0.1:18080',true,$3)").bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-creator','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,1000,30,3)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let probe = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,'probe','synthetic','succeeded',0,0,$4)").bind(probe).bind(version).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    let mut refs = Vec::new();
    for id in ["fair-one", "fair-two", "fair-three", "fair-four", "fair-five", "fair-six"] {
        let package=fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"实践","bodyText":"家庭实践过程。".repeat(300),"authorId":"worker-author","likes":10}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(work);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,config_ref,daily_token_limit) VALUES($1,'xhs',$2,100000)").bind(D).bind(config).execute(db.pool()).await.unwrap();
    let node = std::env::var_os("CREATOR_PROOF_NODE")
        .map(std::path::PathBuf::from)
        .expect("CREATOR_PROOF_NODE required");
    let adapter = PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/creator_discovery_adapter.mjs"),
    );
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE linggan_creator_discovery_policy SET analysis_enabled=true")
        .execute(db.pool())
        .await
        .unwrap();
    for visit in 0..4 {
        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap();
        assert!(run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule).await.unwrap());
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap();
        assert_eq!(after - before, 1, "one model call per creator visit");
        if visit == 1 {
            let author_result: bool = sqlx::query_scalar("SELECT result_at IS NOT NULL FROM linggan_creator_discovery_author_analysis WHERE author_external_id='worker-author'").fetch_one(db.pool()).await.unwrap();
            assert!(author_result, "author must run before the six-note work queue drains");
        }
    }
    let completed: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creator_discovery_work_analysis WHERE job_state='idle'").fetch_one(db.pool()).await.unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creator_discovery_work_analysis WHERE job_state='queued'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(completed, 2, "work lane must continue after author progress");
    assert_eq!(pending, 4, "proof retains a real executable work backlog");
    let sample: serde_json::Value = sqlx::query_scalar("SELECT result_json FROM linggan_creator_discovery_author_analysis WHERE author_external_id='worker-author'").fetch_one(db.pool()).await.unwrap();
    assert!(sample["sampleWorkCount"].as_u64().is_some_and(|n| n>0 && n<=6), "actual input respects the byte budget");
    assert_eq!(sample["availableWorkCount"], 6);
}
