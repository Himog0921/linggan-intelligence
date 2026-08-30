use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
    start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};

const MIGRATIONS: &str = concat!(
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
    include_str!("../../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0023_material_engagement_and_media_components.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../../database/migrations/0025_work_resource_read.sql"),
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
