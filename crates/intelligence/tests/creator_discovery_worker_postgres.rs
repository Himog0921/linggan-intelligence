#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use linggan_evidence::creator_discovery::load;
use linggan_intelligence::{
    creator_discovery_worker::{CreatorDiscoverySchedule, run_once},
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
const D: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn worker_admission_budget_fingerprints_and_bounded_recovery() {
    // One schedule for the whole scenario, as the long-running worker holds one for its lifetime.
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
    // The author waits for both of its works and is then judged once, on their final results.
    let count=sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze' AND state='succeeded'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 3, "2 work analyses and 1 author analysis");
    let maps:Vec<serde_json::Value>=sqlx::query_scalar("SELECT result->'citationMap' FROM linggan_model_invocation WHERE result->>'jobKind'='work' AND state='succeeded'").fetch_all(db.pool()).await.unwrap();
    assert_eq!(maps.len(),2);
    for map in maps {
        assert_eq!(map[0]["citation"],"S1");
        assert!(map[0]["fragmentId"].as_str().unwrap().contains(".title."));
        assert!(map[0]["sourceRef"].as_str().is_some());
        assert!(map[0]["sourceVersion"].as_str().is_some());
        assert!(map[0].get("text").is_none(),"no duplicate original text in audit map");
    }
    let q = serde_json::from_value(json!({"domain":D})).unwrap();
    let data = load(&db, &q).await.unwrap();
    assert!(data.works.iter().all(|w| w.relevance == "related"));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap(),399);
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
    for _ in 0..3 {
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
    for _ in 0..3 {
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap();
    }
    let clipped:(String,serde_json::Value)=sqlx::query_as("SELECT job_state,result_json FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(clipped.0, "idle", "{:?}", sqlx::query_scalar::<_,Option<String>>("SELECT last_error_code FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1").bind(refs[0]).fetch_one(db.pool()).await.unwrap());
    assert_eq!(clipped.1["relevance"]["value"], "related");
    let focus_state:String=sqlx::query_scalar("SELECT job_state FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND author_external_id='worker-author'").bind(D).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        focus_state, "idle",
        "small input budget must settle focus with available fragments"
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
    // This segment proves work-lease supersession, so it starts from a neutral schedule rather
    // than whatever turn the scenario above left behind.
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

/// Synthetic model entry already qualified by a successful probe; the input budget is the only
/// knob the scenarios below need.
async fn model_config(db: &Database, input_token_limit: i32) -> Uuid {
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
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,$3,1000,30,3)").bind(config).bind(model).bind(input_token_limit).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,'probe','synthetic','succeeded',0,0,$4)").bind(Uuid::new_v4()).bind(version).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    config
}

async fn add_policy(db: &Database, config: Uuid, analysis_enabled: bool) {
    sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,config_ref,daily_token_limit,analysis_enabled) VALUES($1,'xhs',$2,100000,$3)").bind(D).bind(config).bind(analysis_enabled).execute(db.pool()).await.unwrap();
}

/// A detail-only work linked to the proof domain; `author` is its attributed author, if any.
async fn add_work_with_body(
    db: &Database,
    external_id: &str,
    author: Option<&str>,
    body: String,
) -> Uuid {
    let mut payload = json!({"title":"实践","bodyText":body,"likes":10});
    if let Some(author) = author {
        payload["authorId"] = json!(author);
    }
    let package = fixture::submit_package(
        db,
        "content_detail",
        json!({"contentExternalId":external_id}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external_id},"payload":payload}),
    )
    .await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}

async fn add_work(db: &Database, external_id: &str, author: Option<&str>) -> Uuid {
    add_work_with_body(db, external_id, author, "家庭实践过程。".repeat(300)).await
}

fn synthetic_adapter() -> PiAdapter {
    let node = std::env::var_os("CREATOR_PROOF_NODE")
        .map(std::path::PathBuf::from)
        .expect("CREATOR_PROOF_NODE required");
    PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/creator_discovery_adapter.mjs"),
    )
}

async fn analyze_calls(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

/// Every model call so far, oldest first, as (kind, key): ("work", work ref) or ("focus", author).
async fn call_log(db: &Database) -> Vec<(String, String)> {
    sqlx::query_as("SELECT result->>'jobKind',result->>'jobKey' FROM linggan_model_invocation WHERE operation='analyze' ORDER BY created_at").fetch_all(db.pool()).await.unwrap()
}

async fn author_judged(db: &Database, author: &str) -> bool {
    sqlx::query_scalar("SELECT result_at IS NOT NULL FROM linggan_creator_discovery_author_analysis WHERE author_external_id=$1").bind(author).fetch_one(db.pool()).await.unwrap()
}

/// Records the given works as analysed for their present input, so they are not due.
async fn settle_works(db: &Database, refs: &[Uuid]) {
    let scope = serde_json::from_value(json!({"domain":D})).unwrap();
    for work in load(db, &scope).await.unwrap().works {
        if refs.contains(&work.work_ref) {
            sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint,job_state) VALUES($1,$2,$3,'idle')").bind(D).bind(work.work_ref).bind(&work.fingerprint).execute(db.pool()).await.unwrap();
        }
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn an_author_is_judged_once_after_all_of_its_works_finish() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_wait_for_works").await;
    let config = model_config(&db, 8192).await;
    for id in [
        "wait-one",
        "wait-two",
        "wait-three",
        "wait-four",
        "wait-five",
        "wait-six",
    ] {
        add_work(&db, id, Some("wait-author")).await;
    }
    add_policy(&db, config, true).await;
    let adapter = synthetic_adapter();

    // Six works are queued. The author is offered the turn after every work but is not
    // claimable while any of them is unfinished, so each visit advances the work queue by
    // exactly one call and the author is never judged mid-backlog.
    let mut visits = 0;
    loop {
        let before = analyze_calls(&db).await;
        let worked = run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap();
        let made = analyze_calls(&db).await - before;
        if !worked {
            assert_eq!(made, 0, "an idle visit makes no call");
            break;
        }
        assert_eq!(made, 1, "one model call per creator visit");
        visits += 1;
        assert!(visits <= 20, "the queues did not drain");
    }
    let kinds: Vec<String> = call_log(&db)
        .await
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(
        kinds,
        ["work", "work", "work", "work", "work", "work", "focus"],
        "seven calls: the author is judged once, after its last work"
    );

    // The judgement matches the final work results and records how much of the author it used.
    let scope = serde_json::from_value(json!({"domain":D})).unwrap();
    let data = load(&db, &scope).await.unwrap();
    let analysis = &data.works[0].author_analysis;
    assert!(
        analysis["resultAt"].is_string(),
        "stored for the final input"
    );
    assert_eq!(analysis["automatic"]["availableWorkCount"], 6);
    assert!(
        analysis["automatic"]["sampleWorkCount"]
            .as_u64()
            .is_some_and(|n| n > 0 && n < 6),
        "the 8192-token budget clips the sixth work out of the model input: {analysis}"
    );
    // Supporting fragments keep the work title, so the page can group them by work.
    let support = analysis["supportFragments"].as_array().unwrap();
    assert!(!support.is_empty());
    assert!(support.iter().all(|f| f["workTitle"] == "实践"));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn an_author_is_not_held_back_by_another_authors_backlog() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_other_backlog").await;
    let config = model_config(&db, 8192).await;
    // Author a still has six works to go; author b's two works are already analysed.
    for id in [
        "backlog-one",
        "backlog-two",
        "backlog-three",
        "backlog-four",
        "backlog-five",
        "backlog-six",
    ] {
        add_work(&db, id, Some("author-a")).await;
    }
    let mut done = Vec::new();
    for id in ["done-one", "done-two"] {
        done.push(add_work(&db, id, Some("author-b")).await);
    }
    add_policy(&db, config, true).await;
    settle_works(&db, &done).await;
    let adapter = synthetic_adapter();

    // Visit one takes one of a's works; at visit two the author turn finds a mid-backlog and b ready.
    for _ in 0..2 {
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
                .await
                .unwrap()
        );
    }
    assert!(
        author_judged(&db, "author-b").await,
        "b is judged while a's backlog continues"
    );
    assert!(
        !author_judged(&db, "author-a").await,
        "a waits for its own works"
    );

    // a is judged once, right after its last work; b is not judged again.
    let mut visits = 2;
    while run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
        .await
        .unwrap()
    {
        visits += 1;
        assert!(visits <= 20, "the queues did not drain");
    }
    let log = call_log(&db).await;
    let kinds: Vec<&str> = log.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "work", "focus", "work", "work", "work", "work", "work", "focus"
        ]
    );
    let judged: Vec<&str> = log
        .iter()
        .filter(|(kind, _)| kind == "focus")
        .map(|(_, author)| author.as_str())
        .collect();
    assert_eq!(judged, ["author-b", "author-a"]);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn a_work_waiting_for_retry_blocks_its_author_until_its_attempts_are_used_up() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_retry_wait").await;
    let config = model_config(&db, 8192).await;
    let good = add_work(&db, "retry-good", Some("retry-author")).await;
    // The synthetic model cannot parse this work's input, so every attempt fails. The marker sits
    // past the 240 characters the author prompt keeps, so the author's own call is unaffected.
    let marker_body = format!("{}SYNTHETIC_INVALID", "家庭实践过程。".repeat(60));
    let bad = add_work_with_body(&db, "retry-bad", Some("retry-author"), marker_body).await;
    add_policy(&db, config, true).await;
    settle_works(&db, &[good]).await;
    let adapter = synthetic_adapter();
    let bad_work = || {
        sqlx::query_as::<_, (String, i32, bool)>("SELECT job_state,attempt_count,next_attempt_at>scope_001_now() FROM linggan_creator_discovery_work_analysis WHERE work_public_ref=$1")
            .bind(bad)
            .fetch_one(db.pool())
    };

    // Attempt one fails and the work waits a minute for its retry.
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    assert_eq!(bad_work().await.unwrap(), ("queued".to_string(), 1, true));
    // The author is due, but its work is waiting: nothing is claimable and the visit is idle.
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap(),
        "a work waiting for retry blocks its author"
    );
    assert!(!author_judged(&db, "retry-author").await);

    // Attempts two and three come due one after the other; the author stays blocked meanwhile.
    for attempts in [2, 3] {
        sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET next_attempt_at=scope_001_now() WHERE work_public_ref=$1").bind(bad).execute(db.pool()).await.unwrap();
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
                .await
                .unwrap()
        );
        assert_eq!(analyze_calls(&db).await, attempts);
        assert!(!author_judged(&db, "retry-author").await);
    }
    assert_eq!(bad_work().await.unwrap().0, "failed");

    // Three attempts are used, so the work counts as finished and the author is judged normally.
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap(),
        "a work that used all its attempts must not block its author"
    );
    assert!(author_judged(&db, "retry-author").await);
    let kinds: Vec<String> = call_log(&db)
        .await
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(kinds, ["work", "work", "work", "focus"]);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn a_work_in_progress_blocks_its_author_until_it_finishes() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_in_progress").await;
    let config = model_config(&db, 8192).await;
    let work = add_work(&db, "held-one", Some("held-author")).await;
    add_policy(&db, config, true).await;
    // Another process holds the work: a running row with a live lease.
    let scope = serde_json::from_value(json!({"domain":D})).unwrap();
    let fingerprint = load(&db, &scope).await.unwrap().works[0]
        .fingerprint
        .clone();
    sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint,job_state,attempt_count,lease_token,lease_expires_at) VALUES($1,$2,$3,'running',1,$4,scope_001_now()+interval '5 minutes')").bind(D).bind(work).bind(fingerprint).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    let adapter = synthetic_adapter();

    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap(),
        "the work is not due and its author must wait for it"
    );
    assert!(!author_judged(&db, "held-author").await);
    assert_eq!(analyze_calls(&db).await, 0);

    // The other process finishes; the author is now claimable.
    sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='idle',lease_token=NULL,lease_expires_at=NULL WHERE work_public_ref=$1").bind(work).execute(db.pool()).await.unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    assert!(author_judged(&db, "held-author").await);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn a_changed_author_input_is_not_held_back_by_the_old_retry_wait() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_requeue").await;
    let config = model_config(&db, 8192).await;
    let one = add_work(&db, "retry-one", Some("retry-author")).await;
    let two = add_work(&db, "retry-two", Some("retry-author")).await;
    add_policy(&db, config, true).await;
    settle_works(&db, &[one, two]).await;
    // The author failed twice on older input and is waiting an hour for its next attempt.
    sqlx::query("INSERT INTO linggan_creator_discovery_author_analysis(domain_ref,platform,author_external_id,requested_fingerprint,job_state,attempt_count,next_attempt_at,last_error_code,updated_at) VALUES($1,'xhs','retry-author','older-input','queued',2,scope_001_now()+interval '1 hour','creator_analysis_invalid_output',scope_001_now()-interval '2 days')").bind(D).execute(db.pool()).await.unwrap();
    let adapter = synthetic_adapter();

    // The works' present input differs from 'older-input': that is new input, so it runs now.
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap(),
        "new input must not inherit the old retry wait"
    );
    let author: (String, i32, bool, bool, bool, Option<String>) = sqlx::query_as("SELECT job_state,attempt_count,result_at IS NOT NULL,next_attempt_at<=scope_001_now(),updated_at>scope_001_now()-interval '1 minute',last_error_code FROM linggan_creator_discovery_author_analysis WHERE author_external_id='retry-author'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        author,
        ("idle".to_string(), 1, true, true, true, None),
        "retry count restarts, the wait and update time are renewed, and the old error is cleared"
    );
    assert_eq!(analyze_calls(&db).await, 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local process; no provider"]
async fn author_result_records_zero_sampled_works_when_no_input_fits_and_makes_no_call() {
    let mut schedule = CreatorDiscoverySchedule::default();
    let db = fixture::proof_database("creator_discovery_sampling").await;
    // 1024 is the smallest configurable input budget: the fixed instructions plus any fragment
    // already exceed it, so no author input can be sent.
    let config = model_config(&db, 1024).await;
    let one = add_work(&db, "narrow-one", Some("narrow-author")).await;
    let two = add_work(&db, "narrow-two", Some("narrow-author")).await;
    add_policy(&db, config, true).await;
    settle_works(&db, &[one, two]).await;
    let adapter = synthetic_adapter();

    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter, &mut schedule)
            .await
            .unwrap()
    );
    let author: (String, serde_json::Value, bool) = sqlx::query_as("SELECT job_state,result_json,result_invocation_ref IS NULL FROM linggan_creator_discovery_author_analysis WHERE author_external_id='narrow-author'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(author.0, "idle");
    assert_eq!(author.1["sampleWorkCount"], 0, "no work reached the model");
    assert_eq!(author.1["availableWorkCount"], 2, "both works existed");
    assert_eq!(author.1["focus"]["value"], "unknown");
    assert!(author.2, "no model call means no invocation receipt");
    assert_eq!(analyze_calls(&db).await, 0, "nothing was sent or charged");
}
