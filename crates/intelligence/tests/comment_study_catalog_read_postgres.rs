//! Isolated catalog proofs. These exercise real SQL, not front-end fixture filtering.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::ai_foundation::comment_research_overview;
use linggan_intelligence::comment_study_catalog::{
    CatalogStudyState, CatalogSummaryQuery, CatalogVoiceRole, CommentCatalogQuery,
    CommentDetailQuery, StudyCatalogError, WordCloudQuery, read_catalog_summary,
    read_comment_catalog, read_comment_detail, read_word_cloud, read_work_catalog,
    refresh_clean_cache,
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
    let domain = query().domain;
    fixture::set_usage_role(&db, domain, "role-work", "reference").await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id='role-work'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    add_creator_context_usage(&db, domain, work, "primary").await;
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
