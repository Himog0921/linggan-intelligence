//! Policy-only P2 proofs against synthetic data in the disposable proof database.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;

use linggan_intelligence::comment_study_policy::{
    CreateStudyPolicyCommand, StudyPolicyQuery, StudyPolicyStoreError,
    create_study_policy, read_study_policies, read_study_policy,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

const BASE: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");
const DELTA: &str = include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql");
const GUARDS: &str = include_str!("../../../database/migrations/0104_comment_study_policy_constraints.sql");
fn domain() -> Uuid { Uuid::parse_str(linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF).unwrap() }
fn reference(value: &Value) -> Uuid { Uuid::parse_str(value["policy"]["policyRef"].as_str().unwrap()).unwrap() }
fn command(config: Uuid, parent: Option<Uuid>) -> CreateStudyPolicyCommand {
    serde_json::from_value(json!({"domainRef":domain(),"methodName":" 合成方法 ","modelConfigRef":config,
        "parentPolicyRef":parent,"defaults":{"commentBudget":100,"contextCharacterBudget":6000},
        "stageInstructions":{"semantic":"  合成说明\n","resolution":"","pair":""}})).unwrap()
}
async fn setup(name: &str, guards: bool) -> (Database, Uuid, Uuid, Uuid) {
    let db = fixture::proof_database(name).await;
    sqlx::raw_sql(BASE).execute(db.pool()).await.unwrap();
    let (connection, version, model, config, legacy) =
        (Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)")
        .bind(connection).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version \
        (version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) \
        VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:18080',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) \
        VALUES($1,$2,'synthetic-model','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config \
        (config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) \
        VALUES($1,$2,8192,1024,30,1)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_policy \
        (policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,$3,'comment-study.v1',50,4000)")
        .bind(legacy).bind(domain()).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_active_policy(singleton,policy_ref) VALUES(true,$1)")
        .bind(legacy).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(DELTA).execute(&mut *tx).await.unwrap();
    if guards { sqlx::raw_sql(GUARDS).execute(&mut *tx).await.unwrap(); }
    tx.commit().await.unwrap();
    (db,config,connection,legacy)
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn save_copy_and_read_preserve_parent_active_pointer_and_zero_model_work() {
    let (db,config,_,legacy) = setup("policy_save_copy",true).await;
    let first = create_study_policy(&db,command(config,None)).await.unwrap();
    let parent = reference(&first);
    let mut copy = command(config,Some(parent)); copy.method_name="合成复制".into();
    copy.stage_instructions.pair="新的补充说明".into();
    let second = create_study_policy(&db,copy).await.unwrap();
    assert_ne!(reference(&second),parent);
    assert_eq!(second["policy"]["parentPolicyRef"],json!(parent));
    assert_eq!(first, read_study_policy(&db,domain(),parent).await.unwrap());
    assert_eq!(first["policy"]["methodName"],"合成方法");
    assert_ne!(first["policy"]["methodHash"],second["policy"]["methodHash"]);
    assert_eq!(first["policy"]["methodManifest"]["stages"]["semantic"],second["policy"]["methodManifest"]["stages"]["semantic"]);
    assert_eq!(first["policy"]["model"]["timeoutSeconds"],30);
    let active: Uuid = sqlx::query_scalar("SELECT policy_ref FROM linggan_comment_study_active_policy")
        .fetch_one(db.pool()).await.unwrap(); assert_eq!(active,legacy);
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_model_invocation)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts,(0,0));
    let text=first.to_string();assert!(!text.contains("secret_ref"));assert!(!text.contains("18080"));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn historical_unrecorded_methods_remain_readable_but_cannot_be_copied() {
    let (db,config,_,legacy)=setup("policy_legacy",true).await;
    let value=read_study_policy(&db,domain(),legacy).await.unwrap();
    assert_eq!(value["policy"]["recordingState"],"legacy_unrecorded");
    assert!(value["policy"]["methodManifest"].is_null());
    assert!(matches!(create_study_policy(&db,command(config,Some(legacy))).await,Err(StudyPolicyStoreError::Unrecorded)));
    assert!(matches!(create_study_policy(&db,command(config,Some(Uuid::new_v4()))).await,Err(StudyPolicyStoreError::NotFound)));
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_policy").fetch_one(db.pool()).await.unwrap();
    assert_eq!(count,1);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn disabled_or_missing_model_rejects_save_without_erasing_saved_methods() {
    let (db,config,connection,_)=setup("policy_model",true).await;
    let saved=create_study_policy(&db,command(config,None)).await.unwrap();
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1")
        .bind(connection).execute(db.pool()).await.unwrap();
    assert!(matches!(create_study_policy(&db,command(config,None)).await,Err(StudyPolicyStoreError::ModelDisabled)));
    assert!(matches!(create_study_policy(&db,command(Uuid::new_v4(),None)).await,Err(StudyPolicyStoreError::ModelUnavailable)));
    let read=read_study_policy(&db,domain(),reference(&saved)).await.unwrap();
    assert_eq!(read["policy"]["methodManifest"],saved["policy"]["methodManifest"]);
    assert_eq!(read["policy"]["model"]["enabled"],false);
    assert!(read_study_policy(&db,Uuid::new_v4(),reference(&saved)).await.is_err());
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn policy_guards_block_mutation_incomplete_insert_and_untracked_reapplication() {
    let (db,config,_,legacy)=setup("policy_guards",true).await;
    let saved=create_study_policy(&db,command(config,None)).await.unwrap();
    for id in [legacy,reference(&saved)] {
        assert!(sqlx::query("UPDATE linggan_comment_study_policy SET comment_budget=1 WHERE policy_ref=$1")
            .bind(id).execute(db.pool()).await.is_err());
        assert!(sqlx::query("DELETE FROM linggan_comment_study_policy WHERE policy_ref=$1")
            .bind(id).execute(db.pool()).await.is_err());
    }
    assert!(sqlx::query("INSERT INTO linggan_comment_study_policy \
        (policy_ref,domain_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,'comment-study.v1',1,1)").bind(Uuid::new_v4()).bind(domain()).execute(db.pool()).await.is_err());
    let mut tx=db.pool().begin().await.unwrap();
    assert!(sqlx::raw_sql("TRUNCATE linggan_comment_study_policy CASCADE").execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    let mut tx=db.pool().begin().await.unwrap();
    assert!(sqlx::raw_sql(GUARDS).execute(&mut *tx).await.is_err());tx.rollback().await.unwrap();
    assert_eq!(saved,read_study_policy(&db,domain(),reference(&saved)).await.unwrap());
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn partial_schema_refuses_new_save_and_corrupted_hash_is_never_regenerated() {
    let (db,config,_,_)=setup("policy_partial",false).await;
    assert!(matches!(create_study_policy(&db,command(config,None)).await,Err(StudyPolicyStoreError::SchemaUnavailable)));
    sqlx::raw_sql(GUARDS).execute(db.pool()).await.unwrap();
    let saved=create_study_policy(&db,command(config,None)).await.unwrap();
    // Direct hostile INSERT simulates imported corruption, without weakening the immutable trigger.
    let id=Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_study_policy \
        (policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget,method_name,method_manifest,method_hash) \
        VALUES($1,$2,$3,'comment-study.v1',100,6000,'SYNTHETIC corrupt',$4,repeat('0',64))")
        .bind(id).bind(domain()).bind(config).bind(saved["policy"]["methodManifest"].clone())
        .execute(db.pool()).await.unwrap();
    assert!(matches!(read_study_policy(&db,domain(),id).await,Err(StudyPolicyStoreError::Contract(_))));
    assert!(matches!(create_study_policy(&db,command(config,Some(id))).await,Err(StudyPolicyStoreError::Contract(_))));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no real model or shared database"]
async fn policy_directory_pages_all_versions_with_ties_without_prompt_payloads() {
    let (db,config,_,_)=setup("policy_pages",true).await;
    let first = create_study_policy(&db,command(config,None)).await.unwrap();
    // 103 valid versions share one timestamp; UUID is the required tie-breaker.
    sqlx::query("INSERT INTO linggan_comment_study_policy \
        (policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget, \
         method_name,parent_policy_ref,method_manifest,method_hash,created_at) \
        SELECT gen_random_uuid(),p.domain_ref,p.model_config_ref,p.contract,p.comment_budget, \
        p.context_character_budget,p.method_name,p.policy_ref,p.method_manifest,p.method_hash, \
        '2026-01-01T00:00:00Z'::timestamptz \
        FROM linggan_comment_study_policy p CROSS JOIN generate_series(1,103) n WHERE p.policy_ref=$1")
        .bind(reference(&first)).execute(db.pool()).await.unwrap();
    let mut query=StudyPolicyQuery {domain:domain(),limit:Some(37),cursor:None};
    let mut seen=std::collections::BTreeSet::new();
    loop {
        let page=read_study_policies(&db,&query).await.unwrap();
        for item in page["items"].as_array().unwrap() {
            assert!(seen.insert(item["policyRef"].as_str().unwrap().to_owned()));
            assert!(item.get("methodManifest").is_none());
        }
        query.cursor=page["page"]["nextCursor"].as_str().map(str::to_owned);
        if query.cursor.is_none(){break;}
    }
    assert_eq!(seen.len(),105);
    query.limit=Some(101);assert!(read_study_policies(&db,&query).await.is_err());
    query.limit=Some(50);query.cursor=Some("invalid-cursor".into());
    assert!(read_study_policies(&db,&query).await.is_err());
}
