//! Isolated catalog proofs. These exercise real SQL, not front-end fixture filtering.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::ai_foundation::comment_research_overview;
use linggan_intelligence::comment_study_catalog::{
    CatalogStudyState, CatalogSummaryQuery, CatalogVoiceRole, CommentCatalogQuery,
    CommentDetailQuery, CommentHistoryQuery, StudyCatalogError, WordCloudQuery,
    read_catalog_summary, read_comment_catalog, read_comment_detail, read_comment_history,
    read_comment_versions, read_word_cloud, read_work_catalog, refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::{
    ADHD_DOMAIN_REF, eligible_sources, eligible_sources_for_works,
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::json;
use std::collections::BTreeSet;
use uuid::Uuid;

async fn database(name: &str) -> Database {
    proof_database(name).await
}

fn query() -> CommentCatalogQuery {
    serde_json::from_value(json!({"domain": ADHD_DOMAIN_REF})).unwrap()
}

async fn add_comment(
    db: &Database,
    note: &str,
    id: &str,
    text: &str,
    author: Option<&str>,
) -> Uuid {
    comment_with_author(db, note, id, text, author, "2026-09-21T08:00:00Z").await
}

async fn cache(db: &Database) {
    // Bounded test helper, never an unbounded production backfill loop.
    for _ in 0..4 {
        let report = refresh_clean_cache(db, query().domain, 200).await.unwrap();
        if report.examined_count == 0 {
            return;
        }
    }
    panic!("synthetic test cache did not finish within four bounded passes");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_paginates_more_than_100_comments_without_a_run_and_searches_literally() {
    let db = database("catalog_read_pagination").await;
    detail_with_author(&db, "catalog-page", "SYNTHETIC 分页作品", Some("creator")).await;
    for index in 0..123 {
        add_comment(
            &db,
            "catalog-page",
            &format!("c-{index:03}"),
            &format!("SYNTHETIC 药物观察 {index}；仅为测试"),
            Some("reader"),
        )
        .await;
    }
    add_comment(
        &db,
        "catalog-page",
        "literal",
        r"SYNTHETIC 药物 50%_\ 原样匹配",
        Some("reader"),
    )
    .await;
    cache(&db).await;
    let mut request = query();
    request.limit = Some(31);
    let mut seen = BTreeSet::new();
    let mut original_as_of = None;
    for page_number in 0..8 {
        let response = read_comment_catalog(&db, &request).await.unwrap();
        assert_eq!(response["summary"]["displayableCommentCount"], 124);
        let as_of = response["page"]["asOf"].as_str().unwrap().to_owned();
        if let Some(expected) = &original_as_of {
            assert_eq!(&as_of, expected);
        } else {
            original_as_of = Some(as_of);
        }
        assert_eq!(response["indexCoverage"]["indexedCount"], 124);
        for row in response["items"].as_array().unwrap() {
            assert_eq!(row["observationRole"], "primary");
            assert!(
                seen.insert(
                    row["commentKey"]["commentExternalId"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                )
            );
            assert!(row["latestStudy"].is_null());
            assert!(row["likeCount"].is_null());
        }
        request.cursor = response["page"]["nextCursor"].as_str().map(str::to_owned);
        if request.cursor.is_none() {
            break;
        }
        assert!(page_number < 7, "pagination did not terminate");
        request.limit = Some(27); // Page size is deliberately not part of scopeHash.
    }
    assert_eq!(seen.len(), 124);
    request.cursor = None;
    request.q = Some(r"50%_\".to_owned());
    let result = read_comment_catalog(&db, &request).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["items"][0]["commentKey"]["commentExternalId"],
        "literal"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_request")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "directory reads must never create a Run");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_likes_only_use_projections_available_at_the_requested_time() {
    let db = database("catalog_like_projection_time").await;
    detail_with_author(&db, "catalog-likes", "SYNTHETIC 点赞", Some("creator")).await;
    let first = add_comment(
        &db,
        "catalog-likes",
        "same",
        "SYNTHETIC 评论内容",
        Some("reader"),
    )
    .await;
    let second = comment_with_author(
        &db,
        "catalog-likes",
        "same",
        "SYNTHETIC 评论内容",
        Some("reader"),
        "2026-09-22T08:00:00Z",
    )
    .await;
    cache(&db).await;

    sqlx::query("ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    for (source, likes) in [(first, 11_i64), (second, 42_i64)] {
        sqlx::query("INSERT INTO linggan_material_comment_engagement(material_ref,like_count,like_count_state,parser_version,created_at) VALUES($1,$2,'KNOWN','comment-like.v1',clock_timestamp()+interval '1 day') ON CONFLICT(material_ref) DO UPDATE SET like_count=$2,like_count_state='KNOWN',created_at=clock_timestamp()+interval '1 day'")
            .bind(source).bind(likes).execute(db.pool()).await.unwrap();
    }
    sqlx::query("ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    let unknown = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(
        unknown["items"][0]["likeCount"].is_null(),
        "later projections must not be visible"
    );

    sqlx::query("ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_material_comment_engagement SET created_at=clock_timestamp()-interval '1 day' WHERE material_ref=$1")
        .bind(first).execute(db.pool()).await.unwrap();
    sqlx::query("ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    let older_known = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(older_known["items"][0]["likeCount"], 11);

    sqlx::query("ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_material_comment_engagement SET created_at=clock_timestamp()-interval '1 day' WHERE material_ref=$1")
        .bind(second).execute(db.pool()).await.unwrap();
    sqlx::query("ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    let newest_known = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(newest_known["items"][0]["likeCount"], 42);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn malformed_observations_are_not_selected_by_catalog_or_study() {
    let db = database("catalog_bad_observation_time").await;
    detail_with_author(&db, "catalog-bad-time", "SYNTHETIC 时间", Some("creator")).await;
    let valid = add_comment(
        &db,
        "catalog-bad-time",
        "same",
        "SYNTHETIC valid voice",
        Some("reader"),
    )
    .await;
    cache(&db).await;
    let invalid = comment_with_author(
        &db,
        "catalog-bad-time",
        "same",
        "SYNTHETIC invalid revision",
        Some("reader"),
        "2026-09-22T08:00:00Z",
    )
    .await;
    let invalid_only = add_comment(
        &db,
        "catalog-bad-time",
        "invalid-only",
        "SYNTHETIC invalid only",
        Some("reader"),
    )
    .await;
    sqlx::raw_sql("ALTER TABLE linggan_material_comment DISABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE linggan_material_comment SET observed_at='not-a-time' WHERE material_ref=ANY($1)",
    )
    .bind(vec![invalid, invalid_only])
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(
        "ALTER TABLE linggan_material_comment ENABLE TRIGGER USER; \
        ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER",
    )
    .execute(db.pool())
    .await
    .unwrap();
    for (source, likes) in [(valid, 11_i64), (invalid, 999_i64)] {
        sqlx::query("INSERT INTO linggan_material_comment_engagement(material_ref,like_count,like_count_state,parser_version,created_at) \
            VALUES($1,$2,'KNOWN','comment-like.v1',clock_timestamp()) ON CONFLICT(material_ref) DO UPDATE \
            SET like_count=$2,like_count_state='KNOWN',created_at=clock_timestamp()")
            .bind(source).bind(likes).execute(db.pool()).await.unwrap();
    }
    sqlx::raw_sql("ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();

    let refresh = refresh_clean_cache(&db, query().domain, 200).await.unwrap();
    assert_eq!(
        refresh.examined_count, 0,
        "bad observations are not cleaning candidates"
    );
    assert_eq!(refresh.unparseable_observation_count, 2);
    let catalog = read_comment_catalog(&db, &query()).await.unwrap();
    let items = catalog["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["sourceRef"], valid.to_string());
    assert_eq!(items[0]["commentText"], "SYNTHETIC valid voice");
    assert_eq!(items[0]["likeCount"], 11);
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(valid)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let dossier_comments = linggan_evidence::read_dossier_comments(&db, work, None)
        .await
        .expect("shared current comments must tolerate invalid observation times");
    assert!(
        dossier_comments["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["sourceRef"] == valid.to_string())
    );
    assert!(
        !dossier_comments
            .to_string()
            .contains("SYNTHETIC invalid revision")
    );
    let detail_query = CommentDetailQuery {
        domain: query().domain,
        work_ref: work,
        comment_external_id: "same".to_owned(),
    };
    let detail = read_comment_detail(&db, &detail_query).await.unwrap();
    assert_eq!(
        detail["materialVersions"]["totalCount"], 2,
        "history retains observation metadata"
    );
    let versions = detail["materialVersions"]["items"].as_array().unwrap();
    let malformed = versions
        .iter()
        .find(|v| v["sourceRef"] == invalid.to_string())
        .unwrap();
    assert!(malformed["observedAt"].is_null());
    assert_eq!(malformed["isCurrent"], false);
    let as_of: String = sqlx::query_scalar(
        "SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', \
        'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let invalid_detail = read_comment_detail(
        &db,
        &CommentDetailQuery {
            domain: query().domain,
            work_ref: work,
            comment_external_id: "invalid-only".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(invalid_detail["source"]["sourceState"], "unknown");
    assert!(invalid_detail["source"]["sourceRef"].is_null());
    assert!(invalid_detail["comment"].is_null());
    assert_eq!(invalid_detail["materialVersions"]["totalCount"], 1);
    assert!(
        !invalid_detail
            .to_string()
            .contains("SYNTHETIC invalid only")
    );
    let history_query = CommentHistoryQuery {
        domain: query().domain,
        work_ref: work,
        comment_external_id: "invalid-only".to_owned(),
        cursor: None,
        limit: Some(5),
    };
    let versions = read_comment_versions(&db, &history_query).await.unwrap();
    assert_eq!(versions["totalCount"], 1);
    assert!(versions["items"][0]["observedAt"].is_null());
    assert_eq!(
        read_comment_history(&db, &history_query).await.unwrap()["totalCount"],
        0
    );
    let foreign_query = CommentDetailQuery {
        domain: Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0002),
        work_ref: work,
        comment_external_id: "invalid-only".to_owned(),
    };
    assert!(matches!(
        read_comment_detail(&db, &foreign_query).await,
        Err(StudyCatalogError::ResourceNotFound)
    ));
    let preview =
        linggan_intelligence::comment_study_source::preview_sources(&db, query().domain, &as_of)
            .await
            .unwrap();
    assert_eq!(
        (preview.total_comment_count, preview.eligible_comment_count),
        (2, 1)
    );
    assert_eq!(preview.excluded_counts.body_unavailable, 1);
    assert_eq!(preview.unparseable_observation_count, 2);
    let sources = eligible_sources(&db, query().domain, &as_of, 100)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_ref, valid);
    let mut tx = db.pool().begin().await.unwrap();
    let role = linggan_intelligence::comment_study_source::StudySourceRole::Primary;
    let selected = linggan_intelligence::comment_study_source::selected_source_in(
        &mut tx,
        query().domain,
        work,
        valid,
        role,
        &as_of,
    )
    .await
    .unwrap();
    assert!(selected.is_some());
    let rejected = linggan_intelligence::comment_study_source::selected_source_in(
        &mut tx,
        query().domain,
        work,
        invalid,
        role,
        &as_of,
    )
    .await
    .unwrap();
    assert!(rejected.is_none());
    assert!(
        linggan_evidence::read_creation_comment_in(&mut tx, invalid)
            .await
            .unwrap()
            .is_none()
    );
    tx.rollback().await.unwrap();

    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'invalid-only','synthetic restriction')")
        .bind(work).execute(db.pool()).await.unwrap();
    let restricted_detail = read_comment_detail(
        &db,
        &CommentDetailQuery {
            domain: query().domain,
            work_ref: work,
            comment_external_id: "invalid-only".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(restricted_detail["source"]["sourceState"], "restricted");
    assert!(restricted_detail["source"]["sourceRef"].is_null());
    assert!(restricted_detail["comment"].is_null());

    let reply = research_fixture::reply_with_author(
        &db,
        "catalog-bad-time",
        "reply",
        "same",
        "SYNTHETIC valid reply",
        Some("reader-two"),
        "2026-09-23T08:00:00Z",
    )
    .await;
    cache(&db).await;
    let reply_detail = read_comment_detail(
        &db,
        &CommentDetailQuery {
            domain: query().domain,
            work_ref: work,
            comment_external_id: "reply".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        reply_detail["parentContext"]["sourceRef"],
        valid.to_string()
    );
    assert_eq!(
        reply_detail["parentContext"]["commentText"],
        "SYNTHETIC valid voice"
    );
    let mut tx = db.pool().begin().await.unwrap();
    let creation_reply = linggan_evidence::read_creation_comment_in(&mut tx, reply)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        creation_reply["parentComment"]["sourceRef"],
        valid.to_string()
    );
    tx.rollback().await.unwrap();
    let sources = eligible_sources(&db, query().domain, &as_of, 100)
        .await
        .unwrap();
    assert!(
        sources.iter().all(|source| source.source_ref != reply),
        "a source created after as_of remains invisible"
    );

    detail_with_author(
        &db,
        "catalog-bad-time",
        "SYNTHETIC bad detail",
        Some("bad-author"),
    )
    .await;
    sqlx::raw_sql("ALTER TABLE linggan_material_content_detail DISABLE TRIGGER USER; \
        UPDATE linggan_material_content_detail SET observed_at='not-a-time' WHERE author_external_id='bad-author'; \
        ALTER TABLE linggan_material_content_detail ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    let catalog = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(
        catalog["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["sourceRef"] == valid.to_string())
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_and_study_sources_share_the_author_known_at_the_requested_time() {
    let db = database("catalog_author_projection_time").await;
    detail_with_author(&db, "catalog-author", "SYNTHETIC 作者时点", Some("creator")).await;
    let source_ref = add_comment(
        &db,
        "catalog-author",
        "same",
        "SYNTHETIC 可研究评论",
        Some("reader"),
    )
    .await;
    cache(&db).await;
    detail_with_author(
        &db,
        "catalog-author",
        "SYNTHETIC 新作者观察",
        Some("reader"),
    )
    .await;
    sqlx::raw_sql("ALTER TABLE linggan_material_content_detail DISABLE TRIGGER USER; \
        UPDATE linggan_material_content_detail SET created_at=clock_timestamp()+interval '1 day', \
          observed_at=to_char((clock_timestamp()+interval '1 hour') AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') \
        WHERE author_external_id='reader'; \
        ALTER TABLE linggan_material_content_detail ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
        WHERE platform='xhs' AND content_external_id='catalog-author'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let now: String = sqlx::query_scalar(
        "SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', \
        'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let later: String = sqlx::query_scalar(
        "SELECT to_char((clock_timestamp()+interval '2 days') AT TIME ZONE 'UTC', \
        'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    let catalog = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(catalog["items"][0]["voiceRole"], "reader");
    let candidates = eligible_sources(&db, query().domain, &now, 100)
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let mut tx = db.pool().begin().await.unwrap();
    let role = linggan_intelligence::comment_study_source::StudySourceRole::Primary;
    let before = linggan_intelligence::comment_study_source::selected_source_in(
        &mut tx,
        query().domain,
        work,
        source_ref,
        role,
        &now,
    )
    .await
    .unwrap();
    assert!(before.is_some());
    let after = linggan_intelligence::comment_study_source::selected_source_in(
        &mut tx,
        query().domain,
        work,
        source_ref,
        role,
        &later,
    )
    .await
    .unwrap();
    assert!(
        after.is_none(),
        "the same source becomes a creator self-comment after the later observation"
    );
    tx.rollback().await.unwrap();
    assert!(
        eligible_sources(&db, query().domain, &later, 100)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
        AS $$ SELECT clock_timestamp()+interval '2 days' $$",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let mut creator = query();
    creator.voice_role = CatalogVoiceRole::Creator;
    let catalog_after = read_comment_catalog(&db, &creator).await.unwrap();
    assert_eq!(catalog_after["items"][0]["voiceRole"], "creator");
    assert_eq!(
        catalog_after["items"][0]["studyEligibility"]["eligible"],
        false
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_distinguishes_visibility_eligibility_and_pending_index() {
    let db = database("catalog_read_voices").await;
    detail_with_author(&db, "catalog-voices", "SYNTHETIC 声音角色", Some("creator")).await;
    add_comment(
        &db,
        "catalog-voices",
        "reader",
        "SYNTHETIC 用户原声",
        Some("reader"),
    )
    .await;
    add_comment(
        &db,
        "catalog-voices",
        "creator",
        "SYNTHETIC 作者说明",
        Some("creator"),
    )
    .await;
    add_comment(
        &db,
        "catalog-voices",
        "unknown",
        "SYNTHETIC 身份未知但有语义",
        None,
    )
    .await;
    add_comment(&db, "catalog-voices", "emoji", "😀😀", Some("reader")).await;
    let before = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(before["items"].as_array().unwrap().is_empty());
    assert_eq!(before["indexCoverage"]["state"], "partial");
    assert_eq!(before["indexCoverage"]["pendingCount"], 3); // reader + unknown + emoji
    cache(&db).await;
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["indexCoverage"]["indexedCount"], 3); // dropped cache still indexed
    let unknown = result["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["voiceRole"] == "unknown")
        .unwrap();
    assert_eq!(unknown["studyEligibility"]["eligible"], false);
    assert_eq!(
        unknown["studyEligibility"]["reasons"],
        serde_json::json!(["commentAuthorUnknown"])
    );
    let summary_query: CatalogSummaryQuery =
        serde_json::from_value(json!({"domain":ADHD_DOMAIN_REF})).unwrap();
    let summary = read_catalog_summary(&db, &summary_query).await.unwrap();
    assert_eq!(summary["summary"]["unknownIdentityCount"], 1);
    let mut creator = query();
    creator.voice_role = CatalogVoiceRole::Creator;
    let result = read_comment_catalog(&db, &creator).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["studyEligibility"]["eligible"], false);
    add_comment(
        &db,
        "catalog-voices",
        "new",
        "SYNTHETIC 尚未清洗",
        Some("reader"),
    )
    .await;
    let mut search = query();
    search.q = Some("用户".to_owned());
    let result = read_comment_catalog(&db, &search).await.unwrap();
    assert_eq!(result["indexCoverage"]["state"], "partial");
    assert!(result["indexCoverage"]["pendingCount"].is_null());
    // A text query cannot claim whether the unindexed source would match after cleaning.
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn newer_unknown_or_restricted_source_never_resurrects_old_cached_text() {
    let db = database("catalog_read_restriction").await;
    detail_with_author(&db, "catalog-safe", "SYNTHETIC 来源边界", Some("creator")).await;
    let old = add_comment(
        &db,
        "catalog-safe",
        "changed",
        "SYNTHETIC 旧可读正文",
        Some("reader"),
    )
    .await;
    let restricted = add_comment(
        &db,
        "catalog-safe",
        "restricted",
        "SYNTHETIC 后来受限正文",
        Some("reader"),
    )
    .await;
    cache(&db).await;
    submit_package_at(&db, "comments", json!({"contentExternalId":"catalog-safe"}), json!({
        "kind":"comment", "sourceObject":{"platform":"xhs","type":"content","externalId":"catalog-safe"},
        "payload":{"commentId":"changed","noteId":"catalog-safe","authorId":"reader"}
    }), "2026-09-22T08:00:00Z").await;
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
        SELECT content_public_ref,comment_external_id,'synthetic restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(restricted).execute(db.pool()).await.unwrap();
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(result["items"].as_array().unwrap().is_empty());
    let retained: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_clean_cache WHERE source_ref=$1")
            .bind(old)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        retained, 1,
        "read restrictions must not delete retained cache or raw history"
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn cursor_rejects_changed_filters_and_summary_does_not_count_only_the_first_page() {
    let db = database("catalog_read_cursor_scope").await;
    detail_with_author(&db, "catalog-cursor", "SYNTHETIC 游标", Some("creator")).await;
    for index in 0..3 {
        add_comment(
            &db,
            "catalog-cursor",
            &format!("cursor-{index}"),
            "SYNTHETIC 搜索证据",
            Some("reader"),
        )
        .await;
    }
    cache(&db).await;
    let mut request = query();
    request.limit = Some(1);
    let first = read_comment_catalog(&db, &request).await.unwrap();
    request.cursor = first["page"]["nextCursor"].as_str().map(str::to_owned);
    request.q = Some("搜索".to_owned());
    assert!(matches!(
        read_comment_catalog(&db, &request).await,
        Err(StudyCatalogError::CursorScopeMismatch)
    ));
    let summary: CatalogSummaryQuery =
        serde_json::from_value(json!({"domain":ADHD_DOMAIN_REF})).unwrap();
    let result = read_catalog_summary(&db, &summary).await.unwrap();
    assert_eq!(result["summary"]["displayableCommentCount"], 3);
    assert_eq!(result["summary"]["eligibleCommentCount"], 3);
    request.cursor = None;
    request.q = None;
    request.study_state = CatalogStudyState::NeverStudied;
    assert_eq!(
        read_comment_catalog(&db, &request).await.unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated disposable PostgreSQL only; validates real source gate and stable comment identity"]
async fn domain_phrase_frequency_counts_distinct_eligible_comments_and_retains_source_keys() {
    let db = database("word_cloud_eligible_gate").await;
    // Producer packages require an explicit UTC timestamp, not PostgreSQL's display format.
    let observed: String = sqlx::query_scalar(
        r#"SELECT to_char((scope_001_now() - interval '2 days') AT TIME ZONE 'UTC',
                         'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    detail_with_author(&db, "word-a", "SYNTHETIC 第一篇", Some("creator")).await;
    detail_with_author(&db, "word-b", "SYNTHETIC 第二篇", Some("creator")).await;
    for index in 0..5 {
        let note = if index < 3 { "word-a" } else { "word-b" };
        comment_with_author(
            &db,
            note,
            &format!("c-{index}"),
            "作业拖拉，作业拖拉；每天还是作业拖拉",
            Some("reader"),
            &observed,
        )
        .await;
    }
    // The creator's own comment must never contribute to reader-language frequency.
    comment_with_author(
        &db,
        "word-a",
        "creator-voice",
        "作业拖拉",
        Some("creator"),
        &observed,
    )
    .await;
    cache(&db).await;
    let domain = query().domain;
    let read = read_word_cloud(
        &db,
        &WordCloudQuery {
            domain,
            days: Some(7),
        },
    )
    .await
    .unwrap();
    assert_eq!(read["eligibleCommentCount"], 5);
    assert_eq!(read["processedCommentCount"], 5);
    assert_eq!(read["truncated"], false);
    let words = read["items"].as_array().unwrap();
    let phrase = words
        .iter()
        .find(|item| item["text"] == "作业拖拉")
        .unwrap();
    assert_eq!(phrase["count"], 5);
    assert_eq!(phrase["workCount"], 2);
    assert_eq!(phrase["category"], "scenario");
    assert_eq!(phrase["examples"].as_array().unwrap().len(), 3);
    assert!(phrase["examples"][0].get("commentText").is_none());
    assert!(
        read_word_cloud(
            &db,
            &WordCloudQuery {
                domain,
                days: Some(30)
            }
        )
        .await
        .is_err()
    );
}

/// 只在作者主页上出现过的作品（作者语境）及其评论，不进评论研究的领域级列表与统计：
/// 评论目录、词云、作品目录、研究候选、总览的语料摘要与观察序列。
/// 按作品引用读取（指定了作品）不受影响。
#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn creator_context_comments_stay_out_of_the_domain_catalog_cloud_works_candidates_and_overview()
 {
    let db = database("catalog_creator_context").await;
    // Producer packages require an explicit UTC timestamp, not PostgreSQL's display format.
    let observed: String = sqlx::query_scalar(
        r#"SELECT to_char((scope_001_now() - interval '2 days') AT TIME ZONE 'UTC',
                         'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')"#,
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    for note in ["ctx-research", "ctx-context"] {
        detail_with_author(&db, note, &format!("SYNTHETIC {note}"), Some("creator")).await;
        for index in 0..2 {
            comment_with_author(
                &db,
                note,
                &format!("{note}-c{index}"),
                "作业拖拉，作业拖拉；每天还是作业拖拉",
                Some("reader"),
                &observed,
            )
            .await;
        }
    }
    let domain = query().domain;
    let context_work = fixture::make_creator_context_only(&db, domain, "ctx-context").await;
    let research_work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id='ctx-research'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    cache(&db).await;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let only_research = BTreeSet::from([research_work.to_string()]);

    // 评论目录与它的可显示数。
    let list = read_comment_catalog(&db, &query()).await.unwrap();
    let listed: BTreeSet<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["commentKey"]["workRef"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(listed, only_research, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert_eq!(list["summary"]["displayableCommentCount"], 2);

    // 词云：只数领域研究作品的评论。
    let cloud = read_word_cloud(
        &db,
        &WordCloudQuery {
            domain,
            days: Some(7),
        },
    )
    .await
    .unwrap();
    assert_eq!(cloud["eligibleCommentCount"], 2, "{cloud}");
    let phrase = cloud["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["text"] == "作业拖拉")
        .unwrap();
    assert_eq!(
        (phrase["count"].clone(), phrase["workCount"].clone()),
        (json!(2), json!(1))
    );

    // 作品目录。
    let works = read_work_catalog(
        &db,
        &serde_json::from_value(json!({"domain": ADHD_DOMAIN_REF})).unwrap(),
    )
    .await
    .unwrap();
    let work_refs: BTreeSet<String> = works["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["workRef"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(work_refs, only_research, "{works}");

    // 研究候选：领域级枚举只给研究作品的评论。
    let sources = eligible_sources(&db, domain, &as_of, 100).await.unwrap();
    assert_eq!(sources.len(), 2);
    assert!(
        sources
            .iter()
            .all(|source| source.content_public_ref == research_work)
    );

    // 总览：语料摘要与 56 天观察序列都不含作者语境作品的评论。
    let overview = comment_research_overview::read(&db, domain).await.unwrap();
    assert_eq!(
        overview["corpusSummary"]["displayableCommentCount"], 2,
        "{overview}"
    );
    let observed_comments: i64 = overview["observationSeries"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| point["newObservedCommentCount"].as_i64().unwrap())
        .sum();
    assert_eq!(observed_comments, 2, "{overview}");

    // 指定了作品（按引用）时不过滤：同一批评论照常读得到、也能被选作研究来源。
    let mut by_work = query();
    by_work.work_ref = Some(context_work);
    let one = read_comment_catalog(&db, &by_work).await.unwrap();
    assert_eq!(one["items"].as_array().unwrap().len(), 2, "{one}");
    let selected = eligible_sources_for_works(&db, domain, &as_of, &[context_work], 100)
        .await
        .unwrap();
    assert_eq!(selected.len(), 2);
}

/// 一条「作者语境」用途的用量行（作品另有研究用途的行）：走「准入复用」这条合法的写入形态。
async fn add_creator_context_usage(db: &Database, domain: Uuid, work: Uuid, role: &str) {
    let package: Uuid = sqlx::query_scalar(
        "SELECT first_package_ref FROM linggan_material_content WHERE public_ref=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let target = Uuid::new_v4();
    let request = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,source) VALUES($1,'xhs','keyword',$2,'manual')",
    )
    .bind(target)
    .bind(format!("synthetic-context-usage-{work}"))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_acquisition_request(request_ref,target_ref,lane,purpose,requested_by,domain_ref,observation_role) VALUES($1,$2,'patrol','synthetic creator context usage proof','person',$3,$4)",
    )
    .bind(request)
    .bind(target)
    .bind(domain)
    .bind(role)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_admission_decision(decision_ref,request_ref,outcome,reason_code) VALUES($1,$2,'reuse','synthetic_creator_context_usage_proof')",
    )
    .bind(Uuid::new_v4())
    .bind(request)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,request_ref,package_ref,use_kind) VALUES($1,$2,$3,$4,'admission_reuse',$5,$6,'creator_context')",
    )
    .bind(Uuid::new_v4())
    .bind(work)
    .bind(domain)
    .bind(role)
    .bind(request)
    .bind(package)
    .execute(db.pool())
    .await
    .unwrap();
}

/// 一篇作品在领域里的「观察角色」只看研究用途的那一行：研究用途是「参照」、
/// 同一作品另有一行作者语境的「主研究」时，仍然是「参照」。
/// 只有作者语境行的作品没有研究用途可看，详情回退到那一行本来的角色（「主研究」），
/// 这样按作品打开的详情仍能做评论研究；领域目录照旧不列它。
#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn the_observation_role_of_a_work_ignores_creator_context_usage_rows() {
    let db = database("catalog_role_creator_context").await;
    detail_with_author(&db, "role-work", "SYNTHETIC 角色作品", Some("creator")).await;
    add_comment(
        &db,
        "role-work",
        "role-c1",
        "SYNTHETIC 评论",
        Some("reader"),
    )
    .await;
    detail_with_author(
        &db,
        "context-only-work",
        "SYNTHETIC 仅语境作品",
        Some("context-only"),
    )
    .await;
    add_comment(
        &db,
        "context-only-work",
        "context-only-c1",
        "SYNTHETIC 评论",
        Some("reader"),
    )
    .await;
    let domain = query().domain;
    fixture::set_usage_role(&db, domain, "role-work", "reference").await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id='role-work'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    add_creator_context_usage(&db, domain, work, "primary").await;
    fixture::make_creator_context_only(&db, domain, "context-only-work").await;
    let context_work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id='context-only-work'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    cache(&db).await;
    let list = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 1, "{list}");
    assert_eq!(list["items"][0]["observationRole"], "reference", "{list}");
    // 按作品打开单条评论详情也走同一判定：作者语境的「主研究」行同样不算数。
    let detail = read_comment_detail(
        &db,
        &CommentDetailQuery {
            domain,
            work_ref: work,
            comment_external_id: "role-c1".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        detail["comment"]["observationRole"], "reference",
        "{detail}"
    );
    // 只有作者语境行的作品：详情回退到那一行本来的角色，研究入口的闸门才匹配。
    let context_detail = read_comment_detail(
        &db,
        &CommentDetailQuery {
            domain,
            work_ref: context_work,
            comment_external_id: "context-only-c1".to_owned(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        context_detail["comment"]["observationRole"], "primary",
        "{context_detail}"
    );
    // 研究准入的角色闸门同样能匹配：详情给出的角色就是这一步传回来的角色。
    let source_ref: Uuid = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment WHERE content_public_ref=$1 AND comment_external_id='context-only-c1' ORDER BY observed_at::timestamptz DESC, material_ref DESC LIMIT 1",
    )
    .bind(context_work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let selected = linggan_intelligence::comment_study_source::selected_source_in(
        &mut tx,
        domain,
        context_work,
        source_ref,
        linggan_intelligence::comment_study_source::StudySourceRole::Primary,
        "9999-12-31T00:00:00Z",
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert!(selected.is_some(), "作者语境作品的评论应能进入评论研究");
}

/// 领域里只有作者语境作品带评论时，总览的观察序列是空的：既没有新观察到的评论，也没有评论覆盖记录。
#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn creator_context_comments_alone_leave_the_overview_observation_series_empty() {
    let db = database("overview_series_creator_context").await;
    detail_with_author(&db, "series-context", "SYNTHETIC 序列作品", Some("creator")).await;
    for index in 0..2 {
        add_comment(
            &db,
            "series-context",
            &format!("series-c{index}"),
            "SYNTHETIC 评论",
            Some("reader"),
        )
        .await;
    }
    let domain = query().domain;
    fixture::make_creator_context_only(&db, domain, "series-context").await;
    let overview = comment_research_overview::read(&db, domain).await.unwrap();
    let points = overview["observationSeries"]["points"].as_array().unwrap();
    assert!(!points.is_empty(), "{overview}");
    let total = |key: &str| -> i64 {
        points
            .iter()
            .map(|point| point[key].as_i64().unwrap())
            .sum()
    };
    assert_eq!(total("newObservedCommentCount"), 0, "{overview}");
    assert_eq!(total("coveredWorkCount"), 0, "{overview}");
    assert!(
        points
            .iter()
            .all(|point| point["observationCoverage"] == "none"),
        "{overview}"
    );
}
