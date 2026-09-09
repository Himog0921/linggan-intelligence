#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
#[allow(dead_code)]
mod research_fixture;
use fixture::proof_database;
use linggan_intelligence::comment_intelligence_actions::{execute_action, source_research};
use linggan_intelligence::comment_intelligence_problems::{
    apply_problem_relation_output, prepare_problem_relation, reconcile_problem_index,
};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

fn command(domain: Uuid, kind: &str, revision: i64, payload: Value) -> Value {
    json!({"commandRef":Uuid::new_v4(),"domain":domain,"kind":kind,"expectedRevision":revision,"payload":payload,"reason":"SYNTHETIC / NOT EVIDENCE · 边界测试"})
}
async fn seed(db: &linggan_storage_postgres::Database, id: &str) -> Uuid {
    let source = research_fixture::comment(
        db,
        "p-boundary",
        id,
        "执行时间不够也担心药物风险",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let result = json!({"sourceRef":source,"sourceSha256":linggan_intelligence::comment_research::comment_source_hash("执行时间不够也担心药物风险"),"contextRefs":{"researchSourceRefs":[source]},"semantic":{"outcome":"interpretable","problems":[
        {"name":"执行时间","meaning":"家长没有时间","evidence":[{"sourceRef":source,"startChar":0,"endChar":6}]},
        {"name":"药物风险","meaning":"家长担心药物风险","evidence":[{"sourceRef":source,"startChar":7,"endChar":13}]}]}});
    sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'synthetic','synthetic','succeeded',$3)")
        .bind(Uuid::new_v4()).bind(source).bind(result).execute(db.pool()).await.unwrap();
    source
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn expression_confirmation_preserves_other_expression_and_reuses_human_rule() {
    let db = proof_database("ci_expression_scope").await;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    research_fixture::detail(&db, "p-boundary", "SYNTHETIC / NOT EVIDENCE").await;
    let source = seed(&db, "first").await;
    reconcile_problem_index(&db, 100).await.unwrap();
    let candidates: Vec<Uuid> = sqlx::query_scalar(
        "SELECT candidate_ref FROM linggan_ci_problem_candidate ORDER BY ordinal",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let created=execute_action(&db,command(domain,"problem_create",0,json!({"name":"执行时间","meaning":"家长没有时间","sourceRefs":[source],"candidateRefs":[candidates[0]]}))).await.unwrap();
    assert_eq!(
        source_research(&db, domain, source).await.unwrap()["locked"],
        false
    );
    let states: Vec<String> =
        sqlx::query_scalar("SELECT state FROM linggan_ci_problem_candidate ORDER BY ordinal")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(states, vec!["assigned", "unmerged"]);
    sqlx::query("DELETE FROM linggan_ci_projection_cursor")
        .execute(db.pool())
        .await
        .unwrap();
    reconcile_problem_index(&db, 100).await.unwrap();
    let assigned: String =
        sqlx::query_scalar("SELECT state FROM linggan_ci_problem_candidate WHERE candidate_ref=$1")
            .bind(candidates[0])
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(assigned, "assigned");
    let second = seed(&db, "second").await;
    reconcile_problem_index(&db, 100).await.unwrap();
    let state=sqlx::query("SELECT c.state,c.problem_ref FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref) WHERE s.source_ref=$1 AND c.ordinal=0")
        .bind(second).fetch_one(db.pool()).await.unwrap();
    assert_eq!(state.get::<String, _>("state"), "assigned");
    assert_eq!(
        json!(state.get::<Uuid, _>("problem_ref")),
        created["problemRef"]
    );
    let reused: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_problem_boundary_decision WHERE origin='boundary_reuse'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(reused, 1);
    execute_action(&db,command(domain,"candidate_resolve",1,json!({"candidateRef":candidates[0],"targetRef":created["problemRef"],"targetDefinitionRevision":1,"relation":"different"}))).await.unwrap();
    let first_members:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem_member m JOIN linggan_ci_source s USING(canonical_ref) WHERE s.source_ref=$1").bind(source).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        first_members, 0,
        "correcting an assigned expression detaches its old membership"
    );
    let other_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_ci_problem_candidate WHERE candidate_ref=$1")
            .bind(candidates[1])
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(other_state, "unmerged");
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn related_decision_cannot_create_membership_and_changed_definition_rejects_task_b() {
    let db = proof_database("ci_relation_guard").await;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    research_fixture::detail(&db, "p-boundary", "SYNTHETIC / NOT EVIDENCE").await;
    let source = seed(&db, "source").await;
    reconcile_problem_index(&db, 100).await.unwrap();
    let refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT candidate_ref FROM linggan_ci_problem_candidate ORDER BY ordinal",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let created=execute_action(&db,command(domain,"problem_create",0,json!({"name":"执行时间","meaning":"家长没有时间","sourceRefs":[source],"candidateRefs":[refs[0]]}))).await.unwrap();
    let problem = Uuid::parse_str(created["problemRef"].as_str().unwrap()).unwrap();
    let task = prepare_problem_relation(&db, domain, refs[1], &[problem], "synthetic-vector.v1")
        .await
        .unwrap();
    let output = json!({"decisions":[{"targetRef":problem,"definitionRevision":1,"relation":"related","reason":"SYNTHETIC 两个独立问题"}]});
    apply_problem_relation_output(&db, &task, &output)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_ci_problem_candidate WHERE candidate_ref=$1"
        )
        .bind(refs[1])
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "resolved"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem_member")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let action = command(
        domain,
        "candidate_resolve",
        0,
        json!({"candidateRef":refs[1],"targetRef":problem,"targetDefinitionRevision":1,"relation":"different"}),
    );
    execute_action(&db, action.clone()).await.unwrap();
    execute_action(&db, action).await.unwrap(); // one immutable decision receipt
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_ci_problem_boundary_decision WHERE candidate_ref=$1 AND origin='manual'").bind(refs[1]).fetch_one(db.pool()).await.unwrap(),1);
    sqlx::query("UPDATE linggan_ci_problem_candidate SET state='unmerged' WHERE candidate_ref=$1")
        .bind(refs[1])
        .execute(db.pool())
        .await
        .unwrap();
    let same = json!({"decisions":[{"targetRef":problem,"definitionRevision":1,"relation":"same","reason":"SYNTHETIC contradiction"}]});
    assert!(
        format!(
            "{:?}",
            apply_problem_relation_output(&db, &task, &same)
                .await
                .unwrap_err()
        )
        .contains("CI_HUMAN_BOUNDARY_CONFLICT")
    );
    sqlx::query("UPDATE linggan_ci_problem SET definition_revision=definition_revision+1 WHERE problem_ref=$1").bind(problem).execute(db.pool()).await.unwrap();
    assert!(
        format!(
            "{:?}",
            apply_problem_relation_output(&db, &task, &output)
                .await
                .unwrap_err()
        )
        .contains("CI_REVISION_CONFLICT")
    );
}

use linggan_intelligence::{
    comment_daily::*, model_invocation::*, model_secrets::*, model_settings::*, pi_adapter::*,
};
use linggan_storage_postgres::Database;
#[path = "support/comment_daily_fixture.rs"]
#[allow(dead_code)]
mod daily_fixture;

#[tokio::test]
#[ignore = "isolated PostgreSQL and loopback synthetic SDK provider"]
async fn qualified_embedding_cache_and_task_b_run_with_shared_budget_without_rewriting_extraction()
{
    let db = proof_database("ci_problem_worker").await;
    let (mut server, url) = daily_fixture::fixture_server().await;
    let (config, _, _) = daily_fixture::configured(&db, &url, "synthetic-good", None).await;
    configure_legacy_semantic_budget(&db, config).await;
    configure_qualified_legacy_embedding(&db, config).await;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    research_fixture::detail(&db, "worker-real-sdk", "SYNTHETIC / NOT EVIDENCE").await;
    let a = daily_fixture::source(&db, "worker-real-sdk", "a", "每天提醒学习很费力").await;
    let b = daily_fixture::source(&db, "worker-real-sdk", "b", "每天陪着写作业很疲惫").await;
    let batch = daily_fixture::selected(&db, config, vec![a, b], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    reconcile_problem_index(&db, 100).await.unwrap();
    set_batch_enabled(&db, batch, false).await.unwrap();
    let calls_before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(
        !linggan_intelligence::comment_intelligence_problems::run_problem_relation_once(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured()
        )
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        calls_before
    );
    set_batch_enabled(&db, batch, true).await.unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_analysis_work WHERE state='succeeded'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    for _ in 0..8 {
        if !linggan_intelligence::comment_intelligence_problems::run_problem_relation_once(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
        )
        .await
        .unwrap()
        {
            break;
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_analysis_work WHERE state='succeeded'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        before
    );
    assert_completed_legacy_relation(&db, batch, domain).await;
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn v4_repeated_names_do_not_bypass_relation_judgment() {
    let db = proof_database("ci_v4_no_exact_bypass").await;
    research_fixture::detail(&db, "p-boundary", "SYNTHETIC / NOT EVIDENCE").await;
    for i in 0..3 {
        seed(&db, &format!("c{i}")).await;
    }
    sqlx::query("UPDATE linggan_comment_analysis_work SET rule_version='comment-research.v4'")
        .execute(db.pool())
        .await
        .unwrap();
    reconcile_problem_index(&db, 100).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_candidate WHERE state='unmerged'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        6
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn uncertain_queue_is_independent_of_target_response_order() {
    let db = proof_database("ci_relation_order").await;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    research_fixture::detail(&db, "p-boundary", "SYNTHETIC / NOT EVIDENCE").await;
    let source = seed(&db, "expression").await;
    reconcile_problem_index(&db, 100).await.unwrap();
    let refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT candidate_ref FROM linggan_ci_problem_candidate ORDER BY ordinal",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    let first=execute_action(&db,command(domain,"problem_create",0,json!({"name":"执行时间","meaning":"家长没有时间","sourceRefs":[source],"candidateRefs":[refs[0]]}))).await.unwrap();
    let other = research_fixture::comment(
        &db,
        "p-boundary",
        "other",
        "SYNTHETIC 不同家庭场景",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let second = execute_action(
        &db,
        command(
            domain,
            "problem_create",
            0,
            json!({"name":"其他问题","meaning":"其他生活场景","sourceRefs":[other]}),
        ),
    )
    .await
    .unwrap();
    let p1 = Uuid::parse_str(first["problemRef"].as_str().unwrap()).unwrap();
    let p2 = Uuid::parse_str(second["problemRef"].as_str().unwrap()).unwrap();
    let task = prepare_problem_relation(&db, domain, refs[1], &[p1, p2], "synthetic-vector.v1")
        .await
        .unwrap();
    let a = json!({"targetRef":p1,"definitionRevision":1,"relation":"uncertain","reason":"SYNTHETIC 尚无充分边界"});
    let b = json!({"targetRef":p2,"definitionRevision":1,"relation":"different","reason":"SYNTHETIC 不同对象"});
    for decisions in [vec![a.clone(), b.clone()], vec![b, a]] {
        apply_problem_relation_output(&db, &task, &json!({"decisions":decisions}))
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_ci_problem_candidate WHERE candidate_ref=$1"
            )
            .bind(refs[1])
            .fetch_one(db.pool())
            .await
            .unwrap(),
            "unmerged"
        );
        let state = linggan_intelligence::comment_intelligence_problems::problem_automation_state(
            &db, domain,
        )
        .await
        .unwrap();
        assert_eq!(state["needsJudgment"].as_array().unwrap().len(), 1);
    }
}

async fn reserve_crash_window(
    db: &Database,
    task: Uuid,
    batch: Uuid,
    source: Uuid,
    config: Uuid,
) -> (Uuid, Uuid) {
    let invocation = Uuid::new_v4();
    let packet = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens) SELECT $1,m.connection_version_ref,m.model_ref,c.config_ref,'analyze',repeat('0',64),'running',18000,18000 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2")
        .bind(invocation).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state,purpose) VALUES($1,$2,$3,$3,repeat('0',64),$4,scope_001_now()-interval '1 second','running','problem_relation')")
        .bind(packet).bind(batch).bind(vec![source]).bind(invocation).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_ci_problem_task SET state='running',invocation_ref=$2,packet_ref=$3,lease_until=scope_001_now()-interval '1 second' WHERE task_ref=$1")
        .bind(task).bind(invocation).bind(packet).execute(db.pool()).await.unwrap();
    (invocation, packet)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and loopback synthetic SDK provider"]
async fn applied_receipt_recovers_crash_before_accounting_without_repeating_dispatch() {
    use linggan_intelligence::comment_intelligence_problems::{
        apply_problem_task_output, queue_problem_task, run_problem_relation_once,
    };
    let db = proof_database("ci_relation_receipt").await;
    let (mut server, url) = daily_fixture::fixture_server().await;
    let (config, _, _) = daily_fixture::configured(&db, &url, "synthetic-good", None).await;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
            .fetch_one(db.pool())
            .await
            .unwrap();
    research_fixture::detail(&db, "crash-window", "SYNTHETIC / NOT EVIDENCE").await;
    let a = daily_fixture::source(&db, "crash-window", "a", "每天提醒学习很费力").await;
    let b = daily_fixture::source(&db, "crash-window", "b", "每天陪着写作业很疲惫").await;
    let batch = daily_fixture::selected(&db, config, vec![a, b], 100000).await;
    assert!(daily_fixture::tick(&db).await);
    reconcile_problem_index(&db, 100).await.unwrap();
    let candidates: Vec<Uuid> = sqlx::query_scalar("SELECT c.candidate_ref FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) ORDER BY s.comment_external_id,c.ordinal").fetch_all(db.pool()).await.unwrap();
    let created = execute_action(
        &db,
        command(
            domain,
            "problem_create",
            0,
            json!({"name":"SYNTHETIC 已有问题","meaning":"SYNTHETIC 既有边界","sourceRefs":[a]}),
        ),
    )
    .await
    .unwrap();
    let target = Uuid::parse_str(created["problemRef"].as_str().unwrap()).unwrap();
    let task = queue_problem_task(
        &db,
        batch,
        domain,
        candidates[1],
        &[target],
        "synthetic-vector.v1",
    )
    .await
    .unwrap();
    let (invocation, packet) = reserve_crash_window(&db, task, batch, b, config).await;
    let output = json!({"decisions":[{"targetRef":target,"definitionRevision":1,"relation":"different","reason":"SYNTHETIC 不同边界，建立早期问题"}]});
    let receipt = apply_problem_task_output(&db, task, &output).await.unwrap();
    assert!(receipt["emergingProblemRef"].is_string());
    let persisted:Value=sqlx::query_scalar("SELECT applied_receipt FROM linggan_ci_problem_task WHERE task_ref=$1 AND applied_at IS NOT NULL AND state='running'").bind(task).fetch_one(db.pool()).await.unwrap();
    assert_eq!(persisted, receipt);
    // Fault point: relationships committed, but no finish_invocation or task finish ran.
    set_batch_enabled(&db, batch, false).await.unwrap();
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    for _ in 0..2 {
        assert!(
            !run_problem_relation_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
                .await
                .unwrap()
        );
    }
    assert_recovered_relation_receipt(&db, invocation, packet, task, &output, &receipt, calls)
        .await;
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn external_identity_collision_cannot_borrow_native_analysis_or_task_b_grant() {
    use linggan_intelligence::comment_intelligence_problems::{
        queue_problem_task, sync_problem_tasks,
    };
    let db = proof_database("ci_relation_domain").await;
    let (mut server, url) = daily_fixture::fixture_server().await;
    let (config, _, _) = daily_fixture::configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "p-boundary", "SYNTHETIC / NOT EVIDENCE").await;
    let source =
        daily_fixture::source(&db, "p-boundary", "collision", "执行时间不够也担心药物风险").await;
    let batch = daily_fixture::selected(&db, config, vec![source], 100000).await;
    assert!(daily_fixture::tick(&db).await);
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let external = Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    sqlx::query("INSERT INTO cross_industry_sample(sample_ref,domain_ref,platform,content_external_id,title) VALUES($1,$2,'xhs','collision','SYNTHETIC external')").bind(work).bind(external).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO cross_industry_comment(comment_ref,sample_ref,domain_ref,comment_external_id,body_text,body_state) VALUES($1,$2,$3,'collision','执行时间不够也担心药物风险','KNOWN')").bind(source).bind(work).bind(external).execute(db.pool()).await.unwrap();
    reconcile_problem_index(&db, 100).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_candidate WHERE domain_ref=$1"
        )
        .bind(external)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem_candidate")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let borrowed: Option<Uuid> = sqlx::query_scalar(
        "SELECT analysis_ref FROM linggan_ci_projection_cursor WHERE canonical_ref=$1 AND domain_ref=$2",
    )
    .bind(source).bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        borrowed, None,
        "external cursor must not inherit native model analysis"
    );
    // Simulate a stale pre-fix candidate referencing the otherwise fully eligible native grant.
    let stale = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ci_problem_candidate(candidate_ref,canonical_ref,domain_ref,analysis_ref,ordinal,name,meaning,definition_key,evidence,state) SELECT $1,$2,$3,analysis_ref,99,name,meaning,definition_key,evidence,'unmerged' FROM linggan_ci_problem_candidate LIMIT 1")
        .bind(stale).bind(source).bind(external).execute(db.pool()).await.unwrap();
    let queued = sync_problem_tasks(&db).await.unwrap();
    assert_eq!(queued, 1, "only native candidate is queued");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_task WHERE candidate_ref=$1"
        )
        .bind(stale)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert!(matches!(
        queue_problem_task(
            &db,
            batch,
            external,
            stale,
            &[Uuid::new_v4()],
            "synthetic-vector.v1"
        )
        .await,
        Err(ModelError::Disabled)
    ));
    server.kill().await.unwrap();
}

async fn configure_legacy_semantic_budget(db: &linggan_storage_postgres::Database, config: Uuid) {
    use linggan_intelligence::comment_daily::{AutoPolicy, DailySchedule, save_schedule};
    // Explicit legacy Task B remains subject to the same purpose and daily budget as atoms.
    save_schedule(
        db,
        &DailySchedule {
            expected_revision: 0,
            enabled: false,
            config_ref: config,
            source_limit: 200,
            token_limit: 100_000,
            auto_policy: AutoPolicy {
                semantic_token_limit: 80_000,
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
}

// Failure-only synthetic metadata; never include source text, prompt or provider credentials.
async fn task_b_fixture_state(db: &linggan_storage_postgres::Database) -> Value {
    sqlx::query_scalar(r#"SELECT jsonb_build_object(
      'tasks',(SELECT jsonb_agg(jsonb_build_object('state',state,'failureCode',failure_code,'receiptPresent',applied_receipt IS NOT NULL)) FROM linggan_ci_problem_task),
      'packets',(SELECT jsonb_agg(jsonb_build_object('purpose',purpose,'state',state)) FROM linggan_comment_daily_packet),
      'invocations',(SELECT jsonb_agg(jsonb_build_object('operation',operation,'state',state,'failureCode',failure_code,'reserved',reserved_tokens,'charged',charged_tokens,'purpose',result->>'budgetPurpose')) FROM linggan_model_invocation),
      'vectors',(SELECT count(*) FROM linggan_ci_definition_vector),
      'candidates',(SELECT jsonb_agg(jsonb_build_object('state',state,'assigned',problem_ref IS NOT NULL)) FROM linggan_ci_problem_candidate))"#)
        .fetch_one(db.pool()).await.unwrap()
}

async fn configure_qualified_legacy_embedding(db: &Database, config: Uuid) {
    let model: Uuid =
        sqlx::query_scalar("SELECT model_ref FROM linggan_model_config WHERE config_ref=$1")
            .bind(config)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let embedding = linggan_intelligence::embedding_settings::save(
        db,
        &linggan_intelligence::embedding_settings::SaveEmbedding {
            expected_revision: 0,
            model_ref: model,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let embedding_ref = Uuid::parse_str(embedding["configRef"].as_str().unwrap()).unwrap();
    let probe = linggan_intelligence::embedding_settings::probe(
        db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &linggan_intelligence::embedding_settings::ProbeEmbedding {
            invocation_ref: Uuid::new_v4(),
            config_ref: embedding_ref,
        },
    )
    .await
    .unwrap();
    assert_eq!(probe["embeddingQualified"], true);
    let settings = linggan_intelligence::embedding_settings::read(db)
        .await
        .unwrap();
    linggan_intelligence::embedding_settings::save(
        db,
        &linggan_intelligence::embedding_settings::SaveEmbedding {
            expected_revision: settings["revision"].as_i64().unwrap(),
            model_ref: model,
            enabled: true,
        },
    )
    .await
    .unwrap();
}

async fn assert_completed_legacy_relation(db: &Database, batch: Uuid, domain: Uuid) {
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem_member")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2,
        "SYNTHETIC legacy Task B state: {}",
        task_b_fixture_state(db).await
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_definition_vector")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    let purposes: Vec<String> = sqlx::query_scalar(
        "SELECT purpose FROM linggan_comment_daily_packet WHERE batch_ref=$1 ORDER BY created_at",
    )
    .bind(batch)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(purposes.contains(&"extraction".into()));
    assert!(purposes.contains(&"problem_embedding".into()));
    assert!(purposes.contains(&"problem_relation".into()));
    let charged:i64=sqlx::query_scalar("SELECT sum(v.charged_tokens)::bigint FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(charged, 1830);
    let invocation:Uuid=sqlx::query_scalar("SELECT invocation_ref FROM linggan_comment_daily_packet WHERE batch_ref=$1 AND purpose='problem_relation'").bind(batch).fetch_one(db.pool()).await.unwrap();
    let trace =
        linggan_intelligence::comment_runtime::request_detail(db, batch, invocation, domain)
            .await
            .unwrap();
    assert_eq!(trace["availability"], "AVAILABLE");
    assert_eq!(trace["purpose"], "problem_relation");
    assert!(trace["input"]["expression"]["comment"].is_string());
    assert!(trace["output"]["json"]["decisions"].is_array());
    let state =
        linggan_intelligence::comment_intelligence_problems::problem_automation_state(db, domain)
            .await
            .unwrap();
    assert_eq!(state["status"], "ready");
    let counts: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(
        !linggan_intelligence::comment_intelligence_problems::run_problem_relation_once(
            db,
            &SyntheticModelSecrets,
            &PiAdapter::configured()
        )
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        counts
    );
}

async fn assert_recovered_relation_receipt(
    db: &Database,
    invocation: Uuid,
    packet: Uuid,
    task: Uuid,
    output: &Value,
    receipt: &Value,
    calls: i64,
) {
    use linggan_intelligence::comment_intelligence_problems::apply_problem_task_output;
    let recovered=sqlx::query("SELECT state,failure_code,result,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1").bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(recovered.get::<String, _>("state"), "succeeded");
    assert_eq!(
        recovered.get::<String, _>("failure_code"),
        "usage_review_required"
    );
    assert_eq!(
        recovered.get::<i64, _>("charged_tokens"),
        18000,
        "unknown usage retains its reservation"
    );
    assert_eq!(
        recovered.get::<Value, _>("result")["usageReviewRequired"],
        true
    );
    assert_eq!(&recovered.get::<Value, _>("result")["receipt"], receipt);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_daily_packet WHERE packet_ref=$1"
        )
        .bind(packet)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT failure_code FROM linggan_ci_problem_task WHERE task_ref=$1 AND state='succeeded'").bind(task).fetch_one(db.pool()).await.unwrap(),"usage_review_required");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        calls
    );
    assert_eq!(
        apply_problem_task_output(db, task, output).await.unwrap(),
        *receipt,
        "receipt replay is idempotent"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ci_problem_boundary_decision WHERE origin='task_b'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ci_problem_member")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
}
