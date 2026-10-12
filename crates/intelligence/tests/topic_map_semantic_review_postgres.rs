//! Isolated PostgreSQL proof of metered proposal/review acceptance; no real provider.
#[path = "support/topic_map_core_fixture.rs"]
mod support;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
    topic_map_research::{ResearchCommand, apply_research_command, read_research_progress},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use support::*;
use uuid::Uuid;

#[path = "topic_map_semantic_review/review_feedback.rs"]
mod review_feedback;

async fn session(name: &str, mode: &str) -> (Database, Uuid, PiAdapter, Uuid) {
    let (db, config, adapter) = setup(name).await;
    let config = model_config_mode(&db, config, &format!("synthetic-review-{mode}"), 2).await;
    let w = work(&db, name, "SYNTHETIC 开始练习前需要明确第一步。🙂").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    (db, config, adapter, w)
}
async fn tick(db: &Database, adapter: &PiAdapter) {
    assert!(run_once(db, &SyntheticModelSecrets, adapter).await.unwrap());
}
async fn calls(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_research_request")
        .fetch_one(db.pool())
        .await
        .unwrap()
}
async fn charges(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)")
        .fetch_one(db.pool()).await.unwrap()
}
async fn accepted_results(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_research_result")
        .fetch_one(db.pool())
        .await
        .unwrap()
}
async fn latest_result(db: &Database) -> Value {
    sqlx::query_scalar("SELECT i.result FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) ORDER BY q.created_at DESC,q.attempt_ordinal DESC LIMIT 1")
        .fetch_one(db.pool()).await.unwrap()
}
async fn pending(db: &Database, phase: &str) -> Uuid {
    let row = sqlx::query("SELECT task_ref,state,phase,last_reason,recall_manifest FROM linggan_topic_map_research_task ORDER BY updated_at DESC LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "queued");
    assert_eq!(row.get::<String, _>("phase"), phase);
    assert_eq!(
        row.get::<String, _>("last_reason"),
        "semantic_review_pending"
    );
    let recall: Value = row.get("recall_manifest");
    let invocation: Uuid = recall["semanticReview"]["sourceInvocationRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let result = latest_result(db).await;
    assert_eq!(result["accepted"], false);
    assert_eq!(result["proposalReady"], true);
    assert_eq!(result["phaseComplete"], false);
    assert!(
        result["proposalHash"]
            .as_str()
            .is_some_and(|v| !v.is_empty())
    );
    assert_eq!(accepted_results(db).await, 0);
    let progress = read_research_progress(db, D).await.unwrap();
    assert_eq!(progress["currentMethodSummary"]["succeededCount"], 0);
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        0
    );
    invocation
}
async fn rejected(db: &Database, adapter: &PiAdapter, expected: &str) {
    let row = sqlx::query("SELECT state,last_reason FROM linggan_topic_map_research_task ORDER BY updated_at DESC LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "failed");
    assert_eq!(row.get::<String, _>("last_reason"), expected);
    assert_eq!(accepted_results(db).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_unit_assignment")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(latest_result(db).await["accepted"], false);
    let count = calls(db).await;
    run_once(db, &SyntheticModelSecrets, adapter).await.unwrap();
    assert_eq!(
        calls(db).await,
        count,
        "semantic rejection is terminal, not automatic rewriting"
    );
}

async fn recalled_then_reviewed(db: &Database, adapter: &PiAdapter, work: Uuid, before: i64) {
    let row = sqlx::query("SELECT state,last_reason,recall_manifest,phase_attempt_count FROM linggan_topic_map_research_task WHERE work_public_ref=$1")
        .bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "queued");
    assert_eq!(
        row.get::<String, _>("last_reason"),
        "definition_changed_recalling"
    );
    assert_eq!(row.get::<i32, _>("phase_attempt_count"), 0);
    assert!(
        row.get::<Value, _>("recall_manifest")
            .get("semanticReview")
            .is_none()
    );
    tick(db, adapter).await;
    assert_eq!(
        calls(db).await,
        before + 1,
        "current catalog requires a fresh proposal"
    );
    assert_eq!(latest_result(db).await["proposalReady"], true);
    assert_eq!(latest_result(db).await["accepted"], false);
    assert_eq!(
        accepted_results(db).await,
        1,
        "a new proposal is still unreviewed"
    );
    tick(db, adapter).await;
    assert_eq!(
        calls(db).await,
        before + 2,
        "the fresh proposal has its own review"
    );
    assert_eq!(accepted_results(db).await, 2);
    assert_eq!(latest_result(db).await["semanticReview"]["passed"], true);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn every_phase_requires_its_own_review_before_coverage_or_concept_creation() {
    let (db, config, adapter, _) = session("semantic_review_pass", "pass").await;
    tick(&db, &adapter).await;
    let extract_proposal = pending(&db, "extract").await;
    assert_eq!(calls(&db).await, 1);
    assert_eq!(charges(&db).await, 399);
    tick(&db, &adapter).await;
    assert_eq!(calls(&db).await, 2);
    assert_eq!(charges(&db).await, 648);
    assert_eq!(accepted_results(&db).await, 0);
    let review: Value = sqlx::query_scalar(
        "SELECT request_manifest FROM linggan_topic_map_research_request WHERE attempt_ordinal=2",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(review["semanticStage"], "review");
    assert_eq!(review["sourceInvocationRef"], json!(extract_proposal));
    assert_eq!(latest_result(&db).await["semanticReview"]["passed"], true);
    tick(&db, &adapter).await;
    let resolve_proposal = pending(&db, "resolve").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    tick(&db, &adapter).await;
    assert_eq!(accepted_results(&db).await, 1);
    assert_eq!(calls(&db).await, 4);
    assert_eq!(charges(&db).await, 1296);
    let review: Value = sqlx::query_scalar(
        "SELECT request_manifest FROM linggan_topic_map_research_request WHERE attempt_ordinal=4",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(review["sourceInvocationRef"], json!(resolve_proposal));
    assert_eq!(latest_result(&db).await["semanticReview"]["passed"], true);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE i.config_ref=$1")
        .bind(config).fetch_one(db.pool()).await.unwrap(), 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let progress = read_research_progress(&db, D).await.unwrap();
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        1
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn unsupported_citations_and_non_atomic_discussions_fail_without_correction() {
    for (mode, reason) in [
        ("citation", "semantic_discussion_unsupported"),
        ("atomic", "semantic_discussion_non_atomic"),
    ] {
        let (db, _, adapter, _) = session(&format!("semantic_{mode}"), mode).await;
        tick(&db, &adapter).await;
        pending(&db, "extract").await;
        tick(&db, &adapter).await;
        rejected(&db, &adapter, reason).await;
        assert_eq!(calls(&db).await, 2);
        assert_eq!(charges(&db).await, 648);
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn assignment_identity_abstraction_and_parent_containment_are_separate_rejections() {
    for (mode, reason) in [
        ("boundary", "semantic_assignment_boundary"),
        ("identity", "semantic_identity_relation"),
        ("concept", "semantic_concept_boundary"),
        ("parent", "semantic_parent_containment"),
    ] {
        let (db, _, adapter, _) = session(&format!("semantic_{mode}"), mode).await;
        for _ in 0..3 {
            tick(&db, &adapter).await;
        }
        pending(&db, "resolve").await;
        tick(&db, &adapter).await;
        rejected(&db, &adapter, reason).await;
        assert_eq!(calls(&db).await, 4);
        assert_eq!(charges(&db).await, 1296);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn review_requires_the_complete_closed_check_set_and_never_treats_uncertainty_as_pass() {
    for (mode, reason) in [
        ("missing", "semantic_review_coverage_invalid"),
        ("duplicate", "semantic_review_coverage_invalid"),
        ("foreign", "semantic_review_coverage_invalid"),
        ("uncertain", "semantic_review_uncertain"),
    ] {
        let (db, _, adapter, _) = session(&format!("semantic_{mode}"), mode).await;
        tick(&db, &adapter).await;
        pending(&db, "extract").await;
        tick(&db, &adapter).await;
        rejected(&db, &adapter, reason).await;
        assert_eq!(calls(&db).await, 2);
        assert_eq!(charges(&db).await, 648);
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn pause_budget_and_withdrawal_deny_the_independent_review_transmission() {
    for guard in ["pause", "budget", "withdraw"] {
        let (db, original, adapter) = setup(&format!("semantic_guard_{guard}")).await;
        let config = model_config_mode(&db, original, "synthetic-review-pass", 2).await;
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
        let package = if guard == "withdraw" {
            Some(fixture::submit_package_at(&db,"comments",json!({"contentExternalId":guard}),
            json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":guard},
                "payload":{"commentId":"semantic-withdraw","noteId":guard,"text":"SYNTHETIC 开始练习前明确一步。","authorId":"synthetic-reader"}}),
            "2026-09-16T08:00:00Z").await)
        } else {
            None
        };
        apply_research_command(&db, &configure(config, 100000, false))
            .await
            .unwrap();
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        tick(&db, &adapter).await;
        pending(&db, "extract").await;
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
                assert_eq!(sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic semantic withdrawal' FROM linggan_material_comment WHERE package_ref=$1")
                .bind(package.unwrap()).execute(db.pool()).await.unwrap().rows_affected(),1);
            }
        }
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        assert_eq!(calls(&db).await, 1, "{guard} cannot authorize review");
        assert_eq!(charges(&db).await, 399);
        assert_eq!(accepted_results(&db).await, 0);
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn unknown_review_remains_unknown_and_is_never_replayed_as_accepted() {
    let (db, _, adapter, _) = session("semantic_unknown", "unknown").await;
    tick(&db, &adapter).await;
    pending(&db, "extract").await;
    tick(&db, &adapter).await;
    assert_eq!(calls(&db).await, 2);
    assert_eq!(accepted_results(&db).await, 0);
    let row = sqlx::query("SELECT t.state,i.state AS invocation_state,i.result FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_request q USING(task_ref) JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.attempt_ordinal=2")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("state"), "unknown_dispatch");
    let result: Value = row.get("result");
    assert_eq!(result["accepted"], false);
    assert_eq!(result["dispatchUnknown"], true);
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(calls(&db).await, 2);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn changed_pending_proposal_cannot_reuse_a_prior_structural_pass() {
    let (db, _, adapter, _) = session("semantic_tamper", "pass").await;
    tick(&db, &adapter).await;
    pending(&db, "extract").await;
    sqlx::query("UPDATE linggan_topic_map_research_task SET recall_manifest=jsonb_set(recall_manifest,'{semanticReview,proposal,discussions,0,statement}',to_jsonb('different synthetic proposal'::text))")
        .execute(db.pool()).await.unwrap();
    tick(&db, &adapter).await;
    let reason: String =
        sqlx::query_scalar("SELECT last_reason FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(reason, "semantic_review_unavailable");
    assert_eq!(calls(&db).await, 1);
    assert_eq!(charges(&db).await, 399);
    assert_eq!(accepted_results(&db).await, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn definition_version_changes_between_proposal_and_review_prevent_old_candidate_acceptance() {
    let (db, config, adapter, _) = session("semantic_version", "pass").await;
    finish_pending(&db, &adapter).await;
    assert_eq!(accepted_results(&db).await, 1);
    let w = work(
        &db,
        "semantic-version-alias",
        "SYNTHETIC ALIAS_START 开始练习前需要明确第一步。",
    )
    .await;
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    for _ in 0..3 {
        tick(&db, &adapter).await;
    }
    let row = sqlx::query("SELECT state,phase,last_reason FROM linggan_topic_map_research_task WHERE work_public_ref=$1")
        .bind(w).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("phase"), "resolve");
    assert_eq!(
        row.get::<String, _>("last_reason"),
        "semantic_review_pending"
    );
    let previous: Uuid =
        sqlx::query_scalar("SELECT definition_ref FROM linggan_topic_map_concept_rule LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let definition = Uuid::new_v4();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) SELECT $1,topic_ref,version+1,display_name||'修订',definition_text,lifecycle_state FROM linggan_topic_definition WHERE definition_ref=$2")
        .bind(definition).bind(previous).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_concept_rule(definition_ref,domain_ref,identity_hash,inclusion_criteria,exclusion_criteria,method_version,invocation_ref) SELECT $1,domain_ref,identity_hash,inclusion_criteria,exclusion_criteria,method_version,invocation_ref FROM linggan_topic_map_concept_rule WHERE definition_ref=$2")
        .bind(definition).bind(previous).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let before = calls(&db).await;
    tick(&db, &adapter).await;
    assert_eq!(
        calls(&db).await,
        before,
        "old definition cannot authorize a new review"
    );
    assert_eq!(
        accepted_results(&db).await,
        1,
        "only the prior independent work remains accepted"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_unit_assignment a JOIN linggan_topic_map_unit_resolution r USING(resolution_ref) JOIN linggan_topic_map_discussion_unit u USING(unit_ref) WHERE u.work_public_ref=$1")
        .bind(w).fetch_one(db.pool()).await.unwrap(),0);
    recalled_then_reviewed(&db, &adapter, w, before).await;
    assert_eq!(sqlx::query_scalar::<_,Uuid>("SELECT a.definition_ref FROM linggan_topic_map_unit_assignment a JOIN linggan_topic_map_unit_resolution r USING(resolution_ref) JOIN linggan_topic_map_discussion_unit u USING(unit_ref) WHERE u.work_public_ref=$1")
        .bind(w).fetch_one(db.pool()).await.unwrap(),definition);
    // The current policy/model identity is unchanged; changing a concept does not buy a new model.
    assert_eq!(sqlx::query_scalar::<_,Uuid>("SELECT model_config_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 ORDER BY revision DESC LIMIT 1")
        .bind(D).fetch_one(db.pool()).await.unwrap(),config);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic adapter only"]
async fn unrelated_definition_during_new_proposal_recalls_without_retiring_the_initial_window() {
    let (db, _, adapter, _) = session("semantic_catalog_race", "pass").await;
    finish_pending(&db, &adapter).await;
    let w = work(
        &db,
        "semantic-catalog-new",
        "SYNTHETIC SUSTAIN_ONLY 开始之后持续练习的不同困难。",
    )
    .await;
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    for _ in 0..3 {
        tick(&db, &adapter).await;
    }
    let proposed: Value = sqlx::query_scalar(
        "SELECT recall_manifest FROM linggan_topic_map_research_task WHERE work_public_ref=$1",
    )
    .bind(w)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        proposed["semanticReview"]["proposal"]["decisions"][0]["status"],
        "new"
    );
    let base = sqlx::query("SELECT d.definition_ref,b.receipt_ref FROM linggan_topic_definition d JOIN linggan_topic_map_binding b USING(topic_ref) LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    let topic = Uuid::new_v4();
    let definition = Uuid::new_v4();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query(
        "INSERT INTO linggan_topic_workspace(topic_ref,domain_key,canonical_key) VALUES($1,$2,$3)",
    )
    .bind(topic)
    .bind(format!("domain-{}", D.simple()))
    .bind(format!("unrelated-{}", topic.simple()))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_topic_definition(definition_ref,topic_ref,version,display_name,definition_text,lifecycle_state) SELECT $1,$2,1,'合成无关主题','选择任务之后评价满意程度，不涉及启动或持续练习','candidate' FROM linggan_topic_definition WHERE definition_ref=$3")
        .bind(definition).bind(topic).bind(base.get::<Uuid,_>("definition_ref")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,version,receipt_ref) VALUES($1,$2,$3,1,$4)")
        .bind(Uuid::new_v4()).bind(topic).bind(D).bind(base.get::<Uuid,_>("receipt_ref")).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let before = calls(&db).await;
    tick(&db, &adapter).await;
    assert_eq!(
        calls(&db).await,
        before,
        "catalog changes cannot validate the old comparison set"
    );
    assert_eq!(accepted_results(&db).await, 1);
    recalled_then_reviewed(&db, &adapter, w, before).await;
    let progress = read_research_progress(&db, D).await.unwrap();
    assert_eq!(progress["currentMethodSummary"]["initialWindowCount"], 2);
    assert_eq!(
        progress["currentMethodSummary"]["completedInitialWindowCount"],
        2
    );
}
