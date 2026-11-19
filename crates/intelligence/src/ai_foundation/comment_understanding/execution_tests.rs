//! The unit flow end to end against an owned PostgreSQL: a synthetic saved L1, the real executor
//! and an owned loopback provider. No real model is called and no credential is read.
//!
//! The provider's reply is decided by what each comment says (see
//! `tests/support/comment_understanding_response.rs`), so each test writes the situation it needs.
#![allow(dead_code)]
use super::{Cancel, Selection, Start, cancel, preview, read_run, start};
use crate::{
    ai_foundation::{
        TaskKey,
        capability::ModelLimits,
        content_understanding::topic_tests,
        control::BudgetPolicy,
        control_store::ControlScope,
        executor::ExecutorSession,
        provider_store::MaterialClass,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    comment_study_source::StudySourceRole,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

#[path = "../../../tests/support/ai_writing_fixture.rs"]
mod setup;
#[path = "../../topic_map_research/tests/saved_source_fixture.rs"]
mod support;
#[path = "../../../tests/support/ai_writing_endpoint.rs"]
mod wire;

struct Env {
    db: Database,
    endpoint: wire::Endpoint,
    adapter: PiAdapter,
    task_config: Uuid,
    task_revision: i64,
    probes: usize,
}

async fn environment(name: &str, variant: &'static str) -> Env {
    let (db, l1_model_config) = support::setup(name).await;
    // The saved L1 Cores are synthetic and need their own task configuration; they are made
    // before the comment task is configured so that nothing else touches its revision.
    topic_tests::configure_tasks_for_test(&db, l1_model_config).await;
    let endpoint = wire::Endpoint::variant(false, variant).await;
    let adapter = PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE")
            .expect("runner supplies explicit Node")
            .into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ExecutorSession::acquire(&db, "synthetic-comment-understanding", 4)
        .await
        .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='comment_understanding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentUnderstanding,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: ModelLimits {
                    input_tokens: 24576,
                    ..configured.limits.clone()
                },
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::CommentUnderstanding),
    ] {
        setup::enable(&db, scope).await;
    }
    for material in [MaterialClass::NoteMaterial, MaterialClass::CommentMaterial] {
        setup::allow(
            &db,
            configured.endpoint,
            TaskKey::CommentUnderstanding,
            material,
        )
        .await;
    }
    let probes = endpoint.requests.lock().unwrap().len();
    Env {
        db,
        endpoint,
        adapter,
        task_config: config.task_config_ref,
        task_revision: config.revision,
        probes,
    }
}

impl Env {
    /// A work with a saved L1 Core, so its comments can be researched.
    async fn work_with_l1(&self, external_id: &str, l1_model_config: Uuid) -> Uuid {
        let work = support::work(&self.db, external_id, "合成正文：讲孩子做事的习惯。").await;
        topic_tests::seed_core(&self.db, work, l1_model_config, false).await;
        work
    }

    async fn comment(&self, work_external_id: &str, id: &str, text: &str, author: &str) -> Uuid {
        support::comments::comment_with_author(
            &self.db,
            work_external_id,
            id,
            text,
            Some(author),
            "2026-10-14T00:00:00Z",
        )
        .await
    }

    async fn run(&self, selections: &[Selection], rerun: bool) -> (Value, Uuid) {
        let plan = preview(&self.db, support::D, selections, rerun)
            .await
            .unwrap();
        let receipt = start(
            &self.db,
            &Start {
                command_ref: Uuid::new_v4(),
                domain_ref: support::D,
                selections: selections.to_vec(),
                knowledge_cutoff: plan["knowledgeCutoff"].as_str().unwrap().into(),
                expected_plan_hash: plan["planHash"].as_str().unwrap().into(),
                task_config_ref: self.task_config,
                expected_task_revision: self.task_revision,
                rerun,
            },
        )
        .await
        .unwrap();
        (plan, receipt.batch_ref)
    }

    /// Runs the real executor until every unit of the batch has settled.
    async fn settle(&self, batch: Uuid) {
        let drain = crate::model_worker_drain::ModelWorkerDrain::new();
        let running = tokio::spawn(crate::ai_foundation::executor::service::run(
            self.db.clone(),
            "synthetic-comment-understanding-service",
            4,
            Arc::new(crate::model_secrets::SyntheticModelSecrets),
            self.adapter.clone(),
            drain.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let states: Vec<(String, Option<String>)> = sqlx::query_as(
                    "SELECT c.state,c.wait_reason FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE i.batch_ref=$1",
                )
                .bind(batch)
                .fetch_all(self.db.pool())
                .await
                .unwrap();
                assert!(states.iter().all(|(_, wait)| wait.is_none()), "executor deferred: {states:?}");
                if states.iter().all(|(state, _)| state != "pending" && state != "running") {
                    break;
                }
                assert!(!running.is_finished(), "executor stopped: {states:?}");
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("the batch did not settle");
        drain.request();
        let _ = running.await;
    }

    /// (comment id, stored result, failure code) of every V2 research.
    async fn results(&self) -> Vec<(String, Option<Value>, Option<String>)> {
        sqlx::query_as(
            "SELECT m.comment_external_id,x.result,x.failure_code FROM linggan_comment_research_request r JOIN linggan_comment_research_result x USING(request_ref) JOIN linggan_material_comment m ON m.material_ref=r.source_ref WHERE r.method_version='comment-understanding.v1' ORDER BY m.comment_external_id",
        )
        .fetch_all(self.db.pool())
        .await
        .unwrap()
    }

    /// label, state, signals, comments, authors, works of every live Problem.
    async fn problems(&self) -> Vec<(String, String, i64, i64, i64, i64)> {
        sqlx::query_as(
            "SELECT label,state,signal_count,comment_count,author_count,work_count FROM linggan_comment_problem_current WHERE domain_ref=$1 ORDER BY label",
        )
        .bind(support::D)
        .fetch_all(self.db.pool())
        .await
        .unwrap()
    }

    /// Requests the provider received for research units, in order.
    fn unit_requests(&self) -> Vec<Value> {
        self.endpoint
            .requests
            .lock()
            .unwrap()
            .iter()
            .skip(self.probes)
            .map(|request| serde_json::from_str(&wire::text(request, "user")).unwrap())
            .collect()
    }
}

fn selection(work: Uuid, source: Uuid) -> Selection {
    Selection {
        work_ref: work,
        source_ref: source,
        observation_role: StudySourceRole::Primary,
    }
}

fn failure_of<'a>(
    results: &'a [(String, Option<Value>, Option<String>)],
    id: &str,
) -> Option<&'a str> {
    results
        .iter()
        .find(|(comment, ..)| comment == id)
        .and_then(|(_, _, f)| f.as_deref())
}

fn result_of<'a>(results: &'a [(String, Option<Value>, Option<String>)], id: &str) -> &'a Value {
    results
        .iter()
        .find(|(comment, ..)| comment == id)
        .and_then(|(_, r, _)| r.as_ref())
        .unwrap_or_else(|| panic!("{id} has no stored result: {results:?}"))
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn one_call_per_work_places_every_comment_and_the_next_unit_reuses_the_catalog() {
    let env = environment("cu_flow", "normal").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let (one, two) = ("work-one", "work-two");
    let w1 = env.work_with_l1(one, l1_config).await;
    let w2 = env.work_with_l1(two, l1_config).await;
    let c1 = env
        .comment(one, "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let c2 = env
        .comment(one, "c2", "我家也是不提醒就不动", "reader-b")
        .await;
    let c3 = env.comment(one, "c3", "+1", "reader-c").await;
    let c4 = env.comment(one, "c4", "写作业特别磨蹭", "reader-d").await;
    let c5 = env.comment(one, "c5", "MISSING 也要提醒", "reader-e").await;
    let c6 = env.comment(one, "c6", "BADEVIDENCE", "reader-f").await;
    let x1 = env
        .comment(two, "x1", "我家孩子也要提醒才写", "reader-g")
        .await;
    let selections: Vec<_> = [
        (w1, c1),
        (w1, c2),
        (w1, c3),
        (w1, c4),
        (w1, c5),
        (w1, c6),
        (w2, x1),
    ]
    .into_iter()
    .map(|(w, c)| selection(w, c))
    .collect();

    let (plan, batch) = env.run(&selections, false).await;
    // Two works make two units, and every unit is one model call; the busier work goes first.
    assert_eq!(plan["researchableCount"], 7, "{plan}");
    assert_eq!(plan["unitCount"], 2);
    assert_eq!(plan["expectedModelCalls"], 2);
    assert_eq!(plan["workCount"], 2);
    assert_eq!(plan["waitingL1"]["targetCount"], 0);
    assert_eq!(plan["units"][0]["workRef"], json!(w1));
    env.settle(batch).await;

    // The comment directory shows V2 research: the state of the latest study and the signal kinds.
    crate::comment_study_catalog::refresh_clean_cache(&env.db, support::D, 200)
        .await
        .unwrap();
    let catalog = crate::comment_study_catalog::read_comment_catalog(
        &env.db,
        &serde_json::from_value(json!({"domain": support::D, "workRef": w1, "limit": 50})).unwrap(),
    )
    .await
    .unwrap();
    let mut shown: Vec<(String, String, Vec<String>)> = catalog["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["commentKey"]["commentExternalId"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                item["latestStudy"]["state"].as_str().unwrap().to_owned(),
                item["signalKinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_str().unwrap().to_owned())
                    .collect(),
            )
        })
        .collect();
    shown.sort();
    let row = |id: &str, state: &str, kinds: &[&str]| {
        (
            id.to_owned(),
            state.to_owned(),
            kinds.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        shown,
        [
            row("c1", "succeeded", &["need"]),
            row("c2", "succeeded", &["need"]),
            row("c3", "succeeded", &[]),
            row("c4", "succeeded", &["problem"]),
            row("c5", "failed", &[]),
            row("c6", "failed", &[]),
        ]
    );
    // The receipt of the run: calls and tokens come from the ledger, targets from their results.
    let run = read_run(&env.db, support::D, batch).await.unwrap();
    assert_eq!(run["usage"]["modelCalls"], 2);
    assert!(run["usage"]["inputTokens"].as_i64().unwrap() > 0, "{run}");
    assert_eq!(
        run["targets"],
        json!([
            {"state":"failed","failureCode":"target_all_signals_invalid","count":1},
            {"state":"failed","failureCode":"target_missing","count":1},
            {"state":"succeeded","failureCode":null,"count":5},
        ])
    );
    assert_eq!(run["units"].as_array().unwrap().len(), 2);
    assert!(
        read_run(&env.db, Uuid::new_v4(), batch).await.is_err(),
        "another domain cannot read the run"
    );
    let requests = env.unit_requests();
    assert_eq!(
        requests.len(),
        2,
        "one provider call per unit, none per comment"
    );
    assert_eq!(requests[0]["targets"].as_array().unwrap().len(), 6);
    assert_eq!(requests[0]["catalog"].as_array().unwrap().len(), 0);
    assert_eq!(requests[1]["targets"].as_array().unwrap().len(), 1);
    // The second unit was read against what the first unit wrote.
    assert_eq!(requests[1]["catalog"].as_array().unwrap().len(), 2);
    assert!(requests[0]["workContext"]["analysisRef"].is_string());
    assert_ne!(
        requests[0]["workContext"]["analysisRef"],
        requests[1]["workContext"]["analysisRef"]
    );
    let sent = requests[0].to_string();
    for private in ["reader-a", "reader-b", "reader-g", "sourceRef", "authorRef"] {
        assert!(!sent.contains(private), "the call carries {private}");
    }

    let results = env.results().await;
    assert_eq!(failure_of(&results, "c5"), Some("target_missing"));
    assert_eq!(
        failure_of(&results, "c6"),
        Some("target_all_signals_invalid")
    );
    assert_eq!(result_of(&results, "c3")["outcome"], "no_signal");
    let first = &result_of(&results, "c1")["signals"][0];
    assert_eq!(first["kind"], "need");
    assert_eq!(first["eligibilityState"], "eligible");
    // The evidence span points at "提醒" in the original text.
    let text = "我家孩子每天都要提醒才动笔";
    let start_at = text.chars().position(|ch| ch == '提').unwrap();
    assert_eq!(
        first["evidenceSpan"],
        json!({"start": start_at, "end": start_at + 2})
    );
    let reminder = result_of(&results, "c1")["signals"][0]["problemRef"].clone();
    assert!(reminder.is_string());
    for id in ["c2", "x1"] {
        assert_eq!(
            result_of(&results, id)["signals"][0]["problemRef"],
            reminder,
            "{id}"
        );
    }
    assert_ne!(
        result_of(&results, "c4")["signals"][0]["problemRef"],
        reminder
    );

    // Two Problems, not three: the second work matched instead of creating a duplicate. Three
    // authors across two works make the reminder need formal.
    assert_eq!(
        env.problems().await,
        [
            (
                "孩子写作业磨蹭".to_owned(),
                "candidate".to_owned(),
                1,
                1,
                1,
                1
            ),
            (
                "孩子需要每天有人提醒".to_owned(),
                "formal".to_owned(),
                3,
                3,
                3,
                2
            ),
        ]
    );
    let dispatched: Vec<(i32,)> = sqlx::query_as("SELECT cardinality(catalog_problem_refs) FROM linggan_comment_research_unit_dispatch d JOIN linggan_comment_research_unit u USING(unit_ref) JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref JOIN linggan_ai_batch b USING(batch_ref) WHERE b.batch_ref=$1 ORDER BY i.position")
        .bind(batch)
        .fetch_all(env.db.pool())
        .await
        .unwrap();
    assert_eq!(dispatched, [(0,), (2,)]);
    let item: Value = sqlx::query_scalar("SELECT c.output FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE i.batch_ref=$1 AND i.position=0")
        .bind(batch)
        .fetch_one(env.db.pool())
        .await
        .unwrap();
    assert_eq!(
        (
            item["acceptedTargets"].clone(),
            item["rejectedTargets"].clone(),
            item["newProblems"].clone()
        ),
        (json!(4), json!(2), json!(2))
    );
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE task_key='comment_understanding' AND state='succeeded'")
        .fetch_one(env.db.pool())
        .await
        .unwrap();
    assert_eq!(invocations, 2);

    // The invariants that must hold for every stored result, counted independently of the views:
    // every evidence span lies inside its own comment, every signal points at its own source,
    // and no support points at a Problem of another domain.
    let violations: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM linggan_comment_research_result x
           JOIN linggan_comment_research_request r USING(request_ref)
           JOIN linggan_material_comment m ON m.material_ref=r.source_ref
           CROSS JOIN LATERAL jsonb_array_elements(x.result->'signals') s
          WHERE x.result IS NOT NULL AND r.method_version='comment-understanding.v1'
            AND ((s->'evidenceSpan'->>'start')::int<0
              OR (s->'evidenceSpan'->>'end')::int<=(s->'evidenceSpan'->>'start')::int
              OR (s->'evidenceSpan'->>'end')::int>char_length(m.body_text)
              OR s->>'sourceRef'<>r.source_ref::text))
         + (SELECT count(*) FROM linggan_comment_problem_membership mb
           JOIN linggan_comment_problem_definition d ON d.definition_ref=mb.definition_ref
           JOIN linggan_comment_research_request r ON r.request_ref=mb.request_ref
          WHERE d.domain_ref<>r.domain_ref)
         + (SELECT count(*) FROM linggan_comment_problem_current WHERE state='formal' AND author_count<2)",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    assert_eq!(violations, 0);

    // Researched comments are not researched again; the two that failed are.
    let again = preview(&env.db, support::D, &selections, false)
        .await
        .unwrap();
    assert_eq!(again["alreadyResearchedCount"], 5);
    assert_eq!(again["researchableCount"], 2);
    assert_eq!(again["unitCount"], 1);
    let forced = preview(&env.db, support::D, &selections, true)
        .await
        .unwrap();
    assert_eq!(forced["researchableCount"], 7);
    env.endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn a_cut_off_response_fails_every_comment_of_its_unit_and_nothing_else() {
    let env = environment("cu_truncated", "understanding_truncated").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let w = env.work_with_l1("work-one", l1_config).await;
    let a = env
        .comment("work-one", "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let b = env
        .comment("work-one", "c2", "我家也是不提醒就不动", "reader-b")
        .await;
    let (_, batch) = env.run(&[selection(w, a), selection(w, b)], false).await;
    env.settle(batch).await;
    let results = env.results().await;
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(
        |(_, result, failure)| result.is_none() && failure.as_deref() == Some("unit_truncated")
    ));
    assert!(env.problems().await.is_empty());
    let (state, failure, counts): (String, Option<String>, bool) = sqlx::query_as("SELECT c.state,c.failure_code,c.counts_as_failure FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE i.batch_ref=$1")
        .bind(batch)
        .fetch_one(env.db.pool())
        .await
        .unwrap();
    assert_eq!(
        (state.as_str(), failure.as_deref(), counts),
        ("failed", Some("unit_truncated"), true)
    );
    let invocation: Option<String> = sqlx::query_scalar(
        "SELECT failure_code FROM linggan_model_invocation WHERE task_key='comment_understanding'",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    assert_eq!(invocation.as_deref(), Some("unit_truncated"));
    env.endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn an_unusable_response_is_a_unit_failure_and_writes_no_problem() {
    let env = environment("cu_invalid", "understanding_invalid").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let w = env.work_with_l1("work-one", l1_config).await;
    let a = env
        .comment("work-one", "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let (_, batch) = env.run(&[selection(w, a)], false).await;
    env.settle(batch).await;
    let results = env.results().await;
    assert_eq!(failure_of(&results, "c1"), Some("unit_output_invalid"));
    assert!(env.problems().await.is_empty());
    env.endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn comments_of_a_work_without_l1_wait_and_are_not_planned() {
    let (db, l1_config) = support::setup("cu_waiting_l1").await;
    let _ = l1_config;
    let work = support::work(&db, "work-one", "合成正文").await;
    let comment = support::comments::comment_with_author(
        &db,
        "work-one",
        "c1",
        "我家孩子每天都要提醒才动笔",
        Some("reader-a"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    let plan = preview(&db, support::D, &[selection(work, comment)], false)
        .await
        .unwrap();
    assert_eq!(plan["researchableCount"], 0);
    assert_eq!(plan["unitCount"], 0);
    assert_eq!(plan["waitingL1"]["targetCount"], 1);
    assert_eq!(plan["waitingL1"]["workRefs"], json!([work]));
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn downstream_readers_see_v2_research_and_stop_at_a_restricted_comment() {
    let env = environment("cu_readers", "normal").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let w = env.work_with_l1("work-one", l1_config).await;
    let c1 = env
        .comment("work-one", "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let c2 = env
        .comment("work-one", "c2", "我家也是不提醒就不动", "reader-b")
        .await;
    let c3 = env
        .comment("work-one", "c3", "MISSING 也要提醒", "reader-c")
        .await;
    let (plan, batch) = env
        .run(
            &[selection(w, c1), selection(w, c2), selection(w, c3)],
            false,
        )
        .await;
    env.settle(batch).await;
    let as_of = plan["knowledgeCutoff"].as_str().unwrap().to_owned();
    let later: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(env.db.pool())
        .await
        .unwrap();
    let _ = as_of;

    // Topic Map reads a research by its stored result, not by the state of a batch item that
    // happens to share the batch: both researched comments appear, the failed one does not.
    use sqlx::Row;
    let mut rows: Vec<String> =
        sqlx::query(include_str!("../../topic_map_research/comment_signals.sql"))
            .bind(support::D)
            .bind(vec![c1, c2, c3])
            .bind(&later)
            .fetch_all(env.db.pool())
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.get("comment_external_id"))
            .collect();
    rows.sort();
    assert_eq!(rows, ["c1", "c2"]);

    let request_of = |comment: Uuid| {
        let db = env.db.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT request_ref FROM linggan_comment_research_request WHERE source_ref=$1",
            )
            .bind(comment)
            .fetch_one(db.pool())
            .await
            .unwrap()
        }
    };
    let (r1, r2) = (request_of(c1).await, request_of(c2).await);
    let mut tx = env.db.pool().begin().await.unwrap();
    let source = crate::ai_foundation::comment_research::current_source_in(&mut tx, r1)
        .await
        .unwrap();
    assert_eq!(
        (
            source.comment_external_id.as_str(),
            source.raw_text.as_str()
        ),
        ("c1", "我家孩子每天都要提醒才动笔")
    );
    assert_eq!(source.work_ref, w);
    let input = crate::ai_foundation::topic_comment_source::load_in(&mut tx, r1)
        .await
        .unwrap();
    assert!(
        input.fragments.iter().any(|f| f.text == "提醒"),
        "the evidence reaches the topic reader"
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        env.problems().await,
        [(
            "孩子需要每天有人提醒".to_owned(),
            "formal".to_owned(),
            2,
            2,
            2,
            1
        )]
    );

    // A restricted comment stops counting at once: the Problem falls back to a candidate, and no
    // reader hands out the withdrawn text.
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'c2','synthetic restriction')")
        .bind(w)
        .execute(env.db.pool())
        .await
        .unwrap();
    assert_eq!(
        env.problems().await,
        [(
            "孩子需要每天有人提醒".to_owned(),
            "candidate".to_owned(),
            1,
            1,
            1,
            1
        )]
    );
    let mut tx = env.db.pool().begin().await.unwrap();
    assert!(
        crate::ai_foundation::comment_research::current_source_in(&mut tx, r2)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    env.endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn a_cancelled_run_calls_no_provider_and_cancels_every_comment() {
    let env = environment("cu_cancel", "normal").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let w = env.work_with_l1("work-one", l1_config).await;
    let a = env
        .comment("work-one", "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let b = env
        .comment("work-one", "c2", "我家也是不提醒就不动", "reader-b")
        .await;
    let (_, batch) = env.run(&[selection(w, a), selection(w, b)], false).await;
    let stop = Cancel {
        command_ref: Uuid::new_v4(),
        domain_ref: support::D,
        batch_ref: batch,
    };
    let receipt = cancel(&env.db, &stop).await.unwrap();
    assert_eq!(
        cancel(&env.db, &stop).await.unwrap(),
        receipt,
        "the same command replays"
    );
    let elsewhere = Cancel {
        command_ref: Uuid::new_v4(),
        domain_ref: Uuid::new_v4(),
        ..stop.clone()
    };
    assert!(
        cancel(&env.db, &elsewhere).await.is_err(),
        "another domain cannot cancel the run"
    );
    env.settle(batch).await;
    assert!(
        env.unit_requests().is_empty(),
        "a cancelled run must not reach the provider"
    );
    let run = read_run(&env.db, support::D, batch).await.unwrap();
    assert_eq!(run["cancelRequested"], true);
    assert_eq!(
        run["targets"],
        json!([{"state":"cancelled","failureCode":null,"count":2}])
    );
    assert_eq!(run["usage"]["modelCalls"], 0);
    assert!(env.results().await.is_empty());
    env.endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL and loopback provider only"]
async fn a_start_is_idempotent_and_refuses_a_plan_that_no_longer_matches() {
    use crate::ai_foundation::store::FoundationError;
    let env = environment("cu_start", "normal").await;
    let l1_config: Uuid = sqlx::query_scalar(
        "SELECT config_ref FROM linggan_model_config ORDER BY created_at LIMIT 1",
    )
    .fetch_one(env.db.pool())
    .await
    .unwrap();
    let w = env.work_with_l1("work-one", l1_config).await;
    let a = env
        .comment("work-one", "c1", "我家孩子每天都要提醒才动笔", "reader-a")
        .await;
    let b = env
        .comment("work-one", "c2", "我家也是不提醒就不动", "reader-b")
        .await;
    let plan = preview(&env.db, support::D, &[selection(w, a)], false)
        .await
        .unwrap();
    let request = Start {
        command_ref: Uuid::new_v4(),
        domain_ref: support::D,
        selections: vec![selection(w, a)],
        knowledge_cutoff: plan["knowledgeCutoff"].as_str().unwrap().into(),
        expected_plan_hash: plan["planHash"].as_str().unwrap().into(),
        task_config_ref: env.task_config,
        expected_task_revision: env.task_revision,
        rerun: false,
    };
    let first = start(&env.db, &request).await.unwrap();
    assert_eq!(
        start(&env.db, &request).await.unwrap(),
        first,
        "the same command replays"
    );
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_request WHERE method_version='comment-understanding.v1'")
        .fetch_one(env.db.pool())
        .await
        .unwrap();
    assert_eq!(requests, 1, "a replay creates nothing");

    // A different command over a plan whose inputs changed is refused, not run as something else.
    let changed = Start {
        command_ref: Uuid::new_v4(),
        selections: vec![selection(w, a), selection(w, b)],
        ..request.clone()
    };
    assert!(matches!(
        start(&env.db, &changed).await,
        Err(FoundationError::Conflict)
    ));
    // The comment already queued is not planned twice.
    let again = preview(&env.db, support::D, &[selection(w, a)], false)
        .await
        .unwrap();
    assert_eq!(
        (
            again["alreadyQueuedCount"].clone(),
            again["researchableCount"].clone()
        ),
        (json!(1), json!(0))
    );
    let nothing = Start {
        command_ref: Uuid::new_v4(),
        expected_plan_hash: again["planHash"].as_str().unwrap().into(),
        knowledge_cutoff: again["knowledgeCutoff"].as_str().unwrap().into(),
        ..request.clone()
    };
    assert!(matches!(
        start(&env.db, &nothing).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    // A stale task revision is a conflict too.
    let stale_revision = Start {
        command_ref: Uuid::new_v4(),
        expected_task_revision: env.task_revision + 1,
        ..request
    };
    assert!(start(&env.db, &stale_revision).await.is_err());
    env.endpoint.finish().await;
}
