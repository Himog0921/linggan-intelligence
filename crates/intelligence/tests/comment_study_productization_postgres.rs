//! P1 read-only counterexample for T44. Uses only an explicitly isolated proof database.
//! Passing this subset does not establish pagination, migration or the complete T44 contract.

#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::proof_database;
use linggan_intelligence::comment_study_read::{
    CommentStudyReadQuery, read_runs,
};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::json;
use uuid::Uuid;

const STUDY_SCHEMA_SQL: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; never use a shared or real-data database"]
async fn run_counts_are_independent_of_work_count_and_keep_empty_runs() {
    let database = proof_database("comment_study_productization_run_counts").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();

    let domain_ref = Uuid::parse_str(ADHD_DOMAIN_REF).unwrap();
    let policy_ref = Uuid::new_v4();
    let run_ref = Uuid::new_v4();
    let hash = "0".repeat(64);
    sqlx::query(
        "INSERT INTO linggan_comment_study_policy \
         (policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget) \
         VALUES($1,$2,NULL,'comment-study.v1',100,6000)",
    )
    .bind(policy_ref)
    .bind(domain_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_run \
         (run_ref,policy_ref,as_of,state,selection_manifest,selection_hash) \
         VALUES($1,$2,scope_001_now(),'prepared','{}'::jsonb,$3)",
    )
    .bind(run_ref)
    .bind(policy_ref)
    .bind(&hash)
    .execute(database.pool())
    .await
    .unwrap();

    let states = ["succeeded", "no_signal", "needs_context", "failed", "excluded", "queued"];
    for (index, state) in states.iter().enumerate() {
        let note = format!("synthetic-count-note-{}", index / 3);
        if index % 3 == 0 {
            detail_with_author(&database, &note, "SYNTHETIC 计数测试作品", Some("creator"))
                .await;
        }
        let source_ref = comment_with_author(
            &database,
            &note,
            &format!("synthetic-count-comment-{index}"),
            "SYNTHETIC / NOT EVIDENCE · 独立计数测试评论",
            Some("reader"),
            "2026-09-16T08:00:00Z",
        )
        .await;
        let work_ref: Uuid = sqlx::query_scalar(
            "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
        )
        .bind(source_ref)
        .fetch_one(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_comment_study_work \
             (run_ref,content_public_ref,domain_ref,selection_reason,context_state,context_manifest,context_hash) \
             VALUES($1,$2,$3,'user_selected','missing',$4,$5) \
             ON CONFLICT(run_ref,content_public_ref) DO NOTHING",
        )
        .bind(run_ref)
        .bind(work_ref)
        .bind(domain_ref)
        .bind(json!({"workRef":work_ref,"sources":[]}))
        .bind(&hash)
        .execute(database.pool())
        .await
        .unwrap();
        let dependency = if *state == "excluded" {
            "input_invalid"
        } else if *state == "needs_context" {
            "parent_required_missing"
        } else {
            "self_contained"
        };
        let exclusion = (*state == "excluded").then_some("frozen_input_restricted");
        sqlx::query(
            "INSERT INTO linggan_comment_study_target \
             (target_ref,run_ref,content_public_ref,source_ref,research_text,research_sha256, \
              dependency_state,state,exclusion_reason,input_manifest,input_hash) \
             VALUES($1,$2,$3,$4,'SYNTHETIC count fixture',$5,$6,$7,$8,'{}'::jsonb,$5)",
        )
        .bind(Uuid::new_v4())
        .bind(run_ref)
        .bind(work_ref)
        .bind(source_ref)
        .bind(&hash)
        .bind(dependency)
        .bind(*state)
        .bind(exclusion)
        .execute(database.pool())
        .await
        .unwrap();
    }

    let query = CommentStudyReadQuery {
        domain: Some(domain_ref),
        run_ref: None,
        limit: Some(50),
    };
    let result = read_runs(&database, &query).await.unwrap();
    let runs = result["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["workCount"], 2);
    assert_eq!(runs[0]["targetCount"], 6);
    for field in ["succeededCount", "noSignalCount", "needsContextCount", "failedCount", "excludedCount"] {
        assert_eq!(runs[0][field], 1, "wrong independent count for {field}");
    }

    let empty_run = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_run \
         (run_ref,policy_ref,as_of,state,selection_manifest,selection_hash) \
         VALUES($1,$2,scope_001_now(),'prepared','{}'::jsonb,$3)",
    )
    .bind(empty_run)
    .bind(policy_ref)
    .bind(&hash)
    .execute(database.pool())
    .await
    .unwrap();
    let result = read_runs(&database, &query).await.unwrap();
    let rows = result["runs"].as_array().unwrap();
    let empty_id = empty_run.to_string();
    let empty = rows.iter().find(|row| row["runRef"].as_str() == Some(empty_id.as_str())).unwrap();
    assert_eq!(empty["workCount"], 0);
    assert_eq!(empty["targetCount"], 0);
    for field in ["succeededCount", "noSignalCount", "needsContextCount", "failedCount", "excludedCount"] {
        assert_eq!(empty[field], 0);
    }
    let mut bounded = query.clone();
    bounded.limit = Some(1);
    assert_eq!(read_runs(&database, &bounded).await.unwrap()["runs"].as_array().unwrap().len(), 1);
    bounded.domain = Some(Uuid::new_v4());
    assert!(read_runs(&database, &bounded).await.unwrap()["runs"].as_array().unwrap().is_empty());
}
