#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::proof_database;
use linggan_intelligence::comment_research_rules::{
    AtomicKind, CreateRuleRevision, EditableFieldDefinition, ExamplePolarity, RuleExample,
    RulePurpose, RuleVersion, V4_RULE_REVISION_REF, create_candidate, read_active_rule, read_rule,
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

fn v5_candidate(rule_revision_ref: Uuid) -> CreateRuleRevision {
    CreateRuleRevision {
        rule_revision_ref,
        parent_rule_revision_ref: V4_RULE_REVISION_REF,
        purpose: RulePurpose {
            title: "评论困难研究".into(),
            instruction: "只提取评论者直接表达的具体困难和需要，不把作品材料当作评论者表达。"
                .into(),
        },
        field_definitions: vec![
            EditableFieldDefinition {
                kind: AtomicKind::Problem,
                name: "困难".into(),
                definition: "评论者直接表达的阻碍、困扰或失败原因。".into(),
            },
            EditableFieldDefinition {
                kind: AtomicKind::Need,
                name: "需要".into(),
                definition: "评论者明确表达的期待、需要或待解决事项。".into(),
            },
        ],
        examples: vec![RuleExample {
            field: AtomicKind::Problem,
            polarity: ExamplePolarity::Positive,
            comment: "我总在第一步卡住".into(),
            expected: "提取 problem，保留精确评论证据。".into(),
        }],
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn candidate_and_active_pointer_never_rewrite_v4_history() {
    let db = proof_database("comment_research_rule_versions").await;
    let seeded = read_active_rule(&db).await.unwrap();
    assert_eq!(seeded.rule_version, RuleVersion::V4);
    assert_eq!(seeded.rule_revision_ref, V4_RULE_REVISION_REF);
    let seeded_hash = seeded.canonical_hash.clone();

    research_fixture::detail(&db, "rule-proof", "合成作品").await;
    let source = research_fixture::comment(
        &db,
        "rule-proof",
        "rule-comment",
        "我不知道第一步怎么开始",
        "2026-09-08T10:00:00Z",
    )
    .await;
    let historical = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,failure_code) VALUES($1,$2,'comment-research.v4','rule-history-proof','failed','invalid_output')")
        .bind(historical)
        .bind(source)
        .execute(db.pool())
        .await
        .unwrap();

    let candidate_ref = Uuid::new_v4();
    let candidate = create_candidate(&db, &v5_candidate(candidate_ref))
        .await
        .unwrap();
    assert_eq!(candidate.rule_version, RuleVersion::V5);
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        V4_RULE_REVISION_REF
    );
    assert_eq!(
        create_candidate(&db, &v5_candidate(candidate_ref))
            .await
            .unwrap()
            .canonical_hash,
        candidate.canonical_hash
    );

    assert!(sqlx::query("UPDATE linggan_comment_research_rule_revision SET purpose=$2 WHERE rule_revision_ref=$1")
        .bind(V4_RULE_REVISION_REF)
        .bind(json!({"title":"篡改","instruction":"不应写入"}))
        .execute(db.pool())
        .await
        .is_err());
    assert_eq!(
        read_rule(&db, V4_RULE_REVISION_REF)
            .await
            .unwrap()
            .canonical_hash,
        seeded_hash
    );

    // P3 is the only production mover of this pointer. This direct isolated-schema mutation proves
    // that a later effective rule selection cannot alter immutable v4 rows already recorded.
    sqlx::query("UPDATE linggan_comment_research_rule_active SET rule_revision_ref=$1,revision=revision+1 WHERE singleton")
        .bind(candidate_ref)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        candidate_ref
    );
    let historical_rule: String =
        sqlx::query("SELECT rule_version FROM linggan_comment_analysis_work WHERE work_ref=$1")
            .bind(historical)
            .fetch_one(db.pool())
            .await
            .unwrap()
            .get("rule_version");
    assert_eq!(historical_rule, "comment-research.v4");
}
