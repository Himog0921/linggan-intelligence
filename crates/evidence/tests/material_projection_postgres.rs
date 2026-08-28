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
    "\n",
    include_str!("../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0018_material_discovery_lane.sql"),
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
async fn discovery_forms_the_same_work_identity_without_claiming_detail() {
    let database = proof_database("material_discovery_slice").await;
    submit_package(&database,"discovery_search",serde_json::json!({"query":"ADHD"}),serde_json::json!({
        "kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"note-discovery-material"},
        "payload":{"title":"发现面标题","authorName":"发现面作者"}
    })).await;
    let row=sqlx::query("SELECT finding.title,finding.title_state,content.content_external_id FROM linggan_material_discovery_finding finding JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref")
        .fetch_one(database.pool()).await.expect("typed discovery finding exists");
    assert_eq!(
        row.get::<String, _>("content_external_id"),
        "note-discovery-material"
    );
    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("发现面标题")
    );
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        detail_count, 0,
        "discovery never manufactures detail material"
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

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn media_slots_preserve_all_candidates_generation_and_honest_unknown_order_components() {
    let database = proof_database("material_media_red").await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-1"}),
        serde_json::json!({
            "kind":"media_slot","slotKey":"xhs:note-media-1:image:1","observationRef":uuid::Uuid::new_v4(),
            "slot":{"role":"image","ordinal":1},
            "observation":{"externalUri":"https://media.example/primary","candidateUris":["https://media.example/primary","https://media.example/backup"],"observedAt":"2026-08-28T10:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-1"}
        }),
    ).await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-live"}),
        serde_json::json!({
            "kind":"media_slot","slotKey":"xhs:note-media-live:live_photo:1","observationRef":uuid::Uuid::new_v4(),
            "slot":{"role":"live_photo","ordinal":1},
            "observation":{"externalUri":"https://media.example/live-unknown","candidateUris":["https://media.example/live-unknown"],"observedAt":"2026-08-28T10:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-live"}
        }),
    ).await;

    let origin = sqlx::query("SELECT purpose,producer_ordinal,display_ordinal,display_order_state,source_generation,candidate_set_state FROM linggan_material_media_origin WHERE slot_key='xhs:note-media-1:image:1'")
        .fetch_one(database.pool()).await.expect("typed media origin exists");
    assert_eq!(origin.get::<String, _>("purpose"), "body_image");
    assert_eq!(origin.get::<Option<i32>, _>("display_ordinal"), None);
    assert_eq!(origin.get::<String, _>("display_order_state"), "UNKNOWN");
    assert_eq!(origin.get::<i32, _>("source_generation"), 1);
    let candidate_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_media_candidate")
            .fetch_one(database.pool())
            .await
            .expect("candidate rows query");
    assert_eq!(candidate_count, 3);
    let live: (String,String,String) = sqlx::query_as("SELECT composite_state,live_photo_still_state,live_photo_motion_state FROM linggan_material_media_origin WHERE purpose='live_photo'")
        .fetch_one(database.pool()).await.expect("live photo stays partial");
    assert_eq!(live, ("PARTIAL".into(), "UNKNOWN".into(), "UNKNOWN".into()));
    submit_package(
        &database,"media_slots",serde_json::json!({"contentExternalId":"note-media-invalid"}),
        serde_json::json!({"kind":"media_slot","slotKey":"xhs:note-media-invalid:image:1","observationRef":uuid::Uuid::new_v4(),"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/primary","candidateUris":["https://media.example/different-first","https://media.example/primary"],"observedAt":"2026-08-28T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-invalid"}}),
    ).await;
    let invalid_disposition: (String,String) = sqlx::query_as("SELECT disposition,reason FROM linggan_runtime_record_disposition disposition JOIN linggan_runtime_capture_package package USING(package_ref) WHERE package.payload::text LIKE '%different-first%'")
        .fetch_one(database.pool()).await.expect("invalid candidate list disposition exists");
    assert_eq!(
        invalid_disposition,
        ("quarantined".into(), "media_origin_contract_invalid".into())
    );
    let invalid_origin_count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_media_origin origin JOIN linggan_material_content content ON content.public_ref=origin.content_public_ref WHERE content.content_external_id='note-media-invalid'")
        .fetch_one(database.pool()).await.expect("invalid origin count reads");
    assert_eq!(invalid_origin_count, 0);

    let concurrent_a = submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-concurrent"}),
        serde_json::json!({"kind":"media_slot","slotKey":"xhs:note-media-concurrent:image:1","observationRef":uuid::Uuid::new_v4(),"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/a","candidateUris":["https://media.example/a"],"observedAt":"2026-08-28T10:01:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-concurrent"}}),
    );
    let concurrent_b = submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-concurrent"}),
        serde_json::json!({"kind":"media_slot","slotKey":"xhs:note-media-concurrent:image:1","observationRef":uuid::Uuid::new_v4(),"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/b","candidateUris":["https://media.example/b"],"observedAt":"2026-08-28T10:02:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-concurrent"}}),
    );
    tokio::join!(concurrent_a, concurrent_b);
    let generations: Vec<i32> = sqlx::query_scalar("SELECT source_generation FROM linggan_material_media_origin WHERE slot_key='xhs:note-media-concurrent:image:1' ORDER BY source_generation")
        .fetch_all(database.pool()).await.expect("concurrent generations query");
    assert_eq!(
        generations,
        vec![1, 2],
        "first concurrent observations serialize without losing a qualified package"
    );
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
