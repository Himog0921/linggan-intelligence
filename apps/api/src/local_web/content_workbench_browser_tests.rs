//! Formal React workbench proof over real Axum + isolated PostgreSQL and synthetic materials.
use super::*;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;

#[path = "../../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod fixture;
#[path = "../../../../crates/intelligence/tests/support/legacy_workbench_fixture.rs"]
mod old_fixture;
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
    let title_proof = std::env::var("CW_BROWSER_TITLE_PROOF").as_deref() == Ok("1");
    if title_proof {
        writing_setup::configure_fake_title(&db).await;
    }
    // The title fixture declares a model by temporarily configuring writing too. Install the
    // writing fixture last so its active endpoint and its egress permission match.
    let config = writing_setup::configure_fake_prerequisites(&db).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_creation_work")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    // Seed an explicit old-reference chain, recover only inside this owned proof DB.
    let (legacy_work, _) = old_fixture::legacy(&db, fixture::cited_doc(&db).await, 1).await;
    let legacy_preview =
        linggan_intelligence::content_workbench::recovery::preview(&db, legacy_work)
            .await
            .unwrap();
    let legacy_saved = linggan_intelligence::content_workbench::recovery::apply(
        &db,
        &legacy_preview,
        &legacy_preview.sha256().unwrap(),
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let login_password = "synthetic-workbench-browser-password";
    let password_hash = Argon2::default()
        .hash_password(
            login_password.as_bytes(),
            &SaltString::encode_b64(b"workbench-browser-proof-salt").unwrap(),
        )
        .unwrap()
        .to_string();
    // This proof traverses the creation workbench and selects a seeded account across
    // the full fixture. The public operator contract is limited to three Ops ledgers.
    sqlx::query("INSERT INTO linggan_ops_member (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) VALUES($1,'workbench-browser@example.test','创作合成管理员','admin','active',$2,scope_001_now())")
        .bind(Uuid::new_v4()).bind(password_hash).execute(db.pool()).await.unwrap();
    let before = side_effect_counts(&db).await;
    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory =
        std::env::var("CW_BROWSER_PROOF_DIR").expect("proof evidence directory supplied");
    fs::create_dir_all(&directory).unwrap();
    let title_worker = if title_proof {
        let path = ProofPath::new(&directory).join("synthetic-title-adapter.py");
        fs::write(&path, r#"import json,sys,os
request=json.load(sys.stdin)
if os.environ.get('LINGGAN_DISPATCH_SIGNAL')=='1':
    sys.stderr.write('LGI_DISPATCH_STARTED\n');sys.stderr.flush()
payload=json.loads(request['prompt'])
if payload['action']=='title_ideas':
    out={'coverTitles':['先从一小步开始','根治拖延'],'noteTitles':['孩子启动困难时先做这一件小事']}
else:
    assert payload['action']=='rewrite_selection'
    out={'title':None,'paragraphs':[{'paragraphId':payload['paragraphs'][0]['id'],'text':'合成经历改写：先做一小步。'}],'pages':[]}
print(json.dumps({'version':'linggan.pi.v1/0.85.1','ok':True,'text':json.dumps(out,ensure_ascii=False),'failureCode':None,'modelIds':None,'modelListOrigin':None,'usage':{'inputTokens':12,'outputTokens':8,'costUsd':None},'elapsedMs':300},ensure_ascii=False))
"#).unwrap();
        let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
            "/usr/bin/python3".into(),
            path,
        );
        let worker_db = db.clone();
        Some(tokio::spawn(async move {
            loop {
                let id:Option<Uuid>=sqlx::query_scalar("SELECT r.request_ref FROM linggan_creation_ai_request r JOIN linggan_creation_ai_preview p USING(preview_ref) WHERE p.action IN ('title_ideas','rewrite_selection') AND r.state='queued' ORDER BY r.created_at LIMIT 1").fetch_optional(worker_db.pool()).await.unwrap();
                if let Some(id) = id {
                    writing_setup::run_request(
                        &worker_db,
                        id,
                        &linggan_intelligence::model_secrets::SyntheticModelSecrets,
                        &adapter,
                    )
                    .await
                    .unwrap();
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }))
    } else {
        None
    };
    fs::write(
        ProofPath::new(&directory).join("fixture.json"),
        json!({
            "origin":origin,"domainRef":fixture::dossier::DOMAIN,
            "legacyWorkId":legacy_work,"legacyCurrentRevision":legacy_saved.revision.id,
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
                .env("CW_BROWSER_AUTH_PASSWORD", login_password)
                .status(),
        )
    };
    server.abort();
    let _ = server.await;
    if let Some(worker) = title_worker {
        worker.abort();
        let _ = worker.await;
    }
    let after = side_effect_counts(&db).await;
    assert_eq!(
        after.1, before.1,
        "browser proof never creates collection work orders"
    );
    assert_eq!(
        after.0,
        before.0 + if title_proof { 2 } else { 0 },
        "only the explicitly enabled synthetic title and experience rewrite calls may run"
    );
    if let Some(result) = result {
        assert!(
            result.expect("browser subprocess starts").success(),
            "formal React workbench browser proof failed; inspect its evidence directory"
        );
    }
}
