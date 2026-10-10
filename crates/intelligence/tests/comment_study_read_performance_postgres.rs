//! Deterministic JIT regression guard through the public catalog reads, on synthetic data only.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_intelligence::comment_study_catalog::{
    CatalogSummaryQuery, CommentCatalogQuery, CommentDetailQuery, WorkCatalogQuery,
    read_catalog_summary, read_comment_catalog, read_comment_detail, read_work_catalog,
    refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use serde_json::json;

#[tokio::test]
#[ignore = "requires isolated pg_stat_statements from test-comment-study-read-performance-postgres.sh"]
async fn catalog_reads_skip_jit_and_restore_pool_defaults() {
    let database = fixture::proof_database("study_read_performance").await;
    sqlx::raw_sql(concat!(
        include_str!("../../../database/bootstrap/comment-study-001.sql"),
        "\n",
        include_str!("../../../database/migrations/0114_comment_study_effective_head.sql"),
        "\n",
        include_str!("../../../database/migrations/0107_comment_study_productization_schema.sql")
    ))
    .execute(database.pool())
    .await
    .unwrap();
    research_fixture::detail_with_author(&database, "jit-proof", "SYNTHETIC work", Some("creator"))
        .await;
    let source = research_fixture::comment_with_author(
        &database,
        "jit-proof",
        "comment",
        "SYNTHETIC 用户问题观察",
        Some("reader"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    let domain = ADHD_DOMAIN_REF.parse().unwrap();
    refresh_clean_cache(&database, domain, 200).await.unwrap();
    let work = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source)
    .fetch_one(database.pool())
    .await
    .unwrap();

    // Hold all four pool slots so every backend is configured. Any catalog statement would
    // compile JIT functions even with this tiny fixture unless its transaction opts out.
    let mut connections = Vec::new();
    for _ in 0..4 {
        let mut connection = database.pool().acquire().await.unwrap();
        sqlx::raw_sql("SET jit=on; SET jit_above_cost=0; SET jit_inline_above_cost=0; SET jit_optimize_above_cost=0;")
            .execute(&mut *connection).await.unwrap();
        connections.push(connection);
    }
    drop(connections);
    sqlx::query("SELECT public.pg_stat_statements_reset()")
        .execute(database.pool())
        .await
        .unwrap();
    let query: CommentCatalogQuery = serde_json::from_value(json!({"domain":domain})).unwrap();
    let page = read_comment_catalog(&database, &query).await.unwrap();
    let summary_query: CatalogSummaryQuery =
        serde_json::from_value(json!({"domain":domain})).unwrap();
    let summary = read_catalog_summary(&database, &summary_query)
        .await
        .unwrap();
    assert_eq!(page["summary"], summary["summary"]);
    assert_eq!(page["summary"]["displayableCommentCount"], 1);
    let works: WorkCatalogQuery = serde_json::from_value(json!({"domain":domain})).unwrap();
    assert_eq!(
        read_work_catalog(&database, &works).await.unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let detail_query = CommentDetailQuery {
        domain,
        work_ref: work,
        comment_external_id: "comment".into(),
    };
    read_comment_detail(&database, &detail_query).await.unwrap();
    let (calls, jit_functions): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(sum(calls),0)::bigint,COALESCE(sum(jit_functions),0)::bigint \
         FROM public.pg_stat_statements WHERE dbid=(SELECT oid FROM pg_database WHERE datname=current_database()) \
         AND query LIKE '%WITH latest AS MATERIALIZED%' AND query NOT LIKE '%pg_stat_statements%'",
    ).fetch_one(database.pool()).await.unwrap();
    assert_eq!(
        calls, 4,
        "must observe every public catalog projection, not an empty stats match"
    );
    assert_eq!(
        jit_functions, 0,
        "interactive catalog reads must not spend time compiling JIT"
    );
    // Exercise repeated searches past the generic-plan threshold; retained generic plans
    // can be much slower while still returning perfectly correct results.
    for index in 0..12 {
        let mut query = query.clone();
        query.q = Some(if index % 2 == 0 { "用户" } else { "absent" }.into());
        let result = read_comment_catalog(&database, &query).await.unwrap();
        assert_eq!(
            result["summary"]["displayableCommentCount"],
            if index % 2 == 0 { 1 } else { 0 }
        );
    }
    let mut connections = Vec::new();
    for _ in 0..4 {
        let mut connection = database.pool().acquire().await.unwrap();
        let default: String = sqlx::query_scalar("SHOW jit")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(
            default, "on",
            "transaction-local setting must not leak to the pool"
        );
        let prepared: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_prepared_statements WHERE statement LIKE '%WITH latest AS MATERIALIZED%' \
             AND statement LIKE '%qualified AS MATERIALIZED%' AND statement NOT LIKE '%pg_prepared_statements%'",
        ).fetch_one(&mut *connection).await.unwrap();
        assert_eq!(
            prepared, 0,
            "optional catalog filters must not retain a generic prepared plan"
        );
        connections.push(connection);
    }
    println!(
        "COMMENT-STUDY-READ-PERFORMANCE-358 catalog_calls={calls}, jit_functions={jit_functions}, pool_default=on"
    );
}
