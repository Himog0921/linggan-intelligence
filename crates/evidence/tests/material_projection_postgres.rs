use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
    start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0016_material_social_lanes.sql"),
);

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn content_detail_submission_forms_a_typed_material_with_field_sources_and_unknowns() {
    let database = proof_database("material_detail_slice").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-1"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"externalId":"note-material-1"},
            "payload":{
                "title":"逐字段来源标题",
                "bodyText":"可检索正文",
                "authorId":"author-material-1"
            }
        }),
    )
    .await;

    let row = sqlx::query(
        "SELECT title, title_state, body_text, body_state, creator_display_name_state, \
                package_ref, record_ordinal \
         FROM linggan_material_content_detail",
    )
    .fetch_one(database.pool())
    .await
    .expect("a qualified detail record forms one typed material row");

    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("逐字段来源标题")
    );
    assert_eq!(row.get::<String, _>("title_state"), "KNOWN");
    assert_eq!(
        row.get::<Option<String>, _>("body_text").as_deref(),
        Some("可检索正文")
    );
    assert_eq!(row.get::<String, _>("body_state"), "KNOWN");
    assert_eq!(
        row.get::<String, _>("creator_display_name_state"),
        "UNKNOWN"
    );
    assert_ne!(row.get::<uuid::Uuid, _>("package_ref"), uuid::Uuid::nil());
    assert_eq!(row.get::<i32, _>("record_ordinal"), 0);

    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-2"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"externalId":"attacker-selected-other-note"},
            "payload":{
                "title":"不得跨目标接纳",
                "creatorName":"不得猜成 authorName"
            }
        }),
    )
    .await;
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .expect("detail count reads");
    assert_eq!(
        detail_count, 1,
        "mismatched source identity is not projected"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn comments_replies_and_author_keep_stable_identity_relationships_and_versions() {
    let database = proof_database("material_social_lanes_red").await;
    submit_package(
        &database,
        "comments",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        serde_json::json!({
            "kind":"comment",
            "payload":{"commentId":"comment-root-1","noteId":"note-social-1","text":"评论可检索原声","authorId":"author-social-1"}
        }),
    ).await;
    submit_package(
        &database,
        "replies",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        serde_json::json!({
            "kind":"reply",
            "payload":{"commentId":"reply-1","noteId":"note-social-1","text":"回复原声","rootCommentId":"comment-root-1","parentCommentId":"comment-root-1"}
        }),
    ).await;
    submit_package(
        &database,
        "author_profile",
        serde_json::json!({"authorExternalId":"author-social-1"}),
        serde_json::json!({
            "kind":"author_profile",
            "sourceObject":{"externalId":"author-social-1"},
            "payload":{"userId":"author-social-1","nickname":"版本化作者","fans":17}
        }),
    )
    .await;
    submit_package(
        &database,
        "replies",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        serde_json::json!({
            "kind":"reply",
            "payload":{"commentId":"reply-without-parent","noteId":"note-social-1","text":"不能猜父级"}
        }),
    ).await;

    let relation = sqlx::query("SELECT comment_external_id,root_comment_external_id,parent_comment_external_id FROM linggan_material_comment ORDER BY is_reply")
        .fetch_all(database.pool()).await.expect("typed comment relation exists");
    assert_eq!(relation.len(), 2);
    assert_eq!(
        relation[1].get::<String, _>("root_comment_external_id"),
        "comment-root-1"
    );
    assert_eq!(
        relation[1]
            .get::<Option<String>, _>("parent_comment_external_id")
            .as_deref(),
        Some("comment-root-1")
    );

    let author_count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_author_profile WHERE author_external_id = 'author-social-1'")
        .fetch_one(database.pool()).await.expect("versioned author observation exists");
    assert_eq!(author_count, 1);
    let disposition: (String, String) = sqlx::query_as(
        "SELECT disposition,reason FROM linggan_runtime_record_disposition disposition \
         JOIN linggan_runtime_capture_package package USING (package_ref) \
         WHERE package.package_kind='replies' AND package.payload::text LIKE '%reply-without-parent%'",
    )
    .fetch_one(database.pool()).await.expect("invalid reply keeps an explicit disposition");
    assert_eq!(disposition.0, "quarantined");
    assert_eq!(disposition.1, "typed_reply_relationship_invalid");
}

async fn submit_package(
    database: &Database,
    capability: &str,
    target: serde_json::Value,
    record: serde_json::Value,
) {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1",
        "taskId":task_id,
        "source":"manual",
        "platform":"xhs",
        "pageType":"synthetic_material_proof",
        "target":target.clone(),
        "capabilitiesRequested":[capability],
        "maximumQuota":1,
        "commentLimit":"not_requested",
        "acquireMedia":"not_requested",
        "riskPolicy":"local_trusted_user_initiated",
        "stopConditions":["maximum_quota"]
    });
    let task = parse_producer_task_spec(&task.to_string()).expect("bounded task validates");
    assert!(matches!(
        create_producer_task(database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));

    let attempt = serde_json::json!({
        "contractVersion":"linggan.producer.attempt.v1",
        "producerInstanceId":producer_instance_id,
        "taskId":task_id,
        "attemptId":attempt_id
    });
    let attempt = parse_producer_attempt(&attempt.to_string()).expect("attempt validates");
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));

    let package = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "packageRef":uuid::Uuid::new_v4(),
        "packageKind":capability,
        "platform":"xhs",
        "observedAt":"2026-08-28T10:00:00Z",
        "capturedAt":"2026-08-28T10:00:01Z",
        "coverage":{
            "target":target,
            "layers":[{
                "capability":capability,
                "observed":1,
                "attempted":1,
                "acquired":1,
                "verified":0,
                "failed":0,
                "notAttempted":0,
                "unknown":0,
                "stoppedReason":"fixture_complete"
            }]
        },
        "records":[record]
    });
    let submission = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "producerInstanceId":producer_instance_id,
        "taskId":task_id,
        "attemptId":attempt_id,
        "submissionId":uuid::Uuid::new_v4(),
        "capturePackage":package
    });
    let submission =
        parse_producer_submission(&submission.to_string()).expect("submission validates");
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}

use sqlx::Row;
