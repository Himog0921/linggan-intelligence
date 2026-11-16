//! Built React + real Axum + disposable PostgreSQL, using the playbook's synthetic dataset.
use super::*;
use linggan_intelligence::opportunity::{
    CardRebuildMode, CommenterDigest, rebuild_opportunity_cards, sync_comment_mentions,
};
use std::{fs, path::Path};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "../../../../crates/intelligence/tests/support/opportunity_creator_dataset.rs"]
mod opportunity_creator_dataset;
#[allow(dead_code)]
#[path = "../../../../crates/intelligence/tests/support/opportunity_dataset.rs"]
mod opportunity_dataset;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "disposable PostgreSQL + built React + browser; scripts/test-web-app-postgres.sh"]
async fn opportunity_page_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_OPPORTUNITY_CARD").as_deref(),
        Ok("enabled")
    );
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &format!("opp_react_{}", Uuid::new_v4().simple()),
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let domain = Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap();
    opportunity_dataset::build(&db).await.unwrap();
    opportunity_creator_dataset::build_standard(&db)
        .await
        .unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/intelligence/tests/opportunity_metrics_postgres.rs"
    ));
    let seed = source
        .split_once("const SEED: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    sqlx::raw_sql(sqlx::AssertSqlSafe(seed))
        .execute(db.pool())
        .await
        .unwrap();
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::raw_sql(r#"
      CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE
      AS $$ SELECT timestamptz '2026-11-10T00:00:00Z' $$;
      ALTER TABLE linggan_opportunity_taxonomy_version_history DISABLE TRIGGER USER;
      UPDATE linggan_opportunity_taxonomy_version_history SET created_at='2026-11-01T00:00:00Z';
      ALTER TABLE linggan_opportunity_taxonomy_version_history ENABLE TRIGGER USER;
      UPDATE linggan_opportunity_problem SET created_at='2026-11-01T00:00:00Z';
      UPDATE linggan_opportunity_metric_definition SET created_at='2026-11-01T00:00:00Z';
      UPDATE linggan_opportunity_metric_definition_current SET switched_at='2026-11-01T00:00:00Z';
      UPDATE linggan_opportunity_comment_mention SET excluded_reason='marketing' WHERE comment_external_id='c-work-0-13';
      ALTER TABLE linggan_material_comment DISABLE TRIGGER USER;
      UPDATE linggan_material_comment SET body_text='SYNTHETIC 原声 '||comment_external_id,body_state='KNOWN'
      WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
      ALTER TABLE linggan_material_comment ENABLE TRIGGER USER;
      INSERT INTO linggan_comment_clean_cache(source_ref,cleaner_version,raw_sha256,research_text,clean_state,clean_reasons,created_at)
      SELECT material_ref,'comment-clean.v2',repeat('a',64),body_text,'direct','[]'::jsonb,'2026-11-09T00:00:00Z'
      FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
      INSERT INTO linggan_material_comment_engagement(material_ref,like_count,like_count_state,parser_version,created_at)
      SELECT material_ref,CASE comment_external_id WHEN 'c-work-0-1' THEN 40 WHEN 'c-work-0-2' THEN 30
        WHEN 'c-work-0-3' THEN 20 ELSE 10 END,'KNOWN','comment-like.v1','2026-11-09T00:00:00Z'
      FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
      CREATE FUNCTION synthetic_preserve_marketing() RETURNS trigger LANGUAGE plpgsql AS $$
      BEGIN IF OLD.excluded_reason='marketing' AND NEW.excluded_reason IS NULL AND
        NEW.comment_external_id IN ('c-work-0-13','creator-comment-c1-a-58') THEN RETURN NULL; END IF; RETURN NEW; END $$;
      CREATE TRIGGER synthetic_preserve_marketing BEFORE UPDATE ON linggan_opportunity_comment_mention
      FOR EACH ROW EXECUTE FUNCTION synthetic_preserve_marketing();
    "#).execute(db.pool()).await.unwrap();
    let report = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(report.cards_built, 3);
    let problems: Vec<Uuid> = sqlx::query_scalar(
        "SELECT md5('metric-problem:' || n)::uuid FROM generate_series(1,3) n ORDER BY n",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
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
    fs::write(
        Path::new(&directory).join("fixture.json"),
        json!({"origin":origin,"domain":domain,"problems":problems}).to_string(),
    )
    .unwrap();
    let status = std::process::Command::new(std::env::var("WEB_APP_PROOF_NODE").unwrap())
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../web/test/e2e/opportunity-real-chain.mjs"),
        )
        .env("WEB_APP_PROOF_DIR", &directory)
        .status()
        .unwrap();
    server.abort();
    let _ = server.await;
    assert!(status.success(), "opportunity browser proof failed");
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
}
