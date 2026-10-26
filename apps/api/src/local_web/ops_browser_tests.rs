//! Stage E: real Router, PostgreSQL, React, target receipts and unmodified export bytes.
use super::*;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, read_ops_accounts, read_ops_as_of, read_ops_daily, read_ops_publication_export,
};
use linggan_intelligence::ops_ledger::{apply_ops_import, preview_ops_import};
use sha2::{Digest, Sha256};
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;
#[path = "../../../../crates/intelligence/tests/support/ops_export_fixture.rs"]
mod ops;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL + Axum + built React; scripts/test-web-app-postgres.sh"]
async fn operations_five_pages_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_react_{}", Uuid::new_v4().simple());
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let second = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成业务二",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let mut export = ops::first_export();
    export.accounts[1]["followerCount"] = json!(0);
    export.accounts[0]["accountName"] = json!("SYNTHETIC 账号甲");
    for index in 0..60 {
        let id = format!("page-{index:02}");
        let mut row = ops::publication(
            &id,
            Some(("acc-1", "SYNTHETIC 账号甲")),
            "xhs",
            &format!("https://xhslink.com/a/SYNTHETIC{index}/"),
            "2026-10-01T04:00:00.000Z",
            None,
        );
        row["title"] = json!(format!("合成发布 {index:02}：{}", "长标题边界".repeat(12)));
        row["likeCount"] = json!(if index == 0 { 1234567 } else { 0 });
        export.publications.push(row);
    }
    export.publications.push(ops::publication(
        "second-business",
        Some(("acc-3", "SYNTHETIC 账号丙")),
        "xhs",
        "https://xhslink.com/a/SECOND/",
        "2026-10-02T04:00:00.000Z",
        None,
    ));
    export
        .daily
        .push(ops::daily("day-extra", "运营甲", "2026-10-02", 0, 0, 0));
    let request = export.request("2026-10-12T22:43:00+08:00");
    let preview = preview_ops_import(&db, &request).await.unwrap();
    apply_ops_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    // These React routes are protected by the actual server-side team-session boundary.
    // This full-ledger and target proof traverses administrator-wide data and controls.
    // Member-scoped routes and writes are covered by the isolated ops auth/native proofs.
    // The browser receives the raw token while PostgreSQL stores its digest.
    let browser_member = Uuid::new_v4();
    let password_hash = Argon2::default()
        .hash_password(
            b"synthetic-browser-proof-password",
            &SaltString::encode_b64(b"ops-browser-proof-salt").unwrap(),
        )
        .unwrap()
        .to_string();
    sqlx::query(
        "INSERT INTO linggan_ops_member \
         (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) \
         VALUES($1,'ops-browser@example.test','合成浏览器管理员','admin','active',$2,scope_001_now())",
    )
    .bind(browser_member)
    .bind(password_hash)
    .execute(db.pool())
    .await
    .unwrap();
    let browser_session_token = format!("ops-browser-{}", Uuid::new_v4().simple());
    let browser_session_digest: [u8; 32] = Sha256::digest(browser_session_token.as_bytes()).into();
    sqlx::query(
        "INSERT INTO linggan_ops_session \
         (session_ref,member_ref,token_sha256,expires_at) \
         VALUES($1,$2,$3,scope_001_now() + interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(browser_member)
    .bind(browser_session_digest.as_slice())
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_ops_account SET domain_ref=$1 WHERE source_record_id='acc-3'")
        .bind(second)
        .execute(db.pool())
        .await
        .unwrap();
    let directory = std::env::var("WEB_APP_PROOF_DIR").unwrap();
    fs::create_dir_all(&directory).unwrap();
    // Register one real tiny PNG under this proof's disposable media root only.
    let hex = "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c020000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082";
    let png: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let hash: String = Sha256::digest(&png)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let storage_key = format!("ops-covers/{hash}.png");
    let media_root = PathBuf::from(std::env::var("LINGGAN_LOCAL_MEDIA_ROOT").unwrap());
    fs::create_dir_all(media_root.join("ops-covers")).unwrap();
    fs::write(media_root.join(&storage_key), &png).unwrap();
    let cover: Uuid = sqlx::query_scalar(
        "SELECT publication_ref FROM linggan_ops_publication WHERE source_record_id='page-00'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_ops_publication_cover(publication_ref,source_path,content_sha256,byte_size,mime_type,storage_key,import_ref) VALUES($1,'synthetic/cover.png',$2,$3,'image/png',$4,$5)")
        .bind(cover).bind(&hash).bind(png.len() as i64).bind(storage_key).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    // Freeze the exact core exporter bytes as an oracle before any browser interaction.
    let accounts = read_ops_accounts(&db, OpsBusiness::All, None)
        .await
        .unwrap();
    let publications = read_ops_publication_export(&db, OpsBusiness::All)
        .await
        .unwrap();
    let daily = read_ops_daily(&db, OpsBusiness::All, None, Some(i64::from(i32::MAX)))
        .await
        .unwrap();
    let as_of = read_ops_as_of(&db).await.unwrap();
    for (kind, table) in [
        ("accounts", ops_export::accounts(&accounts.items)),
        ("publications", ops_export::publications(&publications)),
        ("daily", ops_export::daily(&daily.items)),
    ] {
        for format in ["csv", "json"] {
            let response = ops_export::response(kind, format, &table, as_of.as_ref());
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            fs::write(
                ProofPath::new(&directory).join(format!("expected-{kind}.{format}")),
                bytes,
            )
            .unwrap();
        }
    }
    let before: (i64, i64, i64) = (
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM linggan_ops_import_batch")
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let application = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    fs::write(ProofPath::new(&directory).join("fixture.json"), json!({"origin":origin,"domainOne":ops::ADHD_DOMAIN,"domainTwo":second,"cover":cover,"asOf":as_of}).to_string()).unwrap();
    let result = std::process::Command::new(std::env::var("WEB_APP_PROOF_NODE").unwrap())
        .arg(ProofPath::new(env!("CARGO_MANIFEST_DIR")).join("../web/test/e2e/ops-real-chain.mjs"))
        .env("WEB_APP_PROOF_DIR", &directory)
        .env("WEB_APP_PROOF_SESSION_TOKEN", &browser_session_token)
        .status();
    server.abort();
    let _ = server.await;
    assert!(result.unwrap().success(), "operations browser proof failed");
    let revisions: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ops_target_revision")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        revisions, 4,
        "accepted 100, concurrent 250, confirmed 200, lost/replayed 300 each append once"
    );
    let accepted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ops_target_request WHERE outcome='accepted'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let unchanged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ops_target_request WHERE outcome='unchanged'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let stale: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ops_target_request WHERE outcome='stale'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!((accepted, unchanged, stale), (4, 1, 1));
    let after: (i64, i64, i64) = (
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM linggan_ops_import_batch")
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    assert_eq!(
        before, after,
        "no collection, models or import writes during page use"
    );
}
