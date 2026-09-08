//! Real PostgreSQL and synthetic local model: scope size is independent of packet size.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_evidence::comment_research_read::*;
use linggan_intelligence::{
    comment_daily::*,
    comment_intelligence::{Prepare, ResearchScope, Run, prepare, run},
    comment_research_projection::*,
    model_invocation::*,
    model_secrets::*,
    model_settings::*,
    pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
use daily_fixture::*;
fn all() -> ResearchScope {
    ResearchScope {
        domain: Some(Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap()),
        from: Some("2020-01-01T00:00:00Z".into()),
        to: Some("2099-01-01T00:00:00Z".into()),
        ..Default::default()
    }
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn three_hundred_selected_comments_freeze_exact_scope_without_model_dispatch() {
    let db = fixture::proof_database("preflight_300").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "scope-work", "合成作品").await;
    let mut refs = vec![];
    for i in 0..300 {
        refs.push(
            source(
                &db,
                "scope-work",
                &format!("c{i}"),
                "孩子每天写作业拖延，该怎么帮助他",
            )
            .await,
        );
    }
    let p = prepare(
        &db,
        &Prepare {
            scope: all(),
            source_refs: Some(refs.clone()),
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(p["count"], 300);
    assert_eq!(p["preflight"]["total"], 300);
    assert_eq!(p["preflight"]["counts"]["newAnalysis"], 300);
    assert_eq!(p["sourceRefs"].as_array().unwrap().len(), 300);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_daily_batch")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let batch = run(
        &db,
        &Run {
            prepare_ref: p["prepareRef"].as_str().unwrap().parse().unwrap(),
            config_ref: config,
            token_limit: 100000,
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert!(batch.is_object());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_daily_item")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        300
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_daily_packet")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let error = prepare(
        &db,
        &Prepare {
            scope: all(),
            source_refs: Some(vec![Uuid::new_v4(); 3001]),
            reanalyze: false,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, ModelError::SelectionLimit));
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn preflight_accounts_noise_reuse_and_blocks_changed_context() {
    let db = fixture::proof_database("preflight_reuse").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "reuse-work", "合成作品").await;
    let a = source(&db, "reuse-work", "a", "孩子每天写作业拖延，需要怎样帮助他").await;
    let noise = source(&db, "reuse-work", "noise", "😂😂").await;
    selected(&db, config, vec![a], 100000).await;
    assert!(tick(&db).await);
    let p = prepare(
        &db,
        &Prepare {
            scope: all(),
            source_refs: Some(vec![a, noise]),
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(p["preflight"]["counts"]["reusable"], 1);
    assert_eq!(p["preflight"]["counts"]["dropped"], 1);
    assert_eq!(
        p["preflight"]["counts"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    assert_eq!(p["preflight"]["estimatedCalls"]["max"], 0);
    // The actual title is included in the frozen semantic context; changing it invalidates consent.
    research_fixture::detail(&db, "reuse-work", "已变化的合成作品标题").await;
    let err = run(
        &db,
        &Run {
            prepare_ref: p["prepareRef"].as_str().unwrap().parse().unwrap(),
            config_ref: config,
            token_limit: 100000,
            reanalyze: false,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ModelError::Conflict));
    server.kill().await.unwrap();
}
