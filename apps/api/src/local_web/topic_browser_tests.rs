//! Stage F: built React + real Axum + disposable PostgreSQL. Synthetic inputs only.
use super::*;
use linggan_intelligence::{
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceImport, import_topic_workspace,
    topic_map::{TopicMapCommand, save_topic_map_command},
};
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod material_fixture;

async fn seed_work(db: &Database, domain: Uuid, index: usize) -> Uuid {
    let external = format!("SYNTHETIC-topic-react-{index}");
    let mut payload = json!({
        "title":format!("合成材料 {index:02} · <img src=x onerror=alert(1)>"),
        "bodyText":format!("SYNTHETIC ONLY：第{index}条材料。🧭 先试两分钟，再核对记录与适用边界。"),
        "authorId":format!("synthetic-author-{}",index % 4),
        "authorName":format!("合成作者 {}",index % 4),
        "likedCount":if index==1 {0} else {1200},
    });
    if index == 2 {
        for key in ["title", "authorName", "likedCount"] {
            payload.as_object_mut().unwrap().remove(key);
        }
    }
    let package = material_fixture::submit_package(db,"content_detail",json!({"contentExternalId":external}),json!({
        "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":payload
    })).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(domain).bind(package).execute(db.pool()).await.unwrap();
    work
}

async fn seed_topic(
    db: &Database,
    domain: Uuid,
    key: &str,
    name: &str,
    works: &[Uuid],
) -> linggan_intelligence::TopicWorkspaceReceipt {
    let receipt = import_topic_workspace(
        db,
        &TopicWorkspaceImport {
            idempotency_key: format!("topic-react:{key}"),
            domain_key: "synthetic-topic-react".into(),
            canonical_key: key.into(),
            display_name: name.into(),
            definition_text: "合成定义：明确行为与来源边界；不表示现实研究。".into(),
            expected_version: None,
            adjudication_note: "SYNTHETIC：逐条人工裁定，不外推总体。".into(),
            source_boundary: "仅证明隔离库中明确列出的合成材料。".into(),
            members: works
                .iter()
                .enumerate()
                .map(|(i, r)| TopicMaterialMemberImport {
                    work_public_ref: *r,
                    role: match i % 3 {
                        0 => TopicMaterialRole::Support,
                        1 => TopicMaterialRole::Challenge,
                        _ => TopicMaterialRole::Boundary,
                    },
                    rationale: format!("合成分类理由 {i}"),
                })
                .collect(),
        },
    )
    .await
    .unwrap();
    save_topic_map_command(
        db,
        &TopicMapCommand::BindTopic {
            idempotency_key: format!("topic-react-bind:{key}"),
            domain_ref: domain,
            topic_ref: receipt.topic_ref,
            parent_topic_ref: None,
            expected_version: None,
        },
    )
    .await
    .unwrap();
    receipt
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disposable PostgreSQL + built React + browser; scripts/test-web-app-postgres.sh"]
async fn topics_and_workspace_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &format!("topic_react_{}", Uuid::new_v4().simple()),
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    // Name order differs from insertion order: preserve the old Topic page default.
    let second = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "ZZZ 合成领域二",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "AAA 合成主题领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let mut works = Vec::new();
    for i in 0..26 {
        works.push(seed_work(&db, domain, i).await);
    }
    seed_work(&db, second, 26).await;
    let first = seed_topic(
        &db,
        domain,
        "synthetic-react-workspace",
        "合成同名主题",
        &works[..3],
    )
    .await;
    let peer = seed_topic(
        &db,
        domain,
        "synthetic-react-peer",
        "合成同名主题",
        &works[3..6],
    )
    .await;
    let third = seed_topic(
        &db,
        domain,
        "synthetic-react-third",
        "合成独立方向",
        &works[6..9],
    )
    .await;
    let conn = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision)VALUES($1,1)")
        .bind(conn)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref)VALUES($1,$2,1,'SYNTHETIC UI ONLY','openai-completions','http://127.0.0.1:1',true,$3)").bind(version).bind(conn).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)VALUES($1,$2,'SYNTHETIC UI ONLY','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts)VALUES($1,$2,8192,1000,30,1)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    let search_round = Uuid::new_v4();
    let detail_round = Uuid::new_v4();
    for (reference, kind) in [(search_round, "search"), (detail_round, "details")] {
        sqlx::query("INSERT INTO linggan_topic_map_collection_round(round_ref,domain_ref,scope_hash,purpose,kind,state,request_ref,last_reason)VALUES($1,$2,$3,'SYNTHETIC stopped round',$4,'stopped',$5,'synthetic_no_dispatch')").bind(reference).bind(domain).bind("a".repeat(64)).bind(kind).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    }
    let before: (i64, i64) = (
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    let directory = std::env::var("WEB_APP_PROOF_DIR").unwrap();
    fs::create_dir_all(&directory).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let application = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    fs::write(ProofPath::new(&directory).join("fixture.json"),json!({
        "origin":origin,"domain":domain,"modelConfigRef":config,"searchRound":search_round,"detailRound":detail_round,"domainTwo":second,"workRefs":works,"topicRef":first.topic_ref,"definitionRef":first.definition_ref,
        "peerTopicRef":peer.topic_ref,"peerDefinitionRef":peer.definition_ref,"thirdTopicRef":third.topic_ref,
        "workspaceKey":"synthetic-react-workspace","materials":[{"ref":works[0],"role":"support"},{"ref":works[1],"role":"challenge"},{"ref":works[2],"role":"boundary"}],
        "expectedDefinitionName":"合成同名主题","expectedDefinitionVersion":1,"missingWorkspaceKey":"synthetic-not-found"
    }).to_string()).unwrap();
    let status = std::process::Command::new(std::env::var("WEB_APP_PROOF_NODE").unwrap())
        .arg(
            ProofPath::new(env!("CARGO_MANIFEST_DIR"))
                .join("../web/test/e2e/topics-real-chain.mjs"),
        )
        .env("WEB_APP_PROOF_DIR", &directory)
        .status();
    server.abort();
    let _ = server.await;
    assert!(
        status.unwrap().success(),
        "Topic React browser proof failed"
    );
    let after: (i64, i64) = (
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
            .fetch_one(db.pool())
            .await
            .unwrap(),
    );
    assert_eq!(
        before, after,
        "page proof must not invoke models or create collection work orders"
    );
    // Browser commands must really persist once, including explicit replay after a lost response.
    let own:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_own_creator WHERE domain_ref=$1 AND author_external_id='synthetic-author-0'").bind(domain).fetch_one(db.pool()).await.unwrap();
    assert_eq!(own, 1);
    let alternatives: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_topic_map_alternative WHERE domain_ref=$1",
    )
    .bind(domain)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(alternatives, 1);
}
