//! Actual Submission/Disposition/material proof for the shared incremental root+reply cap.
#[path = "support/material_fixture.rs"]
mod fixture;
use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{start_producer_attempt, submit_producer_package};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
async fn submit(db: &Database, lease: Uuid, sequence: i32, kind: &str, ids: &[String]) -> Uuid {
    let task = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let producer: Uuid =
        sqlx::query_scalar::<_, String>("SELECT install_key FROM plugin_installation LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap()
            .parse()
            .unwrap();
    let mut spec = json!({"contractVersion":"linggan.producer.task-spec.v2","taskId":task,"source":"scheduled","platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"bounded-comments"},"capabilitiesRequested":[kind],"maximumQuota":30,"commentLimit":30,"acquireMedia":"not_requested","riskPolicy":"server_authorized_leased","stopConditions":["maximum_quota"],"incrementalCommentBudget":{"knownCommentIds":["known"],"newUniqueLimit":30,"maxScrollRounds":20,"maxDurationSeconds":180}});
    if kind == "replies" {
        spec["replyExpandLimit"] = json!(2);
    }
    parse_producer_task_spec(&spec.to_string()).unwrap();
    sqlx::query("INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type)VALUES($1,$2,$3,'scheduled','xhs','synthetic_material_proof')").bind(task).bind(linggan_evidence::creator_discovery::hash(&spec.to_string())).bind(&spec).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_lease_task(lease_ref,task_id,sequence_no,execution_state,claimed_at,claimed_by_installation_ref)SELECT $1,$2,$3,'in_progress',scope_001_now(),installation_ref FROM plugin_installation LIMIT 1").bind(lease).bind(task).bind(sequence).execute(db.pool()).await.unwrap();
    start_producer_attempt(db,&parse_producer_attempt(&json!({"contractVersion":"linggan.producer.attempt.v1","taskId":task,"attemptId":attempt,"producerInstanceId":producer}).to_string()).unwrap()).await.unwrap();
    let package = Uuid::new_v4();
    let records:Vec<Value>=ids.iter().map(|id|{let mut payload=json!({"commentId":id,"noteId":"bounded-comments","text":"SYNTHETIC 合成评论新增身份","authorId":"synthetic-reader"});if kind=="replies"{payload["parentCommentId"]=json!("root-0");payload["rootCommentId"]=json!("root-0");}json!({"kind":if kind=="comments"{"comment"}else{"reply"},"sourceObject":{"platform":"xhs","type":"content","externalId":"bounded-comments"},"payload":payload})}).collect();
    let submission = json!({"contractVersion":"linggan.producer.capture-package.v1","submissionId":Uuid::new_v4(),"taskId":task,"attemptId":attempt,"producerInstanceId":producer,"capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":package,"packageKind":kind,"platform":"xhs","observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:00Z","coverage":{"target":{"contentExternalId":"bounded-comments"},"layers":[fixture::coverage_layer(kind,ids.len()as i64)]},"records":records}});
    submit_producer_package(
        db,
        &parse_producer_submission(&submission.to_string()).unwrap(),
    )
    .await
    .unwrap();
    package
}
#[tokio::test]
#[ignore = "disposable PostgreSQL; no browser/platform calls"]
async fn root_reply_new_identity_budget_is_atomic_and_retains_safe_partial() {
    let db = fixture::proof_database("topic_map_comment_budget").await;
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"bounded-comments"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"bounded-comments"},"payload":{"title":"SYNTHETIC","bodyText":"合成材料","authorId":"synthetic-creator"}})).await;
    let target = Uuid::new_v4();
    let auth = Uuid::new_v4();
    let request = Uuid::new_v4();
    let decision = Uuid::new_v4();
    let order = Uuid::new_v4();
    let station = Uuid::new_v4();
    let lease = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,source,lifecycle_state)VALUES($1,'xhs','creator','synthetic-creator','manual','archiving')").bind(target).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_authorization(authorization_ref,platform,target_kind,lane,allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at)VALUES($1,'xhs','creator','deep_archive',ARRAY['creator_archive'],ARRAY['batch'],200,'synthetic bounded proof','person',scope_001_now()+interval '1 day')").bind(auth).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_request(request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by)VALUES($1,$2,'00000000-0000-4000-8000-000000000001','primary','deep_archive','synthetic bounded proof','person')").bind(request).bind(target).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_admission_decision(decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref)VALUES($1,$2,'admitted','synthetic_bounded_proof',$3,$4)").bind(decision).bind(request).bind(auth).bind(target).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO execution_station(station_ref,display_name)VALUES($1,'synthetic station')",
    )
    .bind(station)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO collection_work_order(work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,station_ref)VALUES($1,$2,$3,'deep_archive',1,'[]',$4)").bind(order).bind(decision).bind(target).bind(station).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_lease(lease_ref,work_order_ref,station_ref,capture_identity,expires_at)VALUES($1,$2,$3,'{}',scope_001_now()+interval '1 hour')").bind(lease).bind(order).bind(station).execute(db.pool()).await.unwrap();
    let producer = Uuid::new_v4();
    sqlx::query("INSERT INTO plugin_installation(installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version)VALUES($1,$2,$3,'person',scope_001_now(),'synthetic')").bind(Uuid::new_v4()).bind(producer.to_string()).bind(station).execute(db.pool()).await.unwrap();
    let dummy = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type)VALUES($1,$2,'{}','scheduled','xhs','synthetic_material_proof')").bind(dummy).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_lease_task(lease_ref,task_id,sequence_no)VALUES($1,$2,99)").bind(lease).bind(dummy).execute(db.pool()).await.unwrap();
    let roots: Vec<String> = (0..20).map(|i| format!("root-{i}")).collect();
    let replies: Vec<String> = (0..10).map(|i| format!("reply-{i}")).collect();
    submit(&db, lease, 1, "comments", &roots).await;
    submit(&db, lease, 2, "replies", &replies).await;
    // Duplicate pages do not consume a new identity; a 31st identity and known old ID cannot enter.
    submit(&db, lease, 3, "replies", &replies).await;
    let excess = submit(&db, lease, 4, "comments", &["new-31".into()]).await;
    let known = submit(&db, lease, 5, "comments", &["known".into()]).await;
    for p in [excess, known] {
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*)FROM linggan_runtime_record_disposition WHERE package_ref=$1 AND disposition='quarantined'").bind(p).fetch_one(db.pool()).await.unwrap(),1);
    }
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(DISTINCT comment_external_id)FROM linggan_material_comment WHERE content_public_ref=(SELECT public_ref FROM linggan_material_content WHERE content_external_id='bounded-comments')").fetch_one(db.pool()).await.unwrap(),30);
}
