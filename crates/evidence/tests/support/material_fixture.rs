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
    include_str!("../../../../database/migrations/0031_topic_workspace.sql"),
    include_str!("../../../../database/migrations/0032_author_profile_avatar_media.sql"),
    "\n",
    include_str!("../../../../database/migrations/0033_dispatch_failure_recovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0034_collection_control_closure.sql"),
    "\n",
    include_str!("../../../../database/migrations/0035_claimed_station_auto_acceptance.sql"),
    "\n",
    include_str!("../../../../database/migrations/0036_monitor_scheduling_clarity.sql"),
    "\n",
    include_str!("../../../../database/migrations/0037_collection_scheduler_scale.sql"),
    "\n",
    include_str!("../../../../database/migrations/0038_detail_only_material_scope.sql"),
    "\n",
    include_str!("../../../../database/migrations/0039_comment_research.sql"),
    include_str!("../../../../database/migrations/0040_model_pi.sql"),
    "\n",
    include_str!("../../../../database/migrations/0041_observation_domain.sql"),
    "\n",
    include_str!("../../../../database/migrations/0042_keyword_monitoring_lifecycle.sql"),
    "\n",
    include_str!("../../../../database/migrations/0043_comment_daily.sql"),
    "\n",
    include_str!("../../../../database/migrations/0044_cross_industry_comment.sql"),
    "\n",
    include_str!("../../../../database/migrations/0045_deep_archive_recovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0046_keyword_sampling_policy.sql"),
    "\n",
    include_str!("../../../../database/migrations/0047_collection_detail_failure_boundary.sql"),
    "\n",
    include_str!("../../../../database/migrations/0048_comment_intelligence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0049_comment_research_runtime.sql"),
    include_str!("../../../../database/migrations/0050_comment_research_automation.sql"),
    include_str!("../../../../database/migrations/0051_comment_problem_vectors.sql"),
    "\n",
    include_str!("../../../../database/migrations/0052_work_order_expiry.sql"),
    "\n",
    include_str!("../../../../database/migrations/0053_comment_research_rules.sql"),
    "\n",
    include_str!("../../../../database/migrations/0054_comment_auto_policy.sql"),
    include_str!("../../../../database/migrations/0055_comment_research_replay.sql"),
    include_str!("../../../../database/migrations/0056_comment_semantic_atoms.sql"),
    include_str!("../../../../database/migrations/0057_comment_local_recovery.sql"),
    include_str!("../../../../database/migrations/0058_comment_field_repair.sql"),
    include_str!("../../../../database/migrations/0059_comment_replay_continuity.sql"),
    include_str!("../../../../database/migrations/0060_comment_topic_associations.sql"),
    "\n",
    include_str!("../../../../database/migrations/0061_material_retirement.sql"),
    "\n",
    include_str!("../../../../database/migrations/0062_human_moment.sql"),
    "\n",
    include_str!("../../../../database/migrations/0063_content_author_attribution.sql"),
    "\n",
    include_str!("../../../../database/migrations/0064_account_observation_normalization.sql"),
    "\n",
    include_str!("../../../../database/migrations/0064_comment_research_kernel.sql"),
    "\n",
    include_str!("../../../../database/migrations/0065_comment_research_vector_candidates.sql"),
    "\n",
    include_str!("../../../../database/migrations/0066_comment_research_change_signals.sql"),
    "\n",
    include_str!("../../../../database/migrations/0067_comment_research_run_item_lease.sql"),
    "\n",
    include_str!("../../../../database/migrations/0068_comment_research_problem_resolution.sql"),
    "\n",
    include_str!("../../../../database/migrations/0069_comment_research_v1_cutover.sql"),
    "\n",
    include_str!("../../../../database/migrations/0070_comment_research_v1_derivation_head.sql"),
    "\n",
    include_str!("../../../../database/migrations/0071_cross_industry_sampling_provenance.sql"),
    "\n",
    include_str!("../../../../database/migrations/0072_cross_industry_sample_lane.sql"),
    "\n",
    include_str!("../../../../database/migrations/0073_cross_industry_sample_observation.sql"),
    "\n",
    include_str!("../../../../database/migrations/0074_cross_industry_detail_scope.sql"),
    "\n",
    include_str!("../../../../database/migrations/0075_local_embedding_001.sql"),
    "\n",
    include_str!("../../../../database/migrations/0076_monitor_rule_slots.sql"),
    "\n",
    include_str!("../../../../database/migrations/0077_comment_research_voice_read_index.sql"),
    "\n",
    include_str!("../../../../database/migrations/0078_monitor_rule_owns_its_schedule.sql"),
    "\n",
    include_str!("../../../../database/migrations/0079_cross_industry_sample_detail.sql"),
    "\n",
    include_str!("../../../../database/migrations/0065_account_observation_bootstrap.sql"),
    "\n",
    include_str!("../../../../database/migrations/0080_scheduler_admission_failure_reasons.sql"),
    "\n",
    include_str!("../../../../database/migrations/0081_comment_research_fingerprint.sql"),
    "\n",
    include_str!("../../../../database/migrations/0082_comment_research_execution_recovery.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0083_comment_research_cross_run_resolution_retry.sql"
    ),
    "\n",
    "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES ",
    "('0025_comment_current_projection', '64fd9474647834358f8d2d4f1c25e4345e26a3ff79dbfc53a7846915576b0885'), ",
    "('0026_work_resource_read', '08712c71e9b6f97d270739649a7c264da2f115315bef90fabaedded50cf774bd'), ",
    "('0027_unified_media_resource', '70f09bf56fda491665fad0b5c3c534c74da3516fe65ad66416cae140ddee3100'), ",
    "('0029_author_avatar_media', '72ce163e6696c8e32d543a42b3b786f07067af6713af15798d215ab58dea3b8f'), ",
    "('0030_comment_image_media', '6f3913dca8ca9bb0cb025dea76cfe2222a39407c4a17898b438d20b355fa2c81'), ",
    "('0032_author_profile_avatar_media', '0e1b3511c8f306c95d3ca6a829d83362dddeeec42e65114f77ae746b59325b32'), ",
    "('0033_dispatch_failure_recovery', 'd5a24915664c1660eb0a46ff302c9c324c95b11c5c3d9804269ab02b09b10ecb'), ",
    "('0035_claimed_station_auto_acceptance', 'feed87adc4fe8090975a0c1150b08d862c0975e80ed71bcb88188053ea7e3a36'), ",
    "('0036_monitor_scheduling_clarity', '0b2d3c9ed8525d27ce3cac56d620596210d41512b6acaad0cc98c803c1b84c47'), ",
    "('0037_collection_scheduler_scale', 'e1a9acf277243391032263a24a900771b5e8b3d967890e7c4b58d82632c5a4b0'), ",
    "('0038_detail_only_material_scope', 'a748e3c810b85b523e490e4ac202569e024024f4be4b09f32ca824dedd867cc7'),\n",
    "('0045_deep_archive_recovery', '27b926f404593fd4a65cd88c63afe6e62996281496e8899342c9c647cbf6c349'), ",
    "('0047_collection_detail_failure_boundary', 'bf925085f968c6711ff5a81cda9feeddc2468870a54c33ca37aecd16341b7aeb'), ",
    "('0065_account_observation_bootstrap', '7b375e7b0ad786aa45d3b4ab83b3843f34a2fe88c9599a3c4b5cc95afeb990d6'), ",
    "('0082_comment_research_execution_recovery', '16c4458dea19ee5faeb24c7017860b18b61eb0b448ec987cccfddb0f0929153c'), ",
    "('0083_comment_research_cross_run_resolution_retry', 'be2bb61fbd766e47b209186b837e6e56e20e4584e055ee8a6c57554720d93e43');\n",
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
    submit_package_at(database, capability, target, record, "2026-08-28T10:00:00Z").await
}

pub async fn submit_package_at(
    database: &Database,
    capability: &str,
    target: serde_json::Value,
    record: serde_json::Value,
    observed_at: &str,
) -> uuid::Uuid {
    let coverage = serde_json::json!({
        "target":target.clone(),
        "layers":[coverage_layer(capability,1)]
    });
    submit_custom_package_at(
        database,
        "xhs",
        &[capability],
        target,
        capability,
        "xhs",
        coverage,
        vec![record],
        observed_at,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
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
    submit_custom_package_at(
        database,
        task_platform,
        capabilities,
        task_target,
        package_kind,
        package_platform,
        coverage,
        records,
        "2026-08-28T10:00:00Z",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn submit_custom_package_at(
    database: &Database,
    task_platform: &str,
    capabilities: &[&str],
    task_target: serde_json::Value,
    package_kind: &str,
    package_platform: &str,
    coverage: serde_json::Value,
    records: Vec<serde_json::Value>,
    observed_at: &str,
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
        "observedAt":observed_at,"capturedAt":observed_at,
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
