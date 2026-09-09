//! Explicit synthetic historical-parser fixtures. The seeded retained response represents
//! an old parser rejecting a complete fence; this is not a claim about a real provider call.
use super::*;

async fn historical_fenced_failure(db: &Database, config: Uuid) -> (Uuid, Uuid) {
    let source_ref = source(
        db,
        "local-recovery",
        "one",
        "[BAD_OUTPUT] 每天催促作业需要很多精力",
    )
    .await;
    let batch = selected(db, config, vec![source_ref], 100_000).await;
    assert!(tick(db).await);
    let row = sqlx::query("SELECT p.invocation_ref,t.input_content FROM linggan_comment_daily_packet p JOIN linggan_comment_request_trace t USING(invocation_ref) WHERE p.batch_ref=$1")
        .bind(batch).fetch_one(db.pool()).await.unwrap();
    let invocation: Uuid = row.get("invocation_ref");
    let input: serde_json::Value = row.get("input_content");
    let comment = &input["comments"][0];
    let retained = json!({"comments":[{"commentRef":comment["commentRef"],"outcome":"interpretable",
        "labels":[],"problems":[{"basis":"explicit","contextEvidence":[],"name":"合成精力问题",
        "meaning":"每天催促需要精力","evidence":[{"quote":comment["text"]}]}],"stances":[],
        "contextMissing":[],"uncertaintyReason":null,"limitations":["SYNTHETIC HISTORICAL PARSER FIXTURE"]}]});
    // This is fixture construction only: no production endpoint rewrites retained output.
    sqlx::query(
        "UPDATE linggan_comment_request_trace SET output_content=$2 WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .bind(format!("```json\n{retained}\n```"))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_comment_daily_batch SET enabled=false WHERE batch_ref=$1")
        .bind(batch)
        .execute(db.pool())
        .await
        .unwrap();
    (source_ref, invocation)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn retained_fence_recovers_locally_without_new_call_or_rewriting_failed_ledger() {
    let db = fixture::proof_database("local_fence_recovery").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (source_ref, invocation) = historical_fenced_failure(&db, config).await;
    let before: (String, Option<String>, i64) = sqlx::query_as("SELECT state,failure_code,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(before.0, "failed");
    assert!(before.2 > 0);
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        !tick(&db).await,
        "local recovery must not report a newly dispatched call"
    );
    let accepted: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_analysis_work WHERE source_ref=$1 AND state='succeeded' AND attempts=0 AND result ? 'localRecoveryRef'")
        .bind(source_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(accepted, 1);
    let after: (String, Option<String>, i64) = sqlx::query_as("SELECT state,failure_code,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        before, after,
        "historical failure and its charge remain intact"
    );
    assert!(!tick(&db).await);
    let receipt: serde_json::Value = sqlx::query_scalar(
        "SELECT receipt FROM linggan_comment_local_recovery WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(receipt["accepted"], 1);
    assert_eq!(receipt["newProviderCalls"], 0);
    assert_eq!(receipt["normalization"], "json_fence");
    let after_calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(calls, after_calls);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn expired_retained_output_never_recovers_or_dispatches() {
    let db = fixture::proof_database("expired_local_fence").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (_, invocation) = historical_fenced_failure(&db, config).await;
    sqlx::query("UPDATE linggan_comment_request_trace SET expires_at=scope_001_now()-interval '1 second' WHERE invocation_ref=$1")
        .bind(invocation).execute(db.pool()).await.unwrap();
    assert!(!tick(&db).await);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_local_recovery")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    let purged: bool = sqlx::query_scalar("SELECT output_content IS NULL AND purged_at IS NOT NULL FROM linggan_comment_request_trace WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert!(purged);
    server.kill().await.unwrap();
}
