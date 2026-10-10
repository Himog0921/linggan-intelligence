//! Disposable PostgreSQL + synthetic adapter proof. No model or platform calls.
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::topic_map::{self, TopicMapCommand, TopicMapQuery, TopicMapSnapshot};
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    topic_map_research::{ResearchCommand, apply_research_command, read_research_progress},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn exact_input_pipeline_idempotency_budget_and_stop() {
    let (db, config, adapter) = setup("topic_map_research_pipeline").await;
    let w = work(
        &db,
        "research-one",
        "SYNTHETIC 家庭实践：每次只做一步，并记录反馈。",
    )
    .await;
    let command = configure(config, 100000, false);
    let a = apply_research_command(&db, &command).await.unwrap();
    assert_eq!(a, apply_research_command(&db, &command).await.unwrap());
    read_research_progress(&db, D).await.unwrap();
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let c = start(vec![w]);
    let r = apply_research_command(&db, &c).await.unwrap();
    assert_eq!(r, apply_research_command(&db, &c).await.unwrap());
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT phase FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "resolve"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "extraction alone is not accepted membership"
    );
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_work_annotation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE operation='analyze'").fetch_one(db.pool()).await.unwrap(),798);
    assert_eq!(
        apply_research_command(&db, &start(vec![w])).await.unwrap()["state"],
        "no_new_input"
    );
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    // Different accepted observation time and metrics must advance the physical
    // canonical head without invalidating semantically unchanged source windows.
    let before: Uuid = sqlx::query_scalar("SELECT (input_refs->'fragments'->0->>'sourceRef')::uuid FROM linggan_topic_map_research_task LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    work_at(
        &db,
        "research-one",
        "",
        "SYNTHETIC 家庭实践：每次只做一步，并记录反馈。",
        99,
        "2026-09-29T10:00:00Z",
    )
    .await;
    let latest: Uuid = sqlx::query_scalar("SELECT material_ref FROM linggan_material_content_detail WHERE content_public_ref=$1 ORDER BY observed_at::timestamptz DESC,created_at DESC,material_ref DESC LIMIT 1")
        .bind(w).fetch_one(db.pool()).await.unwrap();
    assert_ne!(
        before, latest,
        "the canonical source observation must actually change"
    );
    assert_eq!(
        apply_research_command(&db, &start(vec![w])).await.unwrap()["state"],
        "no_new_input"
    );
    let w2 = work(&db, "research-two", "SYNTHETIC 下班后安排一次练习。").await;
    apply_research_command(&db, &configure(config, 1024, false))
        .await
        .unwrap();
    let second = apply_research_command(&db, &start(vec![w2])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(second["runRef"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "daily_budget_paused"
    );
    let stop = ResearchCommand::Stop {
        request_ref: Uuid::new_v4(),
        domain_ref: D,
        run_ref: Some(second["runRef"].as_str().unwrap().parse().unwrap()),
    };
    apply_research_command(&db, &stop).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_run SET updated_at=scope_001_now()-interval '2 days'WHERE run_ref=$1").bind(second["runRef"].as_str().unwrap().parse::<Uuid>().unwrap()).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    assert!(
        !run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2
    );
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn rejects_foreign_citation_and_preserves_daily_pause_without_call() {
    let (db, config, adapter) = setup("topic_map_research_attack").await;
    let w = work(&db, "research-attack", "SYNTHETIC ATTACK 仅测试越界引用。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        399
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn atomic_daily_reservation_and_unknown_dispatch_do_not_resend() {
    let (db, config, adapter) = setup("topic_map_research_reservation").await;
    let a = work(&db, "slow-one", "SYNTHETIC SLOW 练习过程合成输入。").await;
    let b = work(&db, "later-two", "SYNTHETIC 下一个实践输入。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![a])).await.unwrap();
    let clone_db = db.clone();
    let clone_adapter = adapter.clone();
    let first = tokio::spawn(async move {
        run_once(&clone_db, &SyntheticModelSecrets, &clone_adapter)
            .await
            .unwrap()
    });
    let mut reserved = 0;
    for _ in 0..100 {
        let running:Option<i64>=sqlx::query_scalar("SELECT i.reserved_tokens FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)WHERE q.dispatch_started_at IS NOT NULL AND i.state='running'LIMIT 1").fetch_optional(db.pool()).await.unwrap();
        if let Some(n) = running {
            reserved = n;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(reserved > 0);
    apply_research_command(&db, &configure(config, reserved + 1, false))
        .await
        .unwrap();
    let r = apply_research_command(&db, &start(vec![b])).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(r["runRef"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "daily_budget_paused"
    );
    assert!(first.await.unwrap());
    // A dispatched request whose process died is terminal unknown, retaining reserved charge.
    let c = work(&db, "unknown-three", "SYNTHETIC 未知发送状态输入。").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let u = apply_research_command(&db, &start(vec![c])).await.unwrap();
    let run = u["runRef"].as_str().unwrap().parse::<Uuid>().unwrap();
    let task: Uuid =
        sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens)SELECT $1,m.connection_version_ref,c.model_ref,c.config_ref,'analyze',$3,'running',2048,2048 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE config_ref=$2").bind(invocation).bind(config).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,dispatch_started_at,deadline_at)VALUES($1,$2,$3,1,$4,'{}',(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()-interval '2 minutes',scope_001_now()-interval '1 minute')").bind(invocation).bind(task).bind(run).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=1,phase_attempt_count=1,lease_token=$2 WHERE task_ref=$1").bind(task).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "unknown_dispatch"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(invocation)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        2048
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_topic_map_research_request WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

struct ComparisonProof {
    works: [Uuid; 2],
    comment: Uuid,
    author_body: &'static str,
    comment_body: &'static str,
    topic: Uuid,
    definition: Uuid,
}

async fn comparison_proof(db: &Database) -> ComparisonProof {
    let first_body = "SYNTHETIC COMPARE 家庭练习每次只做一步。";
    let second_body = "SYNTHETIC COMPARE 外部作者讨论练习先观察反馈。";
    let first = work(db, "comparison-first", first_body).await;
    let second = work(db, "comparison-second", second_body).await;
    // Always inspect the non-primary participant first. The author is the lower
    // UUID and the commenter belongs to the other work, so both sides are real.
    let (a, b, comment_work, author_body) = if first > second {
        (first, second, "comparison-first", second_body)
    } else {
        (second, first, "comparison-second", first_body)
    };
    assert!(
        a > b,
        "this regression must not depend on a random primary UUID"
    );
    let raw_comment = "孩子每天练习都要催，不催就不开始。";
    // Research and saved citations use the qualified, cleaned comment text.
    // Keep this oracle handwritten and distinct from the submitted raw text.
    let comment_body = "孩子每天练习都要催,不催就不开始。";
    let comment = research_fixture::comment_with_author(
        db,
        comment_work,
        "reader-q",
        raw_comment,
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let topic=linggan_intelligence::import_topic_workspace(db,&serde_json::from_value(json!({"idempotencyKey":"synthetic-comparison-topic","domainKey":"adhd-family","canonicalKey":"synthetic-comparison","displayName":"合成实践主题","definitionText":"合成两侧实践边界","expectedVersion":null,"adjudicationNote":"合成测试","sourceBoundary":"SYNTHETIC","members":[{"workPublicRef":a,"role":"support","rationale":"合成支持"},{"workPublicRef":b,"role":"boundary","rationale":"合成边界"}]})).unwrap()).await.unwrap();
    topic_map::save_topic_map_command(
        db,
        &TopicMapCommand::BindTopic {
            idempotency_key: "synthetic-comparison-bind".into(),
            domain_ref: D,
            topic_ref: topic.topic_ref,
            parent_topic_ref: None,
            expected_version: None,
        },
    )
    .await
    .unwrap();
    ComparisonProof {
        works: [a, b],
        comment,
        author_body,
        comment_body,
        topic: topic.topic_ref,
        definition: topic.definition_ref,
    }
}

fn item_work_refs(item: &Value, proof: &ComparisonProof) -> Vec<Uuid> {
    let refs: Vec<Uuid> = serde_json::from_value(item["evidenceWorkRefs"].clone()).unwrap();
    assert_eq!(refs.len(), 2);
    assert!(proof.works.iter().all(|work| refs.contains(work)));
    refs
}

fn both_comparison_items(
    snapshot: &TopicMapSnapshot,
    proof: &ComparisonProof,
    result: Uuid,
) -> (Value, Value) {
    let mut items = Vec::new();
    for work in proof.works {
        let research = snapshot
            .works
            .iter()
            .find(|w| w.work_ref == work)
            .unwrap()
            .research
            .as_ref()
            .expect("each selected work can read the shared comparison");
        for key in ["angles", "productOpportunities"] {
            let matching: Vec<_> = research["output"][key]
                .as_array()
                .unwrap()
                .iter()
                .filter(|item| item["researchResultRef"] == json!(result))
                .collect();
            assert_eq!(
                matching.len(),
                1,
                "one original item is projected once for each participant"
            );
            let item = matching[0];
            item_work_refs(item, proof);
            assert_eq!(item["comparisonScope"]["resultRef"], json!(result));
            let selected: Vec<Uuid> =
                serde_json::from_value(item["comparisonScope"]["selectedWorkRefs"].clone())
                    .unwrap();
            assert_eq!(selected.len(), 2);
            assert!(proof.works.iter().all(|work| selected.contains(work)));
            let cited: Vec<_> = item["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .map(|citation| {
                    research["fragments"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|f| f["fragmentId"] == citation["fragmentId"])
                        .expect("each view restores the exact original cited fragment")
                })
                .collect();
            assert!(cited.iter().any(|f| {
                f["field"] == "body"
                    && f["workRef"] == json!(proof.works[1])
                    && f["fragmentId"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("{}.", proof.works[1]))
                    && f["text"] == proof.author_body
            }));
            assert!(cited.iter().any(|f| {
                f["field"].as_str().unwrap().ends_with("comment")
                    && f["workRef"] == json!(proof.works[0])
                    && f["fragmentId"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("{}.comment.", proof.works[0]))
                    && f["text"] == proof.comment_body
            }));
            assert!(
                research["output"]["responseMatches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|response| response["status"] == "partial"
                        && response["evidence"] == item["evidence"])
            );
            items.push(item.clone());
        }
    }
    assert_eq!(
        items[0], items[2],
        "both views expose the same original angle and scope"
    );
    assert_eq!(
        items[1], items[3],
        "both views expose the same original opportunity and scope"
    );
    // Save the angle from the non-primary work and the opportunity from the other
    // work. Each uses its own persisted result/index and selected evidence refs.
    (items.remove(0), items.remove(2))
}

fn comparison_save_command(
    proof: &ComparisonProof,
    item: &Value,
    product: bool,
) -> (TopicMapCommand, usize) {
    let result = item["researchResultRef"].as_str().unwrap().parse().unwrap();
    let index = item[if product {
        "researchOpportunityIndex"
    } else {
        "researchAngleIndex"
    }]
    .as_u64()
    .unwrap() as usize;
    (
        TopicMapCommand::SaveAlternative {
            idempotency_key: if product {
                "synthetic-product-research"
            } else {
                "synthetic-comment-angle"
            }
            .into(),
            domain_ref: D,
            topic_ref: proof.topic,
            definition_ref: proof.definition,
            title: if product {
                "修改后的合成产品研究说明"
            } else {
                "怎样开始练习"
            }
            .into(),
            angle: "合成评论练习角度与待验证说明".into(),
            rationale: "合成引用测试，不是市场结论".into(),
            evidence_work_refs: item_work_refs(item, proof),
            method_version: item["researchMethodVersion"].as_str().unwrap().into(),
            research_result_ref: Some(result),
            research_angle_index: (!product).then_some(index),
            research_opportunity_index: product.then_some(index),
        },
        index,
    )
}

async fn save_comparison_items(
    db: &Database,
    proof: &ComparisonProof,
    angle: &Value,
    opportunity: &Value,
) -> Vec<Uuid> {
    let mut saved = Vec::new();
    let mut product_command = None;
    for (item, product) in [(angle, false), (opportunity, true)] {
        let (command, index) = comparison_save_command(proof, item, product);
        let receipt = topic_map::save_topic_map_command(db, &command)
            .await
            .unwrap();
        let saved_ref = receipt.subject_ref.unwrap();
        let original = topic_map::read_saved_alternative(db, D, saved_ref)
            .await
            .unwrap();
        assert_eq!(original.source_state, "available");
        assert_eq!(
            original.research_manifest["resultRef"],
            item["researchResultRef"]
        );
        assert_eq!(
            original.research_manifest[if product {
                "opportunityIndex"
            } else {
                "angleIndex"
            }],
            json!(index)
        );
        assert!(
            original
                .fragments
                .iter()
                .any(|f| f.work_ref == proof.works[0]
                    && f.field.ends_with("comment")
                    && !f.cited_ranges.is_empty()
                    && f.text == proof.comment_body)
        );
        assert!(
            original
                .fragments
                .iter()
                .any(|f| f.work_ref == proof.works[1]
                    && f.field == "body"
                    && !f.cited_ranges.is_empty()
                    && f.text == proof.author_body)
        );
        if product {
            assert_eq!(original.kind, "product_research");
            assert_eq!(original.title.as_deref(), Some("修改后的合成产品研究说明"));
            product_command = Some(command);
        }
        saved.push(saved_ref);
    }
    let mut invalid = product_command.unwrap();
    if let TopicMapCommand::SaveAlternative {
        idempotency_key,
        research_angle_index,
        ..
    } = &mut invalid
    {
        *idempotency_key = "synthetic-product-invalid-both".into();
        *research_angle_index = Some(angle["researchAngleIndex"].as_u64().unwrap() as usize);
    }
    assert!(matches!(
        topic_map::save_topic_map_command(db, &invalid).await,
        Err(topic_map::TopicMapError::Invalid(_))
    ));
    saved
}

async fn assert_comparison_withdrawn(
    db: &Database,
    snapshot: &TopicMapSnapshot,
    proof: &ComparisonProof,
    result: Uuid,
    independent: &[(Uuid, Uuid)],
    saved: &[Uuid],
) {
    for work in proof.works {
        let retained = snapshot
            .works
            .iter()
            .find(|w| w.work_ref == work)
            .unwrap()
            .research
            .as_ref()
            .expect("each independent author window remains available");
        assert!(
            retained["resultRefs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r != &json!(result))
        );
        let own_result = independent
            .iter()
            .find(|(owner, _)| *owner == work)
            .unwrap()
            .1;
        assert!(
            retained["resultRefs"]
                .as_array()
                .unwrap()
                .contains(&json!(own_result))
        );
        for key in ["angles", "productOpportunities"] {
            assert!(
                retained["output"][key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|item| {
                        item["researchResultRef"] != json!(result)
                            && item["evidence"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .all(|c| !c["fragmentId"].as_str().unwrap().contains(".comment."))
                    }),
                "withdrawal hides this comparison from every participating work"
            );
        }
        assert!(
            retained["output"]["productOpportunities"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    for reference in saved {
        let alternative = snapshot
            .alternatives
            .iter()
            .find(|a| a["alternativeRef"] == json!(reference))
            .unwrap();
        assert_eq!(alternative["sourceState"], "source_unavailable");
        assert!(alternative["angle"].is_null());
        let original = topic_map::read_saved_alternative(db, D, *reference)
            .await
            .unwrap();
        assert_eq!(original.source_state, "source_unavailable");
        assert!(
            original.fragments.is_empty()
                && original.angle.is_none()
                && original.original_definition.is_none()
        );
    }
}

async fn comparison_runtime_state(db: &Database, run: Uuid) -> Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
            'runRef',r.run_ref,'state',r.state,'reason',r.last_reason,
            'inputTokenLimit',c.input_token_limit,
            'comparisonState',r.input_scope#>'{comparison,state}',
            'comparisonReason',r.input_scope#>'{comparison,reason}',
            'tasks',COALESCE((SELECT jsonb_agg(jsonb_build_object(
                'taskRef',t.task_ref,'phase',t.phase,'state',t.state,
                'reason',t.last_reason,'attempts',t.attempt_count,
                'invocations',(SELECT count(*) FROM linggan_topic_map_research_request q
                    WHERE q.task_ref=t.task_ref),
                'results',(SELECT count(*) FROM linggan_topic_map_research_result result
                    JOIN linggan_topic_map_research_request q USING(invocation_ref)
                    WHERE q.task_ref=t.task_ref)) ORDER BY t.created_at,t.task_ref)
                FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref),'[]'::jsonb))
            FROM linggan_topic_map_research_run r
            JOIN linggan_model_config c ON c.config_ref=r.config_ref WHERE r.run_ref=$1"#,
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn comparison_and_comment_citations_are_visible_then_restriction_invalidates() {
    let (db, config, adapter) = setup("topic_map_research_comparison").await;
    let proof = comparison_proof(&db).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    for work in proof.works {
        apply_research_command(&db, &start(vec![work]))
            .await
            .unwrap();
        finish_pending(&db, &adapter).await;
    }
    let independent: Vec<(Uuid, Uuid)> = sqlx::query_as("SELECT t.work_public_ref,r.result_ref FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request q USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.work_public_ref=ANY($1) AND q.phase='resolve' AND COALESCE(t.input_refs#>>'{coverage,kind}','source')<>'comparison' AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.input_refs->'fragments') f WHERE f->>'field'='body')")
        .bind(proof.works.as_slice()).fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        independent.len(),
        2,
        "both independent author windows have accepted results"
    );
    let start_receipt = apply_research_command(&db, &start(proof.works.to_vec()))
        .await
        .unwrap();
    let run: Uuid = start_receipt["runRef"].as_str().unwrap().parse().unwrap();
    finish_pending(&db, &adapter).await;
    let results: Vec<Uuid> = sqlx::query_scalar("SELECT r.result_ref FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE q.run_ref=$1 AND q.phase='compare'")
        .bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        results.len(),
        1,
        "both work views share one persisted comparison result; runtime: {}",
        comparison_runtime_state(&db, run).await
    );
    let result = results[0];
    let coverage: Value = sqlx::query_scalar("SELECT q.request_manifest#>'{source,coverage}' FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE r.result_ref=$1")
        .bind(result).fetch_one(db.pool()).await.unwrap();
    for field in ["currentSources", "sourceHashes", "fragmentOrigins"] {
        assert!(
            coverage[field]
                .as_object()
                .is_some_and(|map| !map.is_empty()),
            "the complete frozen audit mapping remains available: {field}"
        );
    }
    let query = TopicMapQuery {
        domain_ref: Some(D),
        reference_window_days: Some(0),
        ..Default::default()
    };
    let snapshot = topic_map::read_topic_map(&db, &query).await.unwrap();
    let (angle, opportunity) = both_comparison_items(&snapshot, &proof, result);
    assert_eq!(
        apply_research_command(&db, &start(proof.works.to_vec()))
            .await
            .unwrap()["state"],
        "no_new_input"
    );
    let saved = save_comparison_items(&db, &proof, &angle, &opportunity).await;
    let dispatches: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_research_request WHERE run_ref=$1 AND phase='compare' AND dispatch_started_at IS NOT NULL")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        dispatches, 1,
        "reading/saving from both views does not dispatch twice"
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic restriction'FROM linggan_material_comment WHERE material_ref=$1")
        .bind(proof.comment).execute(db.pool()).await.unwrap();
    let snapshot = topic_map::read_topic_map(&db, &query).await.unwrap();
    assert_comparison_withdrawn(&db, &snapshot, &proof, result, &independent, &saved).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn unsent_recovery_respects_one_attempt_and_platform_own_identity() {
    let (db, config, adapter) = setup("topic_map_research_unsent").await;
    let w = work(&db, "unsent-one", "SYNTHETIC 待恢复练习。").await;
    // Same external ID on another platform is not our creator identity.
    linggan_intelligence::topic_map::save_topic_map_command(
        &db,
        &linggan_intelligence::topic_map::TopicMapCommand::OwnCreator {
            idempotency_key: Uuid::new_v4().to_string(),
            domain_ref: D,
            platform: "douyin".into(),
            author_external_id: "synthetic-creator".into(),
            active: true,
        },
    )
    .await
    .unwrap();
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![w])).await.unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    let row = sqlx::query(
        "SELECT task_ref,input_refs FROM linggan_topic_map_research_task WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    use sqlx::Row;
    let task: Uuid = row.get("task_ref");
    let refs: serde_json::Value = row.get("input_refs");
    assert_eq!(refs["roleMetadata"]["own"], false);
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens)SELECT $1,m.connection_version_ref,c.model_ref,c.config_ref,'analyze',$3,'running',2048,2048 FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE config_ref=$2").bind(invocation).bind(config).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,deadline_at)VALUES($1,$2,$3,1,$4,'{}',(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()-interval '1 minute')").bind(invocation).bind(task).bind(run).bind("0".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=1,phase_attempt_count=1,lease_token=$2 WHERE task_ref=$1").bind(task).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    // Recovery and final maintenance may advance state without dispatching. The
    // ledger below still proves that no retry was reserved or charged.
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "completed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(invocation)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*)FROM linggan_topic_map_research_request WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn title_and_body_windows_finish_without_assuming_task_order() {
    let (db, config, adapter) = setup("topic_map_research_windows").await;
    let w = work_with_title(
        &db,
        "separate-source-windows",
        "SYNTHETIC 先安排一次练习",
        "SYNTHETIC 再从一个具体步骤开始练习。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![w])).await.unwrap();
    let fields: Vec<String> = sqlx::query_scalar("SELECT ARRAY_AGG(DISTINCT f->>'field' ORDER BY f->>'field') FROM linggan_topic_map_research_task t CROSS JOIN LATERAL jsonb_array_elements(t.input_refs->'fragments') f")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(fields, vec!["body", "title"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_task")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    finish_pending(&db, &adapter).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_result")
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
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        apply_research_command(&db, &start(vec![w])).await.unwrap()["state"],
        "no_new_input"
    );
}

async fn progress_for_run(db: &Database, run: Uuid) -> Value {
    read_research_progress(db, D).await.unwrap()["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["runRef"] == json!(run))
        .unwrap()
        .clone()
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and synthetic child, no provider"]
async fn completed_comparison_progress_keeps_failure_and_allows_explicit_new_start() {
    let (db, template, adapter) = setup("topic_map_research_progress_failure").await;
    let config = Uuid::new_v4();
    // Keep the shared 8192 configuration immutable. This dedicated limit fits
    // the independent sources but rejects the larger, three-discussion comparison.
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,model_ref,7000,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$2")
        .bind(config).bind(template).execute(db.pool()).await.unwrap();
    let proof = comparison_proof(&db).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    for work in proof.works {
        apply_research_command(&db, &start(vec![work]))
            .await
            .unwrap();
        finish_pending(&db, &adapter).await;
    }
    let mut previous_task = None;
    let mut previous_run = None;
    // The second iteration is a new explicit command, never an automatic retry.
    for _ in 0..2 {
        let receipt = apply_research_command(&db, &start(proof.works.to_vec()))
            .await
            .unwrap();
        assert_eq!(receipt["state"], "queued");
        let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
        assert_ne!(previous_run, Some(run));
        finish_pending(&db, &adapter).await;
        let progress = progress_for_run(&db, run).await;
        assert_eq!(progress["state"], "completed");
        assert_eq!(progress["queuedCount"], 0);
        assert_eq!(progress["succeededCount"], 0);
        assert_eq!(progress["failedCount"], 1);
        assert_eq!(progress["phases"]["compareQueued"], 0);
        assert_eq!(progress["phases"]["comparing"], 0);
        assert_eq!(progress["lastReason"], "comparison_queued");
        assert_eq!(progress["inputScope"]["comparison"]["state"], "queued");
        let task: Uuid = progress["inputScope"]["comparison"]["taskRef"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_ne!(previous_task, Some(task));
        assert_eq!(
            progress["taskIssues"],
            json!([{
                "taskRef":task,"phase":"compare","state":"failed","reason":"model_input_limit"
            }])
        );
        let calls: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_topic_map_research_request WHERE run_ref=$1",
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(calls, 0, "input rejection occurs before any invocation");
        assert!(
            !run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
        previous_task = Some(task);
        previous_run = Some(run);
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL projection fixture, no provider"]
async fn research_progress_bounds_task_issues_without_losing_totals_or_unknown_reasons() {
    let (db, config, _) = setup("topic_map_research_progress_bounds").await;
    let work = work(&db, "progress-bounds", "SYNTHETIC 进度投影材料。").await;
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_map_research_run(run_ref,domain_ref,request_ref,trigger,config_ref,method_version,token_limit,state) VALUES($1,$2,$3,'on_demand',$4,'topic-map.research.v2',100000,'completed')")
        .bind(run).bind(D).bind(Uuid::new_v4()).bind(config).execute(db.pool()).await.unwrap();
    let mut issue_refs = Vec::new();
    for index in 0..30 {
        let task = Uuid::new_v4();
        let state = match index {
            24 => "unknown_dispatch",
            25 => "succeeded",
            26 => "no_signal",
            27 => "insufficient",
            28 => "stale",
            29 => "stopped",
            _ => "failed",
        };
        let reason = match index {
            23 => None,
            24 => Some("unknown_dispatch".to_string()),
            _ => Some(format!("synthetic_unrecognized_{index}")),
        };
        sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,state,phase,last_reason,updated_at) VALUES($1,$2,$3,$4,$5,'{}',$6,$7,$8,'2026-10-10T00:00:00Z'::timestamptz+$9::int*interval '1 second')")
            .bind(task).bind(run).bind(D).bind(work).bind(format!("{index:064x}"))
            .bind(state).bind(["extract","resolve","compare"][index % 3]).bind(reason)
            .bind(i32::try_from(index).unwrap()).execute(db.pool()).await.unwrap();
        if ["failed", "unknown_dispatch"].contains(&state) {
            issue_refs.push(json!(task));
        }
    }
    let progress = progress_for_run(&db, run).await;
    assert_eq!(progress["failedCount"], 25);
    assert_eq!(progress["succeededCount"], 3);
    assert_eq!(progress["queuedCount"], 0);
    let issues = progress["taskIssues"].as_array().unwrap();
    assert_eq!(issues.len(), 20);
    assert_eq!(
        issues
            .iter()
            .map(|item| item["taskRef"].clone())
            .collect::<Vec<_>>(),
        issue_refs.into_iter().rev().take(20).collect::<Vec<_>>()
    );
    assert_eq!(issues[0]["state"], "unknown_dispatch");
    assert_eq!(issues[0]["phase"], "extract");
    assert_eq!(issues[0]["reason"], "unknown_dispatch");
    assert!(issues[1].get("reason").is_some_and(Value::is_null));
    assert_eq!(issues[2]["reason"], "synthetic_unrecognized_22");
    assert!(
        issues
            .iter()
            .all(|item| ["failed", "unknown_dispatch"].contains(&item["state"].as_str().unwrap()))
    );
}
