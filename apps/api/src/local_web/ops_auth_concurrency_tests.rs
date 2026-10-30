//! Synthetic overlapping credential updates, using the owned PostgreSQL proof runner.
use super::*;
use std::time::Duration;

async fn database() -> linggan_storage_postgres::Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("auth_overlap_{}", Uuid::new_v4().simple());
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap()
}

fn changed_hash() -> String {
    Argon2::default()
        .hash_password(
            b"concurrently-changed-password",
            &SaltString::encode_b64(b"synthetic-overlap-salt").unwrap(),
        )
        .unwrap()
        .to_string()
}

async fn wait_for_member_lock(database: &linggan_storage_postgres::Database, lock: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() \
                 AND pid<>pg_backend_pid() AND wait_event_type='Lock' \
                 AND query LIKE '%linggan_ops_member%' AND query LIKE $1)",
            )
            .bind(format!("%FOR {lock}%"))
            .fetch_one(database.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the request must reach the locked credential recheck");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL only; scripts/test-web-app-postgres.sh with ops-auth scope"]
async fn login_rechecks_changed_password_and_deactivation_before_issuing_a_session() {
    let database = database().await;
    let app = app_with_database(database.clone());
    let changed = changed_hash();
    for action in ["password", "deactivate"] {
        let email = format!("{action}@example.test");
        let member = active_member(database.pool(), &email, "operator").await;
        let mut locker = database.pool().begin().await.unwrap();
        sqlx::query("SELECT member_ref FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE")
            .bind(member)
            .execute(&mut *locker)
            .await
            .unwrap();
        let request = json_request(
            "/api/local/auth/login",
            serde_json::json!({"email":email,"password":"synthetic-password-only"}).to_string(),
        );
        let response = tokio::spawn(app.clone().oneshot(request));
        wait_for_member_lock(&database, "SHARE").await;
        if action == "password" {
            sqlx::query("UPDATE linggan_ops_member SET password_hash=$2,version=version+1 WHERE member_ref=$1")
                .bind(member).bind(&changed).execute(&mut *locker).await.unwrap();
        } else {
            sqlx::query("UPDATE linggan_ops_member SET state='disabled',disabled_at=scope_001_now(),version=version+1 WHERE member_ref=$1")
                .bind(member).execute(&mut *locker).await.unwrap();
        }
        locker.commit().await.unwrap();
        let response = response.await.unwrap().unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "action {action}"
        );
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        let sessions: i64 =
            sqlx::query_scalar("SELECT count(*) FROM linggan_ops_session WHERE member_ref=$1")
                .bind(member)
                .fetch_one(database.pool())
                .await
                .unwrap();
        assert_eq!(sessions, 0, "a stale credential must never issue a session");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL only; scripts/test-web-app-postgres.sh with ops-auth scope"]
async fn password_change_cannot_overwrite_a_concurrent_password_update() {
    let database = database().await;
    let member = active_member(database.pool(), "overlap@example.test", "operator").await;
    let app = app_with_database(database.clone());
    let cookie = login_cookie(app.clone(), "overlap@example.test").await;
    let changed = changed_hash();
    let mut locker = database.pool().begin().await.unwrap();
    sqlx::query("SELECT member_ref FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE")
        .bind(member)
        .execute(&mut *locker)
        .await
        .unwrap();
    let request = authenticated_json_request(
        "/api/local/auth/password/change",
        r#"{"currentPassword":"synthetic-password-only","newPassword":"stale-overwrite"}"#,
        &cookie,
    );
    let response = tokio::spawn(app.oneshot(request));
    wait_for_member_lock(&database, "UPDATE").await;
    sqlx::query(
        "UPDATE linggan_ops_member SET password_hash=$2,version=version+1 WHERE member_ref=$1",
    )
    .bind(member)
    .bind(&changed)
    .execute(&mut *locker)
    .await
    .unwrap();
    locker.commit().await.unwrap();
    let response = response.await.unwrap().unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let stored: String =
        sqlx::query_scalar("SELECT password_hash FROM linggan_ops_member WHERE member_ref=$1")
            .bind(member)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        stored, changed,
        "the concurrent credential must remain intact"
    );
}
