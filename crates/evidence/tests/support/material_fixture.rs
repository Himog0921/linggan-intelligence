use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
    start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};

const MIGRATIONS: &str = concat!(
    "CREATE TABLE linggan_local_schema_migration (\n",
    "  migration_id text PRIMARY KEY,\n",
    "  migration_sha256 text NOT NULL CHECK (migration_sha256 ~ '^[0-9a-f]{64}$'),\n",
    "  applied_at timestamptz NOT NULL DEFAULT clock_timestamp()\n",
    ");\n",
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../../database/migrations/0005_collection_observation_target.sql"),
    "\n",
    include_str!("../../../../database/migrations/0006_collection_acquisition_chain.sql"),
    "\n",
    include_str!("../../../../database/migrations/0007_execution_station.sql"),
    "\n",
    include_str!("../../../../database/migrations/0008_collection_risk_pause.sql"),
    "\n",
    include_str!("../../../../database/migrations/0009_work_order_station.sql"),
    "\n",
    include_str!("../../../../database/migrations/0010_work_order_lease.sql"),
    "\n",
    include_str!("../../../../database/migrations/0011_execution_gate.sql"),
    "\n",
    include_str!("../../../../database/migrations/0012_target_monitor_schedule.sql"),
    "\n",
    include_str!("../../../../database/migrations/0013_drop_execution_gate.sql"),
    "\n",
    include_str!("../../../../database/migrations/0014_target_group.sql"),
    "\n",
    include_str!("../../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../../database/migrations/0019_work_order_lease_task_sequence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!("../../../../database/migrations/0022_material_deepening_scope.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0023_material_engagement_and_media_components.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../../database/migrations/0025_comment_current_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0026_work_resource_read.sql"),
    "\n",
    include_str!("../../../../database/migrations/0027_unified_media_resource.sql"),
    "\n",
    include_str!("../../../../database/migrations/0029_author_avatar_media.sql"),
    "\n",
    include_str!("../../../../database/migrations/0030_comment_image_media.sql"),
    "\n",
    include_str!("../../../../database/migrations/0032_author_profile_avatar_media.sql"),
    "\n",
    "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES ",
    "('0025_comment_current_projection', '64fd9474647834358f8d2d4f1c25e4345e26a3ff79dbfc53a7846915576b0885'), ",
    "('0026_work_resource_read', '08712c71e9b6f97d270739649a7c264da2f115315bef90fabaedded50cf774bd'), ",
    "('0027_unified_media_resource', '70f09bf56fda491665fad0b5c3c534c74da3516fe65ad66416cae140ddee3100'), ",
    "('0029_author_avatar_media', '72ce163e6696c8e32d543a42b3b786f07067af6713af15798d215ab58dea3b8f'), ",
    "('0030_comment_image_media', '6f3913dca8ca9bb0cb025dea76cfe2222a39407c4a17898b438d20b355fa2c81'), ",
    "('0032_author_profile_avatar_media', '0e1b3511c8f306c95d3ca6a829d83362dddeeec42e65114f77ae746b59325b32');\n",
);

pub fn coverage_layer(capability: &str, acquired: i64) -> serde_json::Value {
    serde_json::json!({
        "capability":capability,"observed":acquired,"attempted":acquired,"acquired":acquired,
        "verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"fixture_complete"
    })
}

pub async fn submit_package(
    database: &Database,
    capability: &str,
    target: serde_json::Value,
    record: serde_json::Value,
) -> uuid::Uuid {
    let coverage = serde_json::json!({
        "target":target.clone(),
        "layers":[coverage_layer(capability,1)]
    });
    submit_custom_package(
        database,
        "xhs",
        &[capability],
        target,
        capability,
        "xhs",
        coverage,
        vec![record],
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn submit_custom_package(
    database: &Database,
    task_platform: &str,
    capabilities: &[&str],
    task_target: serde_json::Value,
    package_kind: &str,
    package_platform: &str,
    coverage: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> uuid::Uuid {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual",
        "platform":task_platform,"pageType":"synthetic_material_proof","target":task_target,
        "capabilitiesRequested":capabilities,"maximumQuota":records.len().max(1),
        "commentLimit":"not_requested","acquireMedia":"not_requested",
        "riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]
    });
    let task = parse_producer_task_spec(&task.to_string()).expect("bounded task validates");
    assert!(matches!(
        create_producer_task(database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let attempt = serde_json::json!({
        "contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_instance_id,
        "taskId":task_id,"attemptId":attempt_id
    });
    let attempt = parse_producer_attempt(&attempt.to_string()).expect("attempt validates");
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let package_ref = uuid::Uuid::new_v4();
    let package = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1","packageRef":package_ref,
        "packageKind":package_kind,"platform":package_platform,
        "observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:01Z",
        "coverage":coverage,"records":records
    });
    let submission = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,
        "submissionId":uuid::Uuid::new_v4(),"capturePackage":package
    });
    let submission =
        parse_producer_submission(&submission.to_string()).expect("submission validates");
    let outcome = submit_producer_package(database, &submission).await;
    assert!(
        matches!(outcome, Ok(RuntimeSubmissionOutcome::Acknowledged { .. })),
        "material fixture submission must be acknowledged: {outcome:?}"
    );
    package_ref
}

pub async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
