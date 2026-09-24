//! P2 start proofs: disposable PostgreSQL and synthetic materials only. No model I/O.
#[path = "../../evidence/tests/support/material_fixture.rs"] mod fixture;
#[path = "support/comment_research_fixture.rs"] mod research_fixture;
use linggan_intelligence::{
    comment_study_catalog::refresh_clean_cache,
    comment_study_policy::{create_study_policy, CreateStudyPolicyCommand},
    comment_study_run::{start_study_run, preview_study_selection, StudyStartError, TrustedStudyOrigin},
    comment_study_selection::{StartStudyRunCommand, StudySelectionMode, study_domain_lock_key},
    comment_study_batch::{next_run_needing_batch, prepare_study_batch, PrepareStudyBatchRequest, StudyBatchError},
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use uuid::Uuid;
const GUARDS: &str = include_str!("../../../database/migrations/0105_comment_study_start_constraints.sql");
fn domain() -> Uuid { Uuid::parse_str(linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF).unwrap() }

async fn setup(name: &str, count: usize) -> (Database, StartStudyRunCommand, Uuid) {
    let db=fixture::proof_database(name).await;
    sqlx::raw_sql(include_str!("../../../database/bootstrap/comment-study-001.sql")).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql")).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(include_str!("../../../database/migrations/0104_comment_study_policy_constraints.sql")).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(GUARDS).execute(db.pool()).await.unwrap();
    let (connection,version,model,config)=(Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)")
        .bind(connection).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) \
        VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:18080',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) \
        VALUES($1,$2,8192,1024,30,1)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let policy:CreateStudyPolicyCommand=serde_json::from_value(json!({"domainRef":domain(),"methodName":"SYNTHETIC", "parentPolicyRef":null,
        "modelConfigRef":config,"defaults":{"commentBudget":1,"contextCharacterBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}})).unwrap();
    let policy=create_study_policy(&db,policy).await.unwrap();
    detail_with_author(&db,"selected","SYNTHETIC selected",Some("creator")).await;
    let work:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE domain_ref=$1 LIMIT 1")
        .bind(domain()).fetch_one(db.pool()).await.unwrap();
    for index in 0..count { add(&db,&format!("c{index:04}")).await; }
    for _ in 0..(count/128+1) { refresh_clean_cache(&db,domain(),128).await.unwrap(); }
    let command=serde_json::from_value(json!({"requestRef":Uuid::new_v4(),"domainRef":domain(),"policyRef":policy["policy"]["policyRef"],
        "scope":{"kind":"works","workRefs":[work]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null})).unwrap();
    (db,command,connection)
}
async fn add(db:&Database,id:&str)->Uuid {
    comment_with_author(db,"selected",id,"SYNTHETIC 用户评论",Some("reader"),"2026-09-21T08:00:00Z").await
}
async fn effects(db:&Database)->Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target),(SELECT count(*) FROM linggan_comment_study_start_request), \
        (SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
fn next(command:&StartStudyRunCommand)->StartStudyRunCommand { let mut c=command.clone();c.request_ref=Uuid::new_v4();c }

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn preview_and_two_runs_cover_different_comments_from_688() {
    let (db,c,_)=setup("start_688",688).await;
    let preview=preview_study_selection(&db,c.preview()).await.unwrap();
    assert_eq!(preview["scopeCommentCount"],688);assert_eq!(preview["targetCount"],100);
    assert_eq!(effects(&db).await,json!([0,0,0,0]));
    let first=start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await.unwrap();
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(first.run_ref).execute(db.pool()).await.unwrap();
    let second=start_study_run(&db,next(&c),TrustedStudyOrigin::Manual).await.unwrap();
    assert_eq!(second.target_count,100);assert_eq!(second.exclusion_counts["notSelectedByMode"],100);
    let overlap:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target a JOIN linggan_comment_study_target b \
        ON a.content_public_ref=b.content_public_ref AND a.comment_external_id=b.comment_external_id WHERE a.run_ref=$1 AND b.run_ref=$2")
        .bind(first.run_ref).bind(second.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(overlap,0);assert_eq!(effects(&db).await,json!([2,200,2,0]));
    let limits:Value=sqlx::query_scalar("SELECT jsonb_build_array(comment_budget,context_character_budget,token_limit) FROM linggan_comment_study_run WHERE run_ref=$1")
        .bind(first.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(limits,json!([100,6000,100000]),"no active/default budget substitution");
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_identical_request_replays_one_run_and_conflicting_command_fails() {
    let (db,c,_)=setup("start_replay",3).await;
    let (a,b)=tokio::join!(start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual),start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual));
    let (a,b)=(a.unwrap(),b.unwrap());assert_eq!(a.run_ref,b.run_ref);assert_ne!(a.idempotent_replay,b.idempotent_replay);
    assert_eq!(effects(&db).await,json!([1,3,1,0]));
    let mut changed=c;changed.limits.comment_budget=2;
    assert!(matches!(start_study_run(&db,changed,TrustedStudyOrigin::Manual).await,Err(StudyStartError::IdempotencyConflict)));
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_distinct_requests_never_reserve_the_same_comment() {
    let (db,c,_)=setup("start_race",150).await;
    let (a,b)=tokio::join!(start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual),start_study_run(&db,next(&c),TrustedStudyOrigin::Manual));
    let (a,b)=(a.unwrap(),b.unwrap());assert_eq!(a.target_count+b.target_count,150);assert_ne!(a.run_ref,b.run_ref);
    let duplicates:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT content_public_ref,comment_external_id FROM linggan_comment_study_target \
        GROUP BY 1,2 HAVING count(*)>1) d").fetch_one(db.pool()).await.unwrap();
    assert_eq!(duplicates,0);assert_eq!(effects(&db).await,json!([2,150,2,0]));
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn empty_and_index_pending_receipts_remain_terminal_after_material_changes() {
    let (db,c,connection)=setup("start_empty",0).await;
    let empty=start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await.unwrap();assert_eq!(empty.outcome,"no_work");
    add(&db,"late").await;
    let pending_command=next(&c);
    let pending=start_study_run(&db,pending_command.clone(),TrustedStudyOrigin::Manual).await.unwrap();assert_eq!(pending.outcome,"index_pending");
    refresh_clean_cache(&db,domain(),128).await.unwrap();
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1").bind(connection).execute(db.pool()).await.unwrap();
    assert_eq!(start_study_run(&db,c,TrustedStudyOrigin::Manual).await.unwrap().outcome,"no_work");
    assert_eq!(start_study_run(&db,pending_command,TrustedStudyOrigin::Manual).await.unwrap().outcome,"index_pending");
    assert_eq!(effects(&db).await,json!([0,0,2,0]));
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_parent_settles_without_model_and_reobservation_is_not_changed_input() {
    let (db,c,_)=setup("start_context",0).await;
    reply_with_author(&db,"selected","child","parent","我也是",Some("reader"),"2026-09-21T08:00:00Z").await;
    refresh_clean_cache(&db,domain(),128).await.unwrap();
    let first=start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await.unwrap();assert_eq!(first.needs_context_count,1);
    let state:String=sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1").bind(first.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state,"completed");
    let mut changed=next(&c);changed.mode=StudySelectionMode::InputChanged;changed.reason=Some("SYNTHETIC parent repair".into());
    assert_eq!(start_study_run(&db,changed.clone(),TrustedStudyOrigin::Manual).await.unwrap().outcome,"no_work");
    comment_with_author(&db,"selected","parent","SYNTHETIC 父语境",Some("creator"),"2026-09-21T08:00:00Z").await;
    refresh_clean_cache(&db,domain(),128).await.unwrap();
    let repaired=start_study_run(&db,next(&changed),TrustedStudyOrigin::Manual).await.unwrap();assert_eq!(repaired.queued_count,1);
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(repaired.run_ref).execute(db.pool()).await.unwrap();
    reply_with_author(&db,"selected","child","parent","我也是",Some("reader"),"2026-09-22T08:00:00Z").await;
    refresh_clean_cache(&db,domain(),128).await.unwrap();
    assert_eq!(start_study_run(&db,next(&changed),TrustedStudyOrigin::Manual).await.unwrap().outcome,"no_work");
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn failed_receipt_insert_rolls_back_run_targets_and_can_retry_the_same_request() {
    let (db,c,_)=setup("start_rollback",2).await;
    sqlx::raw_sql("CREATE FUNCTION cs_proof_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'SYNTHETIC'; END $$; \
        CREATE TRIGGER cs_proof_failure BEFORE INSERT ON linggan_comment_study_start_request FOR EACH ROW EXECUTE FUNCTION cs_proof_fail();")
        .execute(db.pool()).await.unwrap();
    assert!(start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await.is_err());
    assert_eq!(effects(&db).await,json!([0,0,0,0]));
    sqlx::raw_sql("DROP TRIGGER cs_proof_failure ON linggan_comment_study_start_request").execute(db.pool()).await.unwrap();
    assert_eq!(start_study_run(&db,c,TrustedStudyOrigin::Manual).await.unwrap().target_count,2);
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn new_run_is_frozen_receipt_is_immutable_and_legacy_dispatcher_cannot_consume_v2() {
    let (db,c,_)=setup("start_guards",2).await;
    let receipt=start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await.unwrap();let run=receipt.run_ref.unwrap();
    assert!(sqlx::query("UPDATE linggan_comment_study_run SET token_limit=200000 WHERE run_ref=$1").bind(run).execute(db.pool()).await.is_err());
    assert!(sqlx::query("DELETE FROM linggan_comment_study_start_request WHERE request_ref=$1").bind(c.request_ref).execute(db.pool()).await.is_err());
    assert!(sqlx::raw_sql("TRUNCATE linggan_comment_study_start_request").execute(db.pool()).await.is_err());
    assert_eq!(next_run_needing_batch(&db,&[]).await.unwrap(),None);
    assert!(matches!(prepare_study_batch(&db,PrepareStudyBatchRequest{run_ref:run,maximum_targets:12}).await,Err(StudyBatchError::RunUnavailable)));
    let v:Value=sqlx::query_scalar("SELECT execution_manifest FROM linggan_comment_study_run WHERE run_ref=$1").bind(run).fetch_one(db.pool()).await.unwrap();
    assert_eq!(v["engineRevision"].as_str().unwrap().len(),40);
    assert_eq!(effects(&db).await,json!([1,2,1,0]));
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn start_acquires_lock_before_choosing_and_sees_the_post_wait_committed_material() {
    let (db,c,_)=setup("start_wait_snapshot",0).await;
    let mut holder=db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(study_domain_lock_key(domain()).unwrap()).execute(&mut *holder).await.unwrap();
    let other=db.clone();let task=tokio::spawn(async move {start_study_run(&other,c,TrustedStudyOrigin::Manual).await});
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted)").fetch_one(db.pool()).await.unwrap();
            if waiting {break;} tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    add(&db,"after-wait").await;refresh_clean_cache(&db,domain(),128).await.unwrap();holder.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap().target_count,1);
}

#[tokio::test]
#[ignore="isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_scope_or_incomplete_guards_never_create_partial_work() {
    let (db,mut c,_)=setup("start_scope",1).await;
    c.scope=serde_json::from_value(json!({"kind":"works","workRefs":[Uuid::new_v4()]})).unwrap();
    assert!(matches!(start_study_run(&db,c.clone(),TrustedStudyOrigin::Manual).await,Err(StudyStartError::NotFound)));
    assert_eq!(effects(&db).await,json!([0,0,0,0]));
    sqlx::raw_sql("ALTER TABLE linggan_comment_study_target DISABLE TRIGGER cs_target_input_guard").execute(db.pool()).await.unwrap();
    assert!(matches!(start_study_run(&db,c,TrustedStudyOrigin::Manual).await,Err(StudyStartError::SchemaUnavailable)));
}
