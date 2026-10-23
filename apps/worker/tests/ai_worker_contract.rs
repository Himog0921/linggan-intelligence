//! Spawn only our dedicated binary with owned PG/HTTP/files. Never launchctl or installed services.
#![recursion_limit = "256"]
#[path = "../../../crates/intelligence/tests/support/ai_service_endpoint.rs"]
mod endpoint;
#[allow(dead_code)]
#[path = "../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod fixture;
#[path = "../../../crates/intelligence/tests/support/ai_service_fixture.rs"]
mod service_fixture;
#[path = "../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;
#[allow(dead_code)]
#[path = "../../../crates/intelligence/tests/support/ai_writing_endpoint.rs"]
mod wire;
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use uuid::Uuid;
const BINARY: &str = env!("CARGO_BIN_EXE_linggan-ai-worker");
struct Process {
    child: Child,
    root: PathBuf,
}
impl Process {
    fn start(url: Option<&str>, concurrency: &str) -> Self {
        Self::start_with_identity(url, concurrency, None)
    }
    fn start_with_identity(url: Option<&str>, concurrency: &str, revision: Option<&str>) -> Self {
        let parent = std::env::var_os("LINGGAN_CREATION_ASSET_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let root = parent.join(format!("linggan-ai-worker-proof-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut command = Command::new(BINARY);
        command.process_group(0);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LINGGAN_AI_DRAIN_ACK_PATH", root.join("ack.json"))
            .env("LINGGAN_SUPPORT_DIR", &root)
            .env("LINGGAN_AI_CONCURRENCY", concurrency)
            .env("LINGGAN_MODEL_SYNTHETIC_PREVIEW", "SYNTHETIC-NOT-EVIDENCE")
            .stdout(Stdio::from(
                fs::File::create(root.join("stdout.log")).unwrap(),
            ))
            .stderr(Stdio::from(
                fs::File::create(root.join("stderr.log")).unwrap(),
            ));
        if let Some(revision) = revision {
            let identity = root.join("synthetic-source-identity.json");
            fs::write(
                &identity,
                serde_json::json!({"revision":revision,"syntheticFixture":true}).to_string(),
            )
            .unwrap();
            command.env("LINGGAN_RUNTIME_IDENTITY_PATH", identity);
        }
        if let Some(url) = url {
            command.env("LINGGAN_LOCAL_DATABASE_URL", url);
        }
        if let Some(node) = std::env::var_os("AI_FOUNDATION_PROOF_NODE") {
            command.env("LINGGAN_PI_NODE", node);
        }
        Self {
            child: command.spawn().unwrap(),
            root,
        }
    }
    async fn wait(&mut self) -> std::process::ExitStatus {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "owned child did not exit: {}",
                fs::read_to_string(self.root.join("stderr.log")).unwrap()
            )
        })
    }
    fn ack(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.join("ack.json")).unwrap()).unwrap()
    }
    fn terminate(&self) {
        assert!(
            Command::new("/bin/kill")
                .args(["-TERM", &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // This test owns a dedicated process group, including any SDK child
        // orphaned by a deliberate SIGKILL of the Rust owner.
        let _ = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[tokio::test]
async fn ai_binary_no_database_exits_failed_and_persists_only_a_failed_ack() {
    let mut process = Process::start(None, "4");
    assert_eq!(process.wait().await.code(), Some(1));
    let ack = process.ack();
    assert_eq!(ack["state"], "failed");
    assert_eq!(ack["reason"], "ai_database_not_configured");
    assert_eq!(ack["processId"], process.child.id());
    assert!(ack.get("receipt").is_none());
}
#[tokio::test]
async fn ai_binary_rejects_out_of_contract_concurrency_before_connecting() {
    let mut process = Process::start(
        Some("postgresql://synthetic:synthetic@127.0.0.1:1/synthetic"),
        "9",
    );
    assert_eq!(process.wait().await.code(), Some(1));
    assert_eq!(process.ack()["reason"], "ai_concurrency_invalid");
}
#[tokio::test]
#[ignore = "owned PostgreSQL only"]
async fn ai_binary_missing_migration_has_no_instance_or_success_ack() {
    let db = fixture::db("ai_binary_missing_migration").await;
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let scoped = format!("{url}?options=-csearch_path%3Dai_binary_missing_migration");
    for migration in [
        "0140_ai_gen1_writing",
        "0146_research_dossier_v2",
        "0148_dossier_consumer_contracts",
    ] {
        let expected: String = sqlx::query_scalar("DELETE FROM linggan_local_schema_migration WHERE migration_id=$1 RETURNING migration_sha256")
            .bind(migration).fetch_one(db.pool()).await.unwrap();
        let mut process = Process::start(Some(&scoped), "4");
        assert_eq!(process.wait().await.code(), Some(1));
        assert_eq!(process.ack()["reason"], "ai_executor_schema_not_ready");
        sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES($1,$2)")
            .bind(migration).bind(expected).execute(db.pool()).await.unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ai_executor_instance")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}
#[tokio::test]
#[ignore = "owned PostgreSQL/HTTP and actual Node/SDK only"]
async fn ai_binary_sigterm_waits_for_four_real_calls_and_agrees_with_database_before_ack() {
    let db = fixture::db("ai_binary_drain").await;
    let endpoint = endpoint::Endpoint::start().await;
    service_fixture::queued(&db, &endpoint.url, 5).await;
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let scoped = format!("{url}?options=-csearch_path%3Dai_binary_drain");
    let mut process = Process::start(Some(&scoped), "4");
    tokio::time::timeout(Duration::from_secs(15), async {
        while endpoint.requests.lock().unwrap().len() < 4 {
            assert!(
                process.child.try_wait().unwrap().is_none(),
                "owned child must stay up"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    process.terminate();
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {let draining:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_executor_instance WHERE process_id=$1 AND state='draining')").bind(process.child.id() as i32).fetch_one(db.pool()).await.unwrap();if draining {break;}tokio::time::sleep(Duration::from_millis(20)).await;}
    }).await.unwrap();
    assert!(
        !process.root.join("ack.json").exists(),
        "no premature file acknowledgement"
    );
    assert!(process.child.try_wait().unwrap().is_none());
    endpoint.release();
    assert_eq!(process.wait().await.code(), Some(0));
    let ack = process.ack();
    assert_eq!(ack["state"], "drained");
    assert_eq!(ack["receipt"]["processId"], process.child.id());
    assert_eq!(
        ack["receipt"]["sourceRevision"], "unknown",
        "synthetic process does not pretend to be a deployed revision"
    );
    let reference: Uuid = serde_json::from_value(ack["receipt"]["instanceRef"].clone()).unwrap();
    let row:(String,String,i32)=sqlx::query_as("SELECT state,generation::text,process_id FROM linggan_ai_executor_instance WHERE instance_ref=$1").bind(reference).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        row,
        (
            "drained".into(),
            ack["receipt"]["generation"].as_str().unwrap().into(),
            process.child.id() as i32
        )
    );
    let counts:(i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE state='succeeded'),count(*) FILTER(WHERE state='pending') FROM linggan_ai_batch_item_control").fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (4, 1));
    assert_eq!(endpoint.requests.lock().unwrap().len(), 4);
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL/loopback and actual dedicated binary; deliberate SIGKILL"]
async fn ai_binary_sigkill_restart_keeps_attempted_calls_unknown_and_only_dispatches_untouched_work()
 {
    let db = fixture::db("ai_binary_sigkill").await;
    let endpoint = endpoint::Endpoint::start().await;
    service_fixture::queued(&db, &endpoint.url, 5).await;
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let scoped = format!("{url}?options=-csearch_path%3Dai_binary_sigkill");
    let mut original = Process::start(Some(&scoped), "4");
    tokio::time::timeout(Duration::from_secs(15), async {
        while endpoint.requests.lock().unwrap().len() < 4 {
            assert!(original.child.try_wait().unwrap().is_none());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let old: (Uuid, i64) = sqlx::query_as(
        "SELECT instance_ref,generation FROM linggan_ai_executor_instance WHERE process_id=$1",
    )
    .bind(original.child.id() as i32)
    .fetch_one(db.pool())
    .await
    .unwrap();
    original.child.kill().unwrap(); // std Child::kill is SIGKILL on this Unix proof host.
    assert!(!original.wait().await.success());
    assert!(
        !original.root.join("ack.json").exists(),
        "abrupt death cannot produce a drain receipt"
    );
    endpoint.release(); // Finish the four owned HTTP sockets and orphaned SDK children.
    let mut replacement = Process::start(Some(&scoped), "4");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            assert!(replacement.child.try_wait().unwrap().is_none());
            let counts: (i64, i64) = sqlx::query_as(
                "SELECT count(*) FILTER(WHERE state='failed' AND failure_code='ai_dispatch_unknown'),count(*) FILTER(WHERE state='succeeded') FROM linggan_ai_batch_item_control",
            ).fetch_one(db.pool()).await.unwrap();
            if counts == (4, 1) { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    replacement.terminate();
    assert!(replacement.wait().await.success());
    let ack = replacement.ack();
    assert_eq!(ack["state"], "drained");
    let current: Uuid = serde_json::from_value(ack["receipt"]["instanceRef"].clone()).unwrap();
    assert_ne!(current, old.0);
    let states: (String, i64, String) = sqlx::query_as(
        "SELECT old.state,current.generation,current.state FROM linggan_ai_executor_instance old CROSS JOIN linggan_ai_executor_instance current WHERE old.instance_ref=$1 AND current.instance_ref=$2",
    ).bind(old.0).bind(current).fetch_one(db.pool()).await.unwrap();
    assert_eq!(states, ("lost".into(), old.1 + 1, "drained".into()));
    let ledger: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),count(*) FILTER(WHERE usage_state='unknown_reserved' AND charged_tokens>0),count(*) FILTER(WHERE usage_state='measured' AND input_tokens=17 AND output_tokens=9 AND charged_tokens=26) FROM linggan_model_invocation",
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(ledger, (5, 4, 1));
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        5,
        "the four attempted calls must never be resent"
    );
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL/HTTP and actual Node/SDK; deliberate receipt transaction failure"]
async fn ai_binary_failed_settlement_keeps_unknown_quota_actual_late_usage_and_a_failed_ack() {
    let db = fixture::db("ai_binary_receipt_failure").await;
    let endpoint = endpoint::Endpoint::start().await;
    service_fixture::queued(&db, &endpoint.url, 1).await;
    sqlx::raw_sql("CREATE FUNCTION synthetic_fail_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF OLD.state='running' AND NEW.state IN ('succeeded','failed') THEN
        RAISE EXCEPTION 'synthetic receipt unavailable' USING ERRCODE='40001'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER synthetic_receipt_failure BEFORE UPDATE OF state ON linggan_model_invocation
        FOR EACH ROW EXECUTE FUNCTION synthetic_fail_receipt();").execute(db.pool()).await.unwrap();
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let scoped = format!("{url}?options=-csearch_path%3Dai_binary_receipt_failure");
    let mut process = Process::start(Some(&scoped), "4");
    tokio::time::timeout(Duration::from_secs(15), async {
        while endpoint.requests.lock().unwrap().is_empty() {
            assert!(process.child.try_wait().unwrap().is_none());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    process.terminate();
    endpoint.release();
    assert_eq!(process.wait().await.code(), Some(1));
    let ack = process.ack();
    assert_eq!(ack["state"], "failed");
    assert_eq!(ack["reason"], "ai_executor_receipt_timed_out");
    assert!(ack.get("receipt").is_none());
    let evidence:(String,String,Option<i64>,i64,i64,i64)=sqlx::query_as("SELECT i.state,i.usage_state,i.measured_tokens,i.charged_tokens,late.input_tokens,late.output_tokens FROM linggan_model_invocation i JOIN linggan_ai_late_usage_receipt late USING(invocation_ref)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        (&evidence.0, &evidence.1, evidence.2, evidence.4, evidence.5),
        (&"running".into(), &"unknown_reserved".into(), None, 17, 9)
    );
    assert!(evidence.3 > 0);
    let drained: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ai_executor_instance WHERE state='drained')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!drained);
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        1,
        "receipt retry cannot resend"
    );
    endpoint.finish().await;
}

fn guard_command(mode: &str) -> Command {
    let mut command = Command::new("/usr/bin/python3");
    command
        .arg("-B")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/runtime/ai_guard.py"))
        .arg(mode)
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    command
}
async fn guard_snapshot(db: &linggan_storage_postgres::Database, presence: &str) -> Value {
    let sql = guard_command("snapshot-query")
        .arg(presence)
        .output()
        .unwrap();
    assert!(sql.status.success());
    // Only the checked-in query builder and its closed relation-presence enum supply SQL.
    sqlx::query_scalar(sqlx::AssertSqlSafe(String::from_utf8(sql.stdout).unwrap()))
        .fetch_one(db.pool())
        .await
        .unwrap()
}
fn check_guard(mode: &str, args: &[String], snapshot: &Value) -> std::process::Output {
    use std::io::Write;
    let mut child = guard_command(mode)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(snapshot.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[tokio::test]
#[ignore = "owned PostgreSQL and real AI binary/file/metadata boundary; no model calls"]
async fn release_guard_verifies_real_database_snapshot_and_process_ack_with_a_declared_synthetic_identity()
 {
    let db = fixture::db("ai_release_guard").await;
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let scoped = format!("{url}?options=-csearch_path%3Dai_release_guard");
    // This is an explicit identity fixture, not a claim that this test deployed a Git revision.
    let revision = "a".repeat(40);
    let mut process = Process::start_with_identity(Some(&scoped), "4", Some(&revision));
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {let running:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_executor_instance WHERE process_id=$1 AND state='running')").bind(process.child.id() as i32).fetch_one(db.pool()).await.unwrap();if running {break;}assert!(process.child.try_wait().unwrap().is_none());tokio::time::sleep(Duration::from_millis(20)).await;}
    }).await.unwrap();
    let active = guard_snapshot(&db, "1,1").await;
    assert_eq!(active["sessionLockHeld"], true);
    assert!(
        !check_guard("no-ai", &[], &active).status.success(),
        "absence of work cannot hide the live session lock"
    );
    process.terminate();
    assert_eq!(process.wait().await.code(), Some(0), "{}", process.ack());
    let drained = guard_snapshot(&db, "1,1").await;
    assert_eq!(drained["sessionLockHeld"], false);
    assert_eq!(drained["executor"]["sourceRevision"], revision);
    let args = [
        process.root.join("ack.json").to_string_lossy().into_owned(),
        process.child.id().to_string(),
        revision.clone(),
    ];
    let check = check_guard("check-ack", &args, &drained);
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let proof: Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(proof["databaseKey"], drained["databaseKey"]);
    assert_eq!(proof["state"], "drained");
    let mut wrong = drained.clone();
    wrong["executor"]["generation"] = serde_json::json!("999");
    assert!(!check_guard("check-ack", &args, &wrong).status.success());
    let legacy = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "ai_guard_legacy",
        "CREATE TABLE linggan_model_invocation(state text NOT NULL);",
    )
    .await
    .unwrap();
    let no_executor = guard_snapshot(&legacy, "0,1").await;
    assert!(no_executor["executor"].is_null());
    assert!(check_guard("no-ai", &[], &no_executor).status.success());
    sqlx::query("INSERT INTO linggan_model_invocation(state) VALUES('running')")
        .execute(legacy.pool())
        .await
        .unwrap();
    let unfinished = guard_snapshot(&legacy, "0,1").await;
    assert!(
        !check_guard("no-ai", &[], &unfinished).status.success(),
        "legacy running reservations cannot become no-worker evidence"
    );
}
