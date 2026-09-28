//! P2 start/dispatch proofs: disposable PostgreSQL and synthetic materials only. No model I/O.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_study_acceptance::{SemanticAcceptanceError, accept_target_output},
    comment_study_batch::{
        PrepareStudyBatchRequest, next_run_needing_batch, next_run_needing_batch_after,
        prepare_study_batch,
    },
    comment_study_batch_acceptance::accept_study_batch_output,
    comment_study_batch_worker::{
        claim_next_study_batch, claim_next_study_batch_after, recover_expired_study_batch_leases,
    },
    comment_study_catalog::refresh_clean_cache,
    comment_study_embedding::register_embedding_profile,
    comment_study_model_dispatch::{
        StudyModelDispatchError, mark_study_batch_model_dispatch_started,
        reserve_study_batch_model_call,
    },
    comment_study_model_runner::{StudyModelRunnerError, call_study_batch_model},
    comment_study_pair_worker::run_one_problem_pair,
    comment_study_policy::{
        CreateStudyPolicyCommand, StudyPolicyDefaults, StudyStageInstructions, create_study_policy,
    },
    comment_study_problem_store::{
        PairSelection, ProblemStoreError, accept_problem_pair, accept_problem_resolution,
        prepare_problem_pair_for_enabled_v2_run, prepare_problem_resolution_for_enabled_v2_run,
    },
    comment_study_recall::{RecallCompleteness, recall_candidates},
    comment_study_resolution_worker::run_one_problem_resolution,
    comment_study_run::{
        StudyStartError, TrustedStudyOrigin, cancel_study_run, preview_study_selection,
        start_study_run,
    },
    comment_study_selection::{
        StartStudyRunCommand, StudyScope, StudySelectionMode, study_domain_lock_key,
    },
    model_invocation::checkpoint_invocation_usage,
    model_runner::{
        ModelWorkerFairness, prepare_next_batch_across_runs_with_fairness, run_model_work_once,
        run_model_worker_with_test_dependencies,
    },
    model_secrets::{ModelSecretStore, SyntheticModelSecrets},
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
    pi_adapter::{PiResponse, PiUsage},
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
const GUARDS: &str =
    include_str!("../../../database/migrations/0107_comment_study_start_constraints.sql");
const REQUEST_GUARDS: &str = include_str!(
    "../../../database/migrations/0108_comment_study_request_snapshot_constraints.sql"
);
const PAIR_FAILURE_STATE: &str =
    include_str!("../../../database/migrations/0109_comment_study_pair_failure_state.sql");
fn domain() -> Uuid {
    Uuid::parse_str(linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF).unwrap()
}

struct NoModelSecrets;

impl ModelSecretStore for NoModelSecrets {
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }

    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        Err(ModelError::SecretUnavailable)
    }

    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
}

async fn setup(name: &str, count: usize) -> (Database, StartStudyRunCommand, Uuid) {
    setup_with_model_input_limit(name, count, 8192).await
}

async fn setup_with_model_input_limit(
    name: &str,
    count: usize,
    model_input_limit: i32,
) -> (Database, StartStudyRunCommand, Uuid) {
    setup_with_model_config(name, count, model_input_limit, 30).await
}

async fn setup_with_model_config(
    name: &str,
    count: usize,
    model_input_limit: i32,
    timeout_seconds: i32,
) -> (Database, StartStudyRunCommand, Uuid) {
    setup_with_model_limits(name, count, model_input_limit, 1024, timeout_seconds).await
}

async fn setup_with_model_limits(
    name: &str,
    count: usize,
    model_input_limit: i32,
    model_output_limit: i32,
    timeout_seconds: i32,
) -> (Database, StartStudyRunCommand, Uuid) {
    let db = fixture::proof_database(name).await;
    sqlx::raw_sql(include_str!(
        "../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0105_comment_study_productization_schema.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0106_comment_study_policy_constraints.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(GUARDS).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(REQUEST_GUARDS)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(PAIR_FAILURE_STATE)
        .execute(db.pool())
        .await
        .unwrap();
    let (connection, version, model, config) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    sqlx::query(
        "INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)",
    )
    .bind(connection)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) \
        VALUES($1,$2,1,'SYNTHETIC','openai-completions','http://127.0.0.1:18080',true,$3)")
        .bind(version).bind(connection).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) \
        VALUES($1,$2,$3,$4,$5,2)").bind(config).bind(model).bind(model_input_limit).bind(model_output_limit).bind(timeout_seconds).execute(db.pool()).await.unwrap();
    let policy: CreateStudyPolicyCommand = serde_json::from_value(
        json!({"domainRef":domain(),"methodName":"SYNTHETIC", "parentPolicyRef":null,
        "modelConfigRef":config,"defaults":{"commentBudget":1,"contextCharacterBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}}),
    )
    .unwrap();
    let policy = create_study_policy(&db, policy).await.unwrap();
    detail_with_author(&db, "selected", "SYNTHETIC selected", Some("creator")).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 LIMIT 1",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();
    for index in 0..count {
        add(&db, &format!("c{index:04}")).await;
    }
    for _ in 0..(count / 128 + 1) {
        refresh_clean_cache(&db, domain(), 128).await.unwrap();
    }
    let command=serde_json::from_value(json!({"requestRef":Uuid::new_v4(),"domainRef":domain(),"policyRef":policy["policy"]["policyRef"],
        "scope":{"kind":"works","workRefs":[work]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null})).unwrap();
    (db, command, connection)
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn semantic_rejection_respects_the_configured_two_attempt_limit() {
    let (db, command, _) = setup("semantic_configured_retry_limit", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let target_ref: Uuid =
        sqlx::query_scalar("SELECT target_ref FROM linggan_comment_study_target WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE linggan_comment_study_target SET state='running' WHERE target_ref=$1")
        .bind(target_ref)
        .execute(db.pool())
        .await
        .unwrap();

    let first = accept_target_output(
        &db,
        target_ref,
        "1111111111111111111111111111111111111111111111111111111111111111",
        None,
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(first.state, "rejected");
    sqlx::query("UPDATE linggan_comment_study_target SET state='running' WHERE target_ref=$1")
        .bind(target_ref)
        .execute(db.pool())
        .await
        .unwrap();
    let second = accept_target_output(
        &db,
        target_ref,
        "2222222222222222222222222222222222222222222222222222222222222222",
        None,
        json!({}),
    )
    .await
    .unwrap();
    assert_eq!(second.state, "rejected");
    assert!(matches!(
        accept_target_output(
            &db,
            target_ref,
            "3333333333333333333333333333333333333333333333333333333333333333",
            None,
            json!({})
        )
        .await,
        Err(SemanticAcceptanceError::TargetNotLeased)
    ));
    let (attempts, target_state): (i64, String) = sqlx::query_as(
        "SELECT count(attempt.attempt_ref),target.state \
         FROM linggan_comment_study_semantic_attempt attempt \
         JOIN linggan_comment_study_target target USING(target_ref) \
         WHERE target.target_ref=$1 GROUP BY target.state",
    )
    .bind(target_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((attempts, target_state.as_str()), (2, "failed"));
}

async fn policy_with_semantic_instruction(
    db: &Database,
    command: &StartStudyRunCommand,
    semantic_instruction: String,
) -> Uuid {
    let model_config_ref: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_comment_study_policy WHERE policy_ref=$1",
    )
    .bind(command.policy_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let policy = create_study_policy(
        db,
        CreateStudyPolicyCommand {
            domain_ref: domain(),
            method_name: "INPUT_LIMIT_FIXTURE".to_owned(),
            parent_policy_ref: Some(command.policy_ref),
            model_config_ref,
            defaults: StudyPolicyDefaults {
                comment_budget: 2,
                context_character_budget: 6000,
            },
            stage_instructions: StudyStageInstructions {
                semantic: semantic_instruction,
                resolution: String::new(),
                pair: String::new(),
            },
        },
    )
    .await
    .unwrap();
    serde_json::from_value(policy["policy"]["policyRef"].clone()).unwrap()
}

async fn policy_with_problem_stage_instructions(
    db: &Database,
    command: &StartStudyRunCommand,
    resolution_instruction: String,
    pair_instruction: String,
) -> Uuid {
    let model_config_ref: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_comment_study_policy WHERE policy_ref=$1",
    )
    .bind(command.policy_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let policy = create_study_policy(
        db,
        CreateStudyPolicyCommand {
            domain_ref: domain(),
            method_name: "PROBLEM_INPUT_LIMIT_FIXTURE".to_owned(),
            parent_policy_ref: Some(command.policy_ref),
            model_config_ref,
            defaults: StudyPolicyDefaults {
                comment_budget: 2,
                context_character_budget: 6000,
            },
            stage_instructions: StudyStageInstructions {
                semantic: String::new(),
                resolution: resolution_instruction,
                pair: pair_instruction,
            },
        },
    )
    .await
    .unwrap();
    serde_json::from_value(policy["policy"]["policyRef"].clone()).unwrap()
}
async fn add(db: &Database, id: &str) -> Uuid {
    comment_with_author(
        db,
        "selected",
        id,
        "SYNTHETIC 用户评论",
        Some("reader"),
        "2026-09-21T08:00:00Z",
    )
    .await
}
async fn effects(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target),(SELECT count(*) FROM linggan_comment_study_start_request), \
        (SELECT count(*) FROM linggan_model_invocation))").fetch_one(db.pool()).await.unwrap()
}
fn next(command: &StartStudyRunCommand) -> StartStudyRunCommand {
    let mut c = command.clone();
    c.request_ref = Uuid::new_v4();
    c
}

async fn seed_problem_stage_fixtures(
    db: &Database,
    run_ref: Uuid,
) -> (Vec<Uuid>, Uuid, Uuid, Uuid) {
    let targets: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_target \
         WHERE run_ref=$1 ORDER BY target_ref",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(targets.len() >= 2);
    let mut signals = Vec::new();
    for (index, target_ref) in targets.iter().take(2).enumerate() {
        let attempt_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_comment_study_semantic_attempt( \
               attempt_ref,target_ref,attempt_ordinal,request_hash,state,output_manifest,finished_at \
             ) VALUES($1,$2,1,$3,'accepted','{}'::jsonb,scope_001_now())",
        )
        .bind(attempt_ref)
        .bind(target_ref)
        .bind(format!("{:064x}", index + 1))
        .execute(db.pool())
        .await
        .unwrap();
        let signal_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_comment_study_signal( \
               signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence,evidence_start,evidence_end, \
               problem_frame,eligibility_state,canonical_text,canonical_hash \
             ) VALUES($1,$2,$3,'problem','孩子开始任务前会拖延','SYNTHETIC',0,9, \
               '{\"actor\":\"parent\",\"goalOrExpectedState\":\"start\",\"barrierOrUnmetNeed\":\"delay\",\"context\":\"homework\"}', \
               'eligible','孩子开始任务前会拖延','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
        )
        .bind(signal_ref)
        .bind(target_ref)
        .bind(attempt_ref)
        .execute(db.pool())
        .await
        .unwrap();
        signals.push(signal_ref);
    }
    let problem_ref = Uuid::new_v4();
    let revision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem(problem_ref,domain_ref,state) \
         VALUES($1,$2,'active')",
    )
    .bind(problem_ref)
    .bind(domain())
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_revision( \
           revision_ref,problem_ref,domain_ref,identity_version,title,definition,core_frame, \
           inclusions,exclusions,seed_signal_refs,canonical_text,canonical_hash,definition_hash,reason \
         ) VALUES($1,$2,$3,1,'合成问题','合成定义','{\"actor\":\"parent\"}'::jsonb, \
           '[\"纳入\"]'::jsonb,'[]'::jsonb,$4,'合成主体与场景','dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd', \
           $5,'synthetic_seed')",
    )
    .bind(revision_ref)
    .bind(problem_ref)
    .bind(domain())
    .bind(&signals)
    .bind(format!("{}eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", run_ref.simple()))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem SET current_revision_ref=$2 WHERE problem_ref=$1",
    )
    .bind(problem_ref)
    .bind(revision_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let resolution_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest \
         ) VALUES($1,$2,$3,'pending', \
           jsonb_build_object('contract','comment-study.problem-candidate-set.v1', \
             'candidateProblemRefs',jsonb_build_array($4::text)))",
    )
    .bind(resolution_ref)
    .bind(signals[0])
    .bind(domain())
    .bind(problem_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let pair_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_pair( \
           pair_ref,first_signal_ref,second_signal_ref,state,pair_manifest \
         ) VALUES($1,$2,$3,'pending','{}'::jsonb)",
    )
    .bind(pair_ref)
    .bind(signals[0])
    .bind(signals[1])
    .execute(db.pool())
    .await
    .unwrap();
    (signals, problem_ref, resolution_ref, pair_ref)
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL and local child processes; no provider or embedding model"]
async fn embedding_runtime_failure_does_not_block_semantic_and_recall_reports_incomplete() {
    let (db, command, _) = setup("embedding_failure_keeps_semantic_live", 3).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (signals, _, _, _) = seed_problem_stage_fixtures(&db, run_ref).await;
    prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let profile_ref = register_embedding_profile(
        &db,
        "synthetic-failure-proof",
        json!({"backend":"local-test","dtype":"test"}),
        Some(json!({"probe":"synthetic-only","dimension":512})),
    )
    .await
    .unwrap();
    // Keep the Signal searchable so this assertion isolates the missing active Problem core.
    // The failing embedding process will have work to do only for the Problem revision.
    let vector = format!(
        "[{}]",
        std::iter::once("1.0")
            .chain(std::iter::repeat_n("0.0", 511))
            .collect::<Vec<_>>()
            .join(",")
    );
    sqlx::query(
        "INSERT INTO linggan_comment_study_embedding_cache(profile_ref,canonical_hash,embedding) \
         VALUES($1,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',$2::text::public.vector)",
    )
    .bind(profile_ref)
    .bind(vector)
    .execute(db.pool())
    .await
    .unwrap();

    let adapter = PiAdapter::configured_with_test_runtimes(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_settlement_adapter.sh"),
        std::path::PathBuf::from("/bin/false"),
        std::path::PathBuf::from("unused-no-network-runtime"),
    );
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap(),
        "the tick must continue into the semantic lane after local embedding fails"
    );

    let semantic_request: (bool, String, Option<String>) = sqlx::query_as(
        "SELECT request.dispatch_started_at IS NOT NULL,invocation.state,invocation.failure_code \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.run_ref=$1 AND request.stage='semantic'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(semantic_request.0, "semantic crossed the dispatch fence");
    assert_eq!(semantic_request.1, "failed");
    assert_eq!(
        semantic_request.2.as_deref(),
        Some("v2_request_snapshot_verified")
    );

    let recalled = recall_candidates(&db, profile_ref, signals[0])
        .await
        .unwrap();
    assert_eq!(
        recalled.completeness,
        RecallCompleteness::Incomplete {
            reason: "problem_core_vectors_incomplete"
        },
        "an unavailable embedding runtime must not be reported as an empty, complete catalogue"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn preview_and_two_runs_cover_different_comments_from_688() {
    let (db, c, _) = setup("start_688", 688).await;
    let preview = preview_study_selection(&db, c.preview()).await.unwrap();
    assert_eq!(preview["scopeCommentCount"], 688);
    assert_eq!(preview["targetCount"], 100);
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    let first = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(first.run_ref).execute(db.pool()).await.unwrap();
    let second = start_study_run(&db, next(&c), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(second.target_count, 100);
    assert_eq!(second.exclusion_counts["notSelectedByMode"], 100);
    let overlap:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target a JOIN linggan_comment_study_target b \
        ON a.content_public_ref=b.content_public_ref AND a.comment_external_id=b.comment_external_id WHERE a.run_ref=$1 AND b.run_ref=$2")
        .bind(first.run_ref).bind(second.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(overlap, 0);
    assert_eq!(effects(&db).await, json!([2, 200, 2, 0]));
    let limits:Value=sqlx::query_scalar("SELECT jsonb_build_array(comment_budget,context_character_budget,token_limit) FROM linggan_comment_study_run WHERE run_ref=$1")
        .bind(first.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        limits,
        json!([100, 6000, 100000]),
        "no active/default budget substitution"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_identical_request_replays_one_run_and_conflicting_command_fails() {
    let (db, c, _) = setup("start_replay", 3).await;
    let (a, b) = tokio::join!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual),
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.run_ref, b.run_ref);
    assert_ne!(a.idempotent_replay, b.idempotent_replay);
    assert_eq!(effects(&db).await, json!([1, 3, 1, 0]));
    let mut changed = c;
    changed.limits.comment_budget = 2;
    assert!(matches!(
        start_study_run(&db, changed, TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::IdempotencyConflict)
    ));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn concurrent_distinct_requests_never_reserve_the_same_comment() {
    let (db, c, _) = setup("start_race", 150).await;
    let (a, b) = tokio::join!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual),
        start_study_run(&db, next(&c), TrustedStudyOrigin::Manual)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.target_count + b.target_count, 150);
    assert_ne!(a.run_ref, b.run_ref);
    let duplicates:i64=sqlx::query_scalar("SELECT count(*) FROM (SELECT content_public_ref,comment_external_id FROM linggan_comment_study_target \
        GROUP BY 1,2 HAVING count(*)>1) d").fetch_one(db.pool()).await.unwrap();
    assert_eq!(duplicates, 0);
    assert_eq!(effects(&db).await, json!([2, 150, 2, 0]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn empty_and_index_pending_receipts_remain_terminal_after_material_changes() {
    let (db, c, connection) = setup("start_empty", 0).await;
    let empty = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(empty.outcome, "no_work");
    add(&db, "late").await;
    let pending_command = next(&c);
    let pending = start_study_run(&db, pending_command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(pending.outcome, "index_pending");
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    sqlx::query("UPDATE linggan_model_connection SET enabled=false WHERE connection_ref=$1")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
    assert_eq!(
        start_study_run(&db, pending_command, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "index_pending"
    );
    assert_eq!(effects(&db).await, json!([0, 0, 2, 0]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_parent_settles_without_model_and_reobservation_is_not_changed_input() {
    let (db, c, _) = setup("start_context", 0).await;
    reply_with_author(
        &db,
        "selected",
        "child",
        "parent",
        "我也是",
        Some("reader"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let first = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(first.needs_context_count, 1);
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1")
            .bind(first.run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "completed");
    let mut changed = next(&c);
    changed.mode = StudySelectionMode::InputChanged;
    changed.reason = Some("SYNTHETIC parent repair".into());
    assert_eq!(
        start_study_run(&db, changed.clone(), TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
    comment_with_author(
        &db,
        "selected",
        "parent",
        "SYNTHETIC 父语境",
        Some("creator"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let repaired = start_study_run(&db, next(&changed), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(repaired.queued_count, 1);
    sqlx::query("UPDATE linggan_comment_study_target SET state='no_signal',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(repaired.run_ref).execute(db.pool()).await.unwrap();
    reply_with_author(
        &db,
        "selected",
        "child",
        "parent",
        "我也是",
        Some("reader"),
        "2026-09-22T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    assert_eq!(
        start_study_run(&db, next(&changed), TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .outcome,
        "no_work"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn failed_receipt_insert_rolls_back_run_targets_and_can_retry_the_same_request() {
    let (db, c, _) = setup("start_rollback", 2).await;
    sqlx::raw_sql("CREATE FUNCTION cs_proof_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'SYNTHETIC'; END $$; \
        CREATE TRIGGER cs_proof_failure BEFORE INSERT ON linggan_comment_study_start_request FOR EACH ROW EXECUTE FUNCTION cs_proof_fail();")
        .execute(db.pool()).await.unwrap();
    assert!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
            .await
            .is_err()
    );
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    sqlx::raw_sql("DROP TRIGGER cs_proof_failure ON linggan_comment_study_start_request")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .target_count,
        2
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn new_run_is_frozen_and_method_bound_dispatch_accepts_only_the_valid_target() {
    let (db, c, _) = setup("start_guards", 2).await;
    let receipt = start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run = receipt.run_ref.unwrap();
    assert!(
        sqlx::query("UPDATE linggan_comment_study_run SET token_limit=200000 WHERE run_ref=$1")
            .bind(run)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM linggan_comment_study_start_request WHERE request_ref=$1")
            .bind(c.request_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::raw_sql("TRUNCATE linggan_comment_study_start_request")
            .execute(db.pool())
            .await
            .is_err()
    );
    assert_eq!(next_run_needing_batch(&db, &[]).await.unwrap(), Some(run));
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref: run,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claimed = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, claimed.lease_token)
        .await
        .unwrap();
    assert_eq!(reservation.run_ref, run);
    let request: Value = sqlx::query_scalar(
        "SELECT request_manifest FROM linggan_comment_study_model_request WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        request["systemInstruction"]
            .as_str()
            .unwrap()
            .contains("补充研究说明只影响研究侧重点")
    );
    assert_eq!(
        request["outputSchema"]["properties"]["contract"]["enum"],
        json!(["comment-study.note-batch.v1"])
    );
    let (snapshot_hash, invocation_hash): (String, String) = sqlx::query_as(
        "SELECT request.request_hash,invocation.request_hash \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(snapshot_hash, invocation_hash);
    let call_started: bool = sqlx::query_scalar(
        "SELECT result->>'callStarted'='false' FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        call_started,
        "reservation alone must not claim provider I/O"
    );
    mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        claimed.lease_token,
    )
    .await
    .unwrap();
    let target_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target WHERE batch_ref=$1 ORDER BY ordinal",
    )
    .bind(prepared.batch_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(target_refs.len(), 2);
    let synthetic_output = json!({
        "contract":"comment-study.note-batch.v1",
        "batchRef":prepared.batch_ref,
        "contentPublicRef":prepared.content_public_ref,
        "results":[{"targetRef":target_refs[0],"outcome":"no_signal","reason":"SYNTHETIC no_signal result","signals":[]}]
    });
    let accepted = accept_study_batch_output(
        &db,
        prepared.batch_ref,
        claimed.lease_token,
        synthetic_output,
    )
    .await
    .unwrap();
    assert_eq!(accepted.accepted_target_count, 1);
    assert_eq!(accepted.retried_target_count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='no_signal'",
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert!(sqlx::query(
        "UPDATE linggan_comment_study_model_request SET request_hash=repeat('0',64) WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .execute(db.pool())
    .await
    .is_err());
    assert!(sqlx::query(
        "UPDATE linggan_comment_study_model_request SET dispatch_started_at=NULL WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .execute(db.pool())
    .await
    .is_err());
    assert!(
        sqlx::query("DELETE FROM linggan_comment_study_model_request WHERE invocation_ref=$1")
            .bind(reservation.invocation_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::raw_sql("TRUNCATE linggan_comment_study_model_request")
            .execute(db.pool())
            .await
            .is_err()
    );
    let v: Value = sqlx::query_scalar(
        "SELECT execution_manifest FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(v["engineRevision"].as_str().unwrap().len(), 40);
    assert_eq!(effects(&db).await, json!([1, 2, 1, 1]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn twelve_target_partial_response_preserves_valid_results_and_retries_only_bad_targets() {
    // Use the maximum supported context so the full twelve-target batch remains one frozen
    // request rather than being split by method-aware packing.
    let (db, command, _) =
        setup_with_model_limits("partial_response_matrix", 12, 32_768, 8_192, 30).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, claim.lease_token)
        .await
        .unwrap();
    mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        claim.lease_token,
    )
    .await
    .unwrap();

    let target_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target \
         WHERE batch_ref=$1 ORDER BY ordinal",
    )
    .bind(prepared.batch_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(target_refs.len(), 12);

    // Two valid no_signal outcomes; target_refs[2] is omitted; the remaining nine have an
    // invalid outcome enum. This exercises mixed target-local acceptance in one dispatched batch.
    let mut results = vec![
        json!({
            "targetRef":target_refs[0],
            "outcome":"no_signal",
            "reason":"SYNTHETIC no-signal result",
            "signals":[]
        }),
        json!({
            "targetRef":target_refs[1],
            "outcome":"no_signal",
            "reason":"SYNTHETIC no-signal result",
            "signals":[]
        }),
    ];
    results.extend(target_refs.iter().skip(3).map(|target_ref| {
        json!({
            "targetRef":target_ref,
            "outcome":"experience",
            "reason":null,
            "signals":[]
        })
    }));
    let receipt = accept_study_batch_output(
        &db,
        prepared.batch_ref,
        claim.lease_token,
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":prepared.batch_ref,
            "contentPublicRef":prepared.content_public_ref,
            "results":results
        }),
    )
    .await
    .unwrap();

    assert_eq!(receipt.state, "completed_with_failures");
    assert_eq!(receipt.accepted_target_count, 2);
    assert_eq!(receipt.retried_target_count, 10);
    assert_eq!(receipt.failed_target_count, 0);
    assert_eq!(receipt.cancelled_target_count, 0);

    let target_states: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT target_ref,state FROM linggan_comment_study_target \
         WHERE run_ref=$1 ORDER BY target_ref",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(target_states.len(), 12);
    assert_eq!(
        target_states
            .iter()
            .filter(|(_, state)| state == "no_signal")
            .count(),
        2
    );
    assert_eq!(
        target_states
            .iter()
            .filter(|(_, state)| state == "queued")
            .count(),
        10
    );
    assert_eq!(
        target_states
            .iter()
            .find(|(target_ref, _)| *target_ref == target_refs[2])
            .map(|(_, state)| state.as_str()),
        Some("queued"),
        "an omitted target must stay retryable, never become no_signal"
    );

    let attempt_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE state='accepted'), \
                count(*) FILTER (WHERE state='rejected' AND rejection_code='semantic_json_schema'), \
                count(*) FILTER (WHERE state='rejected' AND rejection_code='semantic_target_missing') \
         FROM linggan_comment_study_semantic_attempt WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(attempt_counts, (2, 9, 1));

    let invocation: (String, Option<String>, i64, i64) = sqlx::query_as(
        "SELECT state,failure_code,(result->>'acceptedTargetCount')::bigint, \
                (result->>'retriedTargetCount')::bigint \
         FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(invocation, ("succeeded".into(), None, 2, 10));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn worker_preparation_and_claim_rotate_across_queued_runs() {
    // Each Run has more than MAX_TARGETS_PER_BATCH queued targets. A and then B/C retain real
    // backlog after their first prepared batch, so oldest-first selection would keep choosing
    // the same Run and fail the assertions below.
    let (db, mut command, _) = setup("worker_run_round_robin", 75).await;
    command.limits.comment_budget = 25;
    let mut run_refs = Vec::new();
    for _ in 0..3 {
        let run_ref = start_study_run(&db, next(&command), TrustedStudyOrigin::Manual)
            .await
            .unwrap()
            .run_ref
            .unwrap();
        run_refs.push(run_ref);
    }

    // The initial pick remains oldest-first; after it, the process-local cursor walks the stable
    // Run identity order once and wraps, instead of rescanning the same oldest Run every tick.
    let mut cursor = None;
    let mut order = Vec::new();
    for _ in 0..=run_refs.len() {
        let run_ref = next_run_needing_batch_after(&db, &[], cursor)
            .await
            .unwrap()
            .expect("all Runs still have a queued target");
        cursor = Some(run_ref);
        order.push(run_ref);
    }
    assert_eq!(order[0], order[run_refs.len()]);
    assert_eq!(
        order[..run_refs.len()]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        run_refs.len()
    );

    let mut fairness = ModelWorkerFairness::default();
    let mut prepared_runs = Vec::new();
    for _ in 0..run_refs.len() * 2 {
        let before: std::collections::BTreeMap<Uuid, i64> = sqlx::query_as(
            "SELECT run_ref,count(*)::bigint FROM linggan_comment_study_batch \
             WHERE run_ref=ANY($1) AND state='prepared' GROUP BY run_ref",
        )
        .bind(&run_refs)
        .fetch_all(db.pool())
        .await
        .unwrap()
        .into_iter()
        .collect();
        assert!(
            prepare_next_batch_across_runs_with_fairness(&db, &mut fairness)
                .await
                .unwrap(),
            "each eligible Run should get a batch before a Run gets a second turn"
        );
        let after: Vec<(Uuid, i64)> = sqlx::query_as(
            "SELECT run_ref,count(*)::bigint FROM linggan_comment_study_batch \
             WHERE run_ref=ANY($1) AND state='prepared' GROUP BY run_ref",
        )
        .bind(&run_refs)
        .fetch_all(db.pool())
        .await
        .unwrap();
        let newly_prepared: Vec<_> = after
            .into_iter()
            .filter(|(run_ref, count)| *count > before.get(run_ref).copied().unwrap_or(0))
            .map(|(run_ref, _)| run_ref)
            .collect();
        assert_eq!(newly_prepared.len(), 1);
        prepared_runs.push(newly_prepared[0]);
    }
    assert_eq!(
        prepared_runs[..run_refs.len()],
        prepared_runs[run_refs.len()..]
    );
    assert_eq!(
        prepared_runs[..run_refs.len()]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        run_refs.len(),
        "every Run receives its next batch while all three still have queued backlog"
    );
    for run_ref in &run_refs {
        let prepared_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_comment_study_batch \
             WHERE run_ref=$1 AND state='prepared'",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(prepared_count, 2);
        let queued_targets: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='queued'",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(queued_targets > 0, "each Run must retain a true backlog");
    }

    // Give the first Run one extra prepared batch to make the claim side's backlog asymmetric.
    prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref: run_refs[0],
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();

    // Claim has its own cursor over already-prepared work, so a backlog from one Run cannot
    // monopolize a worker after a restart or after concurrent workers expose older batches.
    let mut claim_cursor = None;
    let mut claimed_runs = Vec::new();
    for _ in 0..run_refs.len() * 2 {
        let claim = claim_next_study_batch_after(&db, Uuid::new_v4(), 60, &mut claim_cursor)
            .await
            .unwrap()
            .expect("each Run has at least two prepared batches");
        let run_ref: Uuid = sqlx::query_scalar(
            "SELECT run_ref FROM linggan_comment_study_batch WHERE batch_ref=$1",
        )
        .bind(claim.batch_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        claimed_runs.push(run_ref);
    }
    assert_eq!(
        claimed_runs[..run_refs.len()],
        claimed_runs[run_refs.len()..]
    );
    assert_eq!(
        claimed_runs[..run_refs.len()]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        run_refs.len(),
        "claim rotates across all Runs even though the first Run has the larger queue"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_batch_lease_recovery_is_bounded_to_32_per_tick() {
    let (db, command, _) = setup("expired_batch_recovery_bound", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_comment_study_work \
         WHERE run_ref=$1 ORDER BY content_public_ref LIMIT 1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_batch( \
           batch_ref,run_ref,content_public_ref,state,input_manifest,input_hash, \
           lease_token,leased_by,lease_expires_at \
         ) SELECT gen_random_uuid(),$1,$2,'leased','{}'::jsonb,repeat('a',64), \
                  gen_random_uuid(),gen_random_uuid(),scope_001_now()-interval '1 second' \
           FROM generate_series(1,33)",
    )
    .bind(run_ref)
    .bind(content_public_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 32);
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_batch \
         WHERE run_ref=$1 AND state='leased' AND lease_expires_at<=scope_001_now()",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        remaining, 1,
        "one expired batch must carry over to the next tick"
    );

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 1);
    let (expired_leased, failed): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE state='leased'),count(*) FILTER (WHERE state='failed') \
         FROM linggan_comment_study_batch WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((expired_leased, failed), (0, 33));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn response_after_request_deadline_is_not_accepted_after_run_stop() {
    let (db, command, _) =
        setup_with_model_config("late_response_after_deadline", 2, 8192, 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claimed = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, claimed.lease_token)
        .await
        .unwrap();
    let effective_timeout_ms = mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        claimed.lease_token,
    )
    .await
    .unwrap();
    assert!(effective_timeout_ms > 0 && effective_timeout_ms <= 750);

    let stopped = cancel_study_run(&db, domain(), run_ref).await.unwrap();
    assert_eq!(stopped.dispatch_reason.as_deref(), Some("user_stopped"));
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    sqlx::query(
        "UPDATE linggan_model_invocation SET input_tokens=17,output_tokens=4,charged_tokens=21 \
         WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .execute(db.pool())
    .await
    .unwrap();

    let target_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target WHERE batch_ref=$1 ORDER BY ordinal",
    )
    .bind(prepared.batch_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    let late_output = json!({
        "contract":"comment-study.note-batch.v1",
        "batchRef":prepared.batch_ref,
        "contentPublicRef":prepared.content_public_ref,
        "results":target_refs.iter().map(|target_ref| json!({
            "targetRef":target_ref,
            "outcome":"no_signal",
            "reason":"SYNTHETIC late response",
            "signals":[]
        })).collect::<Vec<_>>()
    });
    let receipt =
        accept_study_batch_output(&db, prepared.batch_ref, claimed.lease_token, late_output)
            .await
            .unwrap();
    assert_eq!(receipt.accepted_target_count, 0);
    assert_eq!(receipt.cancelled_target_count, 2);
    assert_eq!(receipt.retried_target_count, 0);
    let rejected_attempts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_semantic_attempt \
         WHERE batch_ref=$1 AND state='rejected' \
           AND rejection_code='semantic_batch_contract' \
           AND output_manifest->>'failureCode'='request_deadline_expired'",
    )
    .bind(prepared.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rejected_attempts, 2);

    let target_states: Vec<(String, String)> = sqlx::query_as(
        "SELECT state,terminal_reason FROM linggan_comment_study_target \
         WHERE run_ref=$1 ORDER BY target_ref",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(target_states.len(), 2);
    assert!(
        target_states
            .iter()
            .all(|(state, reason)| { state == "cancelled" && reason == "user_stopped" })
    );
    let no_signal_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='no_signal'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(no_signal_count, 0);
    let invocation: (String, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT state,failure_code,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        invocation,
        (
            "failed".to_owned(),
            Some("request_deadline_expired".to_owned()),
            Some(21)
        )
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn insufficient_run_token_budget_stops_the_undispatched_run_without_an_invocation() {
    let (db, mut command, _) = setup("start_token_budget", 2).await;
    command.limits.token_limit = 1024;
    let receipt = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run_ref = receipt.run_ref.unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claimed = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        reserve_study_batch_model_call(&db, prepared.batch_ref, claimed.lease_token).await,
        Err(StudyModelDispatchError::BudgetExhausted)
    ));
    let run: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('state',state,'dispatchState',dispatch_state, \
         'dispatchReason',dispatch_reason,'controlVersion',control_version) \
         FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(run["dispatchState"], "stopped");
    assert_eq!(run["dispatchReason"], "budget_exhausted");
    assert_eq!(run["controlVersion"], 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='cancelled' AND terminal_reason='budget_exhausted' \
               AND finished_at IS NOT NULL",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "budget rejection must precede any provider ledger reservation"
    );
    assert_eq!(next_run_needing_batch(&db, &[]).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn method_aware_batch_packing_splits_targets_before_the_input_limit() {
    let (db, mut command, _) = setup("start_method_input_split", 2).await;
    let baseline = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let rows = sqlx::query(
        "SELECT target.target_ref,target.source_ref,target.research_text, \
                target.dependency_state,target.input_manifest,work.content_public_ref,work.context_manifest \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_work work USING(run_ref,content_public_ref) \
         WHERE target.run_ref=$1 ORDER BY target.created_at,target.target_ref",
    )
    .bind(baseline)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    let work_ref: Uuid = rows[0].get("content_public_ref");
    let context: Value = rows[0].get("context_manifest");
    let target_values = rows
        .iter()
        .map(|row| {
            let input: Value = row.get("input_manifest");
            json!({
                "targetRef":row.get::<Uuid,_>("target_ref"),
                "sourceRef":row.get::<Uuid,_>("source_ref"),
                "researchText":row.get::<String,_>("research_text"),
                "dependencyState":row.get::<String,_>("dependency_state"),
                "parentContext":input["parentContext"],
            })
        })
        .collect::<Vec<_>>();
    let build_input = |targets: &[Value]| {
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":Uuid::nil(),
            "runRef":baseline,
            "workRef":work_ref,
            "workContext":context,
            "targets":targets,
        })
    };
    let baseline_method: Value = sqlx::query_scalar(
        "SELECT to_jsonb(policy)->'method_manifest' FROM linggan_comment_study_policy policy WHERE policy_ref=$1",
    )
    .bind(command.policy_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let schema = baseline_method["stages"]["semantic"]["outputSchema"].clone();
    let system_instruction = baseline_method["stages"]["semantic"]["systemInstruction"]
        .as_str()
        .unwrap();
    let request_size = |input: &Value, instruction: &str| {
        let prompt = serde_json::to_string(&json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":Uuid::nil(),
            "runRef":baseline,
            "input":input,
            "outputSchema":schema,
        }))
        .unwrap();
        let manifest = json!({
            "stage":"semantic",
            "systemInstruction":instruction,
            "prompt":prompt,
            "outputSchema":schema,
        });
        let mut estimate = 256_i64;
        let mut ascii = 0_i64;
        for character in manifest.to_string().chars() {
            if character.is_ascii() {
                ascii += 1;
            } else {
                estimate += 2;
            }
        }
        estimate + (ascii + 3) / 4
    };
    let single_input = build_input(&target_values[..1]);
    let pair_input = build_input(&target_values);
    let base_single = request_size(&single_input, system_instruction);
    let mut extension_chars = usize::try_from((8192 - base_single).max(1) / 2).unwrap_or(1);
    let mut fitting_policy_ref = None;
    for _ in 0..8 {
        let policy_ref = policy_with_semantic_instruction(
            &db,
            &command,
            "界".repeat(extension_chars.clamp(1, 7900)),
        )
        .await;
        let method: Value = sqlx::query_scalar(
            "SELECT to_jsonb(policy)->'method_manifest' FROM linggan_comment_study_policy policy WHERE policy_ref=$1",
        )
        .bind(policy_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let instruction = method["stages"]["semantic"]["systemInstruction"]
            .as_str()
            .unwrap();
        let single = request_size(&single_input, instruction);
        let pair = request_size(&pair_input, instruction);
        if single <= 8192 && pair > 8192 {
            fitting_policy_ref = Some(policy_ref);
            break;
        }
        if single > 8192 {
            extension_chars = extension_chars
                .saturating_sub(usize::try_from((single - 8192 + 1) / 2).unwrap_or(1).max(1));
        } else {
            extension_chars = extension_chars
                .saturating_add(usize::try_from((8192 - pair + 1) / 2).unwrap_or(1).max(1));
        }
    }
    let fitting_policy_ref =
        fitting_policy_ref.expect("a single-target-only input boundary exists");
    sqlx::query(
        "UPDATE linggan_comment_study_target SET state='failed',finished_at=scope_001_now(), \
         terminal_reason='attempts_exhausted' WHERE run_ref=$1 AND state='queued'",
    )
    .bind(baseline)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_run SET state='completed_with_failures',finished_at=scope_001_now() WHERE run_ref=$1",
    )
    .bind(baseline)
    .execute(db.pool())
    .await
    .unwrap();
    command.policy_ref = fitting_policy_ref;
    command.request_ref = Uuid::new_v4();
    command.mode = StudySelectionMode::RetryFailed;
    command.reason = Some("synthetic input fitting boundary".to_owned());
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    let first = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(first.target_refs.len(), 1);
    let second = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(second.target_refs.len(), 1);
    assert_ne!(first.target_refs[0], second.target_refs[0]);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn one_target_over_the_frozen_method_input_limit_is_terminal_without_invocation() {
    let (db, mut command, _) = setup("start_method_input_reject", 1).await;
    command.policy_ref = policy_with_semantic_instruction(&db, &command, "界".repeat(4000)).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    assert!(matches!(
        prepare_study_batch(
            &db,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: 12,
            },
        )
        .await,
        Err(linggan_intelligence::comment_study_batch::StudyBatchError::InputLimitExceeded)
    ));
    let target: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object('state',state,'reason',terminal_reason,'finished',finished_at IS NOT NULL) \
         FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target["state"], "failed");
    assert_eq!(target["reason"], "input_limit_exceeded");
    assert_eq!(target["finished"], true);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT (SELECT count(*) FROM linggan_comment_study_batch WHERE run_ref=$1) + \
                    (SELECT count(*) FROM linggan_model_invocation)",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    let run_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(run_state, "completed_with_failures");
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn valid_1024_model_context_terminalizes_the_default_instruction_overflow() {
    let (db, command, _) =
        setup_with_model_input_limit("start_default_instruction_overflow", 1, 1024).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    assert!(matches!(
        prepare_study_batch(
            &db,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: 12,
            },
        )
        .await,
        Err(linggan_intelligence::comment_study_batch::StudyBatchError::InputLimitExceeded)
    ));
    let target_reason: Option<String> = sqlx::query_scalar(
        "SELECT terminal_reason FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target_reason.as_deref(), Some("input_limit_exceeded"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn measured_running_usage_replaces_the_reservation_for_future_budget_checks() {
    let (db, mut command, _) = setup("start_measured_usage_budget", 2).await;
    command.limits.token_limit = 10_000;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let first = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let second = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let first_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let first_reservation =
        reserve_study_batch_model_call(&db, first.batch_ref, first_lease.lease_token)
            .await
            .unwrap();
    let (reserved_tokens, charged_before_call): (i64, i64) = sqlx::query_as(
        "SELECT reserved_tokens,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(first_reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);
    assert_eq!(charged_before_call, 0);
    assert!(reserved_tokens.saturating_mul(2) <= 10_000);

    mark_study_batch_model_dispatch_started(
        &db,
        first_reservation.invocation_ref,
        first.batch_ref,
        first_lease.lease_token,
    )
    .await
    .unwrap();
    let actual_charge = 10_001 - reserved_tokens;
    let input_tokens = actual_charge.min(8192);
    let output_tokens = actual_charge - input_tokens;
    assert!(input_tokens <= 8192 && output_tokens <= 1024);
    checkpoint_invocation_usage(
        &db,
        first_reservation.invocation_ref,
        Some(&PiResponse {
            version: "synthetic".to_owned(),
            ok: true,
            text: Some("synthetic usage checkpoint".to_owned()),
            failure_code: None,
            model_ids: None,
            model_list_origin: None,
            usage: PiUsage {
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
                cost_usd: None,
            },
            elapsed_ms: Some(10),
            diagnostic: None,
        }),
    )
    .await
    .unwrap();
    let charged_after_checkpoint: i64 = sqlx::query_scalar(
        "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(first_reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(charged_after_checkpoint, actual_charge);

    let second_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second_lease.batch_ref, second.batch_ref);
    assert!(matches!(
        reserve_study_batch_model_call(&db, second.batch_ref, second_lease.lease_token).await,
        Err(StudyModelDispatchError::BudgetDeferred)
    ));
    let second_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1")
            .bind(second.batch_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(second_state, "prepared");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );

    let first_target: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_batch_target WHERE batch_ref=$1",
    )
    .bind(first.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    accept_study_batch_output(
        &db,
        first.batch_ref,
        first_lease.lease_token,
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":first.batch_ref,
            "contentPublicRef":first.content_public_ref,
            "results":[{"targetRef":first_target,"outcome":"no_signal","reason":"SYNTHETIC","signals":[]}]
        }),
    )
    .await
    .unwrap();
    let second_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        reserve_study_batch_model_call(&db, second.batch_ref, second_lease.lease_token).await,
        Err(StudyModelDispatchError::BudgetExhausted)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL and local Pi child; no provider call"]
async fn v2_frozen_method_and_batch_reach_the_configured_worker_adapter() {
    let (db, command, _) = setup("start_v2_adapter_boundary", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let adapter = PiAdapter::configured_with_test_command(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_settlement_adapter.sh"),
    );
    let error = call_study_batch_model(
        &db,
        &SyntheticModelSecrets,
        &adapter,
        prepared.batch_ref,
        claim.lease_token,
    )
    .await
    .expect_err("the local child verifies the frozen v2 request then returns a synthetic failure");
    assert!(matches!(error, StudyModelRunnerError::ProviderFailure));
    let (request_hash, dispatch_started, state, failure_code, call_started): (
        String,
        bool,
        String,
        Option<String>,
        bool,
    ) = sqlx::query_as(
        "SELECT request.request_hash,request.dispatch_started_at IS NOT NULL, \
                invocation.state,invocation.failure_code,invocation.result->>'callStarted'='true' \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.run_ref=$1 AND request.stage='semantic'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(request_hash.len(), 64);
    assert!(dispatch_started);
    assert_eq!(state, "failed");
    assert_eq!(
        failure_code.as_deref(),
        Some("v2_request_snapshot_verified")
    );
    assert!(call_started);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "queued"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_dispatched_invocation_keeps_conservative_budget_charge_without_usage() {
    let (db, command, _) = setup("start_expired_dispatched_budget", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, lease.lease_token)
        .await
        .unwrap();
    let reserved_tokens: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);

    mark_study_batch_model_dispatch_started(
        &db,
        reservation.invocation_ref,
        prepared.batch_ref,
        lease.lease_token,
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET lease_expires_at=scope_001_now()-interval '1 second' WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 1);
    let (state, charged_tokens): (String, i64) = sqlx::query_as(
        "SELECT state,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "failed");
    assert_eq!(charged_tokens, reserved_tokens);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_undispatched_invocation_releases_its_budget_reservation() {
    let (db, command, _) = setup("start_expired_undispatched_budget", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let reservation = reserve_study_batch_model_call(&db, prepared.batch_ref, lease.lease_token)
        .await
        .unwrap();
    let reserved_tokens: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(reserved_tokens > 0);
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET lease_expires_at=scope_001_now()-interval '1 second' WHERE batch_ref=$1",
    )
    .bind(prepared.batch_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert_eq!(recover_expired_study_batch_leases(&db).await.unwrap(), 1);
    let (state, charged_tokens): (String, i64) = sqlx::query_as(
        "SELECT state,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "failed");
    assert_eq!(charged_tokens, 0);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn start_acquires_lock_before_choosing_and_sees_the_post_wait_committed_material() {
    let (db, c, _) = setup("start_wait_snapshot", 0).await;
    let mut holder = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(study_domain_lock_key(domain()).unwrap())
        .execute(&mut *holder)
        .await
        .unwrap();
    let other = db.clone();
    let task =
        tokio::spawn(async move { start_study_run(&other, c, TrustedStudyOrigin::Manual).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted)",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    add(&db, "after-wait").await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    holder.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap().target_count, 1);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn missing_scope_or_incomplete_guards_never_create_partial_work() {
    let (db, mut c, _) = setup("start_scope", 1).await;
    c.scope = serde_json::from_value(json!({"kind":"works","workRefs":[Uuid::new_v4()]})).unwrap();
    assert!(matches!(
        start_study_run(&db, c.clone(), TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::NotFound)
    ));
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
    sqlx::raw_sql("ALTER TABLE linggan_comment_study_target DISABLE TRIGGER cs_target_input_guard")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::SchemaUnavailable)
    ));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn problem_stage_requests_share_budget_and_recover_expired_dispatches() {
    let (db, command, _) = setup("problem_stage_request_ledger", 2).await;
    let receipt = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run_ref = receipt.run_ref.unwrap();
    let method_hash: String = sqlx::query_scalar(
        "SELECT policy.method_hash FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) WHERE run.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let (signals, problem_ref, resolution_ref, pair_ref) =
        seed_problem_stage_fixtures(&db, run_ref).await;

    let adapter = PiAdapter::configured();
    let resolution_result = run_one_problem_resolution(&db, &NoModelSecrets, &adapter).await;
    assert!(
        matches!(
            &resolution_result,
            Err(
                linggan_intelligence::comment_study_resolution_worker::ResolutionWorkerError::Model(
                    ModelError::SecretUnavailable
                )
            )
        ),
        "resolution worker result: {resolution_result:?}"
    );
    let pair_result = run_one_problem_pair(&db, &NoModelSecrets, &adapter).await;
    assert!(
        matches!(
            &pair_result,
            Err(
                linggan_intelligence::comment_study_pair_worker::PairWorkerError::Model(
                    ModelError::SecretUnavailable
                )
            )
        ),
        "pair worker result: {pair_result:?}"
    );

    let rows = sqlx::query(
        "SELECT request.stage,request.attempt_ordinal,request.request_manifest,request.request_hash, \
                request.dispatch_started_at::text AS dispatch_started_at,invocation.state,invocation.charged_tokens \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.run_ref=$1 ORDER BY request.stage",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let stage: String = row.get("stage");
        let request_manifest: Value = row.get("request_manifest");
        let request_hash: String = row.get("request_hash");
        assert_eq!(row.get::<i32, _>("attempt_ordinal"), 1);
        assert_eq!(request_manifest["stage"], stage);
        assert_eq!(request_manifest["methodHash"], method_hash);
        assert_eq!(request_manifest["stageHash"].as_str().unwrap().len(), 64);
        assert_eq!(row.get::<Option<String>, _>("dispatch_started_at"), None);
        assert_eq!(row.get::<String, _>("state"), "failed");
        assert_eq!(row.get::<i64, _>("charged_tokens"), 0);
        assert_eq!(request_hash.len(), 64);
    }
    let pending: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT resolution.model_invocation_ref,pair.model_invocation_ref \
         FROM linggan_comment_study_resolution resolution,linggan_comment_study_problem_pair pair \
         WHERE resolution.resolution_ref=$1 AND pair.pair_ref=$2",
    )
    .bind(resolution_ref)
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(pending, (None, None));

    let resolution_invocation: Uuid = sqlx::query_scalar(
        "SELECT invocation_ref FROM linggan_comment_study_model_request \
         WHERE run_ref=$1 AND stage='resolution'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"UPDATE linggan_model_invocation SET state='running',reserved_tokens=100000,charged_tokens=0,
           input_tokens=NULL,output_tokens=NULL,failure_code=NULL,finished_at=NULL,
           result='{"callStarted":true}'::jsonb WHERE invocation_ref=$1"#,
    )
    .bind(resolution_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_model_request SET dispatch_started_at=scope_001_now() \
         WHERE invocation_ref=$1",
    )
    .bind(resolution_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 \
         WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .bind(resolution_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        accept_problem_resolution(&db, resolution_ref, json!({})).await,
        Err(linggan_intelligence::comment_study_problem_store::ProblemStoreError::ResolutionUnavailable)
    ));
    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap(),
        "an in-flight resolution reservation consumes the same Run budget as pair work"
    );
    let pair_request_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_model_request \
         WHERE run_ref=$1 AND stage='pair'",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        pair_request_count, 1,
        "budget deferral creates no second pair request"
    );

    // A stop fences future dispatch, while this already-dispatched and still-unexpired response
    // remains eligible for settlement as required by the P3 control contract.
    sqlx::query(
        "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
           dispatch_reason='user_stopped',control_version=control_version+1 WHERE run_ref=$1",
    )
    .bind(run_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let settled_after_stop = linggan_intelligence::comment_study_problem_store::
        accept_problem_resolution_from_invocation(
            &db,
            resolution_ref,
            resolution_invocation,
            json!({
                "contract":"comment-study.problem-resolution.v1",
                "candidates":[{
                    "problemRef":problem_ref,
                    "dimensions":{
                        "actor":"different",
                        "goalOrExpectedState":"different",
                        "barrierOrUnmetNeed":"different",
                        "context":"different"
                    }
                }]
            }),
        )
        .await
        .unwrap();
    assert_eq!(settled_after_stop.state, "deferred_novel");

    let (config_ref, model_ref, version_ref, policy_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT config.config_ref,model.model_ref,version.version_ref,run.policy_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let expired_pair_invocation = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',1500,0,'{\"callStarted\":true}'::jsonb)",
    )
    .bind(expired_pair_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,pair_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'pair',$4,2, \
           'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb, \
           'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff', \
           scope_001_now()-interval '1 second',scope_001_now()-interval '1 second')",
    )
    .bind(expired_pair_invocation)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(pair_ref)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(expired_pair_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        accept_problem_pair(&db, pair_ref, json!({})).await,
        Err(linggan_intelligence::comment_study_problem_store::ProblemStoreError::PairUnavailable)
    ));
    assert!(
        run_model_work_once(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let (recovered_state, recovered_charge, recovered_code): (String, i64, String) =
        sqlx::query_as(
            "SELECT state,charged_tokens,failure_code FROM linggan_model_invocation \
             WHERE invocation_ref=$1",
        )
        .bind(expired_pair_invocation)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        (
            recovered_state.as_str(),
            recovered_charge,
            recovered_code.as_str()
        ),
        ("failed", 1500, "request_deadline_expired")
    );
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 \
         WHERE pair_ref=$1 AND state='pending'",
    )
    .bind(pair_ref)
    .bind(expired_pair_invocation)
    .execute(db.pool())
    .await
    .unwrap();

    // Simulate a crash after a terminal resolution committed but before its generic invocation
    // receipt was finalized. Recovery must settle the receipt while preserving the accepted row.
    let terminal_subject_invocation = Uuid::new_v4();
    let terminal_subject_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',1500,0,'{\"callStarted\":true}'::jsonb)",
    )
    .bind(terminal_subject_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(terminal_subject_hash)
    .execute(db.pool())
    .await
    .unwrap();
    let terminal_subject_target: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_comment_study_target WHERE run_ref=$1 ORDER BY target_ref LIMIT 1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let terminal_subject_attempt = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_semantic_attempt( \
           attempt_ref,target_ref,attempt_ordinal,request_hash,state,output_manifest,finished_at \
         ) VALUES($1,$2,2,$3,'accepted','{}'::jsonb,scope_001_now())",
    )
    .bind(terminal_subject_attempt)
    .bind(terminal_subject_target)
    .bind(terminal_subject_hash)
    .execute(db.pool())
    .await
    .unwrap();
    let terminal_subject_signal = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_signal( \
           signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence, \
           evidence_start,evidence_end,eligibility_state \
         ) VALUES($1,$2,$3,'question','合成问题','SYNTHETIC',0,9,'not_applicable')",
    )
    .bind(terminal_subject_signal)
    .bind(terminal_subject_target)
    .bind(terminal_subject_attempt)
    .execute(db.pool())
    .await
    .unwrap();
    let terminal_subject_resolution = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,decision_manifest, \
           model_invocation_ref,resolved_at \
         ) VALUES($1,$2,$3,'deferred_novel', \
           '{\"contract\":\"comment-study.problem-candidate-set.v1\",\"candidateProblemRefs\":[]}'::jsonb, \
           '{\"reason\":\"no_matching_problem\"}'::jsonb,$4,scope_001_now())",
    )
    .bind(terminal_subject_resolution)
    .bind(terminal_subject_signal)
    .bind(domain())
    .bind(terminal_subject_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'resolution',$4,2, \
           '9999999999999999999999999999999999999999999999999999999999999999', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb,$5, \
           scope_001_now()-interval '1 minute',scope_001_now()-interval '1 second')",
    )
    .bind(terminal_subject_invocation)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(terminal_subject_resolution)
    .bind(terminal_subject_hash)
    .execute(db.pool())
    .await
    .unwrap();

    // Simulate the other crash window: invocation failure committed, subject pointer release did
    // not. The second and configured-final attempt must close the resolution; the pair's second
    // and final expired request must also close instead of becoming claimable again.
    let orphan_resolution_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest \
         ) VALUES($1,$2,$3,'pending', \
           jsonb_build_object('contract','comment-study.problem-candidate-set.v1', \
             'candidateProblemRefs',jsonb_build_array($4::text)))",
    )
    .bind(orphan_resolution_ref)
    .bind(signals[1])
    .bind(domain())
    .bind(problem_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let orphan_resolution_invocation = Uuid::new_v4();
    let orphan_hash = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,failure_code,result,finished_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'failed',1500,1500,'provider_failed', \
           '{\"ok\":false}'::jsonb,scope_001_now())",
    )
    .bind(orphan_resolution_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(orphan_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'resolution',$4,2, \
           'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb,$5, \
           scope_001_now()-interval '1 minute',scope_001_now()+interval '1 hour')",
    )
    .bind(orphan_resolution_invocation)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(orphan_resolution_ref)
    .bind(orphan_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 \
         WHERE resolution_ref=$1",
    )
    .bind(orphan_resolution_ref)
    .bind(orphan_resolution_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        run_model_work_once(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let (terminal_resolution_state, terminal_resolution_pointer): (String, Option<Uuid>) =
        sqlx::query_as(
            "SELECT state,model_invocation_ref FROM linggan_comment_study_resolution \
             WHERE resolution_ref=$1",
        )
        .bind(terminal_subject_resolution)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        (
            terminal_resolution_state.as_str(),
            terminal_resolution_pointer
        ),
        ("deferred_novel", Some(terminal_subject_invocation))
    );
    let (terminal_invocation_state, terminal_invocation_charge, terminal_invocation_code): (
        String,
        i64,
        String,
    ) = sqlx::query_as(
        "SELECT state,charged_tokens,failure_code FROM linggan_model_invocation \
         WHERE invocation_ref=$1",
    )
    .bind(terminal_subject_invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (
            terminal_invocation_state.as_str(),
            terminal_invocation_charge,
            terminal_invocation_code.as_str()
        ),
        ("failed", 1500, "request_deadline_expired")
    );
    let (orphan_state, orphan_reason, orphan_pointer): (String, String, Option<Uuid>) =
        sqlx::query_as(
            "SELECT state,decision_manifest->>'reason',model_invocation_ref \
             FROM linggan_comment_study_resolution WHERE resolution_ref=$1",
        )
        .bind(orphan_resolution_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        (
            orphan_state.as_str(),
            orphan_reason.as_str(),
            orphan_pointer
        ),
        (
            "failed",
            "attempts_exhausted",
            Some(orphan_resolution_invocation)
        )
    );
    let (pair_state, pair_pointer): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,model_invocation_ref FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (pair_state.as_str(), pair_pointer),
        ("failed", Some(expired_pair_invocation))
    );
    let (accepted_state, accepted_pointer): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,model_invocation_ref FROM linggan_comment_study_resolution WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (accepted_state.as_str(), accepted_pointer),
        ("deferred_novel", Some(resolution_invocation))
    );
    let late_pair_result =
        linggan_intelligence::comment_study_problem_store::accept_problem_pair_from_invocation(
            &db,
            pair_ref,
            expired_pair_invocation,
            json!({"decision":"create","definition":{}}),
        )
        .await;
    assert!(matches!(
        late_pair_result,
        Err(linggan_intelligence::comment_study_problem_store::ProblemStoreError::
            ModelRequestUnavailable)
    ));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn failed_resolution_and_pair_ledgers_settle_pending_subject_pointers() {
    let (db, command, _) = setup("failed_problem_request_pointers", 2).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let (policy_ref, config_ref, model_ref, version_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT run.policy_ref,config.config_ref,model.model_ref,version.version_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();

    let stages: [(&str, Option<Uuid>, Option<Uuid>, &str, &str); 2] = [
        (
            "resolution",
            Some(resolution_ref),
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ),
        (
            "pair",
            None,
            Some(pair_ref),
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        ),
    ];
    let mut invocation_refs = Vec::new();
    for (stage, resolution_subject, pair_subject, input_hash, request_hash) in stages {
        let invocation_ref = Uuid::new_v4();
        invocation_refs.push(invocation_ref);
        sqlx::query(
            "INSERT INTO linggan_model_invocation( \
               invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
               state,reserved_tokens,charged_tokens,failure_code,result,finished_at \
             ) VALUES($1,$2,$3,$4,'analyze',$5,'failed',1500,1500,'provider_failed', \
               '{\"ok\":false,\"callStarted\":true}'::jsonb,scope_001_now())",
        )
        .bind(invocation_ref)
        .bind(version_ref)
        .bind(model_ref)
        .bind(config_ref)
        .bind(request_hash)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_comment_study_model_request( \
               invocation_ref,run_ref,policy_ref,stage,resolution_ref,pair_ref,attempt_ordinal, \
               input_context_hash,request_manifest,request_hash,dispatch_started_at,deadline_at \
             ) VALUES($1,$2,$3,$4,$5,$6,2,$7, \
               jsonb_build_object('contract','comment-study.model-request.v1','stage',$4),$8, \
               scope_001_now()-interval '1 minute',scope_001_now()+interval '1 hour')",
        )
        .bind(invocation_ref)
        .bind(run_ref)
        .bind(policy_ref)
        .bind(stage)
        .bind(resolution_subject)
        .bind(pair_subject)
        .bind(input_hash)
        .bind(request_hash)
        .execute(db.pool())
        .await
        .unwrap();
        match stage {
            "resolution" => {
                sqlx::query(
                    "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 \
                     WHERE resolution_ref=$1 AND state='pending'",
                )
                .bind(resolution_ref)
                .bind(invocation_ref)
                .execute(db.pool())
                .await
                .unwrap();
            }
            "pair" => {
                sqlx::query(
                    "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 \
                     WHERE pair_ref=$1 AND state='pending'",
                )
                .bind(pair_ref)
                .bind(invocation_ref)
                .execute(db.pool())
                .await
                .unwrap();
            }
            _ => unreachable!("the proof contains only problem stages"),
        }
    }

    // Keep the recovery tick from claiming new work after it settles the stale subject pointers.
    sqlx::query(
        "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
           dispatch_reason='user_stopped',control_version=control_version+1 WHERE run_ref=$1",
    )
    .bind(run_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "a worker tick must settle both failed ledgers whose subjects still point to them"
    );

    let resolution: (String, Option<Uuid>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT state,model_invocation_ref,decision_manifest->>'reason', \
                decision_manifest->>'lastFailureCode' \
         FROM linggan_comment_study_resolution WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        resolution,
        (
            "failed".into(),
            Some(invocation_refs[0]),
            Some("attempts_exhausted".into()),
            Some("provider_failed".into())
        )
    );
    let pair: (String, Option<Uuid>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT state,model_invocation_ref,pair_manifest->'decision'->>'code', \
                pair_manifest->'decision'->>'lastFailureCode' \
         FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        pair,
        (
            "failed".into(),
            Some(invocation_refs[1]),
            Some("attempts_exhausted".into()),
            Some("provider_failed".into())
        )
    );
    let invocations: Vec<(Uuid, String, i64, String)> = sqlx::query_as(
        "SELECT invocation_ref,state,charged_tokens,failure_code \
         FROM linggan_model_invocation WHERE invocation_ref=ANY($1) ORDER BY invocation_ref",
    )
    .bind(&invocation_refs)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(invocations.len(), 2);
    assert!(invocations.iter().all(|(_, state, charged, code)| {
        state == "failed" && *charged == 1500 && code == "provider_failed"
    }));
    let request_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(request_count, 2, "recovery must not invent another request");
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn oversized_semantic_target_is_terminal_and_does_not_block_later_runs() {
    let (db, base_command, _) = setup("oversized_target_scheduler_progress", 0).await;
    let oversized_body = (0..240)
        .map(|index| {
            format!("第 {index} 次记录中，孩子开始作业前先说出遇到的具体困难，再和家长安排下一步。")
        })
        .collect::<String>();
    comment_with_author(
        &db,
        "selected",
        "a-oversized",
        &oversized_body,
        Some("reader-a"),
        "2026-09-19T08:00:00Z",
    )
    .await;
    detail_with_author(
        &db,
        "normal-b",
        "SYNTHETIC normal work B",
        Some("creator-b"),
    )
    .await;
    comment_with_author(
        &db,
        "normal-b",
        "b-normal-1",
        "SYNTHETIC 常规评论一",
        Some("reader-b1"),
        "2026-09-20T08:00:00Z",
    )
    .await;
    comment_with_author(
        &db,
        "normal-b",
        "b-normal-2",
        "SYNTHETIC 常规评论二",
        Some("reader-b2"),
        "2026-09-21T08:00:00Z",
    )
    .await;
    detail_with_author(
        &db,
        "normal-c",
        "SYNTHETIC normal work C",
        Some("creator-c"),
    )
    .await;
    comment_with_author(
        &db,
        "normal-c",
        "c-normal-1",
        "SYNTHETIC 另一 Run 的常规评论",
        Some("reader-c"),
        "2026-09-22T08:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();

    let work_a: Uuid = sqlx::query_scalar(
        "SELECT usage.content_public_ref FROM linggan_material_domain_usage usage \
         JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref \
         WHERE usage.domain_ref=$1 AND content.content_external_id='selected'",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();
    let work_b: Uuid = sqlx::query_scalar(
        "SELECT usage.content_public_ref FROM linggan_material_domain_usage usage \
         JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref \
         WHERE usage.domain_ref=$1 AND content.content_external_id='normal-b'",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();
    let work_c: Uuid = sqlx::query_scalar(
        "SELECT usage.content_public_ref FROM linggan_material_domain_usage usage \
         JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref \
         WHERE usage.domain_ref=$1 AND content.content_external_id='normal-c'",
    )
    .bind(domain())
    .fetch_one(db.pool())
    .await
    .unwrap();

    let mut oversized_command = next(&base_command);
    oversized_command.scope = StudyScope::Works {
        work_refs: vec![work_a],
    };
    let oversized_run = start_study_run(&db, oversized_command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let mut normal_b_command = next(&base_command);
    normal_b_command.scope = StudyScope::Works {
        work_refs: vec![work_b],
    };
    let normal_b_run = start_study_run(&db, normal_b_command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let mut normal_c_command = next(&base_command);
    normal_c_command.scope = StudyScope::Works {
        work_refs: vec![work_c],
    };
    let normal_c_run = start_study_run(&db, normal_c_command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();

    let oversized_target: (String, Option<String>) = sqlx::query_as(
        "SELECT state,terminal_reason FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(oversized_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(oversized_target, ("queued".into(), None));
    let mut fairness = ModelWorkerFairness::default();
    assert!(
        prepare_next_batch_across_runs_with_fairness(&db, &mut fairness)
            .await
            .unwrap()
    );

    let oversized_target: (String, Option<String>) = sqlx::query_as(
        "SELECT state,terminal_reason FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(oversized_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        oversized_target,
        ("failed".into(), Some("input_limit_exceeded".into()))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_batch WHERE run_ref=$1",
        )
        .bind(oversized_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(oversized_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );

    // The first normal Run after the oversized one must prepare in the same bounded scan;
    // the circular cursor must let the other normal Run prepare on the next scan as well.
    let prepared_runs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_batch WHERE run_ref=ANY($1) ORDER BY run_ref",
    )
    .bind(vec![normal_b_run, normal_c_run])
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(prepared_runs.len(), 1);
    assert!(
        prepare_next_batch_across_runs_with_fairness(&db, &mut fairness)
            .await
            .unwrap()
    );
    let prepared_runs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_batch WHERE run_ref=ANY($1) ORDER BY run_ref",
    )
    .bind(vec![normal_b_run, normal_c_run])
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        prepared_runs.len(),
        2,
        "a single oversized target cannot head-of-line block normal work from other Runs"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_batch_target target \
             JOIN linggan_comment_study_batch batch USING(batch_ref) \
             WHERE batch.run_ref=$1",
        )
        .bind(normal_b_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_batch_target target \
             JOIN linggan_comment_study_batch batch USING(batch_ref) \
             WHERE batch.run_ref=$1",
        )
        .bind(normal_c_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn legacy_cross_run_pairs_are_retired_before_model_admission() {
    let (db, mut command, _) = setup("cross_run_legacy_pair", 4).await;
    command.limits.comment_budget = 2;
    let first_run = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (first_signals, _, _, _) = seed_problem_stage_fixtures(&db, first_run).await;
    let second_run = start_study_run(&db, next(&command), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (second_signals, _, _, _) = seed_problem_stage_fixtures(&db, second_run).await;
    let (first_signal_ref, second_signal_ref) = if first_signals[0] < second_signals[0] {
        (first_signals[0], second_signals[0])
    } else {
        (second_signals[0], first_signals[0])
    };
    let legacy_pair_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_pair( \
           pair_ref,first_signal_ref,second_signal_ref,state,pair_manifest \
         ) VALUES($1,$2,$3,'pending','{}'::jsonb)",
    )
    .bind(legacy_pair_ref)
    .bind(first_signal_ref)
    .bind(second_signal_ref)
    .execute(db.pool())
    .await
    .unwrap();

    let (active_first, active_second) = if first_signals[1] < second_signals[1] {
        (first_signals[1], second_signals[1])
    } else {
        (second_signals[1], first_signals[1])
    };
    let active_pair_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_pair( \
           pair_ref,first_signal_ref,second_signal_ref,state,pair_manifest \
         ) VALUES($1,$2,$3,'pending','{}'::jsonb)",
    )
    .bind(active_pair_ref)
    .bind(active_first)
    .bind(active_second)
    .execute(db.pool())
    .await
    .unwrap();
    let (policy_ref, config_ref, model_ref, version_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT run.policy_ref,config.config_ref,model.model_ref,version.version_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(first_run)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let active_invocation_ref = Uuid::new_v4();
    let active_request_hash = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',1500,0,'{\"callStarted\":true}'::jsonb)",
    )
    .bind(active_invocation_ref)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(active_request_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,pair_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'pair',$4,1, \
           'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', \
           '{\"contract\":\"comment-study.model-request.v1\",\"stage\":\"pair\"}'::jsonb,$5, \
           scope_001_now(),scope_001_now()+interval '1 minute')",
    )
    .bind(active_invocation_ref)
    .bind(first_run)
    .bind(policy_ref)
    .bind(active_pair_ref)
    .bind(active_request_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 WHERE pair_ref=$1",
    )
    .bind(active_pair_ref)
    .bind(active_invocation_ref)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        linggan_intelligence::comment_study_problem_store::accept_problem_pair_from_invocation(
            &db,
            active_pair_ref,
            active_invocation_ref,
            json!({"contract":"comment-study.problem-pair.v1"}),
        )
        .await,
        Err(linggan_intelligence::comment_study_problem_store::ProblemStoreError::ModelRequestUnavailable)
    ));

    assert!(
        run_one_problem_pair(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the worker retires one old cross-Run pair as a local queue action"
    );
    let (state, code): (String, String) = sqlx::query_as(
        "SELECT state,pair_manifest->'decision'->>'code' \
         FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(legacy_pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), code.as_str()),
        ("failed", "legacy_cross_run_pair")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE pair_ref=$1",
        )
        .bind(legacy_pair_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "cross-Run legacy evidence receives no model request or ambiguous budget charge"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_study_model_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1,
        "only the explicit synthetic in-flight fixture has a request receipt"
    );

    // Simulate rows imported from the pre-v2 schema. The frozen-field trigger is bypassed only in
    // this disposable fixture to express historical selection manifests.
    sqlx::query("ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_frozen_guard")
        .execute(db.pool())
        .await
        .unwrap();
    for run_ref in [first_run, second_run] {
        sqlx::query(
            "UPDATE linggan_comment_study_run SET selection_manifest='{}' WHERE run_ref=$1",
        )
        .bind(run_ref)
        .execute(db.pool())
        .await
        .unwrap();
    }
    sqlx::query("ALTER TABLE linggan_comment_study_run ENABLE TRIGGER cs_run_frozen_guard")
        .execute(db.pool())
        .await
        .unwrap();
    let (legacy_first, legacy_second) = if first_signals[0] < second_signals[1] {
        (first_signals[0], second_signals[1])
    } else {
        (second_signals[1], first_signals[0])
    };
    let v1_pair_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_pair( \
           pair_ref,first_signal_ref,second_signal_ref,state,pair_manifest \
         ) VALUES($1,$2,$3,'pending','{}'::jsonb)",
    )
    .bind(v1_pair_ref)
    .bind(legacy_first)
    .bind(legacy_second)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the new scheduler leaves legacy-only Run data untouched"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
        )
        .bind(v1_pair_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "pending"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn problem_stage_retry_limit_uses_the_configured_two_attempts() {
    let (db, command, _) = setup("problem_stage_configured_retry_limit", 2).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let adapter = PiAdapter::configured();

    for expected_attempts in 1..=2 {
        let resolution_error = run_one_problem_resolution(&db, &NoModelSecrets, &adapter).await;
        assert!(matches!(
            resolution_error,
            Err(
                linggan_intelligence::comment_study_resolution_worker::ResolutionWorkerError::Model(
                    ModelError::SecretUnavailable
                )
            )
        ));
        let pair_error = run_one_problem_pair(&db, &NoModelSecrets, &adapter).await;
        assert!(matches!(
            pair_error,
            Err(
                linggan_intelligence::comment_study_pair_worker::PairWorkerError::Model(
                    ModelError::SecretUnavailable
                )
            )
        ));
        let counts: (i64, i64) = sqlx::query_as(
            "SELECT count(*) FILTER (WHERE stage='resolution'), \
                    count(*) FILTER (WHERE stage='pair') \
             FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            counts,
            (i64::from(expected_attempts), i64::from(expected_attempts))
        );
    }

    assert!(
        !run_one_problem_resolution(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let states: (String, String, String) = sqlx::query_as(
        "SELECT resolution.state,pair.state,pair.pair_manifest->'decision'->>'code' \
         FROM linggan_comment_study_resolution resolution \
         CROSS JOIN linggan_comment_study_problem_pair pair \
         WHERE resolution.resolution_ref=$1 AND pair.pair_ref=$2",
    )
    .bind(resolution_ref)
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        states,
        (
            "failed".into(),
            "failed".into(),
            "attempts_exhausted".into()
        )
    );
    let request_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(request_count, 4, "the third request is never reserved");
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL and local Pi child; no provider call"]
async fn scheduler_stops_after_a_dispatched_resolution_response_is_no_longer_admissible() {
    let (db, command, _) = setup("late_resolution_uses_tick_budget", 2).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let adapter = PiAdapter::configured_with_test_command(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_late_response_adapter.sh"),
    );
    let worker_database = db.clone();
    let worker = tokio::spawn(async move {
        run_model_work_once(&worker_database, &SyntheticModelSecrets, &adapter).await
    });

    let mut dispatched_invocation: Option<Uuid> = None;
    for _ in 0..300 {
        dispatched_invocation = sqlx::query_scalar(
            "SELECT invocation_ref FROM linggan_comment_study_model_request \
             WHERE run_ref=$1 AND resolution_ref=$2 AND dispatch_started_at IS NOT NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(run_ref)
        .bind(resolution_ref)
        .fetch_optional(db.pool())
        .await
        .unwrap();
        if dispatched_invocation.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dispatched_invocation = dispatched_invocation.expect("resolution crossed dispatch fence");
    // The request ledger deadline is immutable in production. In this disposable proof only,
    // temporarily bypass that trigger to model wall-clock expiry while the synthetic child waits.
    sqlx::raw_sql(
        "ALTER TABLE linggan_comment_study_model_request DISABLE TRIGGER cs_request_immutable",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_model_request \
         SET deadline_at=scope_001_now()-interval '1 second' WHERE invocation_ref=$1",
    )
    .bind(dispatched_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(
        "ALTER TABLE linggan_comment_study_model_request ENABLE TRIGGER cs_request_immutable",
    )
    .execute(db.pool())
    .await
    .unwrap();

    assert!(worker.await.unwrap().unwrap());
    let invocation_checkpoint: (String, Option<i64>, Option<i64>, Option<i64>, bool, bool) =
        sqlx::query_as(
            "SELECT invocation.state,invocation.input_tokens,invocation.output_tokens, \
                    invocation.charged_tokens,invocation.result->>'callStarted'='true', \
                    invocation.result->>'validationPending'='true' \
             FROM linggan_model_invocation invocation WHERE invocation.invocation_ref=$1",
        )
        .bind(dispatched_invocation)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        invocation_checkpoint,
        ("running".into(), Some(1), Some(1), Some(2), true, true),
        "the synthetic provider response must be checkpointed while its invocation stays open after the active-request fence rejects admission"
    );
    let stage_request_counts: (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE stage='resolution'), \
                count(*) FILTER (WHERE stage='pair') \
         FROM linggan_comment_study_model_request WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        stage_request_counts,
        (1, 0),
        "a provider response rejected by the active-request fence still consumes the tick's call allowance"
    );
    let (resolution_state, pair_state): (String, String) = sqlx::query_as(
        "SELECT resolution.state,pair.state \
         FROM linggan_comment_study_resolution resolution \
         CROSS JOIN linggan_comment_study_problem_pair pair \
         WHERE resolution.resolution_ref=$1 AND pair.pair_ref=$2",
    )
    .bind(resolution_ref)
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(resolution_state, "pending");
    assert_eq!(pair_state, "pending");
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn worker_drain_finishes_the_in_flight_receipt_without_reserving_another_call() {
    let (db, command, _) = setup("worker_drain_in_flight_model", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    let drain = ModelWorkerDrain::new();
    let adapter = PiAdapter::configured_with_test_command(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_drain_adapter.sh"),
    );
    let worker_database = db.clone();
    let worker_drain = drain.clone();
    let worker = tokio::spawn(async move {
        run_model_worker_with_test_dependencies(
            worker_database,
            worker_drain,
            Arc::new(SyntheticModelSecrets),
            adapter,
        )
        .await
    });

    let mut dispatched_invocation: Option<Uuid> = None;
    for _ in 0..300 {
        dispatched_invocation = sqlx::query_scalar(
            "SELECT invocation_ref FROM linggan_comment_study_model_request \
             WHERE run_ref=$1 AND batch_ref=$2 AND stage='semantic' \
               AND dispatch_started_at IS NOT NULL ORDER BY created_at DESC LIMIT 1",
        )
        .bind(run_ref)
        .bind(prepared.batch_ref)
        .fetch_optional(db.pool())
        .await
        .unwrap();
        if dispatched_invocation.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dispatched_invocation = dispatched_invocation.expect("worker crossed provider dispatch");
    drain.request();

    tokio::time::timeout(std::time::Duration::from_secs(5), worker)
        .await
        .expect("the worker drains the in-flight local provider call")
        .unwrap()
        .unwrap();

    let request_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(request_count, 1, "drain must prevent another reservation");
    let receipt: (String, Option<String>, bool, bool, String) = sqlx::query_as(
        "SELECT invocation.state,invocation.failure_code, \
                request.dispatch_started_at IS NOT NULL, \
                invocation.result->>'callStarted'='true',batch.state \
         FROM linggan_model_invocation invocation \
         JOIN linggan_comment_study_model_request request USING(invocation_ref) \
         JOIN linggan_comment_study_batch batch USING(batch_ref) \
         WHERE invocation.invocation_ref=$1",
    )
    .bind(dispatched_invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        receipt,
        (
            "failed".into(),
            Some("synthetic_drain_failure".into()),
            true,
            true,
            "failed".into()
        )
    );
    let attempt: (String, String, String) = sqlx::query_as(
        "SELECT target.state,attempt.state,attempt.rejection_code \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_semantic_attempt attempt USING(target_ref) \
         WHERE target.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        attempt,
        (
            "queued".into(),
            "rejected".into(),
            "provider_failure".into()
        ),
        "the active response is settled before the drain exits, leaving only its bounded retry"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn same_work_cold_start_requires_two_known_authors_and_an_unambiguous_pair() {
    let (db, command, _) = setup("same_work_two_author_cold_start", 0).await;
    let authors = ["reader-1", "reader-1", "reader-2", "reader-3"];
    for (index, author) in authors.iter().enumerate() {
        research_fixture::comment_with_author(
            &db,
            "selected",
            &format!("cold-start-comment-{index}"),
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            Some(author),
            &format!("2026-09-28T08:0{index}:00Z"),
        )
        .await;
    }
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let work_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT target.content_public_ref) \
         FROM linggan_comment_study_target target WHERE target.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        work_count, 1,
        "all four comments belong to the same selected work"
    );

    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared.target_refs.len(), authors.len());
    let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let signals = prepared
        .target_refs
        .iter()
        .map(|target_ref| {
            json!({
                "targetRef":target_ref,
                "outcome":"signals",
                "reason":null,
                "signals":[{
                    "kind":"problem",
                    "proposition":"孩子在家庭作业中存在自主启动困难。",
                    "evidence":"孩子每天写作业都要催",
                    "problemFrame":{
                        "actor":{"value":"孩子","basis":"孩子"},
                        "goalOrExpectedState":{"value":"自主开始作业","basis":"不催就不开始"},
                        "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                        "context":{"value":"家庭作业","basis":"写作业"}
                    }
                }]
            })
        })
        .collect::<Vec<_>>();
    let batch_receipt = accept_study_batch_output(
        &db,
        prepared.batch_ref,
        claim.lease_token,
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":prepared.batch_ref,
            "contentPublicRef":prepared.content_public_ref,
            "results":signals
        }),
    )
    .await
    .unwrap();
    assert_eq!(batch_receipt.accepted_target_count, authors.len());

    let signals: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT signal.signal_ref,comment.author_external_id \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE target.run_ref=$1 ORDER BY comment.comment_external_id",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(signals.len(), authors.len());
    for (signal_ref, _) in &signals {
        let resolution = prepare_problem_resolution_for_enabled_v2_run(&db, *signal_ref, vec![])
            .await
            .unwrap();
        let receipt = accept_problem_resolution(
            &db,
            resolution.resolution_ref,
            json!({"contract":"comment-study.problem-resolution.v1","candidates":[]}),
        )
        .await
        .unwrap();
        assert_eq!(receipt.state, "deferred_novel");
    }

    let selection = PairSelection {
        // Pair creation is exercised as a transaction gate here; no recall ranking is claimed.
        profile_ref: Uuid::new_v4(),
        recall_rank: 1,
        admissible_rank: 1,
    };
    let same_author =
        prepare_problem_pair_for_enabled_v2_run(&db, signals[0].0, signals[1].0, selection.clone())
            .await
            .expect_err("two comments from one author are not independent support");
    assert!(matches!(
        same_author,
        ProblemStoreError::PairNotIndependentOrNovel
    ));

    let uncertain =
        prepare_problem_pair_for_enabled_v2_run(&db, signals[2].0, signals[3].0, selection.clone())
            .await
            .unwrap();
    let uncertainty_receipt = accept_problem_pair(
        &db,
        uncertain.pair_ref,
        json!({
            "contract":"comment-study.problem-pair.v1",
            "firstSignalRef":uncertain.first_signal_ref,
            "secondSignalRef":uncertain.second_signal_ref,
            "dimensions":{
                "actor":"same","goalOrExpectedState":"unknown",
                "barrierOrUnmetNeed":"same","context":"same"
            },
            "proposedProblem":null
        }),
    )
    .await
    .unwrap();
    assert_eq!(uncertainty_receipt.state, "rejected");
    assert_eq!(uncertainty_receipt.decision_reason, "ambiguous");
    let problems_after_uncertainty: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_problem")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(problems_after_uncertainty, 0);

    let independent =
        prepare_problem_pair_for_enabled_v2_run(&db, signals[0].0, signals[2].0, selection)
            .await
            .unwrap();
    let accepted = accept_problem_pair(
        &db,
        independent.pair_ref,
        json!({
            "contract":"comment-study.problem-pair.v1",
            "firstSignalRef":independent.first_signal_ref,
            "secondSignalRef":independent.second_signal_ref,
            "dimensions":{
                "actor":"same","goalOrExpectedState":"same",
                "barrierOrUnmetNeed":"same","context":"same"
            },
            "proposedProblem":{
                "title":"家庭作业自主启动困难",
                "definition":"孩子需要持续外部催促才能开始家庭作业",
                "stableIdentity":{"actor":"孩子","barrier":"需要外部催促"},
                "includeCriteria":["需要持续外部催促才能开始家庭作业"],
                "excludeCriteria":["仅一次忘记作业"]
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(accepted.state, "approved");
    let problem_ref = accepted.problem_ref.unwrap();
    let supporter_accounts: Vec<String> = sqlx::query_scalar(
        "SELECT array_agg(DISTINCT comment.author_external_id ORDER BY comment.author_external_id) \
         FROM linggan_comment_study_problem_membership membership \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE membership.problem_ref=$1",
    )
    .bind(problem_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(supporter_accounts, ["reader-1", "reader-2"]);
    let (problems, memberships): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM linggan_comment_study_problem), \
                (SELECT count(*) FROM linggan_comment_study_problem_membership WHERE problem_ref=$1)",
    )
    .bind(problem_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((problems, memberships), (1, 2));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn unknown_authors_are_retained_can_join_existing_problem_but_cannot_support_a_new_pair() {
    let (db, command, _) =
        setup_with_model_input_limit("unknown_author_problem_membership", 2, 32_768).await;
    let seed_run = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, problem_ref, _, _) = seed_problem_stage_fixtures(&db, seed_run).await;

    for (id, author, minute) in [
        ("known-novel", Some("reader-9"), "00"),
        ("unknown-novel-a", None, "01"),
        ("unknown-novel-b", None, "02"),
        ("unknown-assigned-a", None, "03"),
        ("unknown-assigned-b", None, "04"),
    ] {
        research_fixture::comment_with_author(
            &db,
            "selected",
            id,
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            author,
            &format!("2026-09-28T09:{minute}:00Z"),
        )
        .await;
    }
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let run_ref = start_study_run(&db, next(&command), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let target_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        target_count, 5,
        "all four unknown-account comments are frozen targets"
    );

    let mut accepted_target_count = 0;
    for _ in 0..2 {
        let prepared = prepare_study_batch(
            &db,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: 8,
            },
        )
        .await
        .unwrap();
        assert!(!prepared.target_refs.is_empty());
        let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
            .await
            .unwrap()
            .unwrap();
        let results = prepared
            .target_refs
            .iter()
            .map(|target_ref| {
                json!({
                    "targetRef":target_ref,
                    "outcome":"signals",
                    "reason":null,
                    "signals":[{
                        "kind":"problem",
                        "proposition":"孩子在家庭作业中存在自主启动困难。",
                        "evidence":"孩子每天写作业都要催",
                        "problemFrame":{
                            "actor":{"value":"孩子","basis":"孩子"},
                            "goalOrExpectedState":{"value":"自主开始作业","basis":"不催就不开始"},
                            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                            "context":{"value":"家庭作业","basis":"写作业"}
                        }
                    }]
                })
            })
            .collect::<Vec<_>>();
        let semantic_receipt = accept_study_batch_output(
            &db,
            prepared.batch_ref,
            claim.lease_token,
            json!({
                "contract":"comment-study.note-batch.v1",
                "batchRef":prepared.batch_ref,
                "contentPublicRef":prepared.content_public_ref,
                "results":results
            }),
        )
        .await
        .unwrap();
        accepted_target_count += semantic_receipt.accepted_target_count;
    }
    assert_eq!(accepted_target_count, 5);

    let signals: Vec<(String, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT comment.comment_external_id,signal.signal_ref,comment.author_external_id \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE target.run_ref=$1 ORDER BY comment.comment_external_id",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(signals.len(), 5);
    for (comment_id, signal_ref, _) in &signals {
        let candidates = if comment_id.starts_with("unknown-assigned") {
            vec![problem_ref]
        } else {
            Vec::new()
        };
        let resolution =
            prepare_problem_resolution_for_enabled_v2_run(&db, *signal_ref, candidates.clone())
                .await
                .unwrap();
        let output = if candidates.is_empty() {
            json!({"contract":"comment-study.problem-resolution.v1","candidates":[]})
        } else {
            json!({
                "contract":"comment-study.problem-resolution.v1",
                "candidates":[{
                    "problemRef":problem_ref,
                    "dimensions":{
                        "actor":"same",
                        "goalOrExpectedState":"same",
                        "barrierOrUnmetNeed":"same",
                        "context":"same"
                    }
                }]
            })
        };
        let receipt = accept_problem_resolution(&db, resolution.resolution_ref, output)
            .await
            .unwrap();
        assert_eq!(
            receipt.state,
            if comment_id.starts_with("unknown-assigned") {
                "assigned"
            } else {
                "deferred_novel"
            }
        );
        if comment_id.starts_with("unknown-assigned") {
            assert_eq!(receipt.problem_ref, Some(problem_ref));
        }
    }

    let signal_ref = |comment_id: &str| {
        signals
            .iter()
            .find(|(id, _, _)| id == comment_id)
            .map(|(_, signal_ref, _)| *signal_ref)
            .unwrap()
    };
    let pair_selection = PairSelection {
        profile_ref: Uuid::new_v4(),
        recall_rank: 1,
        admissible_rank: 1,
    };
    for (first, second) in [
        ("unknown-novel-a", "unknown-novel-b"),
        ("unknown-novel-a", "known-novel"),
    ] {
        let rejected = prepare_problem_pair_for_enabled_v2_run(
            &db,
            signal_ref(first),
            signal_ref(second),
            pair_selection.clone(),
        )
        .await
        .expect_err("an unknown author cannot count as independent support");
        assert!(matches!(
            rejected,
            ProblemStoreError::PairNotIndependentOrNovel
        ));
    }
    let assigned_signal_refs = signals
        .iter()
        .filter(|(comment_id, _, _)| comment_id.starts_with("unknown-assigned"))
        .map(|(_, signal_ref, _)| *signal_ref)
        .collect::<Vec<_>>();
    assert_eq!(assigned_signal_refs.len(), 2);
    let assigned_membership_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_problem_membership membership \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE membership.problem_ref=$1 AND signal.signal_ref=ANY($2) \
           AND comment.author_external_id IS NULL",
    )
    .bind(problem_ref)
    .bind(&assigned_signal_refs)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(assigned_membership_count, 2);
    let rejected_pair_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_problem_pair pair \
         WHERE (pair.first_signal_ref=$1 AND pair.second_signal_ref=$2) \
            OR (pair.first_signal_ref=$2 AND pair.second_signal_ref=$1) \
            OR (pair.first_signal_ref=$1 AND pair.second_signal_ref=$3) \
            OR (pair.first_signal_ref=$3 AND pair.second_signal_ref=$1)",
    )
    .bind(signal_ref("unknown-novel-a"))
    .bind(signal_ref("unknown-novel-b"))
    .bind(signal_ref("known-novel"))
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rejected_pair_rows, 0);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn expired_pair_recovery_waits_for_the_0109_terminal_state_constraint() {
    let (db, command, _) = setup("problem_stage_partial_0109", 2).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, _, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    sqlx::raw_sql(
        "ALTER TABLE linggan_comment_study_problem_pair \
           DROP CONSTRAINT cs_problem_pair_state_ck, \
           DROP CONSTRAINT cs_problem_pair_terminal_ck; \
         ALTER TABLE linggan_comment_study_problem_pair \
           ADD CONSTRAINT cs_problem_pair_state_ck \
             CHECK(state IN ('pending','approved','rejected')), \
           ADD CONSTRAINT cs_problem_pair_terminal_ck \
             CHECK((state IN ('approved','rejected'))=(resolved_at IS NOT NULL));",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let (config_ref, model_ref, version_ref, policy_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT config.config_ref,model.model_ref,version.version_ref,run.policy_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let invocation_ref = Uuid::new_v4();
    let request_hash = "abababababababababababababababababababababababababababababababab";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',1500,0,'{\"callStarted\":true}'::jsonb)",
    )
    .bind(invocation_ref)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(request_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,pair_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'pair',$4,2, \
           'abababababababababababababababababababababababababababababababab', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb,$5, \
           scope_001_now()-interval '1 minute',scope_001_now()-interval '1 second')",
    )
    .bind(invocation_ref)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(pair_ref)
    .bind(request_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(invocation_ref)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
           dispatch_reason='user_stopped',control_version=control_version+1 WHERE run_ref=$1",
    )
    .bind(run_ref)
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        !run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let partial_state: (String, String) = sqlx::query_as(
        "SELECT pair.state,invocation.state \
         FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_model_invocation invocation ON invocation.invocation_ref=pair.model_invocation_ref \
         WHERE pair.pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(partial_state, ("pending".into(), "running".into()));

    sqlx::raw_sql(PAIR_FAILURE_STATE)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let recovered: (String, String, i64, String) = sqlx::query_as(
        "SELECT pair.state,invocation.state,invocation.charged_tokens,invocation.failure_code \
         FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_model_invocation invocation ON invocation.invocation_ref=pair.model_invocation_ref \
         WHERE pair.pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        recovered,
        (
            "failed".into(),
            "failed".into(),
            1500,
            "request_deadline_expired".into()
        )
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn pre_ledger_v2_problem_stage_receipts_are_closed_conservatively() {
    let (db, mut command, _) = setup("pre_ledger_v2_orphan", 2).await;
    command.limits.comment_budget = 2;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let (_, config_ref, model_ref, version_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT run.policy_ref,config.config_ref,model.model_ref,version.version_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let failed_legacy_invocation = Uuid::new_v4();
    let stale_running_invocation = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,failure_code,result,finished_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'failed',1500,1200,'provider_failed', \
           '{\"callStarted\":true}'::jsonb,scope_001_now())",
    )
    .bind(failed_legacy_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd")
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result,created_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',2000,2500, \
           '{\"callStarted\":true}'::jsonb,scope_001_now()-interval '2 minutes')",
    )
    .bind(stale_running_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .bind(failed_legacy_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(stale_running_invocation)
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the first v2 worker tick closes stale pre-ledger receipts before fresh work"
    );
    let recovered: Vec<(Uuid, String, i64, String, bool)> = sqlx::query_as(
        "SELECT invocation_ref,state,charged_tokens,failure_code, \
                result->>'legacyRequestLedgerMissing'='true' \
         FROM linggan_model_invocation WHERE invocation_ref=ANY($1) ORDER BY invocation_ref",
    )
    .bind(vec![failed_legacy_invocation, stale_running_invocation])
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(recovered.len(), 2);
    assert!(recovered.iter().all(|row| row.1 == "failed" && row.4));
    let (failed_charge, running_charge): (i64, i64) = (
        recovered
            .iter()
            .find(|row| row.0 == failed_legacy_invocation)
            .unwrap()
            .2,
        recovered
            .iter()
            .find(|row| row.0 == stale_running_invocation)
            .unwrap()
            .2,
    );
    assert_eq!((failed_charge, running_charge), (1500, 2500));
    assert_eq!(
        recovered
            .iter()
            .find(|row| row.0 == failed_legacy_invocation)
            .unwrap()
            .3,
        "provider_failed"
    );
    assert_eq!(
        recovered
            .iter()
            .find(|row| row.0 == stale_running_invocation)
            .unwrap()
            .3,
        "legacy_request_ledger_missing"
    );
    let resolution_receipt: (String, String) = sqlx::query_as(
        "SELECT state,decision_manifest->>'reason' FROM linggan_comment_study_resolution \
         WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let pair_receipt: (String, String) = sqlx::query_as(
        "SELECT state,pair_manifest->'decision'->>'code' \
         FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        resolution_receipt,
        ("failed".into(), "legacy_request_ledger_missing".into())
    );
    assert_eq!(
        pair_receipt,
        ("failed".into(), "legacy_request_ledger_missing".into())
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_study_model_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "the compatibility path records the missing receipt instead of inventing its prompt"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn late_semantic_input_limit_dispatch_failure_is_terminal() {
    let (db, command, _) = setup("late_semantic_input_limit", 1).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let batch = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_batch SET input_manifest=jsonb_set( \
           input_manifest,'{syntheticLateOverflow}',to_jsonb($2::text),true) \
         WHERE batch_ref=$1",
    )
    .bind(batch.batch_ref)
    .bind("界".repeat(40_000))
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the worker reaches the prepared semantic batch and handles late overflow"
    );
    let (target_state, terminal_reason): (String, String) = sqlx::query_as(
        "SELECT state,terminal_reason FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (target_state.as_str(), terminal_reason.as_str()),
        ("failed", "input_limit_exceeded")
    );
    let (attempt_state, failure_code, detail): (String, String, Value) = sqlx::query_as(
        "SELECT state,rejection_code,output_manifest FROM linggan_comment_study_semantic_attempt \
         WHERE batch_ref=$1",
    )
    .bind(batch.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(attempt_state, "rejected");
    assert_eq!(failure_code, "provider_failure");
    assert_eq!(detail["failureCode"], "input_limit_exceeded");
    let (batch_state, lease_token): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,lease_token FROM linggan_comment_study_batch WHERE batch_ref=$1",
    )
    .bind(batch.batch_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((batch_state.as_str(), lease_token), ("failed", None));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "the deterministic rejection occurs before any provider invocation"
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn partial_problem_stage_usage_above_reservation_blocks_followup_request() {
    let (db, mut command, _) = setup("problem_stage_partial_usage", 4).await;
    command.limits.comment_budget = 2;
    let calibration_run = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    seed_problem_stage_fixtures(&db, calibration_run).await;
    let semantic_batch = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref: calibration_run,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let semantic_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let semantic_reservation =
        reserve_study_batch_model_call(&db, semantic_batch.batch_ref, semantic_lease.lease_token)
            .await
            .unwrap();
    let semantic_reserved: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(semantic_reservation.invocation_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(semantic_reserved > 0);
    sqlx::query(
        "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
         dispatch_reason='user_stopped', control_version=control_version+1 \
         WHERE run_ref=$1",
    )
    .bind(calibration_run)
    .execute(db.pool())
    .await
    .unwrap();

    let mut budgeted_command = next(&command);
    budgeted_command.limits.token_limit = semantic_reserved + 700;
    let budgeted_run = start_study_run(&db, budgeted_command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (signals, _, resolution_ref, _) = seed_problem_stage_fixtures(&db, budgeted_run).await;
    let (config_ref, model_ref, version_ref, policy_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT config.config_ref,model.model_ref,version.version_ref,run.policy_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(budgeted_run)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let partial_invocation = Uuid::new_v4();
    let partial_hash = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',500,900,900,NULL, \
           '{\"callStarted\":true}'::jsonb)",
    )
    .bind(partial_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(partial_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,attempt_ordinal,input_context_hash, \
           request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'resolution',$4,1, \
           'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb,$5, \
           scope_001_now(),scope_001_now()+interval '1 hour')",
    )
    .bind(partial_invocation)
    .bind(budgeted_run)
    .bind(policy_ref)
    .bind(resolution_ref)
    .bind(partial_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 \
         WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .bind(partial_invocation)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(500 + semantic_reserved <= semantic_reserved + 700);
    assert!(900 + semantic_reserved > semantic_reserved + 700);
    let semantic_batch = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref: budgeted_run,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let semantic_lease = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(semantic_lease.batch_ref, semantic_batch.batch_ref);
    assert!(matches!(
        reserve_study_batch_model_call(&db, semantic_batch.batch_ref, semantic_lease.lease_token)
            .await,
        Err(StudyModelDispatchError::BudgetDeferred)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request \
             WHERE run_ref=$1 AND stage='semantic'",
        )
        .bind(budgeted_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "semantic admission shares partial measured usage from problem stages"
    );

    assert_eq!(signals.len(), 2);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn pre_ledger_invocation_usage_blocks_p3_reservation() {
    let (db, mut command, _) = setup("pre_ledger_p3_budget", 4).await;
    command.limits.comment_budget = 2;
    let calibration_run = start_study_run(&db, command.clone(), TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    seed_problem_stage_fixtures(&db, calibration_run).await;
    assert!(matches!(
        run_one_problem_pair(&db, &NoModelSecrets, &PiAdapter::configured()).await,
        Err(
            linggan_intelligence::comment_study_pair_worker::PairWorkerError::Model(
                ModelError::SecretUnavailable
            )
        )
    ));
    let pair_reservation: i64 = sqlx::query_scalar(
        "SELECT invocation.reserved_tokens \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE request.run_ref=$1 AND request.stage='pair'",
    )
    .bind(calibration_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(pair_reservation > 0);
    sqlx::query(
        "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
         dispatch_reason='user_stopped',control_version=control_version+1 WHERE run_ref=$1",
    )
    .bind(calibration_run)
    .execute(db.pool())
    .await
    .unwrap();

    let mut budgeted_command = next(&command);
    budgeted_command.limits.token_limit = pair_reservation + 700;
    assert!(900 <= budgeted_command.limits.token_limit);
    assert!(900 + pair_reservation > budgeted_command.limits.token_limit);
    let budgeted_run = start_study_run(&db, budgeted_command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, _, pair_ref) = seed_problem_stage_fixtures(&db, budgeted_run).await;
    let (config_ref, model_ref, version_ref): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT config.config_ref,model.model_ref,version.version_ref \
         FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
         JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         WHERE run.run_ref=$1",
    )
    .bind(budgeted_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let legacy_invocation = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,input_tokens,output_tokens,failure_code,result,created_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',500,900,NULL,NULL,'pair_contract_rejected', \
           '{\"callStarted\":true}'::jsonb,scope_001_now()-interval '2 minutes')",
    )
    .bind(legacy_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair \
         SET state='rejected',model_invocation_ref=$2,resolved_at=scope_001_now() \
         WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(legacy_invocation)
    .execute(db.pool())
    .await
    .unwrap();

    assert!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the first worker tick closes the stale pre-ledger invocation"
    );
    let (legacy_state, legacy_charge, legacy_run_ref): (String, i64, Option<String>) =
        sqlx::query_as(
            "SELECT invocation.state,invocation.charged_tokens,invocation.result->>'legacyRunRef' \
         FROM linggan_model_invocation invocation WHERE invocation.invocation_ref=$1",
        )
        .bind(legacy_invocation)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let (subject_state, subject_pointer): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,model_invocation_ref FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let expected_legacy_run_ref = budgeted_run.to_string();
    assert_eq!(
        (
            legacy_state.as_str(),
            legacy_charge,
            legacy_run_ref.as_deref()
        ),
        ("failed", 900, Some(expected_legacy_run_ref.as_str()))
    );
    assert_eq!(
        (subject_state.as_str(), subject_pointer),
        ("failed", None),
        "the same worker tick resumes the subject, then safely stops it when legacy spend exhausts the Run budget"
    );
    assert!(!linggan_intelligence::comment_study_problem_store::
        resume_pre_v2_pair_contract_rejection_for_enabled_v2_run(&db)
        .await
        .unwrap());
    let resumed_pointer: Option<Uuid> = sqlx::query_scalar(
        "SELECT model_invocation_ref FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(resumed_pointer, None);
    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "the next P3 reservation accounts for the recovered, unledgered charge"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request \
             WHERE run_ref=$1 AND stage='pair'",
        )
        .bind(budgeted_run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "legacy invocation charges participate before a new P3 request is recorded"
    );
    let (pair_state, dispatch_state, dispatch_reason): (String, String, Option<String>) =
        sqlx::query_as(
            "SELECT pair.state,run.dispatch_state,run.dispatch_reason \
         FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run USING(run_ref) WHERE pair.pair_ref=$1",
        )
        .bind(pair_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        (
            pair_state.as_str(),
            dispatch_state.as_str(),
            dispatch_reason.as_deref()
        ),
        ("failed", "stopped", Some("budget_exhausted"))
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn oversized_problem_stage_requests_are_terminal_and_do_not_repeat() {
    let (db, mut command, _) =
        setup_with_model_input_limit("problem_stage_input_limit", 2, 1024).await;
    command.policy_ref =
        policy_with_problem_stage_instructions(&db, &command, "界".repeat(1000), "界".repeat(1000))
            .await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (_, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let adapter = PiAdapter::configured();

    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_one_problem_resolution(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_one_problem_pair(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_one_problem_resolution(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );

    let (pair_state, pair_reason, resolution_state, resolution_reason): (
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT pair.state,pair.pair_manifest->'decision'->>'code', \
                resolution.state,resolution.decision_manifest->>'reason' \
         FROM linggan_comment_study_problem_pair pair, \
              linggan_comment_study_resolution resolution \
         WHERE pair.pair_ref=$1 AND resolution.resolution_ref=$2",
    )
    .bind(pair_ref)
    .bind(resolution_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(pair_state, "failed");
    assert_eq!(pair_reason, "input_limit_exceeded");
    assert_eq!(resolution_state, "failed");
    assert_eq!(resolution_reason, "input_limit_exceeded");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_study_model_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn exhausted_problem_stage_budget_closes_all_pending_siblings() {
    let (db, command, _) = setup("problem_stage_budget_exhausted", 2).await;
    let run_ref = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let (signals, _, resolution_ref, pair_ref) = seed_problem_stage_fixtures(&db, run_ref).await;
    let sibling_resolution_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest \
         ) VALUES($1,$2,$3,'pending', \
           jsonb_build_object('contract','comment-study.problem-candidate-set.v1', \
             'candidateProblemRefs','[]'::jsonb))",
    )
    .bind(sibling_resolution_ref)
    .bind(signals[1])
    .bind(domain())
    .execute(db.pool())
    .await
    .unwrap();
    let (config_ref, model_ref, version_ref, policy_ref): (Uuid, Uuid, Uuid, Uuid) =
        sqlx::query_as(
            "SELECT config.config_ref,model.model_ref,version.version_ref,run.policy_ref \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
             JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let spent_invocation = Uuid::new_v4();
    let spent_hash = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,failure_code,result,finished_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'failed',500,100001,'provider_failed', \
           '{\"ok\":false}'::jsonb,scope_001_now())",
    )
    .bind(spent_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(spent_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,attempt_ordinal, \
           input_context_hash,request_manifest,request_hash,dispatch_started_at,deadline_at \
         ) VALUES($1,$2,$3,'resolution',$4,1, \
           'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', \
           '{\"contract\":\"comment-study.model-request.v1\"}'::jsonb,$5, \
           scope_001_now(),scope_001_now()+interval '1 hour')",
    )
    .bind(spent_invocation)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(resolution_ref)
    .bind(spent_hash)
    .execute(db.pool())
    .await
    .unwrap();

    let adapter = PiAdapter::configured();
    assert!(
        !run_one_problem_resolution(&db, &NoModelSecrets, &adapter)
            .await
            .unwrap()
    );

    let (dispatch_state, dispatch_reason, control_version): (String, String, i64) = sqlx::query_as(
        "SELECT dispatch_state,dispatch_reason,control_version \
             FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(dispatch_state, "stopped");
    assert_eq!(dispatch_reason, "budget_exhausted");
    assert_eq!(control_version, 1);
    let resolution_states: Vec<(String, String)> = sqlx::query_as(
        "SELECT state,decision_manifest->>'reason' FROM linggan_comment_study_resolution \
         WHERE signal_ref=ANY($1) ORDER BY resolution_ref",
    )
    .bind(&signals)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(resolution_states.len(), 2);
    assert!(
        resolution_states
            .iter()
            .all(|(state, reason)| state == "budget_stopped" && reason == "budget_exhausted")
    );
    let (pair_state, pair_reason): (String, String) = sqlx::query_as(
        "SELECT state,pair_manifest->'decision'->>'code' \
         FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (pair_state.as_str(), pair_reason.as_str()),
        ("failed", "budget_exhausted")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1,
        "budget exhaustion records no new provider request"
    );
}
