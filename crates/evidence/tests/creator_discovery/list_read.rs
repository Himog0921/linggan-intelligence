use super::*;
use linggan_evidence::creator_discovery::load_list;
use linggan_storage_postgres::Database;
use serde_json::Value;

pub async fn assert_equivalent(db: &Database, scope: &CreatorScope) {
    let full = load(db, scope).await.unwrap();
    let expected = aggregate(&full, scope);
    let mut actual = load_list(db, scope).await.unwrap();
    // Independent read-only transactions have their own read timestamps. Every
    // business field, source, qualification, support fragment and count must match.
    for key in ["asOf", "displayAsOf"] {
        actual[key] = expected[key].clone();
    }
    assert_eq!(actual, expected, "list parity for {scope:?}");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn paged_unjudged_creators_expand_only_visible_dossiers_without_losing_counts() {
    let db = fixture::proof_database("creator_list_bounded_dossiers").await;
    for author in 0..31 {
        for work in 0..4 {
            let id = format!("bounded-creator-{author:02}-{work}");
            let package = fixture::submit_package(&db, "content_detail", json!({"contentExternalId":id}),
                json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":format!("标题 {author} {work}"),"bodyText":format!("独立的完整正文 {author} {work}"),"authorId":format!("bounded-author-{author:02}"),"authorName":format!("作者 {author:02}"),"likes":author+work}})).await;
            let reference: Uuid = sqlx::query_scalar(
                "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
            )
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
            sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
                .bind(Uuid::new_v4()).bind(reference).bind(D).bind(package).execute(db.pool()).await.unwrap();
        }
    }
    let q: CreatorScope = serde_json::from_value(json!({"domain":D,"pageSize":25})).unwrap();
    assert_equivalent(&db, &q).await;
    let page = load_list(&db, &q).await.unwrap();
    assert_eq!(page["total"], 31);
    assert_eq!(page["stats"]["works"], 124);
    assert_eq!(page["stats"]["relatedAuthors"], 0);
    assert_eq!(page["stats"]["outsideViral"], Value::Null);
    let q2 = CreatorScope {
        page: 1,
        ..q.clone()
    };
    assert_equivalent(&db, &q2).await;
    let second = load_list(&db, &q2).await.unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 6);
    assert_eq!(second["stats"], page["stats"]);
    let first_keys: Vec<_> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| &item["creatorKey"])
        .collect();
    assert!(
        second["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| !first_keys.contains(&&item["creatorKey"]))
    );

    sqlx::query("SELECT public.pg_stat_statements_reset()")
        .execute(db.pool())
        .await
        .unwrap();
    let _ = load_list(&db, &q).await.unwrap();
    let inventory_batches: i64 = sqlx::query_scalar("SELECT coalesce(sum(calls),0)::bigint FROM public.pg_stat_statements WHERE query LIKE '%WITH origins AS (%SELECT DISTINCT ON (o.slot_key)%'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        inventory_batches, 1,
        "75 visible representatives, not all 124 works, may expand media inventories"
    );
    let current_reads: i64 = sqlx::query_scalar("SELECT coalesce(sum(calls),0)::bigint FROM public.pg_stat_statements WHERE query LIKE 'WITH requested_work AS MATERIALIZED (SELECT * FROM linggan_material_content WHERE public_ref=ANY%'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        current_reads, 1,
        "statistics and canonical representative dossiers must share the same transaction-local Current read"
    );
    for extra in [
        json!({"sort":"high_likes","pageSize":25}),
        json!({"searchMode":"author","query":"作者 01","pageSize":25}),
        json!({"searchMode":"work","query":"完整正文","pageSize":25}),
        json!({"observation":"outside","pageSize":25}),
    ] {
        let mut query = extra;
        query["domain"] = json!(D);
        let scope: CreatorScope = serde_json::from_value(query).unwrap();
        assert_equivalent(&db, &scope).await;
    }
    let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE schemaname=current_schema() AND indexname IN ('linggan_media_download_attempt_observation_idx','linggan_material_lane_observation_work_lane_idx')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        indexes, 2,
        "the isolated full schema must apply the performance migration"
    );
}
