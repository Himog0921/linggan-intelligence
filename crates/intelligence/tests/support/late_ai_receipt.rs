use linggan_storage_postgres::Database;
use std::time::Duration;

// Shorten only the fixture lease for a dispatched attempt; real executor recovery writes the unknown.
pub async fn short_attempt_lease(db: &Database) {
    sqlx::raw_sql("CREATE FUNCTION proof_short_attempt_lease() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN NEW.lease_expires_at=clock_timestamp()+interval '100 milliseconds'; RETURN NEW; END $$;
        CREATE TRIGGER zz_short_attempt_lease BEFORE UPDATE ON linggan_ai_batch_item_control
        FOR EACH ROW WHEN (NEW.attempt_count>OLD.attempt_count) EXECUTE FUNCTION proof_short_attempt_lease()")
        .execute(db.pool()).await.unwrap();
}

pub async fn wait_unknown(db: &Database) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM linggan_ai_batch_item_control WHERE failure_code='ai_dispatch_unknown')").fetch_one(db.pool()).await.unwrap() { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("normal stale-lease recovery ran before the provider returned");
}

pub async fn wait_reconciled(db: &Database) {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM linggan_ai_returned_response WHERE local_state='succeeded')").fetch_one(db.pool()).await.unwrap() { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.expect("locally reconciled the already-terminal attempt");
    let counted: Vec<(i64,i64,bool)> = sqlx::query_as("SELECT i.charged_tokens,b.charged_tokens,b.quota_reconciled FROM linggan_ai_budget_usage b JOIN linggan_model_invocation i USING(invocation_ref) WHERE b.quota_reconciled")
        .fetch_all(db.pool()).await.unwrap();
    assert!(
        matches!(counted.as_slice(), [(held, 26, true)] if *held > 26),
        "one confirmed 26-token charge replaces its original reservation"
    );
    use linggan_intelligence::ai_foundation::invocation_read::{HistoryQuery, read_history};
    let history = read_history(db, &HistoryQuery::default()).await.unwrap();
    let call = history["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["failureCode"] == "ai_dispatch_unknown")
        .unwrap();
    assert_eq!(call["usageState"], "measured");
    assert_eq!(call["quotaHeldTokens"], "26");
    assert_eq!(call["measuredTokens"], "26");
    let settings = linggan_intelligence::ai_foundation::settings_read::read_settings(db, true)
        .await
        .unwrap();
    let execution = &settings["execution"];
    let daily = execution["dailyUsage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["taskKey"] == call["taskKey"])
        .unwrap();
    assert_eq!(daily["calls"], "1");
    assert_eq!(daily["quotaHeldTokens"], "26");
    assert_eq!(daily["unknownCalls"], "0");
    assert_eq!(execution["lateUsage"]["quotaReconciled"], true);
    let receipts: Vec<(String,String,String,i64,i64)> = sqlx::query_as("SELECT i.failure_code,c.failure_code,r.local_state,u.input_tokens+u.output_tokens,
        (SELECT count(*) FROM linggan_ai_execution_attempt a WHERE a.item_ref=c.item_ref)
        FROM linggan_ai_returned_response r JOIN linggan_model_invocation i USING(invocation_ref)
        JOIN linggan_ai_batch_item_control c USING(invocation_ref,item_ref) JOIN linggan_ai_late_usage_receipt u USING(invocation_ref,item_ref)
        WHERE r.reconciled_at IS NOT NULL").fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        receipts,
        vec![(
            "ai_dispatch_unknown".into(),
            "ai_dispatch_unknown".into(),
            "succeeded".into(),
            26,
            1
        )]
    );
}
