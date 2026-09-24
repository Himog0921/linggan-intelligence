//! Disposable PostgreSQL + actual Axum router. No mock transaction or provider I/O.
use super::*;
use super::tests::{app, send};
use linggan_intelligence::comment_study_catalog::refresh_clean_cache;
use linggan_intelligence::comment_study_policy::{CreateStudyPolicyCommand, create_study_policy};
use linggan_storage_postgres::Database;
use serde_json::Value;
use std::sync::Arc;

#[allow(dead_code)]
#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;

struct Proof {
    db: Arc<Database>,
    application: Router,
    command: Value,
    connection: Uuid,
    config: Uuid,
    legacy: Uuid,
}

fn domain() -> Uuid { Uuid::parse_str(ADHD_DOMAIN_REF).unwrap() }

async fn policy(db: &Database, config: Uuid, name: &str) -> Uuid {
    let command: CreateStudyPolicyCommand = serde_json::from_value(json!({
        "domainRef":domain(),"methodName":name,"parentPolicyRef":null,"modelConfigRef":config,
        "defaults":{"commentBudget":1,"contextCharacterBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}})).unwrap();
    let saved = create_study_policy(db,command).await.unwrap();
    serde_json::from_value(saved["policy"]["policyRef"].clone()).unwrap()
}

async fn add_comment(db: &Database, id: &str) {
    fixture::submit_package(db,"comments",json!({"contentExternalId":"selected"}),json!({
        "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"selected"},
        "payload":{"commentId":id,"noteId":"selected","text":"SYNTHETIC 用户评论","authorId":"reader"}
    })).await;
}

async fn setup(name: &str, count: usize) -> Proof {
    let db = Arc::new(fixture::proof_database(name).await);
    sqlx::raw_sql(include_str!("../../../../database/bootstrap/comment-study-001.sql")).execute(db.pool()).await.unwrap();
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,'comment-study.v1',1,1)").bind(legacy).bind(domain()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_active_policy(singleton,policy_ref) VALUES(true,$1)")
        .bind(legacy).execute(db.pool()).await.unwrap();
    for migration in [include_str!("../../../../database/migrations/0103_comment_study_productization_schema.sql"),
        include_str!("../../../../database/migrations/0104_comment_study_policy_constraints.sql"),
        include_str!("../../../../database/migrations/0105_comment_study_start_constraints.sql")] {
        sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    }
    let (connection,version,model,config) = (Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)")
        .bind(connection).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) \
        VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:18080',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) \
        VALUES($1,$2,8192,1024,30,1)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let method = policy(&db,config,"SYNTHETIC A").await;
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"selected"}),json!({
        "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"selected"},
        "payload":{"noteId":"selected","title":"SYNTHETIC selected","bodyText":"SYNTHETIC 作品语境","authorId":"creator","authorName":"合成作者"}
    })).await;
    let work: Uuid = sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE domain_ref=$1 LIMIT 1")
        .bind(domain()).fetch_one(db.pool()).await.unwrap();
    for i in 0..count { add_comment(&db,&format!("c{i:04}")).await; }
    for _ in 0..(count/128+1) { refresh_clean_cache(&db,domain(),128).await.unwrap(); }
    let command = json!({"requestRef":Uuid::new_v4(),"domainRef":domain(),"policyRef":method,
        "scope":{"kind":"works","workRefs":[work]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null});
    Proof { application:app(LocalDatabaseState::Ready(db.clone())),db,command,connection,config,legacy }
}

async fn effects(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target),(SELECT count(*) FROM linggan_comment_study_start_request), \
        (SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
fn next(value: &Value) -> Value { let mut next=value.clone(); next["requestRef"]=json!(Uuid::new_v4()); next }
fn preview_command(value: &Value) -> Value {
    let command: StartStudyRunCommand = serde_json::from_value(value.clone()).unwrap(); json!(command.preview())
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_preview_start_replay_conflict_bind_the_explicit_method_and_limits() {
    let p=setup("http_start",3).await;
    let (status,preview)=send(p.application.clone(),"/api/local/comment-study/selection-preview",preview_command(&p.command)).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(preview["targetCount"],3);
    assert_eq!(effects(&p.db).await,json!([0,0,0,0]));
    let (a,b)=tokio::join!(send(p.application.clone(),"/api/local/comment-study/runs",p.command.clone()),
        send(p.application.clone(),"/api/local/comment-study/runs",p.command.clone()));
    assert!(matches!((a.0,b.0),(StatusCode::CREATED,StatusCode::OK)|(StatusCode::OK,StatusCode::CREATED)));
    assert_eq!(a.1["runRef"],b.1["runRef"]); assert_ne!(a.1["idempotentReplay"],b.1["idempotentReplay"]);
    assert_eq!(a.1["limits"],p.command["limits"]); assert_eq!(a.1["dispatchState"],"enabled");
    assert_eq!(effects(&p.db).await,json!([1,3,1,0]));
    let stored: Value=sqlx::query_scalar("SELECT jsonb_build_array(policy_ref,comment_budget,context_character_budget,token_limit) FROM linggan_comment_study_run")
        .fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(stored,json!([p.command["policyRef"],100,6000,100000]));
    for field in ["limits","policyRef"] {
        let mut changed=p.command.clone();
        changed[field]=if field=="limits" {json!({"commentBudget":1,"contextCharacterBudget":6000,"tokenLimit":100000})} else {json!(Uuid::new_v4())};
        let (status,body)=send(p.application.clone(),"/api/local/comment-study/runs",changed).await;
        assert_eq!(status,StatusCode::CONFLICT); assert_eq!(body["error"]["code"],"idempotency_conflict");
        assert_eq!(body["requestRef"],p.command["requestRef"]);
    }
    assert_eq!(effects(&p.db).await,json!([1,3,1,0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_distinct_concurrent_requests_do_not_reserve_a_comment_twice() {
    let mut p=setup("http_race",3).await; p.command["limits"]["commentBudget"]=json!(2);
    let (a,b)=tokio::join!(send(p.application.clone(),"/api/local/comment-study/runs",p.command.clone()),
        send(p.application.clone(),"/api/local/comment-study/runs",next(&p.command)));
    assert_eq!(a.0,StatusCode::CREATED); assert_eq!(b.0,StatusCode::CREATED);
    assert_eq!(a.1["targetCount"].as_u64().unwrap()+b.1["targetCount"].as_u64().unwrap(),3);
    let distinct: i64=sqlx::query_scalar("SELECT count(DISTINCT (content_public_ref,comment_external_id)) FROM linggan_comment_study_target")
        .fetch_one(p.db.pool()).await.unwrap(); assert_eq!(distinct,3);
    assert_eq!(effects(&p.db).await,json!([2,3,2,0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_empty_and_index_pending_are_terminal_receipts_not_errors_or_empty_runs() {
    let p=setup("http_empty",0).await;
    let (status,empty)=send(p.application.clone(),"/api/local/comment-study/runs",p.command.clone()).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(empty["outcome"],"no_work"); assert!(empty["runRef"].is_null());
    add_comment(&p.db,"late").await;
    let pending=next(&p.command);
    let (status,body)=send(p.application.clone(),"/api/local/comment-study/runs",pending.clone()).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(body["outcome"],"index_pending"); assert!(body["runRef"].is_null());
    refresh_clean_cache(&p.db,domain(),128).await.unwrap();
    for (command,outcome) in [(p.command.clone(),"no_work"),(pending,"index_pending")] {
        let (status,body)=send(p.application.clone(),"/api/local/comment-study/runs",command).await;
        assert_eq!(status,StatusCode::OK); assert_eq!(body["outcome"],outcome); assert_eq!(body["idempotentReplay"],true);
    }
    assert_eq!(effects(&p.db).await,json!([0,0,2,0]));
    let (status,body)=send(p.application.clone(),"/api/local/comment-study/runs",next(&p.command)).await;
    assert_eq!(status,StatusCode::CREATED); assert_eq!(body["targetCount"],1);
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_activation_cas_preserves_method_rows_and_frozen_runs() {
    let p=setup("http_activate",1).await;
    let second=policy(&p.db,p.config,"SYNTHETIC B").await;
    let (status,_)=send(p.application.clone(),"/api/local/comment-study/runs",p.command.clone()).await;
    assert_eq!(status,StatusCode::CREATED);
    let before: Value=sqlx::query_scalar("SELECT jsonb_build_object('policies',(SELECT jsonb_agg(to_jsonb(p) ORDER BY policy_ref) FROM linggan_comment_study_policy p), \
        'runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY run_ref) FROM linggan_comment_study_run r))").fetch_one(p.db.pool()).await.unwrap();
    let path=format!("/api/local/comment-study/policies/{second}/activate");
    let expected=json!({"expectedActivePolicyRef":p.legacy});
    let (status,body)=send(p.application.clone(),&path,expected.clone()).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(body["policyRef"],json!(second)); assert_eq!(body["isActive"],true);
    let (status,body)=send(p.application.clone(),&path,expected).await;
    assert_eq!(status,StatusCode::CONFLICT); assert_eq!(body["error"]["code"],"control_version_conflict");
    let after: Value=sqlx::query_scalar("SELECT jsonb_build_object('policies',(SELECT jsonb_agg(to_jsonb(p) ORDER BY policy_ref) FROM linggan_comment_study_policy p), \
        'runs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY run_ref) FROM linggan_comment_study_run r))").fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(before,after); assert_eq!(effects(&p.db).await,json!([1,1,1,0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_first_default_is_atomic_and_invalid_methods_never_change_it() {
    let p=setup("http_first_default",0).await;
    sqlx::query("DELETE FROM linggan_comment_study_active_policy").execute(p.db.pool()).await.unwrap();
    let second=policy(&p.db,p.config,"SYNTHETIC B").await;
    let path_a=format!("/api/local/comment-study/policies/{}/activate",p.command["policyRef"].as_str().unwrap());
    let path_b=format!("/api/local/comment-study/policies/{second}/activate");
    let (a,b)=tokio::join!(send(p.application.clone(),&path_a,json!({"expectedActivePolicyRef":null})),
        send(p.application.clone(),&path_b,json!({"expectedActivePolicyRef":null})));
    assert!(matches!((a.0,b.0),(StatusCode::OK,StatusCode::CONFLICT)|(StatusCode::CONFLICT,StatusCode::OK)));
    let current: Uuid=sqlx::query_scalar("SELECT policy_ref FROM linggan_comment_study_active_policy").fetch_one(p.db.pool()).await.unwrap();
    for (reference,code,status) in [(p.legacy,"policy_unrecorded",StatusCode::CONFLICT),(Uuid::new_v4(),"resource_not_found",StatusCode::NOT_FOUND)] {
        let path=format!("/api/local/comment-study/policies/{reference}/activate");
        let (actual,body)=send(p.application.clone(),&path,json!({"expectedActivePolicyRef":current})).await;
        assert_eq!(actual,status); assert_eq!(body["error"]["code"],code);
    }
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1").bind(p.connection).execute(p.db.pool()).await.unwrap();
    let (status,body)=send(p.application.clone(),&path_b,json!({"expectedActivePolicyRef":current})).await;
    assert_eq!(status,StatusCode::CONFLICT); assert_eq!(body["error"]["code"],"policy_unavailable");
    let after: Uuid=sqlx::query_scalar("SELECT policy_ref FROM linggan_comment_study_active_policy").fetch_one(p.db.pool()).await.unwrap();
    assert_eq!(after,current); assert_eq!(effects(&p.db).await,json!([0,0,0,0]));
}

#[tokio::test]
#[ignore = "isolated command HTTP PostgreSQL proof; no shared database or model"]
async fn http_missing_scope_and_partial_schema_fail_without_partial_writes() {
    let p=setup("http_partial",1).await;
    let mut missing=p.command.clone(); missing["scope"]["workRefs"]=json!([Uuid::new_v4()]);
    let (status,body)=send(p.application.clone(),"/api/local/comment-study/runs",missing).await;
    assert_eq!(status,StatusCode::NOT_FOUND); assert_eq!(body["error"]["code"],"resource_not_found");
    sqlx::query("ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_insert_guard").execute(p.db.pool()).await.unwrap();
    for (path,command) in [("/api/local/comment-study/runs",p.command.clone()),
        ("/api/local/comment-study/selection-preview",preview_command(&p.command))] {
        let (status,body)=send(p.application.clone(),path,command).await;
        assert_eq!(status,StatusCode::SERVICE_UNAVAILABLE); assert_eq!(body["error"]["code"],"study_schema_unavailable");
    }
    sqlx::query("ALTER TABLE linggan_comment_study_policy DISABLE TRIGGER cs_policy_immutable").execute(p.db.pool()).await.unwrap();
    let path=format!("/api/local/comment-study/policies/{}/activate",p.command["policyRef"].as_str().unwrap());
    let (status,body)=send(p.application.clone(),&path,json!({"expectedActivePolicyRef":p.legacy})).await;
    assert_eq!(status,StatusCode::SERVICE_UNAVAILABLE); assert_eq!(body["error"]["code"],"study_schema_unavailable");
    assert_eq!(effects(&p.db).await,json!([0,0,0,0]));
}
