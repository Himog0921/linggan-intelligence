#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{coverage_layer, proof_database, submit_custom_package, submit_package};
use sqlx::Row;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn content_detail_submission_forms_a_typed_material_with_field_sources_and_unknowns() {
    let database = proof_database("material_detail_slice").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-1"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-material-1"},
            "payload":{
                "title":"逐字段来源标题",
                "bodyText":"可检索正文",
                "authorId":"author-material-1"
            }
        }),
    )
    .await;

    let row = sqlx::query(
        "SELECT title, title_state, body_text, body_state, creator_display_name_state, \
                package_ref, record_ordinal \
         FROM linggan_material_content_detail",
    )
    .fetch_one(database.pool())
    .await
    .expect("a qualified detail record forms one typed material row");

    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("逐字段来源标题")
    );
    assert_eq!(row.get::<String, _>("title_state"), "KNOWN");
    assert_eq!(
        row.get::<Option<String>, _>("body_text").as_deref(),
        Some("可检索正文")
    );
    assert_eq!(row.get::<String, _>("body_state"), "KNOWN");
    assert_eq!(
        row.get::<String, _>("creator_display_name_state"),
        "UNKNOWN"
    );
    assert_ne!(row.get::<uuid::Uuid, _>("package_ref"), uuid::Uuid::nil());
    assert_eq!(row.get::<i32, _>("record_ordinal"), 0);

    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-2"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"attacker-selected-other-note"},
            "payload":{
                "title":"不得跨目标接纳",
                "creatorName":"不得猜成 authorName"
            }
        }),
    )
    .await;
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .expect("detail count reads");
    assert_eq!(
        detail_count, 1,
        "mismatched source identity is not projected"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn discovery_forms_the_same_work_identity_without_claiming_detail() {
    let database = proof_database("material_discovery_slice").await;
    submit_package(&database,"discovery_search",serde_json::json!({"query":"ADHD"}),serde_json::json!({
        "kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"note-discovery-material"},
        "payload":{
            "title":"发现面标题","authorName":"发现面作者",
            "coverUrl":"https://sns-webpic-qc.xhscdn.com/observed-cover.webp",
            "likes":321,"comments":17,"collects":43,"shares":5
        }
    })).await;
    let row=sqlx::query("SELECT finding.title,finding.title_state,content.content_external_id, \
                                finding.cover_source_url,finding.cover_source_state, \
                                finding.like_count,finding.comment_count,finding.collect_count,finding.share_count \
                         FROM linggan_material_discovery_finding finding \
                         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref")
        .fetch_one(database.pool()).await.expect("typed discovery finding exists");
    assert_eq!(
        row.get::<String, _>("content_external_id"),
        "note-discovery-material"
    );
    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("发现面标题")
    );
    assert_eq!(
        row.get::<Option<String>, _>("cover_source_url").as_deref(),
        Some("https://sns-webpic-qc.xhscdn.com/observed-cover.webp")
    );
    assert_eq!(row.get::<String, _>("cover_source_state"), "KNOWN");
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(321));
    assert_eq!(row.get::<Option<i64>, _>("comment_count"), Some(17));
    assert_eq!(row.get::<Option<i64>, _>("collect_count"), Some(43));
    assert_eq!(row.get::<Option<i64>, _>("share_count"), Some(5));
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        detail_count, 0,
        "discovery never manufactures detail material"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn admission_quarantines_task_package_and_record_identity_attacks_without_poisoning_siblings()
{
    let database = proof_database("material_admission_attacks").await;
    let target = serde_json::json!({"contentExternalId":"note-admission-1"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        target.clone(),
        "content_detail",
        "douyin",
        serde_json::json!({"target":target,"layers":[coverage_layer("content_detail",4)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"douyin","type":"content","externalId":"note-admission-1"},
            "payload":{"title":"platform mismatch must not project"}
        })],
    )
    .await;
    let target = serde_json::json!({"contentExternalId":"note-admission-2"});
    submit_custom_package(
        &database,
        "xhs",
        &["comments"],
        target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("content_detail",1)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-admission-2"},
            "payload":{"title":"capability mismatch must not project"}
        })],
    )
    .await;
    let task_target = serde_json::json!({"contentExternalId":"note-admission-3"});
    let coverage_target = serde_json::json!({"contentExternalId":"attacker-other-target"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        task_target,
        "content_detail",
        "xhs",
        serde_json::json!({"target":coverage_target,"layers":[coverage_layer("content_detail",1)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"attacker-other-target"},
            "payload":{"title":"target mismatch must not project"}
        })],
    )
    .await;

    let healthy_target = serde_json::json!({"contentExternalId":"note-admission-healthy"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        healthy_target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":healthy_target,"layers":[coverage_layer("content_detail",4)]}),
        vec![
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-admission-healthy"},"payload":{"title":"healthy sibling"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"douyin","type":"content","externalId":"note-admission-healthy"},"payload":{"title":"cross platform"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"author","externalId":"note-admission-healthy"},"payload":{"title":"wrong subject type"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","externalId":"note-admission-healthy"},"payload":{"title":"missing subject type"}}),
        ],
    )
    .await;

    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(detail_count, 1, "only the healthy record projects");
    let task_binding_quarantines: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='task_package_contract_mismatch'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(task_binding_quarantines, 3);
    let record_quarantines: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='typed_detail_identity_invalid'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(record_quarantines, 3);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn coverage_selects_the_exact_capability_and_reconciles_producer_acquired() {
    let database = proof_database("material_coverage_attack").await;
    let target = serde_json::json!({"contentExternalId":"note-coverage"});
    let mut wrong_lane = coverage_layer("comments", 99);
    wrong_lane["observed"] = serde_json::json!(99);
    let mut detail_lane = coverage_layer("content_detail", 2);
    detail_lane["observed"] = serde_json::json!(2);
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":target,"layers":[wrong_lane,detail_lane]}),
        vec![serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-coverage"},"payload":{"title":"one retained of two acquired"}})],
    ).await;
    let row = sqlx::query("SELECT observed,producer_acquired,retained FROM linggan_material_lane_observation WHERE lane='detail'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(row.get::<Option<i32>, _>("observed"), Some(2));
    assert_eq!(row.get::<Option<i32>, _>("producer_acquired"), Some(2));
    assert_eq!(row.get::<Option<i32>, _>("retained"), Some(1));
}
