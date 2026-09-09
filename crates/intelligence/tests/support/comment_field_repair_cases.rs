//! Runs the real repair worker against a deliberately partial local synthetic provider.
use super::*;

async fn partial(
    db: &Database,
    config: Uuid,
    marker: &str,
) -> (Uuid, Uuid, Uuid, serde_json::Value) {
    let source_ref = source(
        db,
        "field-repair",
        "one",
        &format!("[PARTIAL_FIELD] {marker} 我每天提醒作业还是很困难，求具体方法"),
    )
    .await;
    let batch = selected(db, config, vec![source_ref], 100_000).await;
    assert!(tick(db).await);
    let (analysis,result):(Uuid,serde_json::Value)=sqlx::query_as("SELECT analysis_ref,a.result FROM linggan_comment_daily_item i JOIN linggan_comment_analysis_work a ON a.work_ref=i.analysis_ref WHERE batch_ref=$1")
        .bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(result["semantic"]["acceptance"], "partial");
    (batch, source_ref, analysis, result)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn field_repair_appends_once_preserves_base_and_is_visible_in_original_run() {
    let db = fixture::proof_database("field_repair_receipt").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (batch, _, base, before) = partial(&db, config, "").await;
    assert!(tick(&db).await);
    let (state,after,invocation):(String,Uuid,Uuid)=sqlx::query_as("SELECT state,supplement_analysis_ref,invocation_ref FROM linggan_comment_field_repair WHERE base_analysis_ref=$1")
        .bind(base).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state, "succeeded");
    assert_ne!(base, after);
    let unchanged: serde_json::Value =
        sqlx::query_scalar("SELECT result FROM linggan_comment_analysis_work WHERE work_ref=$1")
            .bind(base)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(before, unchanged);
    let merged: serde_json::Value =
        sqlx::query_scalar("SELECT result FROM linggan_comment_analysis_work WHERE work_ref=$1")
            .bind(after)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        merged["semantic"]["problems"],
        before["semantic"]["problems"]
    );
    assert_eq!(merged["semantic"]["labels"][0]["label"], "need");
    let details = batch_detail(&db, batch, None).await.unwrap();
    assert_eq!(details["callTotal"], 2);
    assert!(
        details["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["purpose"] == "field_repair")
    );
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let request =
        linggan_intelligence::comment_runtime::request_detail(&db, batch, invocation, domain)
            .await
            .unwrap();
    assert_eq!(request["availability"], "AVAILABLE");
    assert_eq!(request["input"]["selectedFields"][0]["path"], "labels[0]");
    assert_eq!(
        request["repairReceipt"]["supplementAnalysisRef"],
        json!(after)
    );
    assert!(!tick(&db).await);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_field_repair")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn field_repair_invalid_output_keeps_failure_trace_and_never_rewrites_accepted_fields() {
    let db = fixture::proof_database("field_repair_invalid").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (_, _, base, before) = partial(&db, config, "[BAD_REPAIR]").await;
    assert!(tick(&db).await);
    let (state,output,validation,charged):(String,String,serde_json::Value,i64)=sqlx::query_as("SELECT r.state,t.output_content,t.validation,v.charged_tokens FROM linggan_comment_field_repair r JOIN linggan_comment_field_repair_trace t USING(repair_ref) JOIN linggan_model_invocation v ON v.invocation_ref=r.invocation_ref")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(state, "failed");
    assert_eq!(output, "invalid JSON");
    assert!(!validation.as_array().unwrap().is_empty());
    assert!(charged > 0);
    let unchanged: serde_json::Value =
        sqlx::query_scalar("SELECT result FROM linggan_comment_analysis_work WHERE work_ref=$1")
            .bind(base)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(unchanged, before);
    assert!(!tick(&db).await);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic delayed Pi"]
async fn field_repair_withdrawal_during_call_rejects_late_result_and_preserves_charge() {
    let db = fixture::proof_database("field_repair_withdraw").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (_, source_ref, base, _) = partial(&db, config, "[DELAY_REPAIR]").await;
    let adapter = PiAdapter::configured();
    let running = run_daily_once(&db, &SyntheticModelSecrets, &adapter);
    let withdraw = async {
        for _ in 0..200 {
            let started: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM linggan_comment_field_repair WHERE state='running')",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if started {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                restrict_comment_research_source(&db, source_ref, "synthetic in-flight withdrawal")
                    .await
                    .unwrap();
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("repair never started");
    };
    let (result, ()) = tokio::join!(running, withdraw);
    assert!(result.unwrap());
    let (state,supplement,charged):(String,Option<Uuid>,i64)=sqlx::query_as("SELECT r.state,r.supplement_analysis_ref,v.charged_tokens FROM linggan_comment_field_repair r JOIN linggan_model_invocation v USING(invocation_ref) WHERE base_analysis_ref=$1")
        .bind(base).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state, "failed");
    assert!(supplement.is_none());
    assert!(charged > 0);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn schema_failure_is_not_reported_as_queued_when_automatic_recovery_cannot_run_it() {
    let db = fixture::proof_database("nonrecoverable_retry").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source_ref = source(
        &db,
        "invalid-schema",
        "one",
        "[BAD_SCHEMA] 合成整条结构错误",
    )
    .await;
    let first = selected(&db, config, vec![source_ref], 100_000).await;
    assert!(tick(&db).await);
    assert_eq!(
        retry_failed(&db, first, Uuid::new_v4()).await.unwrap()["queued"],
        0
    );
    let second = selected(&db, config, vec![source_ref], 100_000).await;
    assert!(!tick(&db).await);
    assert_eq!(
        retry_failed(&db, second, Uuid::new_v4()).await.unwrap()["queued"],
        0
    );
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 1);
    server.kill().await.unwrap();
}
