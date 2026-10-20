//! Formal React workbench proof over real Axum + isolated PostgreSQL and synthetic materials.
use super::*;
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;

#[path = "../../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod fixture;
#[path = "content_workbench_private_review.rs"]
mod private_review;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod writing_setup;

async fn seed_note(db: &Database, external: &str, title: &str, anchor: &str) -> Uuid {
    let body = format!(
        "{anchor}\n这是浏览器隔离验收使用的合成长文😀，不是现实研究结论。\n孩子写作业不愿开始，第一步可以先做两分钟。\n{}\n{anchor}尾段仍须可读🙂",
        "把一天的计划缩小到一个能开始的动作。换行、标点与原文身份都保留。\n".repeat(100)
    );
    let package = fixture::dossier::fixture::submit_package(db, "content_detail",
        json!({"contentExternalId":external}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},
            "payload":{"title":title,"bodyText":body,"authorId":"synthetic-browser-author","likedCount":0}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(fixture::dossier::DOMAIN).bind(package)
        .execute(db.pool()).await.unwrap();
    work
}
async fn seed_comment(db: &Database, id: &str, text: &str, parent: Option<&str>) -> Uuid {
    let mut payload = json!({"commentId":id,"noteId":"cw-browser-note-1","text":text});
    if let Some(parent) = parent {
        payload["rootCommentId"] = json!(parent);
        payload["parentCommentId"] = json!(parent);
    }
    fixture::dossier::fixture::submit_package(db, if parent.is_some() {"replies"} else {"comments"},
        json!({"contentExternalId":"cw-browser-note-1"}),
        json!({"kind":if parent.is_some() {"reply"} else {"comment"},"sourceObject":{"platform":"xhs","type":"content","externalId":"cw-browser-note-1"},"payload":payload})).await;
    sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap()
}
async fn side_effect_counts(db: &Database) -> (i64, i64) {
    let invocations = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let orders = sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
        .fetch_one(db.pool())
        .await
        .unwrap();
    (invocations, orders)
}

// The blocking browser subprocess must not occupy the only executor thread serving HTTP.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL + built formal React app + local Node/Chromium; explicit proof guard"]
async fn content_workbench_real_chain_browser_proof() {
    assert_eq!(std::env::var("CW_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE")
    );
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("isolated proof URL supplied");
    let schema = format!("cw_browser_{}", Uuid::new_v4().simple());
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .expect("full migrations apply to isolated schema");
    let manual = std::env::var("CW_BROWSER_MANUAL_REVIEW").as_deref() == Ok("1");
    let sample_path = std::env::var("CW_BROWSER_REAL_SAMPLES").ok();
    assert!(
        sample_path.is_none() || manual,
        "private real samples require explicit manual review"
    );
    let seeded = if let Some(path) = sample_path {
        private_review::seed(&db, &path).await
    } else {
        let first = seed_note(
            &db,
            "cw-browser-note-1",
            "合成选题：把开始缩小",
            "浏览器原文锚点一",
        )
        .await;
        let second = seed_note(
            &db,
            "cw-browser-note-2",
            "合成选题：给家庭留出缓冲",
            "浏览器原文锚点二",
        )
        .await;
        let voice_one = seed_comment(
            &db,
            "cw-browser-parent",
            "浏览器匿名原声一😀\n每天列了很多计划，真正困难的是先开始。",
            None,
        )
        .await;
        let voice_two = seed_comment(
            &db,
            "cw-browser-voice-2",
            "浏览器匿名原声二🙂\n我愿意先做两分钟，再决定要不要继续。",
            None,
        )
        .await;
        let reply = seed_comment(
            &db,
            "cw-browser-reply",
            "浏览器回复原声😀\n上面说的先开始，我也有同样的困惑。",
            Some("cw-browser-parent"),
        )
        .await;
        let (account, publication) = fixture::ops_seed(&db).await;
        json!({"noteRefs":[first,second],"voiceRefs":[voice_one,voice_two],"replyRef":reply,"accountRef":account,"publicationRef":publication})
    };
    let config = writing_setup::configure_fake_prerequisites(&db).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_creation_work")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let before = side_effect_counts(&db).await;
    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory =
        std::env::var("CW_BROWSER_PROOF_DIR").expect("proof evidence directory supplied");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        ProofPath::new(&directory).join("fixture.json"),
        json!({
            "origin":origin,"domainRef":fixture::dossier::DOMAIN,
            "otherDomainRef":"00000000-0000-4000-8000-000000000002","accountRef":seeded["accountRef"],
            "noteRefs":seeded["noteRefs"],"voiceRefs":seeded["voiceRefs"],"replyRef":seeded["replyRef"],
            "publicationRef":seeded["publicationRef"],"configRef":config,"privateReview":manual,
            "provenance":seeded
        })
        .to_string(),
    )
    .unwrap();
    let result = if manual {
        println!(
            "Private candidate ready at {origin}; finish by creating review-complete in the proof directory"
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(8 * 60 * 60);
        while !ProofPath::new(&directory).join("review-complete").exists()
            && tokio::time::Instant::now() < deadline
        {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        None
    } else {
        let node = std::env::var("CW_BROWSER_PROOF_NODE").expect("Node binary supplied");
        let script = ProofPath::new(env!("CARGO_MANIFEST_DIR"))
            .join("../web/test/e2e/workbench-real-chain.mjs");
        Some(
            std::process::Command::new(node)
                .arg(script)
                .env("CW_BROWSER_PROOF_DIR", &directory)
                .status(),
        )
    };
    server.abort();
    let _ = server.await;
    let after = side_effect_counts(&db).await;
    assert_eq!(
        before, after,
        "the browser authorship path must not call a model or create collection work orders"
    );
    if let Some(result) = result {
        assert!(
            result.expect("browser subprocess starts").success(),
            "formal React workbench browser proof failed; inspect its evidence directory"
        );
    }
}
