//! Historical P2 transactions/read contracts, plus a permanently closed legacy executor.
//! Sixteen positive old-owner runtime scenarios retired in PR-1B; original source is in Git.
//! Successors: ai_foundation_postgres/retirement and ai_executor_service/ai_worker_contract.
//! Disposable PostgreSQL and synthetic materials only. No provider/model I/O.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;
use dossier_fixture::fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_study_acceptance::{SemanticAcceptanceError, accept_target_output},
    comment_study_batch::{PrepareStudyBatchRequest, next_run_needing_batch, prepare_study_batch},
    comment_study_batch_acceptance::accept_study_batch_output,
    comment_study_batch_worker::{claim_next_study_batch, recover_expired_study_batch_leases},
    comment_study_catalog::refresh_clean_cache,
    comment_study_model_dispatch::{
        StudyModelDispatchError, mark_study_batch_model_dispatch_started,
        reserve_study_batch_model_call,
    },
    comment_study_policy::{
        CreateStudyPolicyCommand, StudyPolicyDefaults, StudyStageInstructions, create_study_policy,
    },
    comment_study_problem_store::{
        PairSelection, ProblemStoreError, accept_problem_pair, accept_problem_resolution,
        prepare_problem_pair_for_enabled_v2_run, prepare_problem_resolution_for_enabled_v2_run,
    },
    comment_study_run::{
        StudyStartError, TrustedStudyOrigin, cancel_study_run, preview_study_selection,
        recover_study_run, start_study_run,
    },
    comment_study_selection::{
        StartStudyRunCommand, StudyScope, StudySelectionMode, study_domain_lock_key,
    },
    model_invocation::checkpoint_invocation_usage,
    model_runner::{
        ModelWorkerFairness, prepare_next_batch_across_runs_with_fairness, run_model_work_once,
    },
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::PiAdapter,
    pi_adapter::{PiResponse, PiUsage},
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
const GUARDS: &str =
    include_str!("../../../database/migrations/0109_comment_study_start_constraints.sql");
const REQUEST_GUARDS: &str = include_str!(
    "../../../database/migrations/0110_comment_study_request_snapshot_constraints.sql"
);
const PAIR_FAILURE_STATE: &str =
    include_str!("../../../database/migrations/0111_comment_study_pair_failure_state.sql");
const MEMBERSHIP_REVISION: &str =
    include_str!("../../../database/migrations/0112_comment_study_membership_revision.sql");
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
        "../../../database/migrations/0107_comment_study_productization_schema.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0108_comment_study_policy_constraints.sql"
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
    sqlx::raw_sql(MEMBERSHIP_REVISION)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0114_comment_study_effective_head.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0147_comment_study_full_context.sql"
    ))
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
        "modelConfigRef":config,"defaults":{"commentBudget":1},
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
        "limits":{"commentBudget":100,"tokenLimit":100000},"reason":null})).unwrap();
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
            defaults: StudyPolicyDefaults { comment_budget: 2 },
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

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn continue_ready_start_freezes_real_parent_context_and_resumes_only_new_or_recoverable_comments()
 {
    let (db, command, _) = setup("continue_ready_start", 2).await;
    let parent_ref: Uuid = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c0000' LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let reply_ref = reply_with_author(
        &db,
        "selected",
        "r0000",
        "c0000",
        "SYNTHETIC 对父评论的回复",
        Some("reader-2"),
        "2026-09-21T08:00:01Z",
    )
    .await;
    linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, domain(), 20)
        .await
        .unwrap();

    let first = start_study_run(&db, next(&command), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let first_run = first.run_ref.unwrap();
    assert_eq!(first.target_count, 3);
    let reply_input: (Option<Uuid>, String, Value) = sqlx::query_as(
        "SELECT target.parent_source_ref,target.dependency_state,target.input_manifest \
         FROM linggan_comment_study_target target WHERE target.run_ref=$1 AND target.source_ref=$2",
    )
    .bind(first_run)
    .bind(reply_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(reply_input.0, Some(parent_ref));
    assert_eq!(reply_input.1, "parent_available");
    assert_eq!(
        reply_input.2["parentContext"]["researchText"],
        "SYNTHETIC 用户评论"
    );
    let root_dependency: String = sqlx::query_scalar(
        "SELECT target.dependency_state FROM linggan_comment_study_target target \
         WHERE target.run_ref=$1 AND target.source_ref=$2",
    )
    .bind(first_run)
    .bind(parent_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(root_dependency, "self_contained");

    // Keep one historical needs_context target unchanged. It must not be silently repeated by
    // the default continuation mode; the other unfinished targets are safely stopped below.
    sqlx::query(
        "UPDATE linggan_comment_study_target SET state='needs_context',finished_at=scope_001_now() \
         WHERE run_ref=$1 AND source_ref=$2",
    )
    .bind(first_run)
    .bind(parent_ref)
    .execute(db.pool())
    .await
    .unwrap();
    cancel_study_run(&db, domain(), first_run).await.unwrap();

    add(&db, "c0002").await;
    linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, domain(), 20)
        .await
        .unwrap();
    let mut continuation = next(&command);
    continuation.mode = StudySelectionMode::ContinueReady;
    let resumed = start_study_run(&db, continuation, TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    assert_eq!(resumed.outcome, "created");
    assert_eq!(resumed.target_count, 3);
    let resumed_external_ids: Vec<String> = sqlx::query_scalar(
        "SELECT source.comment_external_id FROM linggan_comment_study_target target \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         WHERE target.run_ref=$1 ORDER BY source.comment_external_id",
    )
    .bind(resumed.run_ref.unwrap())
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(resumed_external_ids, vec!["c0001", "c0002", "r0000"]);
    assert!(!resumed_external_ids.contains(&"c0000".to_owned()));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn source_run_recovery_uses_exact_failed_keys_and_keeps_original_history() {
    let (db, command, _) = setup("source_run_recovery", 2).await;
    let original = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_target SET \
           state=CASE WHEN comment_external_id='c0000' THEN 'succeeded' ELSE 'failed' END, \
           terminal_reason=CASE WHEN comment_external_id='c0000' THEN NULL ELSE 'provider_failed' END, \
           finished_at=scope_001_now() WHERE run_ref=$1",
    )
    .bind(original)
    .execute(db.pool())
    .await
    .unwrap();
    cancel_study_run(&db, domain(), original).await.unwrap();

    let request_ref = Uuid::new_v4();
    let recovered = recover_study_run(&db, domain(), original, request_ref)
        .await
        .unwrap();
    assert_eq!(recovered.target_count, 1);
    let new_run = recovered.run_ref.unwrap();
    let identity: String = sqlx::query_scalar(
        "SELECT comment_external_id FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(new_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(identity, "c0001");
    let source_ref: String = sqlx::query_scalar(
        "SELECT selection_manifest->>'recoverySourceRunRef' FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(new_run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(source_ref, original.to_string());
    let replay = recover_study_run(&db, domain(), original, request_ref)
        .await
        .unwrap();
    assert_eq!(replay.run_ref, Some(new_run));
    assert!(replay.idempotent_replay);

    sqlx::query("UPDATE linggan_comment_study_target SET state='succeeded',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(new_run).execute(db.pool()).await.unwrap();
    cancel_study_run(&db, domain(), new_run).await.unwrap();
    let later = recover_study_run(&db, domain(), original, Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(later.outcome, "no_work");
    assert!(later.run_ref.is_none());
    let original_failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1 AND state='failed'",
    )
    .bind(original)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(original_failed, 1);
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
        sqlx::query(
            "UPDATE linggan_comment_study_target \
             SET state='succeeded',finished_at=scope_001_now() WHERE target_ref=$1",
        )
        .bind(target_ref)
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
             'candidateProblemRefs',jsonb_build_array($4::text), \
             'candidateProblemRevisions',jsonb_build_array(jsonb_build_object( \
               'problemRef',$4::text,'problemRevisionRef',$5::text))))",
    )
    .bind(resolution_ref)
    .bind(signals[0])
    .bind(domain())
    .bind(problem_ref)
    .bind(revision_ref)
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
    let limits:Value=sqlx::query_scalar("SELECT jsonb_build_array(comment_budget,token_limit) FROM linggan_comment_study_run WHERE run_ref=$1")
        .bind(first.run_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        limits,
        json!([100, 100000]),
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
async fn missing_parent_reaches_model_and_only_changed_context_requeues_after_needs_context() {
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
    assert_eq!(first.needs_context_count, 0);
    assert_eq!(first.queued_count, 1);
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_study_run WHERE run_ref=$1")
            .bind(first.run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "queued");
    let dependency: String = sqlx::query_scalar(
        "SELECT dependency_state FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(first.run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(dependency, "parent_required_missing");
    // The semantic stage, not the input freezer, may decide this particular reply needs context.
    sqlx::query("UPDATE linggan_comment_study_target SET state='needs_context',finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(first.run_ref).execute(db.pool()).await.unwrap();
    cancel_study_run(&db, domain(), first.run_ref.unwrap())
        .await
        .unwrap();
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
async fn measured_running_usage_releases_unspent_reservation_for_next_request() {
    let (db, mut command, _) = setup("start_measured_usage_budget", 2).await;
    command.limits.token_limit = 15_000;
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
    assert_eq!(reserved_tokens, 8192 + 1024);
    assert_eq!(
        charged_before_call, reserved_tokens,
        "held quota is not measured consumption"
    );
    assert!(reserved_tokens.saturating_mul(2) > 15_000);

    mark_study_batch_model_dispatch_started(
        &db,
        first_reservation.invocation_ref,
        first.batch_ref,
        first_lease.lease_token,
    )
    .await
    .unwrap();
    let actual_charge = 1_000;
    let input_tokens = 800;
    let output_tokens = 200;
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
                ..Default::default()
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
    reserve_study_batch_model_call(&db, second.batch_ref, second_lease.lease_token)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_model_request WHERE run_ref=$1",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
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
    let backend_pid = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let waiting: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) LIMIT 1",
            )
            .fetch_optional(db.pool())
            .await
            .unwrap();
            if let Some(pid) = waiting {
                break pid;
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
    assert_start_domain_unlocked(&db).await;
    assert_start_session_closed(&db, backend_pid).await;
}

async fn assert_start_domain_unlocked(db: &Database) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            // Inspect the exact bigint key globally; a pool connection may
            // otherwise reenter its own leaked advisory lock and falsely pass.
            let available: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND granted AND objsubid=1 AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND classid::bigint=(($1::bigint >> 32) & 4294967295) AND objid::bigint=($1::bigint & 4294967295))")
                .bind(study_domain_lock_key(domain()).unwrap())
                .fetch_one(db.pool())
                .await.unwrap();
            if available {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("start session must release its domain lock");
}

async fn assert_start_session_closed(db: &Database, pid: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let closed: bool = sqlx::query_scalar(
                "SELECT NOT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
            )
            .bind(pid)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if closed {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("dedicated start session and its lock_timeout must not return to the pool");
}

async fn cancelled_start_releases_domain_lock(holding: bool) {
    let (db, c, _) = setup(
        if holding {
            "start_cancel_held"
        } else {
            "start_cancel_wait"
        },
        1,
    )
    .await;
    let mut holder = db.pool().begin().await.unwrap();
    if holding {
        // Let start own its advisory lock, then block its first domain row read.
        sqlx::query("SELECT domain_ref FROM observation_domain WHERE domain_ref=$1 FOR UPDATE")
            .bind(domain())
            .execute(&mut *holder)
            .await
            .unwrap();
    } else {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(study_domain_lock_key(domain()).unwrap())
            .execute(&mut *holder)
            .await
            .unwrap();
    }
    let other = db.clone();
    let task =
        tokio::spawn(async move { start_study_run(&other, c, TrustedStudyOrigin::Manual).await });
    let backend_pid = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let blocked: Option<i32> = if holding {
                sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%observation_domain%FOR SHARE%' LIMIT 1")
                    .fetch_optional(db.pool()).await.unwrap()
            } else {
                sqlx::query_scalar("SELECT pid FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()) LIMIT 1")
                    .fetch_optional(db.pool()).await.unwrap()
            };
            if let Some(pid) = blocked { break pid; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("observe the actual blocked start before cancellation");
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    holder.rollback().await.unwrap();
    assert_start_domain_unlocked(&db).await;
    assert_start_session_closed(&db, backend_pid).await;
    assert_eq!(effects(&db).await, json!([0, 0, 0, 0]));
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn cancelled_start_waiting_for_domain_lock_releases_its_session() {
    cancelled_start_releases_domain_lock(false).await;
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn cancelled_start_holding_domain_lock_releases_its_session() {
    cancelled_start_releases_domain_lock(true).await;
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
    assert_start_domain_unlocked(&db).await;
    sqlx::raw_sql("ALTER TABLE linggan_comment_study_target DISABLE TRIGGER cs_target_input_guard")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        start_study_run(&db, c, TrustedStudyOrigin::Manual).await,
        Err(StudyStartError::SchemaUnavailable)
    ));
}

// The old patrol model-loop drain proof retired with that owner in AI-GEN1 / PR-1B.
// Current executable/DB/ack signal proofs live in apps/worker/tests/ai_worker_contract.rs.

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

    // Spend the ambiguous first comparison on different Signals from the later approved pair.
    // A valid non-create outcome must not give either Signal a second automatic first comparison.
    let uncertain =
        prepare_problem_pair_for_enabled_v2_run(&db, signals[1].0, signals[3].0, selection.clone())
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
    let (problems, memberships, lineaged_memberships): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM linggan_comment_study_problem), \
                (SELECT count(*) FROM linggan_comment_study_problem_membership WHERE problem_ref=$1), \
                (SELECT count(*) FROM linggan_comment_study_problem_membership membership \
                   JOIN linggan_comment_study_resolution resolution USING(resolution_ref) \
                   WHERE membership.problem_ref=$1 \
                     AND membership.problem_revision_ref IS NOT NULL \
                     AND resolution.decision_manifest->>'problemRevisionRef'=membership.problem_revision_ref::text)",
    )
    .bind(problem_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((problems, memberships, lineaged_memberships), (1, 2, 2));

    // A membership cannot borrow a resolution for another Signal, even if both
    // decisions name the same Problem revision. Use only known-author Signals.
    let mismatched_membership_signal = signals[1].0;
    let (target_ref, semantic_attempt_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT target_ref,semantic_attempt_ref FROM linggan_comment_study_signal WHERE signal_ref=$1",
    )
    .bind(mismatched_membership_signal)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let resolution_signal_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_signal( \
           signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence, \
           evidence_start,evidence_end,eligibility_state \
         ) VALUES($1,$2,$3,'question','synthetic resolution signal','SYNTHETIC',9000,9010,'not_applicable')",
    )
    .bind(resolution_signal_ref)
    .bind(target_ref)
    .bind(semantic_attempt_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let resolution_revision_ref: Uuid = sqlx::query_scalar(
        "SELECT problem_revision_ref FROM linggan_comment_study_problem_membership WHERE signal_ref=$1",
    )
    .bind(signals[0].0)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let mismatched_resolution_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,decision_manifest, \
           resolved_problem_ref,resolved_at \
         ) VALUES($1,$2,$3,'assigned','{}'::jsonb,$4,$5,scope_001_now())",
    )
    .bind(mismatched_resolution_ref)
    .bind(resolution_signal_ref)
    .bind(domain())
    .bind(json!({"problemRevisionRef":resolution_revision_ref}))
    .bind(problem_ref)
    .execute(db.pool())
    .await
    .unwrap();
    let mismatch_error = sqlx::query(
        "INSERT INTO linggan_comment_study_problem_membership( \
           membership_ref,signal_ref,problem_ref,resolution_ref,problem_revision_ref \
         ) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(Uuid::new_v4())
    .bind(mismatched_membership_signal)
    .bind(problem_ref)
    .bind(mismatched_resolution_ref)
    .bind(resolution_revision_ref)
    .execute(db.pool())
    .await
    .expect_err("membership must reject a resolution for a different Signal");
    assert_eq!(
        mismatch_error
            .as_database_error()
            .unwrap()
            .code()
            .as_deref(),
        Some("23514")
    );
    assert!(
        mismatch_error
            .to_string()
            .contains("comment_study_membership_signal_resolution_mismatch")
    );
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn unknown_authors_remain_readable_but_are_excluded_from_research_and_problem_support() {
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
    let started = start_study_run(&db, next(&command), TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let run_ref = started.run_ref.unwrap();
    assert_eq!(started.target_count, 1);
    assert_eq!(
        started.exclusion_counts.get("commentAuthorUnknown"),
        Some(&4)
    );

    let retained_unknown_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment \
         WHERE comment_external_id LIKE 'unknown-%' \
           AND author_external_id IS NULL AND body_text IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        retained_unknown_count, 4,
        "raw unknown-author comments remain readable"
    );
    let frozen_comment_ids: Vec<String> = sqlx::query_scalar(
        "SELECT comment.comment_external_id FROM linggan_comment_study_target target \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE target.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(frozen_comment_ids, ["known-novel"]);

    let prepared = prepare_study_batch(
        &db,
        PrepareStudyBatchRequest {
            run_ref,
            maximum_targets: 8,
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared.target_refs.len(), 1);
    let claim = claim_next_study_batch(&db, Uuid::new_v4(), 60)
        .await
        .unwrap()
        .unwrap();
    let receipt = accept_study_batch_output(
        &db,
        prepared.batch_ref,
        claim.lease_token,
        json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":prepared.batch_ref,
            "contentPublicRef":prepared.content_public_ref,
            "results":[{
                "targetRef":prepared.target_refs[0],
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
            }]
        }),
    )
    .await
    .unwrap();
    assert_eq!(receipt.accepted_target_count, 1);

    let signal_ref: Uuid = sqlx::query_scalar(
        "SELECT signal.signal_ref FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         WHERE target.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let resolution =
        prepare_problem_resolution_for_enabled_v2_run(&db, signal_ref, vec![problem_ref])
            .await
            .unwrap();
    let resolved = accept_problem_resolution(
        &db,
        resolution.resolution_ref,
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
        }),
    )
    .await
    .unwrap();
    assert_eq!(resolved.state, "assigned");
    assert_eq!(resolved.problem_ref, Some(problem_ref));
    let unknown_support_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_problem_membership membership \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE membership.problem_ref=$1 AND comment.author_external_id IS NULL",
    )
    .bind(problem_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(unknown_support_count, 0);
}

#[tokio::test]
#[ignore = "isolated synthetic PostgreSQL; no shared database or model"]
async fn partial_problem_stage_usage_above_reservation_blocks_followup_request() {
    // Each Run freezes three targets; the P3 fixture completes two, leaving one real queued
    // semantic target whose next reservation must account for partial P3 usage.
    let (db, mut command, _) = setup("problem_stage_partial_usage", 6).await;
    command.limits.comment_budget = 3;
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
           state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,task_key,quota_bucket,budget_run_ref \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',500,900,900,NULL, \
           '{\"callStarted\":true}'::jsonb,'comment_study','run',$6)",
    )
    .bind(partial_invocation)
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(partial_hash)
    .bind(budgeted_run)
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
async fn dossier_partial_image_body_reaches_v2_frozen_context() {
    let (db, mut command, _) = setup("dossier_comment_start", 0).await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::current_image_reading(&db).await;
    comment_with_author(
        &db,
        "dossier-note",
        "reader-one",
        "这些做法里，如何把每个步骤落实到每天的安排？",
        Some("reader"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    refresh_clean_cache(&db, domain(), 128).await.unwrap();
    let dossier = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    assert!(
        dossier
            .fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text.contains("第3行连续"))
    );
    command.scope = StudyScope::Works {
        work_refs: vec![work],
    };
    command.work_roles.clear();
    let preview = preview_study_selection(&db, command.preview())
        .await
        .unwrap();
    assert_eq!(preview["targetCount"], 1);
    let receipt = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap();
    let context: Value = sqlx::query_scalar(
        "SELECT context_manifest FROM linggan_comment_study_work WHERE run_ref=$1",
    )
    .bind(receipt.run_ref.unwrap())
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(context["dossierContract"], "research-dossier.v2");
    assert!(context.to_string().contains("第3行连续"));
    assert!(context.to_string().contains("封面大字独立保留"));
    assert!(context.to_string().contains("用途不清"));
    for source in context["sources"].as_array().unwrap() {
        assert!(source["blockRef"].is_string() && source["asOf"].is_string());
        assert!(source["sourceRef"].is_string() && source["sourceVersion"].is_string());
        assert!(
            source["speaker"].is_string()
                && source["structure"].is_object()
                && source["issues"].is_array()
        );
    }
    // No model invocation was made: the existing batch's frozen work context is
    // the same context exposed to the real model request builder.
    assert_eq!(effects(&db).await[3], 0);
}

#[tokio::test]
#[ignore = "owned PostgreSQL historical queue; retired entry cannot dispatch"]
async fn historical_queued_run_is_preserved_but_legacy_scheduler_and_dispatch_cannot_touch_it() {
    let (db, command, _) = setup("historical_queue_closed", 3).await;
    let run = start_study_run(&db, command, TrustedStudyOrigin::Manual)
        .await
        .unwrap()
        .run_ref
        .unwrap();
    let before = effects(&db).await;
    let original: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM linggan_comment_study_run r WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(matches!(
        run_model_work_once(&db, &NoModelSecrets, &PiAdapter::configured()).await,
        Err(ModelError::LegacyRetired)
    ));
    assert!(matches!(
        prepare_next_batch_across_runs_with_fairness(&db, &mut ModelWorkerFairness::default())
            .await,
        Err(ModelError::LegacyRetired)
    ));
    assert_eq!(before, effects(&db).await);
    let after: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM linggan_comment_study_run r WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        original, after,
        "the old queue remains a readable historical record, never resumed"
    );
    assert_eq!(after["state"], "queued");
}
