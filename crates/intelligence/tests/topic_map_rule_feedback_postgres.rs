//! Disposable rule correction proof. Synthetic child only; no real provider or materials.
#[path = "support/topic_map_core_fixture.rs"]
mod support;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    topic_map_research::{ResearchCommand, apply_research_command},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use support::*;
use uuid::Uuid;

async fn model_mode(db: &Database, original: Uuid, mode: &str, attempts: i32) -> Uuid {
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) SELECT $1,m.connection_version_ref,$3,'manual' FROM linggan_model_entry m JOIN linggan_model_config c USING(model_ref) WHERE c.config_ref=$2")
        .bind(model).bind(original).bind(mode).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,$2,input_token_limit,output_token_limit,timeout_seconds,$4 FROM linggan_model_config WHERE config_ref=$3")
        .bind(config).bind(model).bind(original).bind(attempts).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) SELECT $1,connection_version_ref,model_ref,'probe','synthetic-rule-feedback','succeeded',0,0,$3 FROM linggan_model_entry WHERE model_ref=$2")
        .bind(Uuid::new_v4()).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    config
}
async fn call_count(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_research_request")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn mismatched_feedback_reason_terminates_without_a_second_transmission() {
    let (db, config, adapter) = setup("rule_feedback_identity").await;
    let config = model_mode(&db, config, "synthetic-feedback-schema", 2).await;
    let w = work(&db, "feedback-identity", "SYNTHETIC 开始练习前明确一步。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    first_rejected(&db, "output_schema_invalid").await;
    sqlx::query("UPDATE linggan_topic_map_research_task SET last_reason='new_topic_candidate_coverage_missing'")
        .execute(db.pool()).await.unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(call_count(&db).await, 1);
    let row = sqlx::query("SELECT state,last_reason FROM linggan_topic_map_research_task")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("state"), "failed");
    assert_eq!(
        row.get::<String, _>("last_reason"),
        "rule_feedback_unavailable"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'")
        .fetch_one(db.pool()).await.unwrap(), 399);
}
async fn first_rejected(db: &Database, reason: &str) {
    let row = sqlx::query("SELECT t.state,t.phase_attempt_count,t.last_reason,i.state AS invocation_state,i.charged_tokens,i.failure_code,i.result FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_request q USING(task_ref) JOIN linggan_model_invocation i USING(invocation_ref) ORDER BY q.attempt_ordinal LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "queued");
    assert_eq!(row.get::<i32, _>("phase_attempt_count"), 1);
    assert_eq!(row.get::<String, _>("invocation_state"), "failed");
    assert_eq!(row.get::<String, _>("failure_code"), reason);
    assert_eq!(row.get::<i64, _>("charged_tokens"), 399);
    let result: Value = row.get("result");
    assert_eq!(result["accepted"], false);
    assert_eq!(result["phaseComplete"], false);
    assert_eq!(result["dispatchUnknown"], false);
    assert!(!result.to_string().contains("PRIVATE_PROVIDER_DIAGNOSTIC"));
    let progress = linggan_intelligence::topic_map_research::read_research_progress(db, D)
        .await
        .unwrap();
    assert_eq!(progress["currentMethodSummary"]["correctiveQueuedCount"], 1);
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        0
    );
    assert_eq!(progress["currentMethodSummary"]["succeededCount"], 0);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn schema_and_missing_journey_correct_once_then_require_valid_source_result() {
    for (name, mode, reason) in [
        (
            "rule_schema",
            "synthetic-feedback-schema",
            "output_schema_invalid",
        ),
        (
            "rule_title",
            "synthetic-feedback-title",
            "output_schema_invalid",
        ),
        (
            "rule_journey",
            "synthetic-feedback-journey",
            "journey_evidence_missing",
        ),
    ] {
        let (db, config, adapter) = setup(name).await;
        let config = model_mode(&db, config, mode, 2).await;
        let w = work(&db, name, "SYNTHETIC 开始练习前先明确一步。").await;
        apply_research_command(&db, &configure(config, 100000, false))
            .await
            .unwrap();
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
        first_rejected(&db, reason).await;
        assert_eq!(call_count(&db).await, 1);
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
        let feedback: Value = sqlx::query_scalar("SELECT request_manifest->'validationFeedback' FROM linggan_topic_map_research_request ORDER BY attempt_ordinal DESC LIMIT 1").fetch_one(db.pool()).await.unwrap();
        let original_invocation: Uuid = sqlx::query_scalar("SELECT invocation_ref FROM linggan_topic_map_research_request ORDER BY attempt_ordinal LIMIT 1").fetch_one(db.pool()).await.unwrap();
        assert_eq!(feedback["sourceInvocationRef"], json!(original_invocation));
        assert_eq!(feedback["code"], reason);
        assert!(!feedback.to_string().contains("PRIVATE_PROVIDER_DIAGNOSTIC"));
        let phase: String = sqlx::query_scalar("SELECT phase FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            phase, "resolve",
            "a corrected draft must still pass actual extraction before resolving"
        );
        finish_pending(&db, &adapter).await;
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze' AND state='failed'").fetch_one(db.pool()).await.unwrap(),1);
        assert_eq!(call_count(&db).await, 3);
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn repeated_rejection_stops_after_one_correction_even_when_config_allows_three() {
    let (db, config, adapter) = setup("rule_cap").await;
    let config = model_mode(&db, config, "synthetic-feedback-persistent", 3).await;
    let w = work(&db, "rule-cap", "SYNTHETIC 开始练习前先明确一步。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    first_rejected(&db, "output_schema_invalid").await;
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    finish_pending(&db, &adapter).await;
    assert_eq!(call_count(&db).await, 2);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        798
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn complete_candidate_comparison_is_regenerated_not_filled_with_defaults() {
    let (db, config, adapter) = setup("rule_relations").await;
    let first = work(&db, "existing-start", "SYNTHETIC 开始练习前明确一步。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![first]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let config = model_mode(&db, config, "synthetic-feedback-relations", 2).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let second = work(
        &db,
        "new-sustained",
        "SYNTHETIC SUSTAIN_ONLY 开始之后保持练习。",
    )
    .await;
    apply_research_command(&db, &start(vec![second]))
        .await
        .unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let failed: Value = sqlx::query_scalar("SELECT i.result FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE i.failure_code='new_topic_candidate_coverage_missing'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(failed["accepted"], false);
    assert_eq!(failed["phaseComplete"], false);
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_unit_resolution WHERE reason LIKE '%合成逐个比较%' ").fetch_one(db.pool()).await.unwrap(),2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_concept_relation WHERE relation='distinct'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn corrective_queue_obeys_pause_daily_budget_and_source_withdrawal() {
    for guard in ["pause", "budget", "withdraw"] {
        let (db, config, adapter) = setup(&format!("rule_guard_{guard}")).await;
        let config = model_mode(&db, config, "synthetic-feedback-schema", 2).await;
        let w = work(
            &db,
            guard,
            if guard == "withdraw" {
                ""
            } else {
                "SYNTHETIC 开始练习前明确一步。"
            },
        )
        .await;
        let comment_package = if guard == "withdraw" {
            Some(fixture::submit_package_at(&db, "comments", json!({"contentExternalId":guard}),
                json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":guard},
                    "payload":{"commentId":"corrective-withdrawal","noteId":guard,"text":"SYNTHETIC 开始练习前明确一步。","authorId":"synthetic-reader"}}),
                "2026-09-16T08:00:00Z").await)
        } else {
            None
        };
        apply_research_command(&db, &configure(config, 100000, false))
            .await
            .unwrap();
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
        first_rejected(&db, "output_schema_invalid").await;
        match guard {
            "pause" => {
                apply_research_command(
                    &db,
                    &ResearchCommand::Pause {
                        request_ref: Uuid::new_v4(),
                        domain_ref: D,
                        run_ref: None,
                    },
                )
                .await
                .unwrap();
            }
            "budget" => {
                apply_research_command(&db, &configure(config, 1024, false))
                    .await
                    .unwrap();
            }
            _ => {
                // Match the existing core withdrawal proof: append an authoritative
                // comment restriction, preserving immutable material and Domain usage.
                let restricted = sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic corrective-source withdrawal' FROM linggan_material_comment WHERE package_ref=$1")
                    .bind(comment_package.unwrap()).execute(db.pool()).await.unwrap();
                assert_eq!(restricted.rows_affected(), 1);
            }
        }
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        assert_eq!(
            call_count(&db).await,
            1,
            "{guard} must deny a second transmission"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            399
        );
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic adapter only"]
async fn foreign_citations_unknown_dispatch_and_attempt_limit_do_not_get_correction() {
    for (name, body, limit, expected) in [
        (
            "rule_foreign",
            "SYNTHETIC ATTACK 开始练习前明确一步。",
            2,
            "failed",
        ),
        (
            "rule_unknown",
            "SYNTHETIC SLOW_UNKNOWN 开始练习前明确一步。",
            2,
            "unknown_dispatch",
        ),
        (
            "rule_no_retry",
            "SYNTHETIC 开始练习前明确一步。",
            1,
            "failed",
        ),
    ] {
        let (db, config, adapter) = setup(name).await;
        let config = model_mode(&db, config, "synthetic-feedback-schema", limit).await;
        let w = work(&db, name, body).await;
        apply_research_command(&db, &configure(config, 100000, false))
            .await
            .unwrap();
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT state FROM linggan_topic_map_research_task")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            expected
        );
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        assert_eq!(call_count(&db).await, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
    }
}
