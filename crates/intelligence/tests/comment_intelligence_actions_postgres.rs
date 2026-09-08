#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
#[allow(dead_code)]
mod research_fixture;

use fixture::proof_database;
use linggan_intelligence::comment_intelligence_actions::{execute_action, source_research};
use linggan_intelligence::comment_intelligence_problems::{
    problem_details, reconcile_problem_index,
};
use linggan_storage_postgres::Database;
use research_fixture::{comment, detail};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn domain(db: &Database) -> Uuid {
    sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE is_own_domain")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

fn action(
    domain: Uuid,
    kind: &str,
    source: Option<Uuid>,
    problem: Option<Uuid>,
    revision: i64,
    payload: Value,
) -> Value {
    json!({"commandRef":Uuid::new_v4(),"domain":domain,"kind":kind,"sourceRef":source,"problemRef":problem,"expectedRevision":revision,"payload":payload,"reason":"SYNTHETIC / NOT EVIDENCE · 人工校正证明"})
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn source_commands_are_idempotent_versioned_undoable_and_immutable() {
    let db = proof_database("ci_source_commands").await;
    let domain = domain(&db).await;
    detail(&db, "ci-a", "合成 ADHD 作品").await;
    let source = comment(
        &db,
        "ci-a",
        "c1",
        "A娃执行功能启动困难",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let request = action(
        domain,
        "bookmark",
        Some(source),
        None,
        0,
        json!({"enabled":true,"note":"研究备注"}),
    );
    let first = execute_action(&db, request.clone()).await.unwrap();
    assert_eq!(first["revision"], 1);
    assert_eq!(execute_action(&db, request.clone()).await.unwrap(), first);
    let mut collision = request.clone();
    collision["payload"]["note"] = json!("不同备注");
    assert!(
        format!("{:?}", execute_action(&db, collision).await.unwrap_err())
            .contains("CI_IDEMPOTENCY_CONFLICT")
    );
    let stale = action(
        domain,
        "correct",
        Some(source),
        None,
        0,
        json!({"labels":["need"],"problemRefs":[],"needsContext":false}),
    );
    assert!(
        format!("{:?}", execute_action(&db, stale).await.unwrap_err())
            .contains("CI_REVISION_CONFLICT")
    );
    let corrected = execute_action(
        &db,
        action(
            domain,
            "correct",
            Some(source),
            None,
            1,
            json!({"labels":["need","story"],"problemRefs":[],"needsContext":true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(corrected["revision"], 2);
    assert_eq!(corrected["locked"], true);
    assert_eq!(corrected["labelsLocked"], true);
    let undone = execute_action(
        &db,
        action(domain, "undo", Some(source), None, 2, json!({"revision":2})),
    )
    .await
    .unwrap();
    assert_eq!(undone["revision"], 3);
    assert_eq!(undone["bookmarked"], true);
    assert_eq!(undone["locked"], false);
    assert_eq!(undone["labelsLocked"], false);
    assert_eq!(
        source_research(&db, domain, source).await.unwrap()["history"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        sqlx::query("DELETE FROM linggan_ci_source_revision WHERE canonical_ref=$1")
            .bind(source)
            .execute(db.pool())
            .await
            .is_err()
    );
    let raw: String =
        sqlx::query_scalar("SELECT body_text FROM linggan_material_comment WHERE material_ref=$1")
            .bind(source)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(raw, "A娃执行功能启动困难");
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn problem_identity_survives_rename_split_merge_and_restricts_cross_domain_actions() {
    let db = proof_database("ci_problem_lineage").await;
    let domain = domain(&db).await;
    detail(&db, "ci-b", "合成作品").await;
    let a = comment(&db, "ci-b", "c1", "方法坚持困难", "2026-09-07T10:00:00Z").await;
    let b = comment(&db, "ci-b", "c2", "作业耗时太久", "2026-09-07T10:00:01Z").await;
    let create = action(
        domain,
        "problem_create",
        None,
        None,
        0,
        json!({"name":"执行困难","meaning":"执行过程的困难","sourceRefs":[a,b]}),
    );
    let problem = Uuid::parse_str(create["commandRef"].as_str().unwrap()).unwrap();
    execute_action(&db, create).await.unwrap();
    let renamed = execute_action(
        &db,
        action(
            domain,
            "problem_rename",
            None,
            Some(problem),
            1,
            json!({"name":"家庭执行困难","meaning":"家庭场景内的执行困难"}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(renamed["problemRef"], problem.to_string());
    assert_eq!(renamed["revision"], 2);
    assert_eq!(
        source_research(&db, domain, a).await.unwrap()["labelsLocked"],
        false
    );
    let split = action(
        domain,
        "problem_split",
        None,
        Some(problem),
        2,
        json!({"name":"作业耗时","meaning":"作业执行的时间成本","sourceRefs":[b]}),
    );
    let child = Uuid::parse_str(split["commandRef"].as_str().unwrap()).unwrap();
    execute_action(&db, split).await.unwrap();
    assert_eq!(
        problem_details(&db, domain, child).await.unwrap()["members"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        problem_details(&db, domain, problem).await.unwrap()["members"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    execute_action(
        &db,
        action(
            domain,
            "problem_merge",
            None,
            Some(child),
            1,
            json!({"targetRef":problem}),
        ),
    )
    .await
    .unwrap();
    let alias = problem_details(&db, domain, child).await.unwrap();
    assert_eq!(alias["redirected"], true);
    assert_eq!(alias["resolvedProblemRef"], problem.to_string());
    assert_eq!(alias["members"].as_array().unwrap().len(), 2);
    let other: Uuid = sqlx::query_scalar(
        "SELECT domain_ref FROM observation_domain WHERE NOT is_own_domain LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        execute_action(
            &db,
            action(other, "bookmark", Some(a), None, 1, json!({"enabled":true}))
        )
        .await
        .is_err()
    );
    assert!(
        execute_action(
            &db,
            action(
                other,
                "problem_bookmark",
                None,
                Some(problem),
                4,
                json!({"enabled":true})
            )
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn canonical_feedback_survives_reobservation_and_restriction_blocks_replays() {
    let db = proof_database("ci_canonical_feedback").await;
    let domain = domain(&db).await;
    detail(&db, "ci-c", "合成作品").await;
    let first = comment(
        &db,
        "ci-c",
        "c1",
        "家庭干预需要坚持",
        "2026-09-06T10:00:00Z",
    )
    .await;
    let request = action(
        domain,
        "bookmark",
        Some(first),
        None,
        0,
        json!({"enabled":true,"note":"先前收藏"}),
    );
    execute_action(&db, request.clone()).await.unwrap();
    let latest = comment(
        &db,
        "ci-c",
        "c1",
        "家庭干预需要坚持",
        "2026-09-07T10:00:00Z",
    )
    .await;
    assert_ne!(first, latest);
    let state = source_research(&db, domain, latest).await.unwrap();
    assert_eq!(state["canonicalRef"], first.to_string());
    assert_eq!(state["bookmarked"], true);
    sqlx::query("INSERT INTO linggan_comment_research_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(first).execute(db.pool()).await.unwrap();
    assert!(source_research(&db, domain, latest).await.is_err());
    assert!(execute_action(&db, request).await.is_err());
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn term_projection_is_incremental_document_frequency_and_versioned_hiding() {
    let db = proof_database("ci_term_projection").await;
    let domain = domain(&db).await;
    detail(&db, "ci-d", "仅标题有玻璃心").await;
    let source = comment(
        &db,
        "ci-d",
        "c1",
        "A娃执行功能 A娃执行功能 天线宝宝",
        "2026-09-07T10:00:00Z",
    )
    .await;
    assert_eq!(
        reconcile_problem_index(&db, 100).await.unwrap()["projected"],
        1
    );
    assert_eq!(
        reconcile_problem_index(&db, 100).await.unwrap()["projected"],
        0
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_term_index WHERE canonical_ref=$1 AND term='A娃'",
    )
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let title_only: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ci_term_index WHERE term='玻璃心'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(title_only, 0);
    let hide = execute_action(
        &db,
        action(
            domain,
            "term_hide",
            None,
            None,
            0,
            json!({"term":"A娃","hidden":true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(hide["revision"], 1);
    assert!(
        execute_action(
            &db,
            action(
                domain,
                "term_hide",
                None,
                None,
                0,
                json!({"term":"A娃","hidden":false})
            )
        )
        .await
        .is_err()
    );
    execute_action(
        &db,
        action(
            domain,
            "term_hide",
            None,
            None,
            1,
            json!({"term":"A娃","hidden":false}),
        ),
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn valid_exact_definitions_form_stable_groups_but_singletons_and_human_locks_are_retained() {
    let db = proof_database("ci_problem_candidates").await;
    let domain = domain(&db).await;
    detail(&db, "ci-e", "合成作品").await;
    let mut refs = Vec::new();
    for i in 0..3 {
        let source = comment(
            &db,
            "ci-e",
            &format!("c{i}"),
            "家长没有执行时间",
            "2026-09-07T10:00:00Z",
        )
        .await;
        refs.push(source);
        let hash = linggan_intelligence::comment_research::comment_source_hash("家长没有执行时间");
        let result = json!({"sourceRef":source,"sourceSha256":hash,"contextRefs":{"researchSourceRefs":[source]},"semantic":{"outcome":"interpretable","problems":[{"name":"执行时间不足","meaning":"家长无法安排执行所需时间","evidence":[{"sourceRef":source,"startChar":0,"endChar":8}]}]}});
        sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'synthetic-ci.v3','synthetic-model.v1','succeeded',$3)")
            .bind(Uuid::new_v4()).bind(source).bind(result).execute(db.pool()).await.unwrap();
        reconcile_problem_index(&db, 100).await.unwrap();
        let groups: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(groups, if i == 2 { 1 } else { 0 });
    }
    let problem: Uuid = sqlx::query_scalar("SELECT problem_ref FROM linggan_ci_problem")
        .fetch_one(db.pool())
        .await
        .unwrap();
    execute_action(
        &db,
        action(
            domain,
            "correct",
            Some(refs[0]),
            None,
            0,
            json!({"labels":["story"],"problemRefs":[],"needsContext":false}),
        ),
    )
    .await
    .unwrap();
    reconcile_problem_index(&db, 100).await.unwrap();
    let membership: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_problem_member WHERE problem_ref=$1 AND canonical_ref=$2",
    )
    .bind(problem)
    .bind(refs[0])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(membership, 0);
    assert_eq!(
        source_research(&db, domain, refs[0]).await.unwrap()["labels"],
        json!(["story"])
    );
    execute_action(
        &db,
        action(
            domain,
            "undo",
            Some(refs[0]),
            None,
            1,
            json!({"revision":1}),
        ),
    )
    .await
    .unwrap();
    let origin: String = sqlx::query_scalar(
        "SELECT origin FROM linggan_ci_problem_member WHERE problem_ref=$1 AND canonical_ref=$2",
    )
    .bind(problem)
    .bind(refs[0])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(origin, "exact_definition");
    execute_action(
        &db,
        action(
            domain,
            "problem_rename",
            None,
            Some(problem),
            1,
            json!({"name":"家庭时间安排","meaning":"家长无法安排执行所需时间"}),
        ),
    )
    .await
    .unwrap();
    for i in 3..6 {
        let source = comment(
            &db,
            "ci-e",
            &format!("c{i}"),
            "家长没有执行时间",
            "2026-09-07T10:00:00Z",
        )
        .await;
        let result = json!({"sourceRef":source,"sourceSha256":linggan_intelligence::comment_research::comment_source_hash("家长没有执行时间"),"contextRefs":{"researchSourceRefs":[source]},"semantic":{"outcome":"interpretable","problems":[{"name":"执行时间不足","meaning":"家长无法安排执行所需时间","evidence":[{"sourceRef":source,"startChar":0,"endChar":8}]}]}});
        sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'synthetic-ci.v3','synthetic-model.v1','succeeded',$3)")
            .bind(Uuid::new_v4()).bind(source).bind(result).execute(db.pool()).await.unwrap();
    }
    reconcile_problem_index(&db, 100).await.unwrap();
    let groups: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        groups, 1,
        "renamed historical identity must not reappear as a new problem"
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn restricted_context_withdraws_problem_evidence_without_hiding_raw_comment() {
    let db = proof_database("ci_problem_context_restriction").await;
    let domain = domain(&db).await;
    detail(&db, "ci-f", "合成作品").await;
    let context = comment(
        &db,
        "ci-f",
        "context",
        "父评论合成上下文",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let mut sources = Vec::new();
    for i in 0..3 {
        let source = comment(
            &db,
            "ci-f",
            &format!("target-{i}"),
            "执行时间不足",
            "2026-09-07T10:00:00Z",
        )
        .await;
        sources.push(source);
        let result = json!({"sourceRef":source,"sourceSha256":linggan_intelligence::comment_research::comment_source_hash("执行时间不足"),"contextRefs":{"researchSourceRefs":[source,context]},"semantic":{"outcome":"interpretable","problems":[{"name":"执行时间不足","meaning":"任务执行时间不够","evidence":[{"sourceRef":source,"startChar":0,"endChar":6}]}]}});
        sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'synthetic-ci.v3','synthetic-model.v1','succeeded',$3)")
            .bind(Uuid::new_v4()).bind(source).bind(result).execute(db.pool()).await.unwrap();
    }
    reconcile_problem_index(&db, 100).await.unwrap();
    let problem: Uuid = sqlx::query_scalar("SELECT problem_ref FROM linggan_ci_problem")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        problem_details(&db, domain, problem).await.unwrap()["members"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    sqlx::query("INSERT INTO linggan_comment_research_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(context).execute(db.pool()).await.unwrap();
    assert!(problem_details(&db, domain, problem).await.is_err());
    let visible: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ci_source WHERE source_ref=ANY($1)")
            .bind(sources)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        visible, 3,
        "only derived claims depending on the withdrawn context are unavailable"
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn model_equivalence_uses_frozen_definition_and_never_overwrites_human_locks() {
    let db = proof_database("ci_model_equivalence").await;
    let domain = domain(&db).await;
    detail(&db, "ci-g", "合成作品").await;
    let seed = comment(
        &db,
        "ci-g",
        "seed",
        "家长无法安排训练时间",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let create = action(
        domain,
        "problem_create",
        None,
        None,
        0,
        json!({"name":"家庭时间成本","meaning":"家长无法安排执行时间","sourceRefs":[seed]}),
    );
    let problem: Uuid = serde_json::from_value(create["commandRef"].clone()).unwrap();
    execute_action(&db, create).await.unwrap();
    let row =
        sqlx::query("SELECT name,meaning,definition FROM linggan_ci_problem WHERE problem_ref=$1")
            .bind(problem)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let fingerprint=linggan_intelligence::comment_research::comment_source_hash(&json!({"name":row.get::<String,_>("name"),"meaning":row.get::<String,_>("meaning"),"definition":row.get::<Value,_>("definition")}).to_string());
    let mut refs = Vec::new();
    for i in 0..4 {
        let source = comment(
            &db,
            "ci-g",
            &format!("candidate-{i}"),
            "每天没有精力安排训练",
            "2026-09-07T10:00:00Z",
        )
        .await;
        refs.push(source);
        if i == 2 {
            execute_action(
                &db,
                action(
                    domain,
                    "correct",
                    Some(source),
                    None,
                    0,
                    json!({"labels":["story"],"problemRefs":[],"needsContext":false}),
                ),
            )
            .await
            .unwrap();
        }
        let result = json!({"sourceRef":source,"sourceSha256":linggan_intelligence::comment_research::comment_source_hash("每天没有精力安排训练"),
            "contextRefs":{"researchSourceRefs":[source,seed]},"candidateSnapshot":[{"problemRef":problem,"revision":1,"definitionFingerprint":fingerprint,"sourceRefs":[seed]}],
            "semantic":{"outcome":"interpretable","problems":[{"name":"每日执行精力缺口","meaning":"家长日常没有空闲时间安排训练","evidence":[{"sourceRef":source,"startChar":0,"endChar":10}],
                "candidateRef":problem,"candidateRevision":1,"equivalenceReason":"合成验证：两种表达都明确指向家长安排训练的时间成本","boundaryMatch":true,"retrievalMethod":"lexical_terms.v1","serverValidatedCandidate":i!=3}]}});
        sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'synthetic-ci.v3','synthetic-model.v1','succeeded',$3)")
            .bind(Uuid::new_v4()).bind(source).bind(result).execute(db.pool()).await.unwrap();
    }
    reconcile_problem_index(&db, 100).await.unwrap();
    let origins:Vec<String>=sqlx::query_scalar("SELECT origin FROM linggan_ci_problem_member WHERE problem_ref=$1 AND canonical_ref=ANY($2) ORDER BY canonical_ref")
        .bind(problem).bind(&refs).fetch_all(db.pool()).await.unwrap();
    assert_eq!(origins, vec!["model_equivalence", "model_equivalence"]);
    let row = sqlx::query(
        "SELECT revision,definition_revision FROM linggan_ci_problem WHERE problem_ref=$1",
    )
    .bind(problem)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>("revision"), 3);
    assert_eq!(row.get::<i64, _>("definition_revision"), 1);
    let unmerged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ci_problem_candidate WHERE state='unmerged'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        unmerged, 2,
        "locked and unverified proposals remain researchable candidates"
    );
    assert_eq!(
        source_research(&db, domain, refs[2]).await.unwrap()["labels"],
        json!(["story"])
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn edited_source_deactivates_manual_decision_until_explicit_current_text_correction() {
    let db = proof_database("ci_manual_source_change").await;
    let domain = domain(&db).await;
    detail(&db, "ci-h", "合成作品").await;
    let source = comment(&db, "ci-h", "c1", "需要帮助", "2026-09-06T10:00:00Z").await;
    let original_hash = linggan_intelligence::comment_research::comment_source_hash("需要帮助");
    let mut correction = action(
        domain,
        "correct",
        Some(source),
        None,
        0,
        json!({"labels":["need"],"problemRefs":[],"needsContext":false}),
    );
    correction["expectedSourceSha256"] = json!(original_hash);
    execute_action(&db, correction).await.unwrap();
    let latest = comment(
        &db,
        "ci-h",
        "c1",
        "分享已经解决的经历",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let changed = source_research(&db, domain, latest).await.unwrap();
    assert_eq!(changed["sourceChanged"], true);
    assert_eq!(changed["active"], false);
    assert_eq!(changed["locked"], true);
    let bookmark = execute_action(
        &db,
        action(
            domain,
            "bookmark",
            Some(latest),
            None,
            1,
            json!({"enabled":true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        bookmark["sourceChanged"], true,
        "bookmark does not endorse old semantics on edited text"
    );
    let mut stale = action(
        domain,
        "correct",
        Some(latest),
        None,
        2,
        json!({"labels":["need"],"problemRefs":[],"needsContext":false}),
    );
    stale["expectedSourceSha256"] = json!(original_hash);
    assert!(execute_action(&db, stale).await.is_err());
    let mut current = action(
        domain,
        "correct",
        Some(latest),
        None,
        2,
        json!({"labels":["story","solution"],"problemRefs":[],"needsContext":false}),
    );
    current["expectedSourceSha256"] = changed["sourceSha256"].clone();
    let fixed = execute_action(&db, current).await.unwrap();
    assert_eq!(fixed["sourceChanged"], false);
    assert_eq!(fixed["active"], true);
    assert_eq!(fixed["decisionSourceSha256"], changed["sourceSha256"]);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn legacy_assets_seed_bookmarks_across_observations_without_preventing_unstar() {
    let db = proof_database("ci_legacy_bookmark").await;
    let domain = domain(&db).await;
    detail(&db, "ci-i", "合成作品").await;
    let first = comment(
        &db,
        "ci-i",
        "c1",
        "值得保留的合成经历",
        "2026-09-05T10:00:00Z",
    )
    .await;
    let middle = comment(
        &db,
        "ci-i",
        "c1",
        "值得保留的合成经历",
        "2026-09-06T10:00:00Z",
    )
    .await;
    let asset = research_fixture::asset(middle, "值得保留的合成经历");
    linggan_intelligence::comment_research::save_comment_asset(&db, &asset)
        .await
        .unwrap();
    let latest = comment(
        &db,
        "ci-i",
        "c1",
        "值得保留的合成经历",
        "2026-09-07T10:00:00Z",
    )
    .await;
    let inherited = source_research(&db, domain, middle).await.unwrap();
    assert_eq!(inherited["canonicalRef"], first.to_string());
    assert_eq!(inherited["bookmarked"], true);
    assert_eq!(inherited["bookmarkNote"], "合成研究理由");
    let corrected = execute_action(
        &db,
        action(
            domain,
            "correct",
            Some(latest),
            None,
            0,
            json!({"labels":["story"],"problemRefs":[],"needsContext":false}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        corrected["bookmarked"], true,
        "first correction must preserve legacy star"
    );
    let unstar = execute_action(
        &db,
        action(
            domain,
            "bookmark",
            Some(middle),
            None,
            1,
            json!({"enabled":false}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(unstar["bookmarked"], false);
    assert_eq!(
        source_research(&db, domain, latest).await.unwrap()["bookmarked"],
        false
    );
    let original: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_asset_current WHERE asset_ref=$1 AND NOT withdrawn",
    )
    .bind(asset.asset_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        original, 1,
        "legacy clip and its history remain retained after current bookmark toggle"
    );
}
