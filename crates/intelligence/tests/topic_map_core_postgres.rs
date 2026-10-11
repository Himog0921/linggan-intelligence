//! Native PostgreSQL business proofs with deterministic synthetic model decisions.
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    model_settings::reserve_research_model_semantic_dispatch_permit,
    topic_map::{self, TopicMapQuery},
    topic_map_research::{ResearchCommand, apply_research_command},
    topic_map_research_worker::run_once,
};
use serde_json::{Value, json};
use uuid::Uuid;

fn query() -> TopicMapQuery {
    TopicMapQuery {
        domain_ref: Some(D),
        reference_window_days: Some(0),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn canonical_identity_uses_boundaries_and_preserves_low_frequency_counterexamples() {
    let (db, config, adapter) = setup("topic_core_identity").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let a = work(&db, "core-start", "SYNTHETIC 开始练习前需要提醒。").await;
    let b = work(
        &db,
        "core-alias",
        "SYNTHETIC ALIAS_START 我先拆解第一步才开始。",
    )
    .await;
    for w in [a, b] {
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        finish_pending(&db, &adapter).await;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let c = work(
        &db,
        "core-sustain",
        "SYNTHETIC SUSTAIN_ONLY 练习开始之后反复走神。",
    )
    .await;
    comments::comment_with_author(
        &db,
        "core-start",
        "one-counterexample",
        "SYNTHETIC CHALLENGE 提醒没有帮助我开始练习。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    for w in [c, a] {
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        finish_pending(&db, &adapter).await;
    }
    let snapshot = topic_map::read_topic_map(&db, &query()).await.unwrap();
    let topics: Vec<_> = snapshot
        .topics
        .iter()
        .filter(|t| t.core["definitionSource"] == "machine_induced")
        .collect();
    assert_eq!(
        topics.len(),
        2,
        "same name with a different boundary creates a separate identity"
    );
    assert_eq!(topics[0].display_name, topics[1].display_name);
    assert_ne!(topics[0].definition_text, topics[1].definition_text);
    let start_topic = topics
        .iter()
        .find(|t| t.definition_text.contains("安排并开始"))
        .unwrap();
    assert_eq!(start_topic.core["discussionCount"], 3);
    assert_eq!(
        start_topic.core["evidenceRoles"]["challenge"], 1,
        "one counterexample stays visible"
    );
    assert_eq!(
        start_topic.direct_work_refs.len(),
        2,
        "discussion evidence is not a people/work count"
    );
    assert_eq!(
        topics
            .iter()
            .find(|t| t.topic_ref != start_topic.topic_ref)
            .unwrap()
            .core["discussionCount"],
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_relation relation JOIN linggan_topic_map_unit_resolution resolution USING(resolution_ref) WHERE resolution.status='matched'")
            .fetch_one(db.pool()).await.unwrap(),
        0,
        "a matched unit's proposed relationship cannot become a concept-to-concept assertion"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn accepted_partial_unit_survives_budget_pause_without_publishing_draft_angles() {
    let (db, config, adapter) = setup("topic_core_partial").await;
    let w = work(
        &db,
        "core-multi",
        "SYNTHETIC MULTI\n开始练习前我需要提醒。\n练习开始之后我需要持续注意。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let run = apply_research_command(&db, &start(vec![w])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_unit_resolution")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    sqlx::query(
        "UPDATE linggan_topic_map_research_policy SET daily_token_limit=1024 WHERE domain_ref=$1",
    )
    .bind(D)
    .execute(db.pool())
    .await
    .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(run["runRef"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "daily_budget_paused"
    );
    let snapshot = topic_map::read_topic_map(&db, &query()).await.unwrap();
    let research = snapshot
        .works
        .iter()
        .find(|x| x.work_ref == w)
        .unwrap()
        .research
        .as_ref()
        .unwrap();
    assert_eq!(research["core"]["units"].as_array().unwrap().len(), 1);
    assert_eq!(research["core"]["coverage"]["state"], "partial");
    assert!(research["resultRef"].is_null());
    assert!(
        research["output"]["angles"]
            .as_array()
            .is_none_or(Vec::is_empty)
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
#[ignore = "isolated native PostgreSQL sessions/advisory locks; no real model"]
async fn concurrent_workers_recheck_the_prepared_phase_after_waiting_for_the_model_lock() {
    let (db, config, adapter) = setup("topic_core_phase_race").await;
    let w = work(&db, "core-race", "SYNTHETIC 开始练习。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    let blocker = reserve_research_model_semantic_dispatch_permit(&db, config)
        .await
        .unwrap();
    let launch = || {
        let db = db.clone();
        let adapter = adapter.clone();
        tokio::spawn(async move {
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        })
    };
    let first = launch();
    let second = launch();
    let mut waiting = 0;
    for _ in 0..200 {
        waiting = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        if waiting >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        waiting >= 2,
        "both workers must actually prepare the same queued phase before release"
    );
    blocker.release().await.unwrap();
    let a = first.await.unwrap();
    let b = second.await.unwrap();
    assert!(a || b);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1,
        "stale prepared work must not reserve or transmit"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT phase FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "resolve"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_discussion_unit")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn same_canonical_work_has_distinct_domain_discussion_identity() {
    let (db, config, adapter) = setup("topic_core_domains").await;
    let w = work(&db, "core-shared", "SYNTHETIC 开始练习前需要提醒。").await;
    let other = Uuid::from_u128(0x00000000000040008000000000000002);
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)SELECT $1,content_public_ref,$2,'primary','legacy_domain_migration',package_ref FROM linggan_material_domain_usage WHERE content_public_ref=$3 AND domain_ref=$4 LIMIT 1")
        .bind(Uuid::new_v4()).bind(other).bind(w).bind(D).execute(db.pool()).await.unwrap();
    for domain in [D, other] {
        let mut config_command = configure(config, 100000, false);
        let mut start_command = start(vec![w]);
        if let ResearchCommand::Configure { domain_ref, .. } = &mut config_command {
            *domain_ref = domain;
        }
        if let ResearchCommand::Start { domain_ref, .. } = &mut start_command {
            *domain_ref = domain;
        }
        apply_research_command(&db, &config_command).await.unwrap();
        apply_research_command(&db, &start_command).await.unwrap();
        finish_pending(&db, &adapter).await;
    }
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(DISTINCT domain_ref) FROM linggan_topic_map_discussion_unit WHERE work_public_ref=$1").bind(w).fetch_one(db.pool()).await.unwrap(),2);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(DISTINCT unit_ref) FROM linggan_topic_map_discussion_unit WHERE work_public_ref=$1").bind(w).fetch_one(db.pool()).await.unwrap(),2);
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn withdrawn_comment_hides_machine_definition_and_removes_it_from_future_recall() {
    let (db, config, adapter) = setup("topic_core_withdrawn").await;
    let w = work(&db, "core-private", "").await;
    let comment = comments::comment_with_author(
        &db,
        "core-private",
        "private-rule",
        "SYNTHETIC PRIVATE_RULE 开始练习的特殊边界。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    finish_pending(&db, &adapter).await;
    let before = topic_map::read_topic_map(&db, &query()).await.unwrap();
    assert!(
        serde_json::to_string(&before)
            .unwrap()
            .contains("SYNTHETIC_RESTRICTED_RULE")
    );
    assert!(
        before
            .works
            .iter()
            .find(|x| x.work_ref == w)
            .unwrap()
            .research
            .is_some(),
        "qualified comment-only research is readable"
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(comment).execute(db.pool()).await.unwrap();
    let after = topic_map::read_topic_map(&db, &query()).await.unwrap();
    assert!(
        !serde_json::to_string(&after)
            .unwrap()
            .contains("SYNTHETIC_RESTRICTED_RULE")
    );
    assert_eq!(after.topics[0].core["sourceState"], "source_unavailable");
    let next = work(&db, "core-next", "SYNTHETIC 普通的练习启动。").await;
    apply_research_command(&db, &start(vec![next]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let topics:Vec<Value>=sqlx::query_scalar("SELECT q.request_manifest->'topics' FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.work_public_ref=$1 AND q.phase='resolve'")
        .bind(next).fetch_all(db.pool()).await.unwrap();
    assert!(!topics.is_empty());
    assert!(
        !json!(topics)
            .to_string()
            .contains("SYNTHETIC_RESTRICTED_RULE")
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL sessions/advisory locks; no real model"]
async fn withdrawn_source_during_model_wait_never_reserves_or_transmits() {
    let (db, config, adapter) = setup("topic_core_pre_dispatch_withdrawal").await;
    let w = work(&db, "core-wait-source", "").await;
    let comment = comments::comment_with_author(
        &db,
        "core-wait-source",
        "waiting-comment",
        "SYNTHETIC 开始练习的评论。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    let blocker = reserve_research_model_semantic_dispatch_permit(&db, config)
        .await
        .unwrap();
    let worker_db = db.clone();
    let worker = tokio::spawn(async move {
        run_once(&worker_db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    });
    let mut waiting = false;
    for _ in 0..500 {
        waiting = sqlx::query_scalar::<_, bool>(
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
    assert!(
        waiting,
        "the source must already be prepared before withdrawal"
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic withdrawal while waiting' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(comment).execute(db.pool()).await.unwrap();
    blocker.release().await.unwrap();
    assert!(worker.await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request",)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT last_reason FROM linggan_topic_map_research_task",)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "pre_dispatch_source_changed"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn inferred_parent_is_reused_and_contains_distinct_execution_stages() {
    let (db, config, adapter) = setup("topic_core_parent_tree").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    for (id, text) in [
        (
            "tree-start",
            "SYNTHETIC TREE_START 开始练习前需要明确第一步。",
        ),
        (
            "tree-sustain",
            "SYNTHETIC TREE_SUSTAIN SUSTAIN_ONLY 练习开始之后反复走神。",
        ),
    ] {
        let w = work(&db, id, text).await;
        apply_research_command(&db, &start(vec![w])).await.unwrap();
        finish_pending(&db, &adapter).await;
    }
    let snapshot = topic_map::read_topic_map(&db, &query()).await.unwrap();
    assert_eq!(
        snapshot.topics.len(),
        3,
        "one inferred parent and two independent stages"
    );
    let parent = snapshot
        .topics
        .iter()
        .find(|topic| topic.display_name == "合成任务执行支持")
        .unwrap();
    assert!(parent.parent_topic_ref.is_none());
    assert_eq!(parent.lifecycle_state, "candidate");
    let children: Vec<_> = snapshot
        .topics
        .iter()
        .filter(|topic| topic.parent_topic_ref == Some(parent.topic_ref))
        .collect();
    assert_eq!(
        children.len(),
        2,
        "new candidates actually bind beneath a reusable scope"
    );
    assert_ne!(children[0].definition_text, children[1].definition_text);
    assert!(
        children
            .iter()
            .all(|child| child.lifecycle_state == "candidate")
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule WHERE method_version='topic-map.core.parent.v1'").fetch_one(db.pool()).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_receipt WHERE action='candidate_parent'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_discussion_unit")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2,
        "parent abstraction does not manufacture extra source discussions"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn a_stale_parent_version_cannot_accept_a_new_child() {
    let (db, config, adapter) = setup("topic_core_parent_stale").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let first = work(
        &db,
        "tree-parent-first",
        "SYNTHETIC TREE_START 开始练习前需要提醒。",
    )
    .await;
    apply_research_command(&db, &start(vec![first]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let second = work(
        &db,
        "tree-parent-invalid",
        "SYNTHETIC TREE_BAD_PARENT_VERSION SUSTAIN_ONLY 开始之后持续困难。",
    )
    .await;
    let receipt = apply_research_command(&db, &start(vec![second]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT last_reason FROM linggan_topic_map_research_task WHERE run_ref=$1 ORDER BY created_at DESC LIMIT 1").bind(run).fetch_one(db.pool()).await.unwrap(), "invalid_topic_parent");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_concept_rule")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2,
        "invalid version cannot create a child or replace the parent"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_binding WHERE version=2"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn a_reused_candidate_cannot_be_reparented_or_create_an_unused_parent() {
    let (db, config, adapter) = setup("topic_core_reused_binding").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let first = work(&db, "unchanged-root", "SYNTHETIC 开始练习前先明确第一步。").await;
    apply_research_command(&db, &start(vec![first]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let before = topic_map::read_topic_map(&db, &query()).await.unwrap();
    let original = before.topics[0].topic_ref;
    let definition = before.topics[0].definition_ref;
    let second = work(
        &db,
        "attempt-reparent",
        "SYNTHETIC TREE_REUSE_REPARENT 开始练习时需要提醒。",
    )
    .await;
    apply_research_command(&db, &start(vec![second]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let after = topic_map::read_topic_map(&db, &query()).await.unwrap();
    assert_eq!(
        after.topics.len(),
        1,
        "an exact reused identity does not create an orphan parent"
    );
    assert_eq!(after.topics[0].topic_ref, original);
    assert_eq!(after.topics[0].definition_ref, definition);
    assert!(after.topics[0].parent_topic_ref.is_none());
    assert_eq!(after.topics[0].binding_version, Some(1));
    assert_eq!(
        after.topics[0].direct_work_refs.len(),
        2,
        "reuse still adds new qualified evidence"
    );
}
