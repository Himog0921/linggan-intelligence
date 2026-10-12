//! Shape regeneration is metered, bounded and tied to the same frozen proposal.
use super::*;

async fn correction_pending(db: &Database) -> (Uuid, Value) {
    let row = sqlx::query("SELECT task_ref,state,phase_attempt_count,last_reason,recall_manifest FROM linggan_topic_map_research_task ORDER BY updated_at DESC LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "queued");
    assert_eq!(row.get::<i32, _>("phase_attempt_count"), 1);
    assert_eq!(row.get::<String, _>("last_reason"), "output_schema_invalid");
    let result = latest_result(db).await;
    assert_eq!(result["accepted"], false);
    assert_eq!(result["phaseComplete"], false);
    assert_eq!(result["semanticReview"]["passed"], false);
    assert_eq!(result["reviewCorrectionEligible"], true);
    assert_eq!(result["validationDiagnostic"]["rule"], "unknown_property");
    assert_eq!(result["validationDiagnostic"]["schemaPath"], "checks[0]");
    assert_eq!(accepted_results(db).await, 0);
    let progress = read_research_progress(db, D).await.unwrap();
    assert_eq!(progress["currentMethodSummary"]["correctiveQueuedCount"], 1);
    assert_eq!(progress["currentMethodSummary"]["succeededCount"], 0);
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        0
    );
    (row.get("task_ref"), row.get("recall_manifest"))
}

async fn assert_no_raw_review(db: &Database) {
    let ledger: String = sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('manifest',q.request_manifest,'result',i.result))::text,'[]') FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)")
        .fetch_one(db.pool()).await.unwrap();
    assert!(!ledger.contains("PRIVATE_REVIEW_KEY"));
    assert!(!ledger.contains("PRIVATE_REVIEW_VALUE"));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn malformed_review_is_regenerated_once_without_accepting_or_erasing_the_failure() {
    let (db, _, adapter, _) = session("review_shape_corrected", "schema").await;
    tick(&db, &adapter).await;
    let proposal_invocation = pending(&db, "extract").await;
    tick(&db, &adapter).await;
    let (_, recall) = correction_pending(&db).await;
    let failed = sqlx::query("SELECT i.invocation_ref,i.state,i.failure_code,i.charged_tokens FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE q.attempt_ordinal=2")
        .fetch_one(db.pool()).await.unwrap();
    let failed_invocation: Uuid = failed.get("invocation_ref");
    assert_eq!(failed.get::<String, _>("state"), "failed");
    assert_eq!(
        failed.get::<String, _>("failure_code"),
        "output_schema_invalid"
    );
    assert_eq!(failed.get::<i64, _>("charged_tokens"), 249);
    assert_eq!(calls(&db).await, 2);
    assert_eq!(charges(&db).await, 648);
    tick(&db, &adapter).await;
    assert_eq!(calls(&db).await, 3);
    assert_eq!(charges(&db).await, 897);
    assert_eq!(
        accepted_results(&db).await,
        0,
        "extraction is still not resolved"
    );
    let manifest: Value = sqlx::query_scalar(
        "SELECT request_manifest FROM linggan_topic_map_research_request WHERE attempt_ordinal=3",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let feedback = &manifest["validationFeedback"];
    assert_eq!(feedback["sourceInvocationRef"], json!(proposal_invocation));
    assert_eq!(
        feedback["failedReviewInvocationRef"],
        json!(failed_invocation)
    );
    assert_eq!(
        feedback["proposalHash"],
        latest_result(&db).await["semanticReview"]["proposalHash"]
    );
    assert_eq!(
        manifest["sourceInvocationRef"],
        recall["semanticReview"]["sourceInvocationRef"]
    );
    assert_eq!(latest_result(&db).await["semanticReview"]["passed"], true);
    finish_pending(&db, &adapter).await;
    assert_eq!(
        calls(&db).await,
        6,
        "both phases require their own independent strict review"
    );
    assert_eq!(charges(&db).await, 1794);
    assert_eq!(accepted_results(&db).await, 1);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE i.state='failed' AND i.failure_code='output_schema_invalid' AND i.charged_tokens=249")
        .fetch_one(db.pool()).await.unwrap(), 2);
    let progress = read_research_progress(&db, D).await.unwrap();
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        1
    );
    assert_eq!(progress["currentMethodSummary"]["correctiveQueuedCount"], 0);
    assert_no_raw_review(&db).await;
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn second_shape_failure_is_terminal_even_when_the_model_allows_three_attempts() {
    let (db, config, adapter) = setup("review_shape_cap").await;
    let config = model_config_mode(&db, config, "synthetic-review-schema-persistent", 3).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let w = work(&db, "shape-cap", "SYNTHETIC 开始练习前明确一步。").await;
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    tick(&db, &adapter).await;
    tick(&db, &adapter).await;
    correction_pending(&db).await;
    tick(&db, &adapter).await;
    rejected(&db, &adapter, "output_schema_invalid").await;
    assert_eq!(calls(&db).await, 3);
    assert_eq!(charges(&db).await, 897);
    assert!(
        latest_result(&db)
            .await
            .get("reviewCorrectionEligible")
            .is_none()
    );
    assert_no_raw_review(&db).await;
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn corrective_review_with_unknown_dispatch_is_not_replayed() {
    let (db, _, adapter, _) = session("review_shape_unknown", "schema-unknown").await;
    tick(&db, &adapter).await;
    tick(&db, &adapter).await;
    correction_pending(&db).await;
    tick(&db, &adapter).await;
    let row = sqlx::query("SELECT state,last_reason FROM linggan_topic_map_research_task")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("state"), "unknown_dispatch");
    assert_eq!(calls(&db).await, 3);
    assert_eq!(accepted_results(&db).await, 0);
    assert_eq!(latest_result(&db).await["dispatchUnknown"], true);
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(calls(&db).await, 3);
    assert_no_raw_review(&db).await;
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn extra_properties_do_not_make_other_review_failures_correctable() {
    for mode in [
        "schema-foreign",
        "schema-duplicate",
        "schema-missing",
        "schema-fail",
        "schema-uncertain",
        "schema-reason",
        "schema-long-reason",
    ] {
        let (db, _, adapter, _) =
            session(&format!("review_mixed_{}", mode.replace('-', "_")), mode).await;
        tick(&db, &adapter).await;
        tick(&db, &adapter).await;
        rejected(&db, &adapter, "output_schema_invalid").await;
        assert_eq!(calls(&db).await, 2, "{mode}");
        assert_eq!(charges(&db).await, 648, "{mode}");
        assert!(
            latest_result(&db)
                .await
                .get("reviewCorrectionEligible")
                .is_none(),
            "{mode}"
        );
        assert_no_raw_review(&db).await;
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn corrective_review_rechecks_pause_budget_and_source_eligibility() {
    for guard in ["pause", "budget", "withdraw"] {
        let name = format!("review_corrective_guard_{guard}");
        let (db, original, adapter) = setup(&name).await;
        let config = model_config_mode(&db, original, "synthetic-review-schema", 2).await;
        let w = work(
            &db,
            &name,
            if guard == "withdraw" {
                ""
            } else {
                "SYNTHETIC 开始练习前明确一步。"
            },
        )
        .await;
        let package = if guard == "withdraw" {
            Some(fixture::submit_package_at(&db, "comments", json!({"contentExternalId":name}),
                json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":name},
                    "payload":{"commentId":"review-corrective-withdraw","noteId":name,"text":"SYNTHETIC 开始练习前明确一步。","authorId":"synthetic-reader"}}),
                "2026-09-16T08:00:00Z").await)
        } else {
            None
        };
        apply_research_command(&db, &configure(config, 100000, false))
            .await
            .unwrap();
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        tick(&db, &adapter).await;
        tick(&db, &adapter).await;
        correction_pending(&db).await;
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
                assert_eq!(sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic corrective withdrawal' FROM linggan_material_comment WHERE package_ref=$1")
                .bind(package.unwrap()).execute(db.pool()).await.unwrap().rows_affected(), 1);
            }
        }
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        assert_eq!(
            calls(&db).await,
            2,
            "{guard} denies a corrective model request"
        );
        assert_eq!(charges(&db).await, 648);
        assert_eq!(accepted_results(&db).await, 0);
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn correction_cannot_rebind_a_changed_proposal_reason_or_attempt_counter() {
    for guard in ["proposal", "reason", "attempt", "backfill"] {
        let (db, _, adapter, _) = session(&format!("review_binding_{guard}"), "schema").await;
        tick(&db, &adapter).await;
        tick(&db, &adapter).await;
        correction_pending(&db).await;
        let command = match guard {
            "proposal" => {
                "UPDATE linggan_topic_map_research_task SET recall_manifest=jsonb_set(recall_manifest,'{semanticReview,proposal,discussions,0,statement}',to_jsonb('different synthetic proposal'::text))"
            }
            "reason" => {
                "UPDATE linggan_topic_map_research_task SET last_reason='new_topic_candidate_coverage_missing'"
            }
            "attempt" => "UPDATE linggan_topic_map_research_task SET phase_attempt_count=0",
            _ => {
                "UPDATE linggan_topic_map_research_task SET recall_manifest=recall_manifest||'{\"backfill\":{}}'::jsonb"
            }
        };
        sqlx::query(command).execute(db.pool()).await.unwrap();
        tick(&db, &adapter).await;
        assert_eq!(calls(&db).await, 2, "{guard} cannot authorize regeneration");
        assert_eq!(charges(&db).await, 648);
        assert_eq!(accepted_results(&db).await, 0);
        let reason: String =
            sqlx::query_scalar("SELECT last_reason FROM linggan_topic_map_research_task")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(
            reason,
            if guard == "proposal" {
                "semantic_review_unavailable"
            } else {
                "rule_feedback_unavailable"
            },
            "{guard}"
        );
    }
}
