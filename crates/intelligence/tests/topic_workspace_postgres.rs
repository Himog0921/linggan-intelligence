use linggan_intelligence::{
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceError, TopicWorkspaceImport,
    import_topic_workspace, read_topic_workspace,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use uuid::Uuid;

const TOPIC_PROOF_SCHEMA: &str = concat!(
    "CREATE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT clock_timestamp() $$;\n",
    "CREATE FUNCTION linggan_plugin_runtime_forbid_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only relation'; END $$;\n",
    "CREATE TABLE linggan_material_content (public_ref uuid NOT NULL UNIQUE);\n",
    include_str!("../../../database/migrations/0027_topic_workspace.sql")
);

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("isolated proof database URL is supplied");
    isolated_proof_schema(&url, schema, TOPIC_PROOF_SCHEMA)
        .await
        .expect("Topic proof schema applies")
}

async fn admit_work(database: &Database, public_ref: Uuid) {
    sqlx::query("INSERT INTO linggan_material_content(public_ref) VALUES ($1)")
        .bind(public_ref)
        .execute(database.pool())
        .await
        .expect("fixture Work Resource identity is admitted");
}

fn import_request(
    idempotency_key: &str,
    expected_version: Option<i32>,
    support_ref: Uuid,
    challenge_ref: Uuid,
) -> TopicWorkspaceImport {
    TopicWorkspaceImport {
        idempotency_key: idempotency_key.to_owned(),
        domain_key: "adhd-family".to_owned(),
        canonical_key: "task-initiation-difficulty".to_owned(),
        display_name: "任务启动困难".to_owned(),
        definition_text: "在有明确意图时仍难以开始第一步的可观察困难。".to_owned(),
        expected_version,
        adjudication_note: "由研究者按定义逐条裁定，未使用自动分类。".to_owned(),
        source_boundary: "仅含当前已接纳的两条小红书 Work Resource；不代表总体分布。".to_owned(),
        members: vec![
            TopicMaterialMemberImport {
                work_public_ref: support_ref,
                role: TopicMaterialRole::Support,
                rationale: "直接描述想做但无法开始。".to_owned(),
            },
            TopicMaterialMemberImport {
                work_public_ref: challenge_ref,
                role: TopicMaterialRole::Challenge,
                rationale: "提供了可能由疲劳解释的反例。".to_owned(),
            },
        ],
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn imports_and_reads_an_exact_provisional_topic_material_pack() {
    let database = proof_database("topic_workspace_exact_pack").await;
    let support_ref = Uuid::new_v4();
    let challenge_ref = Uuid::new_v4();
    admit_work(&database, support_ref).await;
    admit_work(&database, challenge_ref).await;

    let receipt = import_topic_workspace(
        &database,
        &import_request(
            "topic-proof:exact-pack:0001",
            None,
            support_ref,
            challenge_ref,
        ),
    )
    .await
    .expect("valid adjudication is admitted");
    assert_eq!(receipt.definition_version, 1);

    let workspace = read_topic_workspace(&database, "task-initiation-difficulty")
        .await
        .expect("workspace read succeeds")
        .expect("workspace exists");
    assert_eq!(workspace.definition.version, 1);
    assert_eq!(workspace.definition.lifecycle_state, "provisional");
    assert_eq!(workspace.classification_run.run_kind, "human_adjudicated");
    assert_eq!(
        workspace.material_pack.material_pack_ref,
        receipt.material_pack_ref
    );
    assert_eq!(workspace.members.len(), 2);
    assert_eq!(workspace.members[0].work_public_ref, support_ref);
    assert_eq!(workspace.members[0].role, TopicMaterialRole::Support);
    assert_eq!(workspace.members[1].work_public_ref, challenge_ref);
    assert_eq!(workspace.members[1].role, TopicMaterialRole::Challenge);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn rejects_an_unknown_work_resource_without_partial_topic_rows() {
    let database = proof_database("topic_workspace_unknown_work").await;
    let support_ref = Uuid::new_v4();
    let unknown_ref = Uuid::new_v4();
    admit_work(&database, support_ref).await;

    let error = import_topic_workspace(
        &database,
        &import_request(
            "topic-proof:unknown-work:0001",
            None,
            support_ref,
            unknown_ref,
        ),
    )
    .await
    .expect_err("unknown Work Resource cannot enter a pack");
    assert_eq!(error, TopicWorkspaceError::UnknownWorkResource(unknown_ref));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_topic_workspace")
        .fetch_one(database.pool())
        .await
        .expect("topic row count reads");
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn same_idempotency_key_replays_only_the_identical_request() {
    let database = proof_database("topic_workspace_idempotency").await;
    let support_ref = Uuid::new_v4();
    let challenge_ref = Uuid::new_v4();
    admit_work(&database, support_ref).await;
    admit_work(&database, challenge_ref).await;
    let request = import_request(
        "topic-proof:idempotency:0001",
        None,
        support_ref,
        challenge_ref,
    );

    let first = import_topic_workspace(&database, &request)
        .await
        .expect("first import succeeds");
    let replay = import_topic_workspace(&database, &request)
        .await
        .expect("identical replay succeeds");
    assert_eq!(replay, first);

    let mut changed = request;
    changed.definition_text.push_str(" 内容已改变。");
    let error = import_topic_workspace(&database, &changed)
        .await
        .expect_err("changed replay conflicts");
    assert_eq!(error, TopicWorkspaceError::IdempotencyConflict);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_new_definition_version_requires_the_current_expected_version() {
    let database = proof_database("topic_workspace_version_guard").await;
    let support_ref = Uuid::new_v4();
    let challenge_ref = Uuid::new_v4();
    admit_work(&database, support_ref).await;
    admit_work(&database, challenge_ref).await;
    import_topic_workspace(
        &database,
        &import_request("topic-proof:version:0001", None, support_ref, challenge_ref),
    )
    .await
    .expect("first version succeeds");

    let stale = import_topic_workspace(
        &database,
        &import_request(
            "topic-proof:version:0002",
            Some(0),
            support_ref,
            challenge_ref,
        ),
    )
    .await
    .expect_err("stale expected version is rejected");
    assert_eq!(
        stale,
        TopicWorkspaceError::VersionConflict {
            expected: Some(0),
            actual: Some(1),
        }
    );

    let second = import_topic_workspace(
        &database,
        &import_request(
            "topic-proof:version:0003",
            Some(1),
            support_ref,
            challenge_ref,
        ),
    )
    .await
    .expect("current version can advance");
    assert_eq!(second.definition_version, 2);
}
