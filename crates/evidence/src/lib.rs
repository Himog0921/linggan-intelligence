//! Immutable source evidence. SCOPE-001 authorizes only synthetic Package/Record ingress after
//! the semantic code gate; real producer evidence remains out of scope.

mod acquisition_chain;
mod archive_completeness;
mod archive_ledger;
mod collection_control;
mod collection_target;
mod collection_task_read;
pub mod comment_research_read;
mod content_reobservation;
mod creator_lifecycle;
mod cross_industry_admission;
pub mod cross_industry_read;
mod directory_boundary;
mod dispatch;
mod execution_station;
mod ingress;
mod local_discovery;
mod local_producer;
mod material_admission;
mod material_asset_read;
mod material_contract_validation;
mod material_cursor;
mod material_detail_read;
mod material_disposition;
mod material_evidence_fragment;
mod material_media;
mod material_media_read;
mod material_processing;
mod material_processing_validation;
mod material_projection;
mod material_projection_types;
mod material_query_sql;
mod material_social_read;
mod material_storage_key;
mod media_acquisition;
pub mod observation_domain;
mod observation_summary;
mod patrol_scheduler;
mod producer_runtime;
mod receipt;
mod runtime_capacity;
mod station_read;
mod target_catalog;
mod target_enrichment;
mod target_inspector;
mod work_order;
mod work_order_lease;
mod work_resource_current;
mod work_resource_read;

pub use acquisition_chain::{
    AcquisitionChainError, AuthorizationGrant, MaterialDeepeningTarget,
    ProgressiveArchiveTickSummary, RequestLeaseError, RequestLeaseOutcome, RequestOutcome,
    acquisition_chain_schema_is_ready, grant_authorization, read_capacity, request_admit_and_lease,
    request_admit_material_targets_and_lease, request_and_admit,
    request_and_admit_material_targets, request_and_admit_material_targets_under_authorization,
    request_progressive_archive, request_progressive_archive_and_lease, run_progressive_archives,
};
pub use archive_completeness::{
    ArchiveCompleteness, ArchiveDirectoryBaseline, BlockedMaterial, read_archive_completeness,
    read_blocked_materials, retire_materials,
};
pub use collection_control::{
    AccountEligibilityReceipt, AccountEligibilitySignal, AccountEligibilityState,
    CapacitySelection, CollectionControlError, ComparableObservationRound,
    DEFAULT_MONITOR_INTERVAL_SECONDS, DynamicCadence, InstallationCredentialSecret,
    IssuedInstallationCredential, MAXIMUM_MONITOR_INTERVAL_SECONDS,
    MINIMUM_MONITOR_INTERVAL_SECONDS, MINIMUM_PLUGIN_VERSION, MonitorCommandActor,
    MonitorCommandKind, MonitorCommandOutcomeKind, MonitorRuleCommand, MonitorRuleCommandError,
    MonitorRuleCommandReceipt, MonitorRuleDraft, MonitorRuleMode, activate_installation_credential,
    apply_manual_observe_command, apply_monitor_rule_command, bind_observation_account,
    collection_control_schema_is_ready, dynamic_cadence, report_account_eligibility,
    rotate_installation_credential, set_station_accepting, toggle_target_patrol, version_at_least,
};
pub use collection_target::{
    CollectionTargetError, ObservationTarget, ObservationTargetAvatar, StoreOutcome, TargetCounts,
    TargetDeletionOutcome, TargetDeletionPreview, collection_target_schema_is_ready, count_targets,
    delete_observation_target, list_targets, list_targets_in_state, read_target,
    read_target_avatars, read_target_deletion_preview, store_pending_target, transition_target,
};
pub use collection_task_read::{
    CollectionTaskExecution, CollectionTaskTimeline, read_collection_task_timeline,
};
pub use content_reobservation::{
    ContentReobservation, ContentReobservationEligibility, ContentReobservationError,
    ContentReobservationStatus, ReobservationMediaPolicy, ReobservationTask, content_reobservation,
    read_content_reobservation, read_content_reobservation_eligibility,
};
pub use creator_lifecycle::{
    CreatorLifecycleAssociation, CreatorLifecycleExclusions, CreatorLifecycleMetric,
    CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleQuery,
    CreatorLifecycleQueryError, CreatorLifecycleReadError, CreatorLifecycleReceipt,
    CreatorLifecycleStatus, CreatorLifecycleSummary, CreatorLifecycleWindow,
    read_creator_lifecycle,
};
pub use dispatch::{
    DISPATCH_FAILURE_RETRY_AFTER_SECONDS, DispatchDecision, DispatchError, DispatchFailureCode,
    DispatchFailureError, DispatchFailureOutcome, decide_dispatch, dispatch_schema_is_ready,
    record_dispatch_answer, requeue_failed_dispatch,
};
pub use execution_station::{
    CheckInOutcome, InstallationCheckIn, InstallationClaimOutcome, StationError,
    check_in_installation, claim_installation, close_claim_window, open_claim_window,
    register_station, rename_station, retire_station, station_schema_is_ready,
};
pub use ingress::{
    IngressError, IngressFault, IngressOptions, PreRoutingCode, ingest_capture_package,
    ingest_capture_package_with,
};
pub use local_discovery::{
    DiscoveryIngressError, DiscoveryIngressOutcome, DiscoveryLibraryCard,
    DiscoveryLibraryProjection, ingest_discovery_package, local_discovery_schema_is_ready,
    read_discovery_library,
};
pub use local_producer::{
    LocalAttemptOutcome, LocalProducerError, LocalSubmissionOutcome, LocalTaskOutcome,
    create_manual_task, local_producer_schema_is_ready, start_local_attempt, submit_local_package,
};
pub use material_asset_read::{
    LocalMaterialAsset, read_local_derivative, read_local_materialization,
};
pub use material_disposition::{
    MaterialMediaDisposition, record_blob_disposition, record_derivative_disposition,
    record_materialization_disposition, record_slot_disposition,
};
pub use material_processing::{
    MediaProcessingClaim, claim_media_processing_work, complete_media_processing_derivative,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work, record_media_derivative_completion,
};
pub use material_social_read::read_authorized_research_comments;
pub use media_acquisition::{
    MediaAcquisitionDecision, MediaAcquisitionError, MediaAcquisitionFailureOutcome,
    claim_media_acquisition, ensure_discovery_cover_media_work, media_acquisition_schema_is_ready,
    record_media_acquisition_failure,
};
pub use observation_summary::{
    PatrolReadState, TargetObservationSummary, read_target_observation_summaries,
};
pub use patrol_scheduler::{
    PatrolTickSummary, SchedulerHeartbeat, patrol_schema_is_ready, read_dynamic_cadence_for_rule,
    read_scheduler_heartbeat, record_scheduler_started, run_due_patrols, set_group_for_many,
    set_monitoring_for_many, set_target_monitoring, target_monitoring_enabled,
};
pub use producer_runtime::{
    MediaBlobAdmission, MediaUploadFinalizeClaim, MediaUploadSession, ProducerRuntimeError,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, admit_media_blob,
    begin_media_upload, claim_media_upload_finalize, complete_media_upload, create_producer_task,
    producer_runtime_has_packages, producer_runtime_schema_is_ready, read_media_upload_session,
    read_runtime_library, record_media_download_failure, record_media_upload_chunk,
    release_media_upload_finalize, start_producer_attempt, submit_producer_package,
};
pub use receipt::{IngressOutcome, RejectionCode};
pub use runtime_capacity::{
    ACCOUNT_CHECK_NOT_CONNECTED, ActiveRiskPause, DispatchLaneBacklog, LaneVerdict, LiveLease,
    MonitorRuleSchedule, PatrolOutlook, PlatformDispatchCapacity, RuntimeCapacityOverview,
    read_runtime_capacity,
};
pub use station_read::{
    CapabilityState, StationCapability, StationOverview, UnclaimedInstallation,
    read_station_capabilities, read_station_overview, station_daily_note_usage,
};
pub use target_catalog::{
    CatalogDetailState, CatalogSource, CatalogWork, CreatorDirectoryProjection,
    KeywordHitProjection, read_creator_directory, read_keyword_hits,
};
pub use target_enrichment::{
    TargetEnrichmentError, TargetSyncOutcome, sync_target_from_author_profile,
};
pub use target_inspector::{
    TargetInspectorAction, TargetInspectorArchive, TargetInspectorArchiveState,
    TargetInspectorCount, TargetInspectorCoverage, TargetInspectorDirectoryState,
    TargetInspectorExecution, TargetInspectorExecutionState, TargetInspectorPatrol,
    TargetInspectorPatrolState, TargetInspectorProjection, TargetInspectorReadError,
    read_target_inspector,
};
pub use work_order_lease::{
    DETAIL_STEP_DEFERRED_REASON, IssuedLease, LeaseError, expire_lapsed_leases,
    issue_work_order_lease, lease_schema_is_ready, recover_released_orphaned_work_orders,
    release_work_order_lease,
};
pub use work_resource_read::{
    WorkResource, WorkResourceCollectionContext, WorkResourceDisplay, WorkResourceEngagement,
    WorkResourceIdentity, WorkResourceLaneSummary, WorkResourcePage, WorkResourcePreview,
    WorkResourceReadError, WorkResourceSummary, read_work_resource, read_work_resources,
    work_resource_schema_is_ready,
};
