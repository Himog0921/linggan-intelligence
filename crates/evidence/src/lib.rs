//! Immutable source evidence. SCOPE-001 authorizes only synthetic Package/Record ingress after
//! the semantic code gate; real producer evidence remains out of scope.

mod acquisition_chain;
mod archive_completeness;
mod archive_ledger;
mod collection_control;
mod collection_target;
mod collection_task_read;
mod content_reobservation;
mod creator_lifecycle;
mod cross_industry_admission;
mod cross_industry_observation;
pub mod cross_industry_read;
mod cross_industry_sample_facts;
mod directory_boundary;
mod dispatch;
mod execution_input_eligibility;
pub use runtime_readiness::{collection_governance_enabled, collection_upgrade_phase};
mod execution_station;
mod ingress;
mod keyword_archive_detail;
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
mod monitor_rule_read;
pub use keyword_archive_detail::{
    KeywordDetailAdvance, KeywordDetailTickSummary, advance_keyword_archive_detail,
    keyword_targets_pending_detail, run_keyword_archive_details,
};

pub use queue_position::{TargetQueuePosition, read_target_queue_positions};

pub use monitor_rule_read::{
    MonitorRuleRetireError, MonitorRuleSummary, read_target_monitor_rules, retire_monitor_rule,
};

pub mod observation_domain;
mod observation_summary;
mod patrol_scheduler;
mod producer_runtime;
mod qualified_detail;
mod queue_position;
mod receipt;
mod runtime_capacity;
mod runtime_event;
mod runtime_readiness;
mod scheduler_tick;
mod selector_health;
mod station_read;
mod step_report;
mod target_catalog;
mod target_enrichment;
mod target_inspector;
mod work_order;
mod work_order_lease;
mod work_resource_current;
mod work_resource_read;

pub use acquisition_chain::{
    AcquisitionChainError, AuthorizationGrant, DETAIL_WINDOW_COMMENT_LIMIT,
    DETAIL_WINDOW_REPLY_EXPAND_LIMIT, MaterialDeepeningTarget, ProgressiveArchiveTickSummary,
    RequestLeaseError, RequestLeaseOutcome, RequestOutcome, acquisition_chain_schema_is_ready,
    grant_authorization, read_capacity, request_admit_and_lease,
    request_admit_material_targets_and_lease, request_and_admit,
    request_and_admit_material_targets, request_and_admit_material_targets_under_authorization,
    request_creator_directory_gaps, request_progressive_archive,
    request_progressive_archive_and_lease, run_progressive_archives,
};
pub use archive_completeness::{
    ArchiveCompleteness, ArchiveDirectoryBaseline, BlockedMaterial, read_archive_completeness,
    read_blocked_materials, retire_materials,
};
pub use collection_control::{
    AccountEligibilityObservation, AccountEligibilityReceipt, AccountEligibilityState,
    CONTROL_FRESHNESS_MINUTES, CapacitySelection, CollectionControlError,
    ComparableObservationRound, DEFAULT_MONITOR_INTERVAL_SECONDS, DynamicCadence,
    ExplicitAccountEligibilitySignal, InstallationCredentialSecret, IssuedInstallationCredential,
    MAXIMUM_MONITOR_INTERVAL_SECONDS, MINIMUM_MONITOR_INTERVAL_SECONDS, MINIMUM_PLUGIN_VERSION,
    MonitorCommandActor, MonitorCommandKind, MonitorCommandOutcomeKind, MonitorRuleCommand,
    MonitorRuleCommandError, MonitorRuleCommandReceipt, MonitorRuleDraft, MonitorRuleMode,
    activate_installation_credential, apply_manual_observe_command, apply_monitor_rule_command,
    bind_observation_account, collection_control_schema_is_ready, dynamic_cadence,
    keyword_baselines_qualified, report_account_eligibility,
    report_claimed_task_account_eligibility, rotate_installation_credential, set_station_accepting,
    toggle_target_patrol, version_at_least,
};
pub use collection_target::{
    CollectionTargetError, ObservationTarget, ObservationTargetAvatar, StoreOutcome, TargetCounts,
    TargetDeletionOutcome, TargetDeletionPreview, TargetDomainAssignmentOutcome,
    assign_target_domain, collection_target_schema_is_ready, count_targets,
    delete_observation_target, list_targets, list_targets_in_state, read_target,
    read_target_avatars, read_target_deletion_preview, store_pending_target, transition_target,
};
pub use collection_task_read::{
    CollectionTaskExecution, CollectionTaskTimeline, DeliveryConclusion,
    DetailDeliveryReconciliation, read_collection_task_timeline,
    read_detail_delivery_reconciliation,
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
    DISPATCH_FAILURE_RETRY_AFTER_SECONDS, DetailPageRiskSignalError, DetailPageRiskSignalReceipt,
    DetailPageSessionGrant, DetailPageSessionGrantError, DetailPageSessionNavigationError,
    DetailPageSessionProgress, DispatchDecision, DispatchError, DispatchFailureCode,
    DispatchFailureError, DispatchFailureOutcome, PreparedLaneDelivery, decide_dispatch,
    dispatch_schema_is_ready, grant_detail_page_session,
    grant_detail_page_session_with_lane_deliveries, record_detail_page_session_navigation,
    record_detail_page_session_progress, record_dispatch_answer, report_detail_page_risk_signal,
    requeue_failed_dispatch,
};
pub use execution_input_eligibility::{MaterialExecutionKind, MaterialExecutionState};
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
    ClaimGateReadiness, MediaProcessingClaim, MediaProcessingClaimOutcome, OcrCompletionInput,
    OcrExcludedLineInput, OcrLayeringInput, OcrLineInput, ProcessorRequeueSummary,
    REGISTERED_PROCESSOR_KINDS, claim_media_processing_work, complete_media_processing_derivative,
    complete_media_processing_ocr, complete_media_processing_text,
    complete_media_processing_without_output, ensure_media_processing_work,
    fail_media_processing_work, processor_version_for_kind, read_claim_gate_readiness,
    record_media_derivative_completion, requeue_outdated_processor_jobs,
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
    PatrolStepError, PatrolTickSummary, SchedulerHeartbeat, patrol_schema_is_ready,
    read_dynamic_cadence_for_rule, read_scheduler_heartbeat, record_scheduler_started,
    run_due_patrol_step, run_due_patrols, set_group_for_many, set_monitoring_for_many,
    set_target_monitoring, target_monitoring_enabled,
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
pub use runtime_event::{
    EVENT_FIELD_WHITELIST, EVENT_READINESS, EVENT_STARTUP, EVENT_TICK, EVENT_TICK_STEP,
    RuntimeEvent, SERVICE_MEDIA_WORKER, SERVICE_WORKER, runtime_revision,
};
pub use runtime_readiness::{
    COLLECTION_RUNTIME_REQUIREMENTS, READINESS_RETRY_START, ReadinessState, RuntimeReadiness,
    RuntimeRequirements, connect_runtime_readiness, next_readiness_retry, probe_runtime_readiness,
};
pub use scheduler_tick::{
    STEP_KEYWORD_DETAILS, STEP_MEDIA_ACQUISITION, STEP_PATROL, STEP_PROGRESSIVE_DOSSIERS,
    TICK_STEP_KEYS, TickLedger, record_readiness, tick_outcome,
};
pub use selector_health::{
    SELECTOR_HEALTH_MAX_BYTES, SELECTOR_HEALTH_MAX_CATEGORIES, SELECTOR_HEALTH_PAGE_TYPES,
    SELECTOR_HEALTH_PLATFORMS, SELECTOR_HEALTH_SNAPSHOT_FIELDS, SelectorHealthRejection,
    accepted_selector_health, normalize_selector_health,
};
pub use station_read::{
    CapabilityState, StationCapability, StationOverview, StationSelectorHealth,
    UnclaimedInstallation, read_station_capabilities, read_station_overview,
    station_daily_note_usage,
};
pub use step_report::{StepFailure, StepOutcome, StepReport};
pub use target_catalog::{
    CatalogDetailState, CatalogSource, CatalogWork, CreatorDirectoryProjection,
    KeywordCatalogCounts, KeywordHitProjection, read_creator_directory, read_cross_industry_hits,
    read_keyword_catalog_counts, read_keyword_hits,
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

pub mod collection_repair;
