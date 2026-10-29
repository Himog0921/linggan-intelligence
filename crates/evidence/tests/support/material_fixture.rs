#[path = "../../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema;
use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
    start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};

pub const MIGRATIONS: &str = concat!(
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
    include_str!("../../../../database/migrations/0084_comment_research_problem_resolution_v2.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0085_comment_research_problem_pair_evaluation.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0086_comment_research_v2_backlog_recall.sql"),
    "\n",
    include_str!("../../../../database/migrations/0087_cross_industry_detail_comment_policy.sql"),
    "\n",
    include_str!("../../../../database/migrations/0088_media_processing_concurrency_policy.sql"),
    "\n",
    include_str!("../../../../database/migrations/0089_detail_page_session_replay_safety.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0090_detail_page_grant_recovery_and_risk_cooldown.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0091_ocr_content_layering.sql"),
    "\n",
    include_str!("../../../../database/migrations/0092_detail_page_session_recovery_boundary.sql"),
    "\n",
    include_str!("../../../../database/migrations/0093_capture_delivery_rejection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0094_corpus_evidence_read_recovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0095_detail_page_url_rejection.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0096_detail_page_session_lane_delivery_identities.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0097_collection_execution_input_eligibility.sql"),
    "\n",
    include_str!("../../../../database/migrations/0098_scheduler_tick_steps_and_readiness.sql"),
    "\n",
    include_str!("../../../../database/migrations/0099_collection_selector_health.sql"),
    include_str!("../../../../database/migrations/0100_collection_hot_path_indexes.sql"),
    include_str!("../../../../database/migrations/0101_collection_command_reason_vocabulary.sql"),
    "\n",
    include_str!("../../../../database/migrations/0102_cross_industry_creator_directory.sql"),
    "\n",
    include_str!("../../../../database/migrations/0103_domain_membership_and_usage.sql"),
    "\n",
    include_str!("../../../../database/migrations/0104_unified_domain_schema_cleanup.sql"),
    "\n",
    include_str!("../../../../database/migrations/0105_reply_disposition_requalification.sql"),
    "\n",
    include_str!(
        "../../../../database/migrations/0106_reply_disposition_quota_requalification.sql"
    ),
    "\n",
    include_str!("../../../../database/migrations/0113_creator_patrol_detail_follow.sql"),
    "\n",
    include_str!("../../../../database/migrations/0115_creator_discovery.sql"),
    include_str!("../../../../database/migrations/0116_topic_map.sql"),
    include_str!("../../../../database/migrations/0117_topic_map_research.sql"),
    include_str!("../../../../database/migrations/0118_topic_map_structure.sql"),
    include_str!("../../../../database/migrations/0119_topic_map_collection_search.sql"),
    include_str!("../../../../database/migrations/0120_topic_map_core.sql"),
    include_str!("../../../../database/migrations/0121_topic_map_manual_alternative.sql"),
    include_str!("../../../../database/migrations/0122_comment_like_library.sql"),
    include_str!("../../../../database/migrations/0123_ops_ledger.sql"),
    include_str!("../../../../database/migrations/0124_ops_publication_cover.sql"),
    include_str!("../../../../database/migrations/0125_collection_single_run.sql"),
    include_str!("../../../../database/migrations/0126_ops_target.sql"),
    include_str!("../../../../database/migrations/0129_note_material_hashtags.sql"),
    include_str!("../../../../database/migrations/0130_note_material_snapshot.sql"),
    include_str!("../../../../database/migrations/0131_ai_gen1_provider.sql"),
    include_str!("../../../../database/migrations/0132_ai_gen1_control.sql"),
    include_str!("../../../../database/migrations/0133_ai_gen1_skill.sql"),
    include_str!("../../../../database/migrations/0134_ai_gen1_task_config.sql"),
    include_str!("../../../../database/migrations/0135_ai_gen1_batch.sql"),
    include_str!("../../../../database/migrations/0136_ai_gen1_skill_binding.sql"),
    include_str!("../../../../database/migrations/0137_ai_gen1_round.sql"),
    include_str!("../../../../database/migrations/0138_ai_gen1_invocation.sql"),
    include_str!("../../../../database/migrations/0139_ai_gen1_executor.sql"),
    include_str!("../../../../database/migrations/0142_ocr_adoption.sql"),
    include_str!("../../../../database/migrations/0143_ocr_quality_audit.sql"),
    include_str!("../../../../database/migrations/0144_ocr_manual_recognition.sql"),
    include_str!("../../../../database/migrations/0145_ocr_manual_human_adoption.sql"),
    include_str!("../../../../database/migrations/0146_research_dossier_v2.sql"),
    include_str!("../../../../database/migrations/0150_collection_run_comment_target.sql"),
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
    "('0083_comment_research_cross_run_resolution_retry', 'be2bb61fbd766e47b209186b837e6e56e20e4584e055ee8a6c57554720d93e43'), ",
    "('0084_comment_research_problem_resolution_v2', 'da5673d1a64bf470a09b1c6ecc15c56a86500822e83c7397badbfe75613f0f3a'),\n",
    "('0085_comment_research_problem_pair_evaluation', '631acb353568569f7b315b0b682654e095f13c77d0852f3bfdd0af4e4ccac74b'),\n",
    "('0086_comment_research_v2_backlog_recall', '5435cd7e28c9e8bb7421c9c8659b4460ea462e30b3adff72e419b7cba8d5bd22'), ",
    "('0087_cross_industry_detail_comment_policy', '1b5ea42c8360714f0da7ae2425df61d74d2afa94c421b28a66fba378043e2a5a'), ",
    "('0088_media_processing_concurrency_policy', 'fa33e15b88b98226914302e6adeb250242710479851b17eaecb1864879c2cbef'),\n",
    "('0089_detail_page_session_replay_safety', '7b7e5160467e8533cb213bbda1a71c661d63fd461aceb6a43528cb68f7dbe226'),\n",
    "('0090_detail_page_grant_recovery_and_risk_cooldown', 'a6d5e5e8ed230bacd6d03b2175f61bd263c2b48e04eb6be710df078a7771acff'),\n",
    "('0091_ocr_content_layering', 'c7b8390696dc02bcff0a494cbfb55077ea6773a4f9c8109dd116cc7d6d99d770'),\n",
    "('0092_detail_page_session_recovery_boundary', '836095cc3923684684a1d22b6f38171692cadbee5c748e3f5cf004e387d701ad'),\n",
    "('0093_capture_delivery_rejection', 'aaaecf81357c2d4b63719e9729b4e8787b0fab38c0b96953b597850bb69e443e'),\n",
    "('0094_corpus_evidence_read_recovery', '5557cd2dfa4b48426caa230d72742b56d5dd7bc5f85ca53acc953aec1b57fd4b'),\n",
    "('0095_detail_page_url_rejection', '8daf8717a6eb55887dd4b83f92f1c79c93b7a42156e272bc04ceacb112797eb8'),\n",
    "('0096_detail_page_session_lane_delivery_identities', '5bca0b9c770f16ebeed7f510fa0080e7cde07687dfb11bc485fe6b9da8d462eb'),\n",
    "('0097_collection_execution_input_eligibility', '4163a67a9b8556460c816818e5e96829f825610a1ecb0a32550c9b0ab1b43237'),\n",
    "('0098_scheduler_tick_steps_and_readiness', '1933b8978c73c094f8e41c04d119021c17a25483ba71c0d7751af7556e1ba310'), ",
    "('0099_collection_selector_health', '31bd088d59d027abceceef3584aba221803795a576cda21cb462e1f5466baf4f'), ",
    "('0100_collection_hot_path_indexes', 'a26c7006e1c210f226ab8e432651b949f0456ca99055dc41df055b1e6a52dc6c'), ",
    "('0101_collection_command_reason_vocabulary', 'fda3711ea7bb38af6bb5a6a28264a39ac0c04024aef3e6feeb931e07a9b074c1'),\n",
    "('0102_cross_industry_creator_directory', '972ece37c29d79a2a5a9f37cfe3dd4fe71a25446a9884873fb014d36dc7ba8e4'), ",
    "('0103_domain_membership_and_usage', '4345fbd8ed530f5a5184fb9a44d42b32e4702e150ec3301896750a2b7eb80001'), ",
    "('0104_unified_domain_schema_cleanup', '3a75ffdc34bf60cb69f8a9b8d04cfa3cd6c4207522083a122aeedbb9ac1f3788'), ",
    "('0105_reply_disposition_requalification', 'bef9c6cee526068148daddbaeb863c03a5a232bce74bbdf9604f32a53338ec38'), ",
    "('0106_reply_disposition_quota_requalification', '184f91bff325cf335424df212874a53a621169fa388d6b29290c70a5260e0773'), ",
    "('0113_creator_patrol_detail_follow', '8b4aa5eb9a78ab904ad949bbfff510de3cf5a6d9b66e29d8f382c91941914715'), ",
    "('0115_creator_discovery', 'b935e46868c90819235981749a680ff1a5ae9eb0d5ba69cb9d51e5521c4aed7c'),\n",
    "('0116_topic_map', '6d87e950cb1341c0c30397d26a3ea09feddca6e383a3aadd4a87810ac5be463e'),\n",
    "('0117_topic_map_research', '84528ecb0f4f88791d4f6bde33beccdb609dc46326e6b4dc88b2d5c305f7cb5c'),\n",
    "('0118_topic_map_structure', 'cb769810af69e7d6b70b79579379514bc371e64400e2b2fbe75cf3f6bb98e0b1'),\n",
    "('0119_topic_map_collection_search', 'f205b6a644cdf46296aadb4d73a68b639d34ee00e4764fbefc765ef2e7c57e74'),\n",
    "('0120_topic_map_core', '2675fc250e5b496b48ba89360e542dc0946021faefd059d2ff21da9d9a722fdf'),\n",
    "('0121_topic_map_manual_alternative', 'a3d2141098cdf3fbab9cd67e405e8ad27c99c5eb071564814e4012f5f06b5102'),\n",
    "('0122_comment_like_library', '34be42c8c56fd4ee621a592ffeb7f72f8e4ea2a10589ae6a26bf943b1c30157b'),\n",
    "('0123_ops_ledger', '0b7269401ea713c939db2669e29834c84a7a9a34ae66e9933f0395b4201af361'),\n",
    "('0124_ops_publication_cover', 'd879223368b2a1f2023384c40b6cc6c94ca4a959ee3ec9a09712c2c96fac5b3c'),\n",
    "('0125_collection_single_run', '0413aa2357eadb2ba382509f16f37220adb01b4a2e358eb794b14c85466a73b5'),\n",
    "('0126_ops_target', '92ce85c849d586be51bcc07781e0dc384f9c2df8ae91fc6ed85c589e874fdaaa'), ('0129_note_material_hashtags', '7d1cdc0786e217ef500b3a2b7b31e47971fd48da110b41c64c617f99117baa05'), ('0130_note_material_snapshot', 'eeed1797b3f748dfe2dd5e58fd3eba354ae957891ba37f8815f4301953418350'), ('0131_ai_gen1_provider', '235332ca3ee7ad73ba9d0079d0943b387e6e64acfd20b809643aed2b3b73c259'), ('0132_ai_gen1_control', '55eea68854ef7052c691a22c3285d51e3e0beb12f7934b60c685fc2169d730b2'), ('0133_ai_gen1_skill', '0642f95501a9ac56a5d62f969503b24ea658c47ad8041e96f8b4cdd651bc3408'), ('0134_ai_gen1_task_config', '77a9ac05b37d2cf310859301c30d47d2dc1374400c67ea7d5d735a946a17a31c'), ('0135_ai_gen1_batch', '2c558c20d84167b2dc7a649a6be180afe2a37be27b889ac126cf3a5d3402cff8'), ('0136_ai_gen1_skill_binding', 'ee0c19ddb4df3e08fe3dab3b3b114fdfd74d815d5c017667d9c9a57a8d5e1411'), ('0137_ai_gen1_round', '7262239689e3369bd64c18e4103d7e430aa51c805f4625916767c64a013285ae'), ('0138_ai_gen1_invocation', 'b7568a12dc7b1c41bbfabaa423a469aff2783a01b25c3673880a264e5185ec43'), ('0139_ai_gen1_executor', 'd541b2003337d9d50b4591f69760cfae0c78cec2cb3d4e7b56a3f9c2e8b09d6f');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0142_ocr_adoption','baad716a89398d3e923838028358db7e6189f66c79408c4502287f8b75498057');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0143_ocr_quality_audit','11d316460ceb7d49b27751efe4b2382b88a1a068f86784f427771bbdf30bdb6c');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0144_ocr_manual_recognition','de82e4ddabe0e9045165a01e3e32ab2ae24625853def47934907c66d0fc12990');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0145_ocr_manual_human_adoption','49ba4e27a8923ea2e12fefee6e209797aa5d0e7e09b7c299e6ae11e236dbbfbc');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0146_research_dossier_v2','0f3e2ce3720363d8510c448df66a06fb907a955e83817ba9d011d36b4c063881');\n",
    "INSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0150_collection_run_comment_target','c1d380d7085bd2f1a0cafe15a42cbe969e7e2ad1060f444d1c41ce92071c6425');\n",
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
        "contractVersion":"linggan.producer.task-spec.v2","taskId":task_id,"source":"manual",
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
    isolated_proof_schema(&url, schema, full_schema::FULL_MIGRATIONS)
        .await
        .expect("migrations apply")
}

/// Current proofs use the same complete schema and migration ledger as the API runtime.
pub async fn proof_database_with_ops_ledger(schema: &str) -> Database {
    proof_database(schema).await
}

#[allow(dead_code)]
pub async fn proof_database_before_domain_cleanup(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let (before, _) = MIGRATIONS
        .split_once(include_str!(
            "../../../../database/migrations/0104_unified_domain_schema_cleanup.sql"
        ))
        .expect("0104 is present exactly once in the complete proof ledger");
    isolated_proof_schema(&url, schema, before)
        .await
        .expect("migrations before 0104 apply")
}

/// Marks a fixture work as seen only on an author's homepage look (`creator_context`) in `domain`.
/// The usage table is append-only, so the kind is rewritten with `session_replication_role=replica`
/// for this one transaction instead of loosening the production trigger.
#[allow(dead_code)]
pub async fn make_creator_context_only(
    database: &Database,
    domain: uuid::Uuid,
    content_external_id: &str,
) -> uuid::Uuid {
    let mut tx = database.pool().begin().await.expect("begin");
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *tx)
        .await
        .expect("replica role for the fixture rewrite");
    let work: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(&mut *tx)
    .await
    .expect("fixture work exists");
    let changed = sqlx::query(
        "UPDATE linggan_material_domain_usage SET use_kind='creator_context' WHERE content_public_ref=$1 AND domain_ref=$2",
    )
    .bind(work)
    .bind(domain)
    .execute(&mut *tx)
    .await
    .expect("rewrite usage kind")
    .rows_affected();
    assert!(
        changed >= 1,
        "the fixture work must already be in the domain"
    );
    tx.commit().await.expect("commit");
    work
}
