//! Cancellation at the actual PG BEGIN reply boundary, using only the owned AI proof runner.
#[path = "support/ai_begin_proxy.rs"]
mod ai_begin_proxy;

use linggan_storage_postgres::Database;
use sqlx::Connection;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires owned AI-GEN1 disposable PostgreSQL runner"]
async fn pool_clears_cancelled_begin_without_losing_commits_or_session_settings() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let proxy = ai_begin_proxy::BeginProxy::start(&url).await;
    let mut setup = sqlx::PgConnection::connect(&url).await.unwrap();
    sqlx::raw_sql("CREATE SCHEMA ai_pool_guard_proof")
        .execute(&mut setup)
        .await
        .unwrap();
    setup.close().await.unwrap();
    let db = Database::connect_within_schema(&proxy.url, "ai_pool_guard_proof")
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE ai_pool_cancel_probe(id integer PRIMARY KEY)")
        .execute(db.pool())
        .await
        .unwrap();
    idle(&db).await;
    let original_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    idle(&db).await;
    proxy.arm();
    let task = tokio::spawn({
        let db = db.clone();
        async move { db.pool().begin().await }
    });
    tokio::time::timeout(Duration::from_secs(5), proxy.observed.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    proxy.release.notify_one();
    idle(&db).await;
    // A leaked backend BEGIN gives this otherwise autocommit read a persistent snapshot.
    let reused_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        reused_pid, original_pid,
        "reuse the actual faulted connection"
    );
    idle(&db).await;
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let (zone, path): (String, String) =
        sqlx::query_as("SELECT current_setting('TimeZone'),current_setting('search_path')")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(zone, "Asia/Shanghai");
    assert_eq!(path, "ai_pool_guard_proof");
    tx.commit().await.unwrap();
    idle(&db).await;
    assert_transactions_preserved(&db).await;
    assert_aborted_connection_is_discarded(&db).await;
    db.pool().close().await;
}

async fn assert_transactions_preserved(db: &Database) {
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO ai_pool_cancel_probe VALUES(1)")
        .execute(&mut *tx)
        .await
        .unwrap();
    drop(tx);
    idle(db).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_pool_cancel_probe")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "uncommitted data must not leak");
    idle(db).await;
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO ai_pool_cancel_probe VALUES(2)")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    idle(db).await;
    // The identical prepared query is deliberately reused after the release barrier.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ai_pool_cancel_probe")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "committed data and prepared statements must survive"
    );
    idle(db).await;
}

async fn assert_aborted_connection_is_discarded(db: &Database) {
    let mut connection = db.pool().acquire().await.unwrap();
    let original_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert!(
        sqlx::raw_sql("BEGIN; SELECT 1/0")
            .execute(&mut *connection)
            .await
            .is_err()
    );
    drop(connection);
    let new_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_ne!(
        new_pid, original_pid,
        "a failed release must not return to the pool"
    );
    idle(db).await;
}

async fn idle(db: &Database) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while db.pool().num_idle() != 1 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(db.pool().size(), 1, "do not substitute a clean connection");
    })
    .await
    .unwrap();
}
